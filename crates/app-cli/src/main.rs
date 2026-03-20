use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use speeko_audio::capture;
use speeko_audio::wav;
use speeko_common::config::SpeekConfig;
use speeko_common::error::SpeekError;
use speeko_common::types::Template;
use speeko_dsp::preprocess;
use speeko_common::types::MfccSequence;
use speeko_features::cmn;
use speeko_features::delta;
use speeko_features::mfcc::MfccExtractor;
use speeko_classifier::dataset as cnn_dataset;
use speeko_classifier::inference::CnnRecognizer;
use speeko_classifier::training as cnn_training;
use speeko_recognizer::averaging;
use speeko_recognizer::matcher::TemplateMatcher;
use speeko_store::templates::TemplateStore;
use speeko_store::vocabulary;
use speeko_vad::energy_vad;

/// Speeko — Offline spoken-word recognition for edge devices.
#[derive(Parser)]
#[command(name = "speeko", version, about)]
struct Cli {
    /// Config file path.
    #[arg(long, default_value = "config/speeko.toml")]
    config: PathBuf,

    /// Verbosity level (-v = info, -vv = debug, -vvv = trace).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Train a word by recording samples from the microphone.
    Train {
        /// The word to train.
        word: String,
        /// Number of samples to record.
        #[arg(long, default_value_t = 3)]
        samples: usize,
        /// Recording duration per sample in seconds.
        #[arg(long)]
        duration: Option<f32>,
        /// Append to existing samples instead of starting fresh.
        #[arg(long)]
        append: bool,
        /// Force training even if word is not in vocabulary.
        #[arg(long)]
        force: bool,
    },
    /// Test recognition from the microphone.
    Test {
        /// Run continuously until Ctrl+C.
        #[arg(long)]
        continuous: bool,
        /// Timeout in seconds (for non-continuous mode).
        #[arg(long)]
        timeout: Option<f32>,
    },
    /// List trained words and sample counts.
    ListWords {
        /// Show detailed information.
        #[arg(long)]
        detailed: bool,
    },
    /// Evaluate recognition accuracy against stored WAV recordings.
    Evaluate {
        /// Directory containing test WAV files (organized as word/file.wav).
        #[arg(long)]
        wav_dir: Option<PathBuf>,
    },
    /// Record audio and save to a WAV file.
    Record {
        /// Output WAV file path.
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Recording duration in seconds.
        #[arg(long)]
        duration: Option<f32>,
        /// Trim silence from the recording.
        #[arg(long)]
        trim: bool,
    },
    /// Extract and display MFCC features from a WAV file.
    Extract {
        /// Path to the WAV file.
        wav: PathBuf,
        /// Dump full MFCC matrix to stdout.
        #[arg(long)]
        dump: bool,
    },
    /// Print diagnostic information about audio devices and configuration.
    Diagnose,
    /// Delete templates for a word or all words.
    Reset {
        /// Word to delete, or use --all.
        word: Option<String>,
        /// Delete all templates.
        #[arg(long)]
        all: bool,
        /// Skip confirmation prompt.
        #[arg(long)]
        confirm: bool,
    },
    /// Pre-record WAV samples for one or more words (review/delete before training).
    RecordSamples {
        /// Words to record samples for (e.g. start stop yes no).
        words: Vec<String>,
        /// Number of samples per word.
        #[arg(long, default_value_t = 5)]
        samples: usize,
        /// Recording duration per sample in seconds.
        #[arg(long)]
        duration: Option<f32>,
        /// Output directory for WAV files (default: data/recordings).
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Train templates from a folder of pre-recorded WAV files.
    ///
    /// Folder structure: <dir>/<word>/sample_000.wav, sample_001.wav, ...
    /// Delete any bad recordings before running this command.
    TrainFrom {
        /// Directory containing word subfolders with WAV files.
        dir: PathBuf,
        /// Reset existing templates before training.
        #[arg(long)]
        reset: bool,
    },
    /// Run recognition test suite from pre-recorded WAV files.
    ///
    /// Folder structure: <dir>/<word>/sample_000.wav, ...
    /// Reports per-word and overall accuracy.
    TestFrom {
        /// Directory containing word subfolders with WAV files.
        dir: PathBuf,
        /// Show per-file details.
        #[arg(long)]
        verbose: bool,
    },
    /// Train a CNN classifier from pre-recorded WAV files.
    ///
    /// Uses the same folder structure as train-from: <dir>/<word>/sample_*.wav
    /// Set recognizer.mode = "cnn" in speeko.toml to use the CNN for recognition.
    CnnTrain {
        /// Directory containing word subfolders with WAV files.
        dir: PathBuf,
        /// Number of training epochs.
        #[arg(long)]
        epochs: Option<usize>,
        /// Training batch size.
        #[arg(long, alias = "batch-size")]
        batch_size: Option<usize>,
        /// Learning rate.
        #[arg(long)]
        lr: Option<f64>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging based on verbosity.
    let log_level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .format_timestamp_millis()
        .init();

    // Set up Ctrl+C handler.
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);
    ctrlc::set_handler(move || {
        cancel_clone.store(true, Ordering::SeqCst);
        eprintln!("\nInterrupted. Shutting down gracefully...");
    })
    .context("Failed to set Ctrl+C handler")?;

    // Load config.
    let config = SpeekConfig::load(&cli.config)?;

    match cli.command {
        Commands::Train {
            word,
            samples,
            duration,
            append,
            force,
        } => cmd_train(&config, &word, samples, duration, append, force, &cancel),
        Commands::Test {
            continuous,
            timeout,
        } => cmd_test(&config, continuous, timeout, &cancel),
        Commands::ListWords { detailed } => cmd_list_words(&config, detailed),
        Commands::Evaluate { wav_dir } => cmd_evaluate(&config, wav_dir),
        Commands::Record {
            output,
            duration,
            trim,
        } => cmd_record(&config, output, duration, trim, &cancel),
        Commands::Extract { wav, dump } => cmd_extract(&config, &wav, dump),
        Commands::Diagnose => cmd_diagnose(&config),
        Commands::Reset {
            word,
            all,
            confirm,
        } => cmd_reset(&config, word, all, confirm),
        Commands::RecordSamples {
            words,
            samples,
            duration,
            output,
        } => cmd_record_samples(&config, &words, samples, duration, output, &cancel),
        Commands::TrainFrom { dir, reset } => cmd_train_from(&config, &dir, reset),
        Commands::TestFrom { dir, verbose } => cmd_test_from(&config, &dir, verbose),
        Commands::CnnTrain {
            dir,
            epochs,
            batch_size,
            lr,
        } => cmd_cnn_train(&config, &dir, epochs, batch_size, lr),
    }
}

/// Apply post-extraction feature transforms: CMN and/or delta+delta-delta.
fn apply_feature_transforms(mfcc: MfccSequence, config: &SpeekConfig) -> MfccSequence {
    let mut features = mfcc;

    if config.mfcc.use_cmn {
        features = cmn::normalize(&features);
        log::debug!("Applied CMN normalization");
    }

    if config.mfcc.use_deltas {
        features = delta::append_deltas_and_double_deltas(&features);
        log::debug!(
            "Applied delta+delta-delta: {} dims per frame",
            features.first().map_or(0, |f| f.len())
        );
    }

    features
}

fn cmd_train(
    config: &SpeekConfig,
    word: &str,
    num_samples: usize,
    duration_override: Option<f32>,
    append: bool,
    force: bool,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    let word = word.to_lowercase();
    let duration = duration_override.unwrap_or(config.audio.record_duration_secs);

    // Check vocabulary.
    if config.paths.vocabulary_file.exists() {
        let vocab = vocabulary::load_vocabulary(&config.paths.vocabulary_file)?;
        if !vocabulary::is_in_vocabulary(&vocab, &word) && !force {
            eprintln!("'{}' is not in vocabulary.txt.", word);
            eprint!("Add it and continue? [y/N] ");
            io::stderr().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Training cancelled.");
                return Ok(());
            }
        }
    }

