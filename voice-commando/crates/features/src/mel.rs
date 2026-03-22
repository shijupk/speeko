/// Convert frequency in Hz to mel scale.
pub fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

/// Convert mel scale to frequency in Hz.
pub fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10.0f32.powf(mel / 2595.0) - 1.0)
}

/// A triangular mel-scaled filterbank.
pub struct MelFilterbank {
    filters: Vec<Vec<f32>>,
    num_filters: usize,
}

impl MelFilterbank {
    /// Create a new mel filterbank.
    pub fn new(
        num_filters: usize,
        fft_size: usize,
        sample_rate: u32,
        low_freq: f32,
        high_freq: f32,
    ) -> Self {
        let num_bins = fft_size / 2 + 1;
        let low_mel = hz_to_mel(low_freq);
        let high_mel = hz_to_mel(high_freq);

        let num_points = num_filters + 2;
        let mel_points: Vec<f32> = (0..num_points)
            .map(|i| low_mel + (high_mel - low_mel) * i as f32 / (num_points - 1) as f32)
            .collect();

        let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();

        let bin_points: Vec<usize> = hz_points
            .iter()
            .map(|&hz| {
                let bin = (hz * (fft_size as f32 + 1.0) / sample_rate as f32).floor() as usize;
                bin.min(num_bins - 1)
            })
            .collect();

        let mut filters = vec![vec![0.0f32; num_bins]; num_filters];

        for i in 0..num_filters {
            let left = bin_points[i];
            let center = bin_points[i + 1];
            let right = bin_points[i + 2];

            if center > left {
                for k in left..=center {
                    filters[i][k] = (k - left) as f32 / (center - left) as f32;
                }
            }

            if right > center {
                for k in center..=right {
                    filters[i][k] = (right - k) as f32 / (right - center) as f32;
                }
            }
        }

        log::debug!(
            "Created mel filterbank: {} filters, {} bins, {:.0}-{:.0} Hz",
            num_filters,
            num_bins,
            low_freq,
            high_freq
        );

        Self {
            filters,
            num_filters,
        }
    }

    /// Apply the filterbank to a power spectrum.
    pub fn apply(&self, power_spectrum: &[f32]) -> Vec<f32> {
        self.filters
            .iter()
            .map(|filter| {
                filter
                    .iter()
                    .zip(power_spectrum.iter())
                    .map(|(&w, &p)| w * p)
                    .sum::<f32>()
            })
            .collect()
    }

    /// Apply the filterbank and take the log of each energy.
    pub fn apply_log(&self, power_spectrum: &[f32]) -> Vec<f32> {
        self.apply(power_spectrum)
            .iter()
            .map(|&e| (e + 1e-10).ln())
            .collect()
    }

    pub fn num_filters(&self) -> usize {
        self.num_filters
    }
}
