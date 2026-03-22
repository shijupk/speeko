use vc_common::types::MfccSequence;

/// Compute delta (first derivative) coefficients from an MFCC sequence.
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

/// Append delta and delta-delta (acceleration) coefficients to each MFCC frame,
/// tripling the feature dimension: [MFCC | Δ | ΔΔ].
pub fn append_deltas_and_double_deltas(mfcc_sequence: &MfccSequence) -> MfccSequence {
    let deltas = compute_deltas(mfcc_sequence);
    let double_deltas = compute_deltas(&deltas);
    mfcc_sequence
        .iter()
        .zip(deltas.iter())
        .zip(double_deltas.iter())
        .map(|((mfcc, delta), ddelta)| {
            let mut combined = mfcc.clone();
            combined.extend_from_slice(delta);
            combined.extend_from_slice(ddelta);
            combined
        })
        .collect()
}