    let store = TemplateStore::new(&config.paths.templates_dir)?;
    let recordings_dir = config.paths.recordings_dir.join(&word);
    std::fs::create_dir_all(&recordings_dir)?;

    let start_index = if append {
        store.next_sample_index(&word)?
    } else {
        0
    };

    let mut mfcc_extractor =
        MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    println!("Training word: '{}'", word);
    println!(
        "Recording {} samples ({:.1}s each)...",
        num_samples, duration
    );
    println!();

    for i in 0..num_samples {
        if cancel.load(Ordering::Relaxed) {
            println!("Training cancelled.");
            return Ok(());
        }

        let sample_index = start_index + i;
        println!(
            "[{}/{}] Say '{}' now... (press Enter when ready)",
            i + 1,
            num_samples,
            word
        );

        // Wait for Enter.
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if cancel.load(Ordering::Relaxed) {
            println!("Training cancelled.");
            return Ok(());
        }

        println!("  Recording in 1s...");
        std::thread::sleep(std::time::Duration::from_millis(1000));
        println!("  Recording...");
        let timer = Instant::now();
        let mut samples = capture::record_audio(config.audio.sample_rate, duration, cancel)?;
        log::debug!("  Capture took {:?}", timer.elapsed());

        // Check for clipping.
        if capture::is_clipped(&samples, 0.99) {
            eprintln!(
                "  WARNING: Audio appears clipped. Move further from mic or reduce volume."
            );
        }

        // Preprocess.
        preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

        // VAD trim.
        let frame_length = config.frame_length_samples();
        let frame_step = config.frame_step_samples();
        let trimmed = energy_vad::trim_silence(
            &samples,
            config.audio.sample_rate,
            frame_length,
            frame_step,
            &config.vad,
        );

        let speech_samples = match trimmed {
            Some(s) => s,
            None => {
                eprintln!("  No speech detected. Please speak louder or check mic.");
                eprint!("  Try again? [y/N] ");
                io::stderr().flush()?;
                let mut retry = String::new();
                io::stdin().read_line(&mut retry)?;
                if retry.trim().eq_ignore_ascii_case("y") {
                    continue;
                }
                println!("  Skipping sample.");
                continue;
            }
        };

        // Check minimum duration.
        let duration_ms =
            (speech_samples.len() as f32 / config.audio.sample_rate as f32 * 1000.0) as u32;
        if duration_ms < config.vad.min_utterance_ms {
            eprintln!(
                "  Recording too short ({}ms). Minimum is {}ms.",
                duration_ms, config.vad.min_utterance_ms
            );
            continue;
        }

        // Save raw WAV.
        let wav_path = recordings_dir.join(format!("sample_{:03}.wav", sample_index));
        wav::save_wav(&wav_path, &speech_samples, config.audio.sample_rate)?;

        // Extract MFCC features + apply transforms (CMN, deltas).
        let timer = Instant::now();
        let mfcc = mfcc_extractor.extract(&speech_samples);
        let mfcc = apply_feature_transforms(mfcc, config);
        log::debug!("  MFCC extraction took {:?}", timer.elapsed());

        // Save template.
        let template = Template {
            word: word.clone(),
            sample_index,
            mfcc,
        };
        store.save_template(&template)?;

        println!(
            "  ✓ Saved sample {} ({:.0}ms, {} frames)",
            sample_index,
            duration_ms,
            template.mfcc.len()
        );
    }

