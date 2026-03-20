use rand::Rng;
use speeko_common::types::MfccSequence;

/// Apply a suite of data augmentations to an MFCC sequence.
/// Returns the original plus several augmented variants.
pub fn augment_sample(mfcc: &MfccSequence, num_augments: usize) -> Vec<MfccSequence> {
    let mut rng = rand::thread_rng();
    let mut results = Vec::with_capacity(num_augments + 1);
    results.push(mfcc.clone()); // always include original

    for _ in 0..num_augments {
        let mut augmented = mfcc.clone();

        // Randomly apply 1-3 augmentations per variant.
        let num_ops = rng.gen_range(1..=3);
        for _ in 0..num_ops {
            match rng.gen_range(0..4) {
                0 => augmented = time_stretch(&augmented, &mut rng),
                1 => gaussian_noise(&mut augmented, &mut rng),
                2 => time_shift(&mut augmented, &mut rng),
                3 => frequency_mask(&mut augmented, &mut rng),
                _ => unreachable!(),
            }
        }
        results.push(augmented);
    }

    results
}

/// Time stretch: resample MFCC along the time axis by a random factor in [0.8, 1.2].
fn time_stretch(mfcc: &MfccSequence, rng: &mut impl Rng) -> MfccSequence {
    if mfcc.len() < 2 {
        return mfcc.clone();
    }
    let factor: f64 = rng.gen_range(0.8..1.2);
    let new_len = ((mfcc.len() as f64) * factor).round() as usize;
    let new_len = new_len.max(2);

    let num_coeffs = mfcc[0].len();
    let src_len = mfcc.len();
    let mut result = Vec::with_capacity(new_len);

    for t in 0..new_len {
        let src_pos = t as f64 * (src_len - 1) as f64 / (new_len - 1).max(1) as f64;
        let lo = (src_pos.floor() as usize).min(src_len - 1);
        let hi = (lo + 1).min(src_len - 1);
        let frac = (src_pos - lo as f64) as f32;

        let frame: Vec<f32> = (0..num_coeffs)
            .map(|c| mfcc[lo][c] * (1.0 - frac) + mfcc[hi][c] * frac)
            .collect();
        result.push(frame);
    }

    result
}

/// Additive Gaussian noise on MFCC coefficients, σ ∈ [0.001, 0.01].
fn gaussian_noise(mfcc: &mut MfccSequence, rng: &mut impl Rng) {
    let sigma: f32 = rng.gen_range(0.001..0.01);
    for frame in mfcc.iter_mut() {
        for coeff in frame.iter_mut() {
            // Box-Muller transform for Gaussian.
            let u1: f32 = rng.gen_range(0.0001f32..1.0);
            let u2: f32 = rng.gen::<f32>();
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
            *coeff += z * sigma;
        }
    }
}

/// Random time shift: circular shift by ±5 frames.
fn time_shift(mfcc: &mut MfccSequence, rng: &mut impl Rng) {
    if mfcc.len() < 3 {
        return;
    }
    let max_shift = 5.min(mfcc.len() / 4);
    if max_shift == 0 {
        return;
    }
    let shift: i32 = rng.gen_range(-(max_shift as i32)..=(max_shift as i32));
    if shift == 0 {
        return;
    }

    let len = mfcc.len();
    let mut shifted = Vec::with_capacity(len);
    for i in 0..len {
        let src = ((i as i32 - shift).rem_euclid(len as i32)) as usize;
        shifted.push(mfcc[src].clone());
    }
    *mfcc = shifted;
}

/// SpecAugment-style frequency band masking: zero 1-3 random coefficient bands.
fn frequency_mask(mfcc: &mut MfccSequence, rng: &mut impl Rng) {
    if mfcc.is_empty() || mfcc[0].is_empty() {
        return;
    }
    let num_coeffs = mfcc[0].len();
    let num_masks = rng.gen_range(1..=3.min(num_coeffs));
    for _ in 0..num_masks {
        let band = rng.gen_range(0..num_coeffs);
        for frame in mfcc.iter_mut() {
            frame[band] = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mfcc(frames: usize, coeffs: usize) -> MfccSequence {
        (0..frames)
            .map(|f| (0..coeffs).map(|c| (f * coeffs + c) as f32).collect())
            .collect()
    }

    #[test]
    fn test_augment_sample_count() {
        let mfcc = make_mfcc(50, 13);
        let results = augment_sample(&mfcc, 4);
        assert_eq!(results.len(), 5); // original + 4 augmented
        assert_eq!(results[0], mfcc); // first is always original
    }

    #[test]
    fn test_time_stretch_changes_length() {
        let mfcc = make_mfcc(50, 13);
        let mut rng = rand::thread_rng();
        // Run several times — length should sometimes differ.
        let mut lengths = std::collections::HashSet::new();
        for _ in 0..20 {
            let stretched = time_stretch(&mfcc, &mut rng);
            lengths.insert(stretched.len());
        }
        assert!(lengths.len() > 1, "time stretch should vary lengths");
    }

    #[test]
    fn test_gaussian_noise_modifies_values() {
        let original = make_mfcc(10, 13);
        let mut noisy = original.clone();
        let mut rng = rand::thread_rng();
        gaussian_noise(&mut noisy, &mut rng);
        // At least some values should differ.
        let differs = original
            .iter()
            .flatten()
            .zip(noisy.iter().flatten())
            .any(|(a, b)| (a - b).abs() > 1e-8);
        assert!(differs, "noise should modify at least some values");
    }

    #[test]
    fn test_frequency_mask_zeros_bands() {
        let mut mfcc = make_mfcc(10, 13);
        let mut rng = rand::thread_rng();
        frequency_mask(&mut mfcc, &mut rng);
        // At least one coefficient column should be all zeros.
        let num_coeffs = 13;
        let has_zeroed_band = (0..num_coeffs).any(|c| {
            mfcc.iter().all(|frame| frame[c] == 0.0)
        });
        assert!(has_zeroed_band, "frequency mask should zero at least one band");
    }
}
