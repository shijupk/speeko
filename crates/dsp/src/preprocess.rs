/// Remove DC offset by subtracting the mean from all samples.
pub fn remove_dc(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    for s in samples.iter_mut() {
        *s -= mean;
    }
}

/// Apply pre-emphasis filter: y[n] = x[n] - coeff * x[n-1].
/// Typical coefficient is 0.97.
pub fn pre_emphasis(samples: &mut [f32], coeff: f32) {
    if samples.len() < 2 {
        return;
    }
    // Process in reverse to avoid needing a separate buffer.
    for i in (1..samples.len()).rev() {
        samples[i] -= coeff * samples[i - 1];
    }
    // First sample: apply with assumed zero predecessor.
    samples[0] *= 1.0 - coeff;
}

/// Normalize samples to [-1.0, 1.0] range based on peak amplitude.
/// Returns the scaling factor applied (for logging).
pub fn normalize(samples: &mut [f32]) -> f32 {
    if samples.is_empty() {
        return 1.0;
    }
    let peak = samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);

    if peak < 1e-10 {
        return 1.0;
    }

    let scale = 1.0 / peak;
    for s in samples.iter_mut() {
        *s *= scale;
    }
    scale
}

/// Full preprocessing pipeline: DC removal → pre-emphasis → normalization.
pub fn preprocess(samples: &mut [f32], pre_emphasis_coeff: f32) {
    remove_dc(samples);
    pre_emphasis(samples, pre_emphasis_coeff);
    normalize(samples);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_dc() {
        let mut data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        remove_dc(&mut data);
        let mean: f32 = data.iter().sum::<f32>() / data.len() as f32;
        assert!(mean.abs() < 1e-6, "DC should be removed, mean={mean}");
    }

    #[test]
    fn test_pre_emphasis() {
        let mut data = vec![0.0, 1.0, 2.0, 3.0];
        pre_emphasis(&mut data, 0.97);
        // y[1] = 1.0 - 0.97*0.0 = 1.0
        // y[2] = 2.0 - 0.97*1.0 = 1.03
        // y[3] = 3.0 - 0.97*2.0 = 1.06
        assert!((data[1] - 1.0).abs() < 1e-6);
        assert!((data[2] - 1.03).abs() < 1e-6);
        assert!((data[3] - 1.06).abs() < 1e-6);
    }

    #[test]
    fn test_normalize() {
        let mut data = vec![-2.0, 0.0, 1.0];
        normalize(&mut data);
        assert!((data[0] - (-1.0)).abs() < 1e-6);
        assert!((data[2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_silent() {
        let mut data = vec![0.0, 0.0, 0.0];
        let scale = normalize(&mut data);
        assert_eq!(scale, 1.0);
    }
}