    let counts = store.template_counts()?;
    let word_count = counts
        .iter()
        .find(|(w, _)| w == &word)
        .map_or(0, |(_, c)| *c);
    println!();
    println!("Trained '{}' with {} total samples.", word, word_count);
    Ok(())
}

fn cmd_test(
    config: &SpeekConfig,
    continuous: bool,
    timeout: Option<f32>,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    let use_cnn = config.recognizer.mode == "cnn";

    // Load DTW templates (needed unless using CNN).
    let templates;
    let matcher;
    if !use_cnn {
        let store = TemplateStore::new(&config.paths.templates_dir)?;
        let all_templates = store.load_all_templates()?;
        if all_templates.is_empty() {
            bail!("{}", SpeekError::NoTemplates);
        }
        let mean_templates = averaging::compute_mean_templates(&all_templates);
        log::info!("Using {} mean templates for matching", mean_templates.len());
        let counts = store.template_counts()?;
        println!("Mode: DTW");
        println!("Loaded templates for {} words:", counts.len());
        for (word, count) in &counts {
            println!("  {} ({} samples -> 1 mean template)", word, count);
        }
        if config.paths.vocabulary_file.exists() {
            let vocab = vocabulary::load_vocabulary(&config.paths.vocabulary_file)?;
            let trained_words: Vec<&str> = counts.iter().map(|(w, _)| w.as_str()).collect();
            let missing: Vec<&str> = vocab
                .iter()
                .filter(|w| !trained_words.contains(&w.as_str()))
                .map(|w| w.as_str())
                .collect();
            if !missing.is_empty() {
                eprintln!(
                    "WARNING: {} vocabulary words not trained: {}",
                    missing.len(),
                    missing.join(", ")
                );
            }
        }
        templates = Some(mean_templates);
        matcher = Some(TemplateMatcher::new(
            config.recognizer.sakoe_chiba_width,
            config.recognizer.confidence_threshold,
            config.recognizer.max_distance,
        ));
    } else {
        templates = None;
        matcher = None;
    }

    // Load CNN model if needed.
    let cnn = if use_cnn {
        println!("Mode: CNN");
        let recognizer = CnnRecognizer::load(
            &config.classifier.model_dir,
            config.recognizer.confidence_threshold,
            config.classifier.max_frames,
        )?;
        println!("CNN model loaded from {:?}", config.classifier.model_dir);
        Some(recognizer)
    } else {
        None
    };

    let mut mfcc_extractor =
        MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    let duration = timeout.unwrap_or(config.audio.record_duration_secs);

    println!();
    if continuous {
        println!("Listening continuously (Ctrl+C to stop)...");
    } else {
        println!("Listening for a single word...");
    }

    let mut recognized_count = 0u32;
    let mut total_count = 0u32;

    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        println!();
        println!("Speak now...");
        std::thread::sleep(std::time::Duration::from_millis(500));

        let mut samples =
            match capture::record_audio(config.audio.sample_rate, duration, cancel) {
                Ok(s) => s,
                Err(e) => {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    eprintln!("Audio capture error: {}", e);
                    continue;
                }
            };

        let timer = Instant::now();

        // Preprocess.
        preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

        // VAD trim.
        let frame_length = config.frame_length_samples();
        let frame_step = config.frame_step_samples();
        let trimmed = energy_vad::trim_silence(
            &samples,
            config.audio.sample_rate,
            frame_length,
            frame_step,
            &config.vad,
        );

        let speech_samples = match trimmed {
            Some(s) => s,
            None => {
                println!("  (no speech detected)");
                total_count += 1;
                if !continuous {
                    break;
                }
                continue;
            }
        };

        // Extract MFCC + apply transforms (CMN, deltas).
        let mfcc = mfcc_extractor.extract(&speech_samples);
        let mfcc = apply_feature_transforms(mfcc, config);

        // Recognize using the configured mode.
        let result = if let Some(ref cnn_recognizer) = cnn {
            cnn_recognizer.predict(&mfcc)
        } else {
            matcher.as_ref().unwrap().recognize(&mfcc, templates.as_ref().unwrap())
        };
        let elapsed = timer.elapsed();

        total_count += 1;

        match &result.word {
            Some(word) => {
                recognized_count += 1;
                println!(
                    "  Recognized: {} (confidence: {:.0}%, distance: {:.2}, time: {:?})",
                    word,
                    result.confidence * 100.0,
                    result.best_distance,
                    elapsed
                );
            }
            None => {
                println!(
                    "  Unknown word (confidence: {:.0}%, best distance: {:.2}, time: {:?})",
                    result.confidence * 100.0,
                    result.best_distance,
                    elapsed
                );
            }
        }

        if !continuous {
            break;
        }
    }

    if total_count > 0 {
        println!();
        println!(
            "Session summary: {}/{} recognized ({:.0}%)",
            recognized_count,
            total_count,
            recognized_count as f32 / total_count as f32 * 100.0
        );
    }

    Ok(())
}

