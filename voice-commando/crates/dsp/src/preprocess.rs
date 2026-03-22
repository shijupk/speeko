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
    for i in (1..samples.len()).rev() {
        samples[i] -= coeff * samples[i - 1];
    }
    samples[0] *= 1.0 - coeff;
}

/// Normalize samples to [-1.0, 1.0] range based on peak amplitude.
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
