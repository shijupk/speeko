use vc_common::types::MfccSequence;

/// Cepstral Mean Normalization: subtract the per-utterance mean from each coefficient.
pub fn normalize(mfcc_sequence: &MfccSequence) -> MfccSequence {
    if mfcc_sequence.is_empty() {
        return vec![];
    }

    let num_coeffs = mfcc_sequence[0].len();
    let n = mfcc_sequence.len() as f32;

    let mut means = vec![0.0f32; num_coeffs];
    for frame in mfcc_sequence {
        for (i, &val) in frame.iter().enumerate() {
            means[i] += val;
        }
    }
    for m in &mut means {
        *m /= n;
    }

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
