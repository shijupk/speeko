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

    let mut buffer: Vec<Complex<f32>> = Vec::with_capacity(fft_size);
    for i in 0..fft_size {
        let re = if i < frame.len() { frame[i] } else { 0.0 };
        buffer.push(Complex::new(re, 0.0));
    }

    fft.process(&mut buffer);

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
    pub fn power_spectrum(&mut self, frame: &[f32]) -> Vec<f32> {
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
