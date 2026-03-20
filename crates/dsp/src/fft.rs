use rustfft::num_complex::Complex;
use rustfft::{FftDirection, FftPlanner};

/// Compute the power spectrum of a real-valued signal frame.
///
/// The input frame is zero-padded to `fft_size` (must be power of 2).
/// Returns `fft_size / 2 + 1` power spectrum values (non-negative frequencies).
pub fn power_spectrum(frame: &[f32], fft_size: usize) -> Vec<f32> {
    debug_assert!(fft_size.is_power_of_two(), "FFT size must be a power of 2");

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft(fft_size, FftDirection::Forward);

    // Zero-pad and convert to complex.
    let mut buffer: Vec<Complex<f32>> = Vec::with_capacity(fft_size);
    for i in 0..fft_size {
        let re = if i < frame.len() { frame[i] } else { 0.0 };
        buffer.push(Complex::new(re, 0.0));
    }

    fft.process(&mut buffer);

    // Compute power spectrum for non-negative frequencies.
    let num_bins = fft_size / 2 + 1;
    buffer[..num_bins]
        .iter()
        .map(|c| c.norm_sqr())
        .collect()
}

/// Reusable FFT processor to avoid re-planning for every frame.
pub struct FftProcessor {
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    fft_size: usize,
    buffer: Vec<Complex<f32>>,
}

impl FftProcessor {
    pub fn new(fft_size: usize) -> Self {
        assert!(fft_size.is_power_of_two(), "FFT size must be a power of 2");
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft(fft_size, FftDirection::Forward);
        let buffer = vec![Complex::new(0.0, 0.0); fft_size];
        Self {
            fft,
            fft_size,
            buffer,
        }
    }

    /// Compute the power spectrum of a frame, reusing internal buffers.
    /// Returns `fft_size / 2 + 1` values.
    pub fn power_spectrum(&mut self, frame: &[f32]) -> Vec<f32> {
        // Fill buffer with zero-padded input.
        for i in 0..self.fft_size {
            let re = if i < frame.len() { frame[i] } else { 0.0 };
            self.buffer[i] = Complex::new(re, 0.0);
        }

        self.fft.process(&mut self.buffer);

        let num_bins = self.fft_size / 2 + 1;
        self.buffer[..num_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect()
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    pub fn num_bins(&self) -> usize {
        self.fft_size / 2 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_spectrum_length() {
        let frame = vec![0.0; 400];
        let ps = power_spectrum(&frame, 512);
        assert_eq!(ps.len(), 257); // 512/2 + 1
    }

    #[test]
    fn test_power_spectrum_silent() {
        let frame = vec![0.0; 400];
        let ps = power_spectrum(&frame, 512);
        assert!(ps.iter().all(|&v| v < 1e-10));
    }

    #[test]
    fn test_fft_processor_reuse() {
        let mut proc = FftProcessor::new(512);
        let frame1 = vec![1.0; 400];
        let frame2 = vec![0.5; 400];
        let ps1 = proc.power_spectrum(&frame1);
        let ps2 = proc.power_spectrum(&frame2);
        assert_eq!(ps1.len(), 257);
        assert_eq!(ps2.len(), 257);
        // Louder input should have more energy.
        assert!(ps1[0] > ps2[0]);
    }
}
