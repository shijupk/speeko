use speeko_common::types::MfccSequence;

/// Cepstral Mean Normalization: subtract the per-utterance mean from each coefficient.
///
/// This removes channel/microphone bias so that templates are more comparable
/// across recording sessions and devices.
pub fn normalize(mfcc_sequence: &MfccSequence) -> MfccSequence {
    if mfcc_sequence.is_empty() {
        return vec![];
    }

    let num_coeffs = mfcc_sequence[0].len();
    let n = mfcc_sequence.len() as f32;

    // Compute per-coefficient mean across all frames.
    let mut means = vec![0.0f32; num_coeffs];
    for frame in mfcc_sequence {
        for (i, &val) in frame.iter().enumerate() {
            means[i] += val;
        }
    }
    for m in &mut means {
        *m /= n;
    }

    // Subtract mean from each frame.
    mfcc_sequence
        .iter()
        .map(|frame| {
            frame
                .iter()
                .zip(means.iter())
                .map(|(&val, &mean)| val - mean)
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmn_zero_mean() {
        let mfcc: MfccSequence = vec![
            vec![2.0, 4.0],
            vec![4.0, 6.0],
            vec![6.0, 8.0],
        ];
        let normed = normalize(&mfcc);
        assert_eq!(normed.len(), 3);
        assert_eq!(normed[0].len(), 2);

        // After CMN, per-coefficient mean should be ~0.
        let num_coeffs = normed[0].len();
        for c in 0..num_coeffs {
            let mean: f32 = normed.iter().map(|f| f[c]).sum::<f32>() / normed.len() as f32;
            assert!(mean.abs() < 1e-6, "Mean of coeff {} should be ~0, got {}", c, mean);
        }
    }

    #[test]
    fn test_cmn_values() {
        let mfcc: MfccSequence = vec![
            vec![1.0],
            vec![3.0],
            vec![5.0],
        ];
        let normed = normalize(&mfcc);
        // mean = 3.0, so: -2.0, 0.0, 2.0
        assert!((normed[0][0] - (-2.0)).abs() < 1e-6);
        assert!((normed[1][0] - 0.0).abs() < 1e-6);
        assert!((normed[2][0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_cmn_empty() {
        let normed = normalize(&vec![]);
        assert!(normed.is_empty());
    }
}