fn cmd_list_words(config: &SpeekConfig, detailed: bool) -> Result<()> {
    let store = TemplateStore::new(&config.paths.templates_dir)?;
    let counts = store.template_counts()?;

    if counts.is_empty() {
        println!("No trained words. Run `speeko train <word>` to get started.");
        return Ok(());
    }

    println!("Trained words ({}):", counts.len());
    for (word, count) in &counts {
        if detailed {
            let templates = store.load_word_templates(word)?;
            let avg_frames: f32 = if templates.is_empty() {
                0.0
            } else {
                templates.iter().map(|t| t.mfcc.len() as f32).sum::<f32>() / templates.len() as f32
            };
            println!(
                "  {} — {} samples, avg {:.0} frames",
                word, count, avg_frames
            );
        } else {
            println!("  {} ({} samples)", word, count);
        }
    }

    // Check against vocabulary.
    if config.paths.vocabulary_file.exists() {
        let vocab = vocabulary::load_vocabulary(&config.paths.vocabulary_file)?;
        let trained: Vec<&str> = counts.iter().map(|(w, _)| w.as_str()).collect();
        let missing: Vec<&str> = vocab
            .iter()
            .filter(|w| !trained.contains(&w.as_str()))
            .map(|w| w.as_str())
            .collect();
        if !missing.is_empty() {
            println!();
            println!("Not yet trained ({}):", missing.len());
            for w in &missing {
                println!("  {}", w);
            }
        }
    }

    Ok(())
}

fn cmd_evaluate(config: &SpeekConfig, wav_dir: Option<PathBuf>) -> Result<()> {
    let test_dir = wav_dir.unwrap_or_else(|| config.paths.recordings_dir.clone());

    if !test_dir.exists() {
        bail!("No WAV files found in {:?}. Provide test recordings.", test_dir);
    }

    let use_cnn = config.recognizer.mode == "cnn";

    let templates;
    let matcher;
    let cnn;
    if use_cnn {
        println!("Mode: CNN");
        let recognizer = CnnRecognizer::load(
            &config.classifier.model_dir,
            config.recognizer.confidence_threshold,
            config.classifier.max_frames,
        )?;
        cnn = Some(recognizer);
        templates = None;
        matcher = None;
    } else {
        let store = TemplateStore::new(&config.paths.templates_dir)?;
        let all_templates = store.load_all_templates()?;
        if all_templates.is_empty() {
            bail!("{}", SpeekError::NoTemplates);
        }
        templates = Some(averaging::compute_mean_templates(&all_templates));
        matcher = Some(TemplateMatcher::new(
            config.recognizer.sakoe_chiba_width,
            config.recognizer.confidence_threshold,
            config.recognizer.max_distance,
        ));
        cnn = None;
    }

    let mut mfcc_extractor =
        MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    let mut correct = 0u32;
    let mut total = 0u32;
    let mut rejected = 0u32;

    // Walk word directories.
    let entries = std::fs::read_dir(&test_dir)?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let expected_word = entry
            .file_name()
            .to_str()
            .unwrap_or("")
            .to_lowercase();

        let wav_files = std::fs::read_dir(entry.path())?;
        for wav_entry in wav_files {
            let wav_entry = wav_entry?;
            let path = wav_entry.path();
            if path.extension().map_or(true, |e| e != "wav") {
                continue;
            }

            let (mut samples, _sr) = wav::load_wav(&path)?;
            preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

            let frame_length = config.frame_length_samples();
            let frame_step = config.frame_step_samples();
            let trimmed = energy_vad::trim_silence(
                &samples,
                config.audio.sample_rate,
                frame_length,
                frame_step,
                &config.vad,
            );

            let speech = match trimmed {
                Some(s) => s,
                None => {
                    println!("  {:?}: no speech detected", path.file_name().unwrap_or_default());
                    total += 1;
                    continue;
                }
            };

            let mfcc = mfcc_extractor.extract(&speech);
            let mfcc = apply_feature_transforms(mfcc, config);
            let result = if let Some(ref cnn_recognizer) = cnn {
                cnn_recognizer.predict(&mfcc)
            } else {
                matcher.as_ref().unwrap().recognize(&mfcc, templates.as_ref().unwrap())
            };

            total += 1;
            match &result.word {
                Some(predicted) if predicted == &expected_word => {
                    correct += 1;
                    println!(
                        "  ✓ {}/{}: {} (conf={:.0}%)",
                        expected_word,
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        predicted,
                        result.confidence * 100.0
                    );
                }
                Some(predicted) => {
                    println!(
                        "  ✗ {}/{}: {} (expected {}, conf={:.0}%)",
                        expected_word,
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        predicted,
                        expected_word,
                        result.confidence * 100.0
                    );
                }
                None => {
                    rejected += 1;
                    println!(
                        "  ? {}/{}: rejected (conf={:.0}%)",
                        expected_word,
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        result.confidence * 100.0
                    );
                }
            }
        }
    }

    println!();
    println!("Evaluation results:");
    println!(
        "  Accuracy: {}/{} ({:.1}%)",
        correct,
        total,
        if total > 0 {
            correct as f32 / total as f32 * 100.0
        } else {
            0.0
        }
    );
    println!("  Rejected: {}/{}", rejected, total);

    Ok(())
}

