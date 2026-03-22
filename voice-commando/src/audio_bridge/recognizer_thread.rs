use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use ringbuf::traits::Consumer;
use vc_classifier::inference::CnnRecognizer;
use vc_common::config::{DspConfig, MfccConfig, VadConfig};
use vc_common::types::RecognitionResult;
use vc_dsp::preprocess;
use vc_features::cmn;
use vc_features::delta;
use vc_features::mfcc::MfccExtractor;
use vc_vad::energy_vad;

/// Wire type sent from recognizer thread to ECS.
#[derive(Debug, Clone)]
pub struct RecognizedWord {
    pub result: RecognitionResult,
    pub recognized_at: Instant,
}

/// Configuration for the recognizer thread.
pub struct RecognizerThreadConfig {
    pub sample_rate: u32,
    pub dsp: DspConfig,
    pub vad: VadConfig,
    pub mfcc: MfccConfig,
    pub model_dir: std::path::PathBuf,
    pub max_frames: usize,
    pub confidence_threshold: f32,
    pub poll_interval_ms: u64,
    pub post_recognition_cooldown_ms: u64,
    pub use_cmn: bool,
    pub use_deltas: bool,
}

/// Handle to the recognizer thread.
pub struct RecognizerThread {
    _handle: Option<std::thread::JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
}

impl RecognizerThread {
    /// Start the recognizer thread.
    pub fn start(
        mut consumer: ringbuf::HeapCons<f32>,
        config: RecognizerThreadConfig,
        sender: Sender<RecognizedWord>,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);

        let handle = std::thread::spawn(move || {
            let poll_interval = Duration::from_millis(config.poll_interval_ms);

            // Load CNN model
            let cnn = match CnnRecognizer::load(
                &config.model_dir,
                config.confidence_threshold,
                config.max_frames,
            ) {
                Ok(c) => {
                    tracing::info!("RecognizerThread: CNN model loaded from {:?}", config.model_dir);
                    Some(c)
                }
                Err(e) => {
                    tracing::warn!(
                        "RecognizerThread: no CNN model at {:?}: {}. \
                         Voice recognition disabled. Train with: `speeko cnn-train`",
                        config.model_dir,
                        e
                    );
                    None
                }
            };

            // Initialize MFCC extractor
            let mut mfcc_extractor =
                MfccExtractor::new(config.sample_rate, &config.dsp, &config.mfcc);

            let frame_length =
                (config.sample_rate as f32 * config.dsp.frame_length_ms / 1000.0) as usize;
            let frame_step =
                (config.sample_rate as f32 * config.dsp.frame_step_ms / 1000.0) as usize;

            // Sliding window buffer — 3 seconds gives VAD enough context
            // to estimate a good noise floor
            let window_seconds = 3.0f32;
            let window_capacity = (config.sample_rate as f32 * window_seconds) as usize;
            let mut window: Vec<f32> = Vec::with_capacity(window_capacity);

            // Minimum samples before attempting VAD (1.5s = enough for noise floor)
            let min_window_samples = (config.sample_rate as f32 * 1.5) as usize;

            let cooldown_duration =
                Duration::from_millis(config.post_recognition_cooldown_ms);
            let mut last_recognition = Instant::now() - cooldown_duration;

            tracing::info!(
                "RecognizerThread: started (poll={}ms, cooldown={}ms, window={:.1}s, model={})",
                config.poll_interval_ms, config.post_recognition_cooldown_ms,
                window_seconds,
                if cnn.is_some() { "loaded" } else { "none" }
            );

            while !cancel_clone.load(Ordering::Relaxed) {
                // 1. Drain samples from ring buffer (always, to prevent overflow)
                let mut temp = [0.0f32; 1600]; // ~100ms at 16kHz
                let count = consumer.pop_slice(&mut temp);
                if count > 0 {
                    window.extend_from_slice(&temp[..count]);
                    // Cap window size
                    if window.len() > window_capacity {
                        let drain = window.len() - window_capacity;
                        window.drain(..drain);
                    }
                }

                // If no model loaded, just drain and discard
                let Some(ref cnn) = cnn else {
                    window.clear();
                    std::thread::sleep(poll_interval);
                    continue;
                };

                // 2. Check cooldown
                if last_recognition.elapsed() < cooldown_duration {
                    std::thread::sleep(poll_interval);
                    continue;
                }

                // 3. Need enough audio for reliable VAD
                if window.len() < min_window_samples {
                    std::thread::sleep(poll_interval);
                    continue;
                }

                // 4. Pre-emphasis on the whole window (matching training pipeline)
                let mut processed = window.clone();
                preprocess::preprocess(&mut processed, config.dsp.pre_emphasis);

                // 5. VAD — detect speech in pre-emphasized window
                if let Some(region) = energy_vad::detect_speech(
                    &processed,
                    config.sample_rate,
                    frame_length,
                    frame_step,
                    &config.vad,
                ) {
                    let speech = &processed[region.start..region.end];
                    let speech_duration_ms = speech.len() as f32 / config.sample_rate as f32 * 1000.0;

                    tracing::debug!(
                        "RecognizerThread: VAD speech region {:.0}ms ({} samples)",
                        speech_duration_ms,
                        speech.len()
                    );

                    // 6. MFCC extraction (on already pre-emphasized speech)
                    let mfcc = mfcc_extractor.extract(speech);

                    // 7. Feature transforms (CMN + deltas)
                    let mfcc = if config.use_cmn {
                        cmn::normalize(&mfcc)
                    } else {
                        mfcc
                    };
                    let mfcc = if config.use_deltas {
                        delta::append_deltas_and_double_deltas(&mfcc)
                    } else {
                        mfcc
                    };

                    // 8. CNN inference
                    let timestamp = Instant::now();
                    let result = cnn.predict(&mfcc);

                    // 9. Only send if word was accepted (above confidence threshold)
                    if result.word.is_some() {
                        let _ = sender.try_send(RecognizedWord {
                            result,
                            recognized_at: timestamp,
                        });
                    } else {
                        tracing::debug!(
                            "RecognizerThread: rejected (confidence={:.3})",
                            result.confidence
                        );
                    }

                    // 10. Clear window and enter cooldown.
                    //     The 1.5s min_window_samples gate ensures the noise
                    //     floor will be re-estimated from fresh mic samples
                    //     before the next detection attempt.
                    window.clear();
                    last_recognition = Instant::now();
                }

                std::thread::sleep(poll_interval);
            }

            tracing::info!("RecognizerThread: stopped");
        });

        Self {
            _handle: Some(handle),
            cancel,
        }
    }

    /// Stop the recognizer thread.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

impl Drop for RecognizerThread {
    fn drop(&mut self) {
        self.stop();
    }
}
