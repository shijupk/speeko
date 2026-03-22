use vc_common::config::{DspConfig, MfccConfig};
use vc_common::types::MfccSequence;
use vc_dsp::fft::FftProcessor;
use vc_dsp::framing;

use crate::mel::MelFilterbank;

/// Type-II DCT (used to compute MFCCs from log-mel energies).
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
    pub fn extract(&mut self, samples: &[f32]) -> MfccSequence {
        let frames = framing::split_frames(samples, self.frame_length, self.frame_step);

        log::debug!("Extracting MFCCs from {} frames", frames.len());

        let mut mfcc_sequence = Vec::with_capacity(frames.len());

        for mut frame in frames {
            framing::apply_window(&mut frame, &self.window);
            let power_spec = self.fft.power_spectrum(&frame);
            let log_mel = self.filterbank.apply_log(&power_spec);
            let mfcc = dct_ii(&log_mel, self.num_coefficients);
            mfcc_sequence.push(mfcc);
        }

        log::debug!(
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