fn cmd_record(
    config: &SpeekConfig,
    output: Option<PathBuf>,
    duration_override: Option<f32>,
    trim: bool,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    let duration = duration_override.unwrap_or(config.audio.record_duration_secs);
    let output_path = output.unwrap_or_else(|| PathBuf::from("recording.wav"));

    println!("Recording for {:.1}s...", duration);
    let mut samples = capture::record_audio(config.audio.sample_rate, duration, cancel)?;

    if trim {
        preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);
        let frame_length = config.frame_length_samples();
        let frame_step = config.frame_step_samples();
        if let Some(trimmed) = energy_vad::trim_silence(
            &samples,
            config.audio.sample_rate,
            frame_length,
            frame_step,
            &config.vad,
        ) {
            println!(
                "Trimmed: {} -> {} samples ({:.0}ms)",
                samples.len(),
                trimmed.len(),
                trimmed.len() as f32 / config.audio.sample_rate as f32 * 1000.0
            );
            samples = trimmed;
        } else {
            eprintln!("No speech detected in recording.");
        }
    }

    wav::save_wav(&output_path, &samples, config.audio.sample_rate)?;
    println!("Saved to {:?}", output_path);
    Ok(())
}

fn cmd_extract(config: &SpeekConfig, wav_path: &PathBuf, dump: bool) -> Result<()> {
    let (mut samples, sample_rate) = wav::load_wav(wav_path)?;
    println!(
        "Loaded: {} samples, {}Hz, {:.2}s",
        samples.len(),
        sample_rate,
        samples.len() as f32 / sample_rate as f32
    );

    preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

    let mut mfcc_extractor =
        MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    let timer = Instant::now();
    let mfcc = mfcc_extractor.extract(&samples);
    let mfcc = apply_feature_transforms(mfcc, config);
    let elapsed = timer.elapsed();

    let dims = mfcc.first().map_or(0, |f| f.len());
    println!(
        "Features: [{} x {}] extracted in {:?}",
        mfcc.len(),
        dims,
        elapsed
    );

    if dump {
        println!();
        println!("Frame | Coefficients");
        println!("------+{}", "-".repeat(dims * 10));
        for (i, frame) in mfcc.iter().enumerate() {
            let vals: Vec<String> = frame.iter().map(|v| format!("{:8.3}", v)).collect();
            println!("{:5} | {}", i, vals.join(" "));
        }
    }

    Ok(())
}

fn cmd_diagnose(config: &SpeekConfig) -> Result<()> {
    println!("=== Speeko Diagnostics ===");
    println!();

    // Audio devices.
    println!("Audio Input Devices:");
    match capture::list_input_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("  (none found)");
            } else {
                for d in &devices {
                    println!("  - {}", d);
                }
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
    println!(
        "  Default device available: {}",
        capture::has_input_device()
    );

    println!();
    println!("Configuration:");
    println!("  Sample rate: {}Hz", config.audio.sample_rate);
    println!(
        "  Record duration: {:.1}s",
        config.audio.record_duration_secs
    );
    println!(
        "  Frame: {:.0}ms / {:.0}ms step",
        config.dsp.frame_length_ms, config.dsp.frame_step_ms
    );
    println!("  FFT size: {}", config.dsp.fft_size);
    println!("  Mel filters: {}", config.mfcc.num_mel_filters);
    println!("  MFCC coefficients: {}", config.mfcc.num_coefficients);
    println!(
        "  Confidence threshold: {:.1}%",
        config.recognizer.confidence_threshold * 100.0
    );
    println!(
        "  Max distance: {:.1}",
        config.recognizer.max_distance
    );

    println!();
    println!("Templates:");
    let store = TemplateStore::new(&config.paths.templates_dir)?;
    let counts = store.template_counts()?;
    if counts.is_empty() {
        println!("  (none)");
    } else {
        for (word, count) in &counts {
            println!("  {} — {} samples", word, count);
        }
    }

    println!();
    println!("Vocabulary:");
    if config.paths.vocabulary_file.exists() {
        let vocab = vocabulary::load_vocabulary(&config.paths.vocabulary_file)?;
        println!("  {} words: {}", vocab.len(), vocab.join(", "));
    } else {
        println!("  (no vocabulary file found)");
    }

    Ok(())
}

