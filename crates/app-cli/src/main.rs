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
use speeko_features::mfcc::MfccExtractor;
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
    }
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

        // Extract MFCC features.
        let timer = Instant::now();
        let mfcc = mfcc_extractor.extract(&speech_samples);
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
    let store = TemplateStore::new(&config.paths.templates_dir)?;
    let templates = store.load_all_templates()?;

    if templates.is_empty() {
        bail!("{}", SpeekError::NoTemplates);
    }

    // Report trained words.
    let counts = store.template_counts()?;
    println!("Loaded templates for {} words:", counts.len());
    for (word, count) in &counts {
        println!("  {} ({} samples)", word, count);
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

    let matcher = TemplateMatcher::new(
        config.recognizer.sakoe_chiba_width,
        config.recognizer.confidence_threshold,
        config.recognizer.max_distance,
    );

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

        // Extract MFCC.
        let mfcc = mfcc_extractor.extract(&speech_samples);

        // Recognize.
        let result = matcher.recognize(&mfcc, &templates);
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

    let store = TemplateStore::new(&config.paths.templates_dir)?;
    let templates = store.load_all_templates()?;

    if templates.is_empty() {
        bail!("{}", SpeekError::NoTemplates);
    }

    let matcher = TemplateMatcher::new(
        config.recognizer.sakoe_chiba_width,
        config.recognizer.confidence_threshold,
        config.recognizer.max_distance,
    );

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
            let result = matcher.recognize(&mfcc, &templates);

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
    let elapsed = timer.elapsed();

    println!(
        "MFCC: [{} x {}] extracted in {:?}",
        mfcc.len(),
        config.mfcc.num_coefficients,
        elapsed
    );

    if dump {
        println!();
        println!("Frame | Coefficients");
        println!("------+{}", "-".repeat(config.mfcc.num_coefficients * 10));
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
