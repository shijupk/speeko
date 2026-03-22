use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use vc_common::error::SpeekError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Pick a supported stream config from the device, preferring mono and
/// the requested sample rate, but falling back to whatever the device supports.
pub fn choose_input_config(
    device: &cpal::Device,
    desired_rate: u32,
) -> Result<(cpal::StreamConfig, u16, u32)> {
    let supported = device
        .supported_input_configs()
        .context("Failed to query supported input configs")?;

    let configs: Vec<cpal::SupportedStreamConfigRange> = supported.collect();

    if configs.is_empty() {
        anyhow::bail!("Device reports no supported input configurations");
    }

    for c in &configs {
        log::debug!(
            "  Supported: channels={}, rate={}..={}, format={:?}",
            c.channels(),
            c.min_sample_rate().0,
            c.max_sample_rate().0,
            c.sample_format()
        );
    }

    let desired_sr = cpal::SampleRate(desired_rate);

    let mut best: Option<(cpal::SupportedStreamConfig, u16, u32)> = None;
    let mut best_score = i64::MIN;

    for range in &configs {
        let clamped_rate = if desired_sr >= range.min_sample_rate() && desired_sr <= range.max_sample_rate() {
            desired_rate
        } else if desired_sr < range.min_sample_rate() {
            range.min_sample_rate().0
        } else {
            range.max_sample_rate().0
        };

        let mut score: i64 = 0;
        if range.sample_format() == SampleFormat::F32 {
            score += 1000;
        }
        if range.channels() == 1 {
            score += 500;
        }
        if clamped_rate == desired_rate {
            score += 200;
        }
        score -= (clamped_rate as i64 - desired_rate as i64).unsigned_abs() as i64 / 100;
        score -= (range.channels() as i64 - 1) * 50;

        if score > best_score {
            let config = range.clone().with_sample_rate(cpal::SampleRate(clamped_rate));
            best = Some((config, range.channels(), clamped_rate));
            best_score = score;
        }
    }

    match best {
        Some((config, channels, rate)) => {
            let stream_config: cpal::StreamConfig = config.into();
            Ok((stream_config, channels, rate))
        }
        None => {
            let default = device
                .default_input_config()
                .context("No default input config available")?;
            let channels = default.channels();
            let rate = default.sample_rate().0;
            let stream_config: cpal::StreamConfig = default.into();
            Ok((stream_config, channels, rate))
        }
    }
}

/// Simple linear resampling from `from_rate` to `to_rate`.
pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;

        let sample = if idx + 1 < samples.len() {
            samples[idx] as f64 * (1.0 - frac) + samples[idx + 1] as f64 * frac
        } else if idx < samples.len() {
            samples[idx] as f64
        } else {
            0.0
        };
        output.push(sample as f32);
    }

    output
}

/// Downmix multi-channel interleaved audio to mono by averaging channels.
pub fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Record audio from the default input device.
///
/// Returns a buffer of f32 samples in [-1.0, 1.0] at the requested sample rate (mono).
/// Recording stops after `duration_secs` or when `cancel` is set to true.
pub fn record_audio(
    sample_rate: u32,
    duration_secs: f32,
    cancel: &AtomicBool,
) -> Result<Vec<f32>> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or(SpeekError::NoAudioDevice)?;

    log::info!("Using input device: {}", device.name().unwrap_or_default());

    let (stream_config, native_channels, native_rate) =
        choose_input_config(&device, sample_rate)?;

    log::info!(
        "Stream config: {}ch, {}Hz (desired: 1ch, {}Hz)",
        native_channels,
        native_rate,
        sample_rate
    );

    let native_total =
        (native_rate as f32 * duration_secs) as usize * native_channels as usize;
    let samples: Arc<Mutex<Vec<f32>>> =
        Arc::new(Mutex::new(Vec::with_capacity(native_total)));
    let samples_clone = Arc::clone(&samples);
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = Arc::clone(&done);

    let err_fn = |err: cpal::StreamError| {
        log::error!("Audio stream error: {}", err);
    };

    let stream = device
        .build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut buf = samples_clone.lock().unwrap();
                let remaining = native_total.saturating_sub(buf.len());
                if remaining == 0 {
                    done_clone.store(true, Ordering::Relaxed);
                    return;
                }
                let take = remaining.min(data.len());
                buf.extend_from_slice(&data[..take]);
                if buf.len() >= native_total {
                    done_clone.store(true, Ordering::Relaxed);
                }
            },
            err_fn,
            None,
        )
        .context("Failed to build input stream")?;

    stream.play().context("Failed to start audio stream")?;
    log::info!(
        "Recording for {:.1}s at {}Hz ({}ch)...",
        duration_secs,
        native_rate,
        native_channels
    );

    while !done.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    drop(stream);

    if cancel.load(Ordering::Relaxed) {
        log::info!("Recording cancelled.");
        return Err(SpeekError::Cancelled.into());
    }

    let raw = samples.lock().unwrap().clone();

    let mono = downmix_to_mono(&raw, native_channels);
    let result = resample(&mono, native_rate, sample_rate);

    log::info!(
        "Recorded {} samples ({:.2}s) [native: {}ch/{}Hz -> mono/{}Hz]",
        result.len(),
        result.len() as f32 / sample_rate as f32,
        native_channels,
        native_rate,
        sample_rate
    );
    Ok(result)
}

/// List available audio input devices.
pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .context("Failed to enumerate input devices")?;

    let names: Vec<String> = devices
        .filter_map(|d| d.name().ok())
        .collect();
    Ok(names)
}

/// Check if a default audio input device is available.
pub fn has_input_device() -> bool {
    let host = cpal::default_host();
    host.default_input_device().is_some()
}

/// Check for audio clipping in the buffer.
pub fn is_clipped(samples: &[f32], threshold: f32) -> bool {
    let clip_count = samples.iter().filter(|&&s| s.abs() >= threshold).count();
    let ratio = clip_count as f32 / samples.len() as f32;
    ratio > 0.01
}