fn cmd_record_samples(
    config: &SpeekConfig,
    words: &[String],
    num_samples: usize,
    duration_override: Option<f32>,
    output_dir: Option<PathBuf>,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    if words.is_empty() {
        bail!("Provide at least one word to record. Example: speeko record-samples start stop yes no");
    }

    let duration = duration_override.unwrap_or(config.audio.record_duration_secs);
    let base_dir = output_dir.unwrap_or_else(|| config.paths.recordings_dir.clone());

    let total_recordings = words.len() * num_samples;
    println!("Recording plan:");
    println!("  Words: {}", words.iter().map(|w| w.to_lowercase()).collect::<Vec<_>>().join(", "));
    println!("  Samples per word: {}", num_samples);
    println!("  Duration per sample: {:.1}s", duration);
    println!("  Output directory: {:?}", base_dir);
    println!("  Total recordings: {}", total_recordings);
    println!();
    println!("Recordings are saved as WAV files. After recording, review them and");
    println!("delete any mis-pronounced ones before running `speeko train-from`.");
    println!();

    for word in words {
        let word = word.to_lowercase();
        let word_dir = base_dir.join(&word);
        std::fs::create_dir_all(&word_dir)?;

        // Find next available index in this folder.
        let existing_count = std::fs::read_dir(&word_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wav"))
            .count();

        println!("=== Recording '{}' ({} existing WAVs in folder) ===", word, existing_count);
        println!();

        for i in 0..num_samples {
            if cancel.load(Ordering::Relaxed) {
                println!("\nRecording cancelled.");
                return Ok(());
            }

            let file_index = existing_count + i;
            println!(
                "[{}/{}] Say '{}' now... (press Enter when ready, 's' to skip word)",
                i + 1,
                num_samples,
                word
            );

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if input.trim().eq_ignore_ascii_case("s") {
                println!("  Skipping remaining samples for '{}'.", word);
                break;
            }

            if cancel.load(Ordering::Relaxed) {
                println!("\nRecording cancelled.");
                return Ok(());
            }

            println!("  Recording in 1s...");
            std::thread::sleep(std::time::Duration::from_millis(1000));
            println!("  Recording...");

            let samples = match capture::record_audio(config.audio.sample_rate, duration, cancel) {
                Ok(s) => s,
                Err(e) => {
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    eprintln!("  Audio capture error: {}. Skipping.", e);
                    continue;
                }
            };

            if capture::is_clipped(&samples, 0.99) {
                eprintln!("  WARNING: Audio appears clipped.");
            }

            let wav_path = word_dir.join(format!("sample_{:03}.wav", file_index));
            wav::save_wav(&wav_path, &samples, config.audio.sample_rate)?;

            let duration_ms = samples.len() as f32 / config.audio.sample_rate as f32 * 1000.0;
            println!("  ✓ Saved {:?} ({:.0}ms)", wav_path.file_name().unwrap_or_default(), duration_ms);
        }
        println!();
    }

    println!("Recording complete! Files saved to {:?}", base_dir);
    println!();
    println!("Next steps:");
    println!("  1. Review the WAV files (play them back, delete bad ones)");
    println!("  2. Run: speeko train-from {:?}", base_dir);
    println!("  3. Test: speeko test-from <test_wav_dir>");
    Ok(())
}

fn cmd_train_from(config: &SpeekConfig, wav_dir: &PathBuf, reset: bool) -> Result<()> {
    if !wav_dir.exists() {
        bail!("Directory {:?} does not exist.", wav_dir);
    }

    // Discover words (subdirectories) and WAV files.
    let mut word_files: Vec<(String, Vec<PathBuf>)> = Vec::new();
    let entries = std::fs::read_dir(wav_dir)
        .with_context(|| format!("Failed to read directory {:?}", wav_dir))?;

    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let word = entry
            .file_name()
            .to_str()
            .unwrap_or("")
            .to_lowercase();
        if word.is_empty() {
            continue;
        }

        let mut wavs: Vec<PathBuf> = std::fs::read_dir(entry.path())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "wav"))
            .collect();
        wavs.sort();

        if !wavs.is_empty() {
            word_files.push((word, wavs));
        }
    }
    word_files.sort_by(|a, b| a.0.cmp(&b.0));

    if word_files.is_empty() {
        bail!(
            "No word folders with WAV files found in {:?}.\n\
             Expected structure: <dir>/<word>/sample_000.wav",
            wav_dir
        );
    }

    // Summary.
    let total_wavs: usize = word_files.iter().map(|(_, files)| files.len()).sum();
    println!("Training from pre-recorded WAVs:");
    println!("  Source: {:?}", wav_dir);
    for (word, files) in &word_files {
        println!("  {} — {} WAV files", word, files.len());
    }
    println!("  Total: {} files across {} words", total_wavs, word_files.len());
    println!();

    let store = TemplateStore::new(&config.paths.templates_dir)?;

    if reset {
        store.delete_all()?;
        println!("Cleared existing templates.");
    }

    let mut mfcc_extractor =
        MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    let mut trained = 0usize;
    let mut skipped = 0usize;

    for (word, files) in &word_files {
        println!("Training '{}'...", word);

        let start_index = if reset {
            0
        } else {
            store.next_sample_index(word)?
        };

        for (i, wav_path) in files.iter().enumerate() {
            let sample_index = start_index + i;
            let filename = wav_path.file_name().unwrap_or_default().to_string_lossy();

            // Load WAV.
            let (mut samples, _sr) = match wav::load_wav(wav_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("  ✗ {}: load error: {}", filename, e);
                    skipped += 1;
                    continue;
                }
            };

            // Preprocess.
            preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

            // VAD trim.
            let frame_length = config.frame_length_samples();
            let frame_step = config.frame_step_samples();
            let trimmed = energy_vad::trim_silence(
                &samples,
                config.audio.sample_rate,
                frame_length,
                frame_step,
                &config.vad,
            );

            let speech_samples = match trimmed {
                Some(s) => s,
                None => {
                    eprintln!("  ✗ {}: no speech detected, skipping", filename);
                    skipped += 1;
                    continue;
                }
            };

            let duration_ms =
                (speech_samples.len() as f32 / config.audio.sample_rate as f32 * 1000.0) as u32;
            if duration_ms < config.vad.min_utterance_ms {
                eprintln!(
                    "  ✗ {}: too short ({}ms < {}ms), skipping",
                    filename, duration_ms, config.vad.min_utterance_ms
                );
                skipped += 1;
                continue;
            }

            // Extract features.
            let mfcc = mfcc_extractor.extract(&speech_samples);
            let mfcc = apply_feature_transforms(mfcc, config);

            let template = Template {
                word: word.clone(),
                sample_index,
                mfcc,
            };
            store.save_template(&template)?;
            trained += 1;

            println!(
                "  ✓ {} -> sample {} ({:.0}ms, {} frames)",
                filename, sample_index, duration_ms, template.mfcc.len()
            );
        }
    }

    println!();
    println!("Training complete: {} templates created, {} skipped.", trained, skipped);

    let counts = store.template_counts()?;
    println!("Templates:");
    for (word, count) in &counts {
        println!("  {} — {} samples", word, count);
    }
    Ok(())
}

