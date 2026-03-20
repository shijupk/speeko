use speeko_common::config::{DspConfig, MfccConfig};
use speeko_common::types::MfccSequence;
use speeko_dsp::fft::FftProcessor;
use speeko_dsp::framing;

use crate::mel::MelFilterbank;

/// Type-II DCT (used to compute MFCCs from log-mel energies).
///
/// Computes `num_coefficients` DCT coefficients from an input of `input_length` values.
/// Returns coefficients 1..=num_coefficients (0th coefficient / energy is discarded).
fn dct_ii(input: &[f32], num_coefficients: usize) -> Vec<f32> {
    let n = input.len();
    let mut result = Vec::with_capacity(num_coefficients);

    for k in 1..=num_coefficients {
        let sum: f32 = input
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                x * (std::f32::consts::PI * k as f32 * (2.0 * i as f32 + 1.0) / (2.0 * n as f32))
                    .cos()
            })
            .sum();
        result.push(sum);
    }

    result
}

/// MFCC extraction pipeline.
///
/// Reusable across multiple utterances. Precomputes the mel filterbank
/// and FFT planner.
pub struct MfccExtractor {
    fft: FftProcessor,
    filterbank: MelFilterbank,
    frame_length: usize,
    frame_step: usize,
    num_coefficients: usize,
    window: Vec<f32>,
}

impl MfccExtractor {
    /// Create a new MFCC extractor from DSP and MFCC config.
    pub fn new(sample_rate: u32, dsp_config: &DspConfig, mfcc_config: &MfccConfig) -> Self {
        let frame_length =
            (sample_rate as f32 * dsp_config.frame_length_ms / 1000.0) as usize;
        let frame_step =
            (sample_rate as f32 * dsp_config.frame_step_ms / 1000.0) as usize;

        let fft = FftProcessor::new(dsp_config.fft_size);
        let filterbank = MelFilterbank::new(
            mfcc_config.num_mel_filters,
            dsp_config.fft_size,
            sample_rate,
            mfcc_config.low_freq,
            mfcc_config.high_freq,
        );
        let window = framing::hamming_window(frame_length);

        log::debug!(
            "MFCC extractor: frame_length={}, frame_step={}, fft_size={}, mel_filters={}, coefficients={}",
            frame_length,
            frame_step,
            dsp_config.fft_size,
            mfcc_config.num_mel_filters,
            mfcc_config.num_coefficients
        );

        Self {
            fft,
            filterbank,
            frame_length,
            frame_step,
            num_coefficients: mfcc_config.num_coefficients,
            window,
        }
    }

    /// Extract MFCC features from preprocessed audio samples.
    ///
    /// Returns a sequence of MFCC vectors, one per frame.
    /// Each vector has `num_coefficients` elements.
    pub fn extract(&mut self, samples: &[f32]) -> MfccSequence {
        let frames = framing::split_frames(samples, self.frame_length, self.frame_step);

        log::info!("Extracting MFCCs from {} frames", frames.len());

        let mut mfcc_sequence = Vec::with_capacity(frames.len());

        for mut frame in frames {
            // Apply window.
            framing::apply_window(&mut frame, &self.window);

            // Compute power spectrum.
            let power_spec = self.fft.power_spectrum(&frame);

            // Apply mel filterbank + log.
            let log_mel = self.filterbank.apply_log(&power_spec);

            // DCT to get MFCCs.
            let mfcc = dct_ii(&log_mel, self.num_coefficients);

            mfcc_sequence.push(mfcc);
        }

        log::info!(
            "MFCC extraction complete: [{} x {}]",
            mfcc_sequence.len(),
            self.num_coefficients
        );

        mfcc_sequence
    }

    pub fn frame_length(&self) -> usize {
        self.frame_length
    }

    pub fn frame_step(&self) -> usize {
        self.frame_step
    }

    pub fn num_coefficients(&self) -> usize {
        self.num_coefficients
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speeko_common::config::{DspConfig, MfccConfig};

    fn test_configs() -> (DspConfig, MfccConfig) {
        let dsp = DspConfig {
            pre_emphasis: 0.97,
            frame_length_ms: 25.0,
            frame_step_ms: 10.0,
            fft_size: 512,
        };
        let mfcc = MfccConfig {
            num_mel_filters: 26,
            num_coefficients: 13,
            low_freq: 0.0,
            high_freq: 8000.0,
        };
        (dsp, mfcc)
    }

    #[test]
    fn test_dct_ii_length() {
        let input = vec![1.0; 26];
        let result = dct_ii(&input, 13);
        assert_eq!(result.len(), 13);
    }

    #[test]
    fn test_mfcc_extraction() {
        let (dsp, mfcc_cfg) = test_configs();
        let mut extractor = MfccExtractor::new(16000, &dsp, &mfcc_cfg);

        // 1 second of a 440Hz sine wave.
        let samples: Vec<f32> = (0..16000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();

        let mfccs = extractor.extract(&samples);
        assert!(!mfccs.is_empty());
        assert_eq!(mfccs[0].len(), 13);
        // All coefficients should be finite.
        assert!(mfccs.iter().all(|frame| frame.iter().all(|c| c.is_finite())));
    }

    #[test]
    fn test_mfcc_deterministic() {
        let (dsp, mfcc_cfg) = test_configs();
        let mut extractor = MfccExtractor::new(16000, &dsp, &mfcc_cfg);

        let samples: Vec<f32> = (0..16000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();

        let mfccs1 = extractor.extract(&samples);
        let mfccs2 = extractor.extract(&samples);
        assert_eq!(mfccs1.len(), mfccs2.len());
        for (f1, f2) in mfccs1.iter().zip(mfccs2.iter()) {
            for (a, b) in f1.iter().zip(f2.iter()) {
                assert!((a - b).abs() < 1e-6, "MFCC not deterministic: {a} vs {b}");
            }
        }
    }
}
