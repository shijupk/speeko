use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::*;
use vc_common::error::SpeekError;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::capture::{choose_input_config, downmix_to_mono, resample};

/// Continuous audio capture that writes PCM samples into a ring buffer.
///
/// Unlike `record_audio()`, this runs indefinitely until stopped.
/// The cpal audio callback writes directly into the ring buffer producer,
/// making it safe to use from a real-time audio thread.
pub struct ContinuousCapture {
    _stream: cpal::Stream,
    cancel: Arc<AtomicBool>,
}

impl ContinuousCapture {
    /// Start continuous capture from the default input device.
    ///
    /// Samples are written to `producer` at the negotiated sample rate.
    /// If the ring buffer is full, new samples are dropped (overflow).
    /// Call `stop()` or drop the struct to end capture.
    pub fn start(
        desired_sample_rate: u32,
        mut producer: ringbuf::HeapProd<f32>,
    ) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(SpeekError::NoAudioDevice)?;

        log::info!(
            "ContinuousCapture: using input device: {}",
            device.name().unwrap_or_default()
        );

        let (stream_config, native_channels, native_rate) =
            choose_input_config(&device, desired_sample_rate)?;

        log::info!(
            "ContinuousCapture: stream config: {}ch, {}Hz (desired: 1ch, {}Hz)",
            native_channels,
            native_rate,
            desired_sample_rate
        );

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);
        let overflow_counter = Arc::new(AtomicU64::new(0));
        let overflow_counter_clone = Arc::clone(&overflow_counter);
        let last_overflow_log = Arc::new(AtomicU64::new(0));
        let last_overflow_log_clone = Arc::clone(&last_overflow_log);

        let err_fn = |err: cpal::StreamError| {
            log::error!("ContinuousCapture stream error: {}", err);
        };

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if cancel_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    // Downmix to mono if needed
                    let mono = if native_channels > 1 {
                        downmix_to_mono(data, native_channels)
                    } else {
                        data.to_vec()
                    };
                    // Resample if needed
                    let samples = if native_rate != desired_sample_rate {
                        resample(&mono, native_rate, desired_sample_rate)
                    } else {
                        mono
                    };
                    // Write to ring buffer (drop new samples if full)
                    let written = producer.push_slice(&samples);
                    if written < samples.len() {
                        let dropped = (samples.len() - written) as u64;
                        let total = overflow_counter_clone.fetch_add(dropped, Ordering::Relaxed) + dropped;
                        // Rate-limit overflow warnings to ~once per second
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let last = last_overflow_log_clone.load(Ordering::Relaxed);
                        if now_ms.saturating_sub(last) > 5000 {
                            last_overflow_log_clone.store(now_ms, Ordering::Relaxed);
                            log::warn!(
                                "Ring buffer overflow: {} total samples dropped (consumer may be idle)",
                                total
                            );
                        }
                    }
                },
                err_fn,
                None,
            )
            .context("ContinuousCapture: failed to build input stream")?;

        stream
            .play()
            .context("ContinuousCapture: failed to start audio stream")?;

        log::info!("ContinuousCapture: started");

        Ok(Self {
            _stream: stream,
            cancel,
        })
    }

    /// Stop the audio capture stream.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

impl Drop for ContinuousCapture {
    fn drop(&mut self) {
        self.stop();
        log::info!("ContinuousCapture: stopped (dropped)");
    }
}
