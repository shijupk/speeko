use vc_common::config::VadConfig;
use vc_common::types::VadRegion;

/// Compute short-time energy for each frame.
fn frame_energies(samples: &[f32], frame_length: usize, frame_step: usize) -> Vec<f32> {
    let mut energies = Vec::new();
    let mut start = 0;
    while start + frame_length <= samples.len() {
        let energy: f32 = samples[start..start + frame_length]
            .iter()
            .map(|&s| s * s)
            .sum::<f32>()
            / frame_length as f32;
        energies.push(energy);
        start += frame_step;
    }
    energies
}

/// Detect the speech region in an audio buffer using energy-based VAD.
///
/// Returns `Some(VadRegion)` with the start/end sample indices of detected speech,
/// or `None` if no speech is detected.
pub fn detect_speech(
    samples: &[f32],
    sample_rate: u32,
    frame_length: usize,
    frame_step: usize,
    config: &VadConfig,
) -> Option<VadRegion> {
    let energies = frame_energies(samples, frame_length, frame_step);

    if energies.len() < config.silence_frames + 1 {
        log::warn!("Audio too short for VAD analysis ({} frames)", energies.len());
        return None;
    }

    let skip_frames = 3.min(energies.len() / 2);
    let mut sorted_energies: Vec<f32> = energies[skip_frames..].to_vec();
    sorted_energies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let percentile_count = (sorted_energies.len() as f32 * 0.2)
        .max(config.silence_frames as f32)
        .min(sorted_energies.len() as f32) as usize;
    let quiet_frames = &sorted_energies[..percentile_count];
    let mean = quiet_frames.iter().sum::<f32>() / quiet_frames.len() as f32;
    let variance = quiet_frames
        .iter()
        .map(|&e| (e - mean).powi(2))
        .sum::<f32>()
        / quiet_frames.len() as f32;
    let stddev = variance.sqrt();
    let adaptive_threshold = mean + config.threshold_factor * stddev;

    // Enforce absolute minimum energy floor so ambient mic noise never triggers
    let threshold = adaptive_threshold.max(config.min_energy);

    log::debug!(
        "VAD threshold: {:.6} (silence mean={:.6}, stddev={:.6}, min_energy={:.6})",
        threshold,
        mean,
        stddev,
        config.min_energy,
    );

    let active: Vec<bool> = energies.iter().map(|&e| e > threshold).collect();

    let first_active = active.iter().position(|&a| a);
    let last_active = active.iter().rposition(|&a| a);

    let (first, last) = match (first_active, last_active) {
        (Some(f), Some(l)) => (f, l),
        _ => {
            log::trace!("No speech detected by VAD");
            return None;
        }
    };

    // Check that peak energy in the active region is significantly above the noise floor.
    // This prevents low-level ambient fluctuations from being classified as speech.
    // Two conditions must be met:
    //   1. Peak energy must exceed the absolute min_energy floor
    //   2. Peak energy must be at least 3x the noise floor mean (adaptive)
    let peak_energy = energies[first..=last]
        .iter()
        .cloned()
        .fold(0.0f32, f32::max);
    let noise_gate = (mean * 3.0).max(config.min_energy);
    log::debug!(
        "VAD: peak={:.6}, noise_gate={:.6} (mean={:.6}, min_energy={:.6})",
        peak_energy, noise_gate, mean, config.min_energy,
    );
    if peak_energy < noise_gate {
        log::debug!(
            "VAD: peak energy {:.6} below noise gate {:.6}, rejecting",
            peak_energy, noise_gate,
        );
        return None;
    }

    let hangover_frames =
        (config.hangover_ms as f32 / 1000.0 * sample_rate as f32 / frame_step as f32) as usize;
    let last_with_hangover = (last + hangover_frames).min(energies.len() - 1);

    let start_sample = first * frame_step;
    let end_sample = (last_with_hangover * frame_step + frame_length).min(samples.len());

    let padding_samples = (config.padding_ms as f32 / 1000.0 * sample_rate as f32) as usize;
    let start_padded = start_sample.saturating_sub(padding_samples);
    let end_padded = (end_sample + padding_samples).min(samples.len());

    let mut duration_samples = end_padded - start_padded;
    let mut duration_ms = (duration_samples as f32 / sample_rate as f32 * 1000.0) as u32;

    if duration_ms < config.min_utterance_ms {
        log::debug!(
            "Detected speech too short: {}ms (min {}ms)",
            duration_ms,
            config.min_utterance_ms
        );
        return None;
    }

    let mut start_final = start_padded;
    let mut end_capped = end_padded;
    if config.max_utterance_ms > 0 && duration_ms > config.max_utterance_ms {
        let max_samples = (config.max_utterance_ms as f32 / 1000.0 * sample_rate as f32) as usize;

        let region_start_frame = start_padded / frame_step;
        let region_end_frame = (end_padded / frame_step).min(energies.len());
        let peak_frame = if region_start_frame < region_end_frame {
            energies[region_start_frame..region_end_frame]
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| region_start_frame + i)
                .unwrap_or(region_start_frame)
        } else {
            region_start_frame
        };
        let peak_sample = peak_frame * frame_step;

        let half = max_samples / 2;
        let cap_start = if peak_sample > half { peak_sample - half } else { 0 };
        let cap_end = (cap_start + max_samples).min(samples.len());
        start_final = cap_start.max(start_padded);
        end_capped = cap_end.min(end_padded);

        duration_samples = end_capped - start_final;
        duration_ms = (duration_samples as f32 / sample_rate as f32 * 1000.0) as u32;
        log::info!(
            "VAD capped utterance from {:.0}ms to {}ms (centered on peak at {:.0}ms)",
            (end_padded - start_padded) as f32 / sample_rate as f32 * 1000.0,
            duration_ms,
            peak_sample as f32 / sample_rate as f32 * 1000.0
        );
    }

    let region = VadRegion {
        start: start_final,
        end: end_capped,
    };

    log::debug!(
        "VAD detected speech: {:.0}ms - {:.0}ms (duration {:.0}ms)",
        start_final as f32 / sample_rate as f32 * 1000.0,
        end_capped as f32 / sample_rate as f32 * 1000.0,
        duration_ms as f32
    );

    Some(region)
}

/// Trim audio buffer to the detected speech region.
pub fn trim_silence(
    samples: &[f32],
    sample_rate: u32,
    frame_length: usize,
    frame_step: usize,
    config: &VadConfig,
) -> Option<Vec<f32>> {
    let region = detect_speech(samples, sample_rate, frame_length, frame_step, config)?;
    Some(samples[region.start..region.end].to_vec())
}