fn cmd_test_from(config: &SpeekConfig, wav_dir: &PathBuf, verbose: bool) -> Result<()> {
    if !wav_dir.exists() {
        bail!("Directory {:?} does not exist.", wav_dir);
    }

    let use_cnn = config.recognizer.mode == "cnn";

    // Load DTW templates or CNN model.
    let templates;
    let matcher;
    let cnn;
    if use_cnn {
        println!("Mode: CNN");
        let recognizer = CnnRecognizer::load(
            &config.classifier.model_dir,
            config.recognizer.confidence_threshold,
            config.classifier.max_frames,
        )?;
        println!("CNN model loaded from {:?}", config.classifier.model_dir);
        cnn = Some(recognizer);
        templates = None;
        matcher = None;
    } else {
        let store = TemplateStore::new(&config.paths.templates_dir)?;
        let all_templates = store.load_all_templates()?;
        if all_templates.is_empty() {
            bail!("{}", SpeekError::NoTemplates);
        }
        let mean_templates = averaging::compute_mean_templates(&all_templates);
        let trained_words: Vec<&str> = mean_templates.iter().map(|t| t.word.as_str()).collect();
        println!("Mode: DTW");
        println!("Trained words: {}", trained_words.join(", "));
        println!("Using {} mean templates", mean_templates.len());
        matcher = Some(TemplateMatcher::new(
            config.recognizer.sakoe_chiba_width,
            config.recognizer.confidence_threshold,
            config.recognizer.max_distance,
        ));
        templates = Some(mean_templates);
        cnn = None;
    }

    let mut mfcc_extractor =
        MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    println!("Test suite from: {:?}", wav_dir);
    println!();

    // Discover test WAVs.
    let entries = std::fs::read_dir(wav_dir)?;
    let mut per_word_stats: Vec<(String, u32, u32, u32, Vec<f32>)> = Vec::new(); // word, correct, wrong, rejected, confidences

    let mut total_correct = 0u32;
    let mut total_wrong = 0u32;
    let mut total_rejected = 0u32;
    let mut total_no_speech = 0u32;

    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let expected_word = entry
            .file_name()
            .to_str()
            .unwrap_or("")
            .to_lowercase();
        if expected_word.is_empty() {
            continue;
        }

        let mut wav_files: Vec<PathBuf> = std::fs::read_dir(entry.path())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "wav"))
            .collect();
        wav_files.sort();

        if wav_files.is_empty() {
            continue;
        }

        let mut word_correct = 0u32;
        let mut word_wrong = 0u32;
        let mut word_rejected = 0u32;
        let mut word_confidences: Vec<f32> = Vec::new();

        if verbose {
            println!("--- {} ({} files) ---", expected_word, wav_files.len());
        }

        for wav_path in &wav_files {
            let filename = wav_path.file_name().unwrap_or_default().to_string_lossy();

            let (mut samples, _sr) = match wav::load_wav(wav_path) {
                Ok(s) => s,
                Err(e) => {
                    if verbose {
                        eprintln!("  ✗ {}: load error: {}", filename, e);
                    }
                    continue;
                }
            };

            preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

            let frame_length = config.frame_length_samples();
            let frame_step = config.frame_step_samples();
            let trimmed = energy_vad::trim_silence(
                &samples,
                config.audio.sample_rate,
                frame_length,
                frame_step,
                &config.vad,
            );

            let speech = match trimmed {
                Some(s) => s,
                None => {
                    if verbose {
                        println!("  - {}: no speech detected", filename);
                    }
                    total_no_speech += 1;
                    continue;
                }
            };

            let mfcc = mfcc_extractor.extract(&speech);
            let mfcc = apply_feature_transforms(mfcc, config);
            let result = if let Some(ref cnn_recognizer) = cnn {
                cnn_recognizer.predict(&mfcc)
            } else {
                matcher.as_ref().unwrap().recognize(&mfcc, templates.as_ref().unwrap())
            };

            word_confidences.push(result.confidence);

            match &result.word {
                Some(predicted) if predicted == &expected_word => {
                    word_correct += 1;
                    if verbose {
                        println!(
                            "  ✓ {}: {} (conf={:.0}%, dist={:.2})",
                            filename, predicted, result.confidence * 100.0, result.best_distance
                        );
                    }
                }
                Some(predicted) => {
                    word_wrong += 1;
                    if verbose {
                        println!(
                            "  ✗ {}: {} (expected {}, conf={:.0}%, dist={:.2})",
                            filename, predicted, expected_word, result.confidence * 100.0, result.best_distance
                        );
                    }
                }
                None => {
                    word_rejected += 1;
                    if verbose {
                        println!(
                            "  ? {}: rejected (conf={:.0}%, dist={:.2})",
                            filename, result.confidence * 100.0, result.best_distance
                        );
                    }
                }
            }
        }

        total_correct += word_correct;
        total_wrong += word_wrong;
        total_rejected += word_rejected;
        per_word_stats.push((expected_word, word_correct, word_wrong, word_rejected, word_confidences));
    }

    // Summary.
    println!("========================================");
    println!("TEST RESULTS");
    println!("========================================");
    println!();

    println!("{:<12} {:>6} {:>6} {:>6} {:>6} {:>8}", "Word", "OK", "Wrong", "Rej", "Total", "Avg Conf");
    println!("{}", "-".repeat(52));

    for (word, correct, wrong, rejected, confidences) in &per_word_stats {
        let total = correct + wrong + rejected;
        let avg_conf = if confidences.is_empty() {
            0.0
        } else {
            confidences.iter().sum::<f32>() / confidences.len() as f32 * 100.0
        };
        println!(
            "{:<12} {:>6} {:>6} {:>6} {:>6} {:>7.1}%",
            word, correct, wrong, rejected, total, avg_conf
        );
    }

    println!("{}", "-".repeat(52));
    let grand_total = total_correct + total_wrong + total_rejected;
    let accuracy = if grand_total > 0 {
        total_correct as f32 / grand_total as f32 * 100.0
    } else {
        0.0
    };
    println!(
        "{:<12} {:>6} {:>6} {:>6} {:>6} {:>7.1}%",
        "TOTAL", total_correct, total_wrong, total_rejected, grand_total, accuracy
    );

    if total_no_speech > 0 {
        println!();
        println!("Note: {} files had no speech detected (excluded from stats).", total_no_speech);
    }

    println!();
    println!(
        "Overall accuracy: {}/{} = {:.1}%",
        total_correct, grand_total, accuracy
    );

    Ok(())
}

