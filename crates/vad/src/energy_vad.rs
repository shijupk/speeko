use speeko_common::config::VadConfig;
use speeko_common::types::VadRegion;

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

/// Compute zero-crossing rate for each frame.
#[allow(dead_code)]
fn frame_zcr(samples: &[f32], frame_length: usize, frame_step: usize) -> Vec<f32> {
    let mut zcrs = Vec::new();
    let mut start = 0;
    while start + frame_length <= samples.len() {
        let frame = &samples[start..start + frame_length];
        let crossings: usize = frame
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        zcrs.push(crossings as f32 / (frame_length - 1) as f32);
        start += frame_step;
    }
    zcrs
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

    // Estimate silence threshold robustly:
    // 1) Skip the first few frames (mic settling / keystroke transients).
    // 2) Sort remaining energies and take the bottom 20% as silence estimate.
    // 3) Apply threshold_factor * stddev above the mean.
    // 4) Enforce a minimum floor so near-zero silence doesn't trigger on everything.
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

    // Minimum energy floor: typical speech energy is 0.001-0.1+, so a floor
    // of 0.0001 prevents triggering on near-silent digital noise.
    let threshold = adaptive_threshold.max(0.0001);

    log::debug!(
        "VAD threshold: {:.6} (silence mean={:.6}, stddev={:.6})",
        threshold,
        mean,
        stddev
    );

    // Mark active frames.
    let active: Vec<bool> = energies.iter().map(|&e| e > threshold).collect();

    // Find first and last active frame.
    let first_active = active.iter().position(|&a| a);
    let last_active = active.iter().rposition(|&a| a);

    let (first, last) = match (first_active, last_active) {
        (Some(f), Some(l)) => (f, l),
        _ => {
            log::info!("No speech detected by VAD");
            return None;
        }
    };

    // Apply hangover: extend the end by hangover frames.
    let hangover_frames =
        (config.hangover_ms as f32 / 1000.0 * sample_rate as f32 / frame_step as f32) as usize;
    let last_with_hangover = (last + hangover_frames).min(energies.len() - 1);

    // Convert frame indices to sample indices.
    let start_sample = first * frame_step;
    let end_sample = (last_with_hangover * frame_step + frame_length).min(samples.len());

    // Apply padding.
    let padding_samples = (config.padding_ms as f32 / 1000.0 * sample_rate as f32) as usize;
    let start_padded = start_sample.saturating_sub(padding_samples);
    let end_padded = (end_sample + padding_samples).min(samples.len());

    // Check minimum duration.
    let mut duration_samples = end_padded - start_padded;
    let mut duration_ms = (duration_samples as f32 / sample_rate as f32 * 1000.0) as u32;

    if duration_ms < config.min_utterance_ms {
        log::info!(
            "Detected speech too short: {}ms (min {}ms)",
            duration_ms,
            config.min_utterance_ms
        );
        return None;
    }

    // Cap maximum utterance length, centering on the highest-energy region.
    let mut start_final = start_padded;
    let mut end_capped = end_padded;
    if config.max_utterance_ms > 0 && duration_ms > config.max_utterance_ms {
        let max_samples = (config.max_utterance_ms as f32 / 1000.0 * sample_rate as f32) as usize;

        // Find the peak energy frame within the detected region to center around.
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

        // Center the cap window around the peak.
        let half = max_samples / 2;
        let cap_start = if peak_sample > half { peak_sample - half } else { 0 };
        let cap_end = (cap_start + max_samples).min(samples.len());
        // Clamp to the original detected region boundaries.
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

    log::info!(
        "VAD detected speech: {:.0}ms - {:.0}ms (duration {:.0}ms)",
        start_final as f32 / sample_rate as f32 * 1000.0,
        end_capped as f32 / sample_rate as f32 * 1000.0,
        duration_ms as f32
    );

    Some(region)
}

/// Trim audio buffer to the detected speech region.
/// Returns the trimmed audio, or None if no speech detected.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn default_vad_config() -> VadConfig {
        VadConfig {
            silence_frames: 10,
            threshold_factor: 3.5,
            min_utterance_ms: 100,
            max_utterance_ms: 1500,
            hangover_ms: 50,
            padding_ms: 30,
        }
    }

    #[test]
    fn test_silent_audio() {
        let samples = vec![0.0; 16000]; // 1s silence
        let config = default_vad_config();
        let result = detect_speech(&samples, 16000, 400, 160, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_loud_audio() {
        // 0.5s silence + 0.5s loud + 0.5s silence
        let mut samples = vec![0.001; 8000]; // low noise
        samples.extend(vec![0.5; 8000]); // speech
        samples.extend(vec![0.001; 8000]); // low noise
        let config = default_vad_config();
        let result = detect_speech(&samples, 16000, 400, 160, &config);
        assert!(result.is_some());
        let region = result.unwrap();
        // Speech should start somewhere around sample 8000.
        assert!(region.start < 9000);
        assert!(region.end > 15000);
    }

    #[test]
    fn test_frame_energies() {
        let samples = vec![1.0; 1600]; // 100ms at 16kHz
        let energies = frame_energies(&samples, 400, 160);
        assert!(!energies.is_empty());
        assert!((energies[0] - 1.0).abs() < 1e-6); // energy of all-ones = 1.0
    }
}
