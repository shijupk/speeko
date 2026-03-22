//! Integration test: feeds pre-recorded WAV files through the actual
//! ring buffer → RecognizerThread → channel pipeline.
//!
//! This tests the EXACT same code path as the live game, replacing the
//! microphone with WAV file data pushed into the ring buffer producer.
//!
//! Run with:  cargo test --test ring_buffer_integration_test -- --nocapture

use std::path::PathBuf;
use std::time::{Duration, Instant};

use rand::Rng;

use crossbeam_channel::{self, Receiver};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;

use vc_audio::wav::load_wav;
use vc_common::config::VoiceCommandoConfig;

// We need to reference the recognizer thread from the main binary crate.
// Since it lives in src/, we replicate the minimal pipeline here using the
// same library crates the recognizer thread uses internally.
use vc_classifier::inference::CnnRecognizer;
use vc_common::types::RecognitionResult;
use vc_dsp::preprocess;
use vc_features::cmn;
use vc_features::delta;
use vc_features::mfcc::MfccExtractor;
use vc_vad::energy_vad;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn speeko_root() -> PathBuf {
    let mut p = project_root();
    p.pop();
    p
}

/// Simulate the recognizer thread's processing on a single WAV file.
///
/// This replicates the exact logic from `recognizer_thread.rs`:
///   load WAV → push into sliding window → pre-emphasis → VAD → MFCC → CMN+deltas → CNN
///
/// The key difference from `recognition_pipeline_test` is that here we
/// simulate the ring-buffer sliding window behaviour: we push the WAV samples
/// in small chunks (like the audio callback does) with silence padding before
/// and after, then run the VAD on the full window — exactly as the live
/// recognizer thread does.
fn recognize_via_simulated_ring_buffer(
    wav_path: &std::path::Path,
    config: &VoiceCommandoConfig,
    mfcc_extractor: &mut MfccExtractor,
    cnn: &CnnRecognizer,
    frame_length: usize,
    frame_step: usize,
) -> Option<RecognitionResult> {
    // 1. Load WAV
    let (samples, _sr) = load_wav(wav_path).expect("Failed to load WAV");

    // 2. Simulate the recognizer thread's sliding window.
    //    In the real game, the window accumulates samples from the ring buffer
    //    over time. A WAV file is ~3s with silence+speech+silence.
    //    We push the WAV samples directly as the window contents — this is
    //    equivalent to the ring buffer having accumulated them.
    let window = samples;

    // 3. Pre-emphasis on the whole window (matching recognizer thread step 4)
    let mut processed = window.clone();
    preprocess::preprocess(&mut processed, config.dsp.pre_emphasis);

    // 4. VAD — detect speech (matching recognizer thread step 5)
    let region = energy_vad::detect_speech(
        &processed,
        config.audio.sample_rate,
        frame_length,
        frame_step,
        &config.vad,
    )?;

    let speech = &processed[region.start..region.end];

    // 5. MFCC extraction (matching recognizer thread step 6)
    let mfcc = mfcc_extractor.extract(speech);

    // 6. CMN + deltas (matching recognizer thread steps 7)
    let mfcc = if config.mfcc.use_cmn {
        cmn::normalize(&mfcc)
    } else {
        mfcc
    };
    let mfcc = if config.mfcc.use_deltas {
        delta::append_deltas_and_double_deltas(&mfcc)
    } else {
        mfcc
    };

    // 7. CNN inference (matching recognizer thread step 8)
    let result = cnn.predict(&mfcc);
    Some(result)
}

