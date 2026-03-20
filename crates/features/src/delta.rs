use speeko_common::types::MfccSequence;

/// Compute delta (first derivative) coefficients from an MFCC sequence.
///
/// Uses a simple 2-frame window: delta[t] = (mfcc[t+1] - mfcc[t-1]) / 2.
/// Endpoints are handled by repeating the first/last frame.
pub fn compute_deltas(mfcc_sequence: &MfccSequence) -> MfccSequence {
    let n = mfcc_sequence.len();
    if n < 2 {
        return mfcc_sequence
            .iter()
            .map(|frame| vec![0.0; frame.len()])
            .collect();
    }

    let num_coeffs = mfcc_sequence[0].len();
    let mut deltas = Vec::with_capacity(n);

    for t in 0..n {
        let prev = if t > 0 { t - 1 } else { 0 };
        let next = if t < n - 1 { t + 1 } else { n - 1 };

        let delta: Vec<f32> = (0..num_coeffs)
            .map(|c| (mfcc_sequence[next][c] - mfcc_sequence[prev][c]) / 2.0)
            .collect();
        deltas.push(delta);
    }

    deltas
}

/// Append delta coefficients to each MFCC frame, doubling the feature dimension.
pub fn append_deltas(mfcc_sequence: &MfccSequence) -> MfccSequence {
    let deltas = compute_deltas(mfcc_sequence);
    mfcc_sequence
        .iter()
        .zip(deltas.iter())
        .map(|(mfcc, delta)| {
            let mut combined = mfcc.clone();
            combined.extend_from_slice(delta);
            combined
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_shape() {
        let mfcc: MfccSequence = vec![
            vec![1.0, 2.0, 3.0],
            vec![2.0, 3.0, 4.0],
            vec![3.0, 4.0, 5.0],
        ];
        let deltas = compute_deltas(&mfcc);
        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].len(), 3);
    }

    #[test]
    fn test_delta_values() {
        let mfcc: MfccSequence = vec![
            vec![0.0],
            vec![2.0],
            vec![4.0],
        ];
        let deltas = compute_deltas(&mfcc);
        // delta[0] = (mfcc[1] - mfcc[0]) / 2 = 1.0
        assert!((deltas[0][0] - 1.0).abs() < 1e-6);
        // delta[1] = (mfcc[2] - mfcc[0]) / 2 = 2.0
        assert!((deltas[1][0] - 2.0).abs() < 1e-6);
        // delta[2] = (mfcc[2] - mfcc[1]) / 2 = 1.0
        assert!((deltas[2][0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_append_deltas() {
        let mfcc: MfccSequence = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![5.0, 6.0],
        ];
        let combined = append_deltas(&mfcc);
        assert_eq!(combined.len(), 3);
        assert_eq!(combined[0].len(), 4); // 2 MFCC + 2 delta
    }
}