/// Process a single WAV file through the feature pipeline.
/// Returns None if no speech detected.
fn process_wav_to_mfcc(
    wav_path: &std::path::Path,
    config: &SpeekConfig,
    mfcc_extractor: &mut MfccExtractor,
) -> Result<Option<MfccSequence>> {
    let (mut samples, _sr) = wav::load_wav(wav_path)?;
    preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

    let frame_length = config.frame_length_samples();
    let frame_step = config.frame_step_samples();
    let trimmed = energy_vad::trim_silence(
        &samples,
        config.audio.sample_rate,
        frame_length,
        frame_step,
        &config.vad,
    );

    match trimmed {
        Some(speech) => {
            let mfcc = mfcc_extractor.extract(&speech);
            let mfcc = apply_feature_transforms(mfcc, config);
            Ok(Some(mfcc))
        }
        None => Ok(None),
    }
}

fn cmd_cnn_train(
    config: &SpeekConfig,
    wav_dir: &PathBuf,
    epochs_override: Option<usize>,
    batch_size_override: Option<usize>,
    lr_override: Option<f64>,
) -> Result<()> {
    if !wav_dir.exists() {
        bail!("Directory {:?} does not exist.", wav_dir);
    }

    let epochs = epochs_override.unwrap_or(config.classifier.epochs);
    let batch_size = batch_size_override.unwrap_or(config.classifier.batch_size);
    let learning_rate = lr_override.unwrap_or(config.classifier.learning_rate);
    let max_frames = config.classifier.max_frames;
    let model_dir = &config.classifier.model_dir;

    // Load samples using shared MFCC pipeline.
    let mut mfcc_extractor =
        MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    println!("Loading WAV files from {:?}...", wav_dir);
    let (samples, vocab) = cnn_dataset::load_samples_from_dir(wav_dir, |path| {
        process_wav_to_mfcc(path, config, &mut mfcc_extractor)
    })?;

    if samples.len() < 2 {
        bail!("Need at least 2 samples to train. Found {}.", samples.len());
    }

    // Split into train/val.
    let (train_samples, val_samples) =
        cnn_dataset::train_val_split(&samples, config.classifier.validation_split);

    // Save vocabulary mapping.
    std::fs::create_dir_all(model_dir)?;
    let vocab_path = model_dir.join("cnn_vocab.json");
    vocab.save(&vocab_path)?;
    println!("Vocabulary saved to {:?}", vocab_path);
    println!();

    // Train.
    cnn_training::train_cnn(
        &train_samples,
        &val_samples,
        &vocab,
        max_frames,
        epochs,
        batch_size,
        learning_rate,
        config.classifier.early_stopping_patience,
        model_dir,
        4, // num_augments per sample
    )?;

    println!();
    println!("To use CNN for recognition, set in speeko.toml:");
    println!("  [recognizer]");
    println!("  mode = \"cnn\"");

    Ok(())
}

fn cmd_reset(
    config: &SpeekConfig,
    word: Option<String>,
    all: bool,
    confirm: bool,
) -> Result<()> {
    let store = TemplateStore::new(&config.paths.templates_dir)?;

    if all {
        if !confirm {
            eprint!("Delete ALL templates? This cannot be undone. [y/N] ");
            io::stderr().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }
        store.delete_all()?;
        println!("All templates deleted.");
    } else if let Some(word) = word {
        let word = word.to_lowercase();
        if !confirm {
            eprint!(
                "Delete all templates for '{}'? This cannot be undone. [y/N] ",
                word
            );
            io::stderr().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }
        store.delete_word(&word)?;
        println!("Templates for '{}' deleted.", word);
    } else {
        bail!("Specify a word to reset, or use --all to delete everything.");
    }

    Ok(())
}