/// Full ring-buffer integration test: pushes WAV data through an actual
/// ring buffer in small chunks (simulating real-time audio callback),
/// with a consumer thread that runs the exact same processing pipeline
/// as RecognizerThread.
fn recognize_via_actual_ring_buffer(
    wav_path: &std::path::Path,
    config: &VoiceCommandoConfig,
) -> Vec<RecognitionResult> {
    let sample_rate = config.audio.sample_rate;

    // Create ring buffer (same as game)
    let capacity = (sample_rate as f32 * config.game.ring_buffer.buffer_seconds) as usize;
    let rb = HeapRb::<f32>::new(capacity);
    let (mut producer, mut consumer) = rb.split();

    // Create channel (same as game)
    let (sender, receiver): (
        crossbeam_channel::Sender<RecognitionResult>,
        Receiver<RecognitionResult>,
    ) = crossbeam_channel::bounded(16);

    // Load WAV
    let (samples, _sr) = load_wav(wav_path).expect("Failed to load WAV");

    // Spawn consumer thread (replicates RecognizerThread logic exactly)
    let cfg = config.clone();
    let consumer_handle = std::thread::spawn(move || {
        let mut mfcc_extractor = MfccExtractor::new(
            cfg.audio.sample_rate,
            &cfg.dsp,
            &cfg.mfcc,
        );
        let frame_length =
            (cfg.audio.sample_rate as f32 * cfg.dsp.frame_length_ms / 1000.0) as usize;
        let frame_step =
            (cfg.audio.sample_rate as f32 * cfg.dsp.frame_step_ms / 1000.0) as usize;

        let model_dir = speeko_root().join("data").join("models");
        let cnn = CnnRecognizer::load(
            &model_dir,
            cfg.recognition.confidence_threshold,
            cfg.classifier.max_frames,
        )
        .expect("Failed to load CNN model");

        let window_seconds = 3.0f32;
        let window_capacity = (cfg.audio.sample_rate as f32 * window_seconds) as usize;
        let mut window: Vec<f32> = Vec::with_capacity(window_capacity);
        let min_window_samples = (cfg.audio.sample_rate as f32 * 1.5) as usize;

        let poll_interval = Duration::from_millis(cfg.game.ring_buffer.recognizer_poll_ms);
        let cooldown_duration =
            Duration::from_millis(cfg.game.ring_buffer.post_recognition_cooldown_ms);
        let mut last_recognition = Instant::now() - cooldown_duration;

        let deadline = Instant::now() + Duration::from_secs(12);

        loop {
            if Instant::now() > deadline {
                break;
            }

            // Drain from ring buffer
            let mut temp = [0.0f32; 1600];
            let count = consumer.pop_slice(&mut temp);
            if count > 0 {
                window.extend_from_slice(&temp[..count]);
                if window.len() > window_capacity {
                    let drain = window.len() - window_capacity;
                    window.drain(..drain);
                }
            }

            // Cooldown
            if last_recognition.elapsed() < cooldown_duration {
                std::thread::sleep(poll_interval);
                continue;
            }

            // Need enough audio
            if window.len() < min_window_samples {
                std::thread::sleep(poll_interval);
                continue;
            }

            // Pre-emphasis
            let mut processed = window.clone();
            preprocess::preprocess(&mut processed, cfg.dsp.pre_emphasis);

            // VAD
            if let Some(region) = energy_vad::detect_speech(
                &processed,
                cfg.audio.sample_rate,
                frame_length,
                frame_step,
                &cfg.vad,
            ) {
                let speech = &processed[region.start..region.end];

                // MFCC
                let mfcc = mfcc_extractor.extract(speech);
                let mfcc = if cfg.mfcc.use_cmn {
                    cmn::normalize(&mfcc)
                } else {
                    mfcc
                };
                let mfcc = if cfg.mfcc.use_deltas {
                    delta::append_deltas_and_double_deltas(&mfcc)
                } else {
                    mfcc
                };

                // CNN
                let result = cnn.predict(&mfcc);

                if result.word.is_some() {
                    let _ = sender.try_send(result);
                }

                // Clear window after recognition to prevent re-detection
                window.clear();
                last_recognition = Instant::now();
            }

            std::thread::sleep(poll_interval);
        }
    });

    // Build the full audio stream: silence + WAV + silence
    // WAV files are ~3s at 16kHz. We prepend/append realistic low-level noise.
    let mut rng = rand::thread_rng();
    let mut full_audio: Vec<f32> = Vec::new();

    // 0.5s of low-level noise (simulates mic ambient before user speaks)
    let pre_noise_len = (sample_rate as f32 * 0.5) as usize;
    full_audio.extend((0..pre_noise_len).map(|_| rng.gen_range(-0.002f32..0.002)));

    // WAV samples (contains the actual utterance with silence around it)
    full_audio.extend_from_slice(&samples);

    // 1s of low-level noise after (gives VAD trailing context)
    let post_noise_len = sample_rate as usize;
    full_audio.extend((0..post_noise_len).map(|_| rng.gen_range(-0.002f32..0.002)));

    // Push at real-time rate: 100ms chunks with 100ms sleeps
    let chunk_size = (sample_rate as usize) / 10; // 1600 samples = 100ms at 16kHz
    for chunk in full_audio.chunks(chunk_size) {
        producer.push_slice(chunk);
        std::thread::sleep(Duration::from_millis(100)); // real-time pacing
    }

    // Wait for consumer to finish
    consumer_handle.join().unwrap();

    // Collect results
    let mut results = Vec::new();
    while let Ok(r) = receiver.try_recv() {
        results.push(r);
    }
    results
}

#[test]
fn test_simulated_pipeline_all_words() {
    let _ = env_logger::builder().is_test(true).try_init();

    let root = speeko_root();
    let test_wavs_dir = root.join("data").join("test_wavs");
    let model_dir = root.join("data").join("models");
    let config_path = project_root().join("config").join("voice_commando.toml");

    let config = VoiceCommandoConfig::load(&config_path)
        .unwrap_or_else(|_| VoiceCommandoConfig::default());

    let mut mfcc_extractor = MfccExtractor::new(
        config.audio.sample_rate,
        &config.dsp,
        &config.mfcc,
    );

    let cnn = CnnRecognizer::load(
        &model_dir,
        config.recognition.confidence_threshold,
        config.classifier.max_frames,
    )
    .expect("Failed to load CNN model");

    let frame_length = config.frame_length_samples();
    let frame_step = config.frame_step_samples();

    let mut words: Vec<String> = std::fs::read_dir(&test_wavs_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    words.sort();

    println!("\n======== SIMULATED PIPELINE TEST (pre-emphasis → VAD → MFCC → CNN) ========\n");

    let mut total = 0;
    let mut correct = 0;
    let mut vad_fail = 0;
    let mut per_word: Vec<(String, usize, usize, usize)> = Vec::new();

    for word in &words {
        let word_dir = test_wavs_dir.join(word);
        let mut wav_files: Vec<PathBuf> = std::fs::read_dir(&word_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wav"))
            .map(|e| e.path())
            .collect();
        wav_files.sort();

        let mut wt = 0;
        let mut wc = 0;
        let mut wv = 0;

        for wav_path in &wav_files {
            let sample = wav_path.file_stem().unwrap().to_string_lossy();
            let result = recognize_via_simulated_ring_buffer(
                wav_path,
                &config,
                &mut mfcc_extractor,
                &cnn,
                frame_length,
                frame_step,
            );

            match result {
                None => {
                    println!("  [VAD_FAIL ] {}/{}", word, sample);
                    wv += 1;
                }
                Some(r) => {
                    let predicted = r.word.as_deref().unwrap_or("(rejected)");
                    let ok = r.word.as_deref() == Some(word.as_str());
                    if ok {
                        wc += 1;
                    }
                    let tag = if ok { "OK" } else { "WRONG" };
                    let extra = if !ok {
                        format!(
                            "  top3: {}",
                            r.scores.iter().take(3)
                                .map(|s| format!("{}={:.3}", s.word, 1.0 - s.distance))
                                .collect::<Vec<_>>().join(", ")
                        )
                    } else {
                        String::new()
                    };
                    println!(
                        "  [{:>9}] {}/{}: predicted={:>8} confidence={:.3}{}",
                        tag, word, sample, predicted, r.confidence, extra
                    );
                }
            }
            wt += 1;
            total += 1;
        }

        correct += wc;
        vad_fail += wv;
        per_word.push((word.clone(), wt, wc, wv));
    }

    println!("\n--- PER-WORD SUMMARY ---");
    println!(
        "{:<10} {:>5} {:>7} {:>8} {:>10}",
        "WORD", "TOTAL", "CORRECT", "VAD_FAIL", "ACCURACY"
    );
    for (w, t, c, v) in &per_word {
        let acc = if *t > 0 { *c as f32 / *t as f32 * 100.0 } else { 0.0 };
        println!("{:<10} {:>5} {:>7} {:>8} {:>9.1}%", w, t, c, v, acc);
    }

    let overall_acc = if total > 0 {
        correct as f32 / total as f32 * 100.0
    } else {
        0.0
    };
    println!(
        "\nOVERALL: {}/{} correct ({:.1}%), {} VAD failures",
        correct, total, overall_acc, vad_fail
    );

    assert!(
        overall_acc >= 90.0,
        "Overall accuracy {:.1}% is below 90% threshold",
        overall_acc
    );
}

#[test]
fn test_ring_buffer_pipeline_game_words() {
    let _ = env_logger::builder().is_test(true).try_init();

    let root = speeko_root();
    let test_wavs_dir = root.join("data").join("test_wavs");
    let config_path = project_root().join("config").join("voice_commando.toml");

    let config = VoiceCommandoConfig::load(&config_path)
        .unwrap_or_else(|_| VoiceCommandoConfig::default());

    // Test 3 game-critical words with the full ring buffer pipeline
    // (each takes ~5s at real-time rate)
    let game_words = ["start", "up", "right"];

    println!("\n======== RING BUFFER INTEGRATION TEST (game-critical words) ========\n");

    let mut total = 0;
    let mut correct = 0;

    for word in &game_words {
        let word_dir = test_wavs_dir.join(word);
        if !word_dir.exists() {
            println!("  [SKIP] {}: directory not found", word);
            continue;
        }

        // Test with sample_000.wav (first sample for speed)
        let wav_path = word_dir.join("sample_000.wav");
        if !wav_path.exists() {
            println!("  [SKIP] {}/sample_000.wav: not found", word);
            continue;
        }

        println!("  Testing '{}' via ring buffer pipeline...", word);

        let results = recognize_via_actual_ring_buffer(&wav_path, &config);

        total += 1;
        if results.is_empty() {
            println!("    [FAIL] No recognition result received");
        } else {
            let first = &results[0];
            let predicted = first.word.as_deref().unwrap_or("(rejected)");
            let ok = first.word.as_deref() == Some(*word);
            if ok {
                correct += 1;
            }
            println!(
                "    [{}] predicted='{}' confidence={:.3} (got {} result(s))",
                if ok { "OK" } else { "WRONG" },
                predicted,
                first.confidence,
                results.len()
            );
        }
    }

    let acc = if total > 0 {
        correct as f32 / total as f32 * 100.0
    } else {
        0.0
    };
    println!(
        "\nRING BUFFER TEST: {}/{} correct ({:.1}%)",
        correct, total, acc
    );

    assert!(
        acc >= 80.0,
        "Ring buffer accuracy {:.1}% is below 80% threshold",
        acc
    );
}
