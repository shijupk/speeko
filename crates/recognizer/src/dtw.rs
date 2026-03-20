use speeko_common::types::MfccSequence;

/// Compute weighted Euclidean distance between two feature vectors.
///
/// Lower-order coefficients receive higher weight since they carry more
/// spectral shape information. Weight decays as `1 / (1 + 0.1 * i)` within
/// each group of 13 coefficients (MFCC, delta, delta-delta).
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .enumerate()
        .map(|(i, (&x, &y))| {
            let idx_in_group = i % 13;
            let w = 1.0 / (1.0 + 0.1 * idx_in_group as f32);
            w * (x - y).powi(2)
        })
        .sum::<f32>()
        .sqrt()
}

/// Compute DTW distance between two MFCC sequences.
///
/// Uses standard dynamic programming with optional Sakoe-Chiba band constraint.
/// - `band_fraction`: fraction of max sequence length for the band width (0.0 = no constraint, 0.2 = typical).
///   Set to 0.0 or 1.0 to disable the band constraint.
///
/// Returns the normalized DTW distance (total cost / alignment path length).
pub fn dtw_distance(query: &MfccSequence, template: &MfccSequence, band_fraction: f32) -> f32 {
    let n = query.len();
    let m = template.len();

    if n == 0 || m == 0 {
        return f32::INFINITY;
    }

    let max_len = n.max(m);
    let band = if band_fraction <= 0.0 || band_fraction >= 1.0 {
        max_len
    } else {
        (max_len as f32 * band_fraction).ceil() as usize
    };

    // Use two-row DP to save memory: O(M) instead of O(N*M).
    let mut prev_row = vec![f32::INFINITY; m];
    let mut curr_row = vec![f32::INFINITY; m];

    // Initialize first cell.
    prev_row[0] = euclidean_distance(&query[0], &template[0]);

    // Initialize first row within band.
    for j in 1..m {
        if j <= band {
            prev_row[j] = prev_row[j - 1] + euclidean_distance(&query[0], &template[j]);
        }
    }

    // Fill the DP matrix row by row.
    for i in 1..n {
        // Determine band limits for this row.
        let j_center = (i as f32 * m as f32 / n as f32) as usize;
        let j_min = if j_center > band { j_center - band } else { 0 };
        let j_max = (j_center + band).min(m - 1);

        // Reset current row.
        for j in 0..m {
            curr_row[j] = f32::INFINITY;
        }

        for j in j_min..=j_max {
            let cost = euclidean_distance(&query[i], &template[j]);
            let mut min_prev = prev_row[j]; // match (diagonal)
            if j > 0 {
                min_prev = min_prev.min(curr_row[j - 1]); // insertion
                min_prev = min_prev.min(prev_row[j - 1]); // diagonal (redundant but explicit)
            }
            curr_row[j] = cost + min_prev;
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    // Normalize by path length (approximated as n + m).
    let raw_distance = prev_row[m - 1];
    raw_distance / (n + m) as f32
}

/// Compute DTW distance without band constraint (full matrix).
pub fn dtw_distance_full(query: &MfccSequence, template: &MfccSequence) -> f32 {
    dtw_distance(query, template, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_sequences() {
        let seq = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let dist = dtw_distance_full(&seq, &seq);
        assert!(dist < 1e-6, "Distance between identical sequences should be ~0, got {dist}");
    }

    #[test]
    fn test_different_sequences() {
        let a = vec![vec![0.0, 0.0], vec![0.0, 0.0]];
        let b = vec![vec![10.0, 10.0], vec![10.0, 10.0]];
        let dist = dtw_distance_full(&a, &b);
        assert!(dist > 1.0, "Distance between different sequences should be large, got {dist}");
    }

    #[test]
    fn test_variable_length() {
        let short = vec![vec![1.0], vec![2.0]];
        let long = vec![vec![1.0], vec![1.5], vec![2.0]];
        let dist = dtw_distance_full(&short, &long);
        assert!(dist.is_finite());
        assert!(dist < 1.0, "Similar sequences of different length should have small distance");
    }

    #[test]
    fn test_band_constraint() {
        let a: MfccSequence = (0..50).map(|i| vec![i as f32]).collect();
        let b: MfccSequence = (0..50).map(|i| vec![i as f32 + 0.1]).collect();
        let dist_full = dtw_distance(&a, &b, 0.0);
        let dist_band = dtw_distance(&a, &b, 0.2);
        // Band should give similar or slightly higher distance for similar sequences.
        assert!(dist_band.is_finite());
        assert!((dist_full - dist_band).abs() < 1.0);
    }

    #[test]
    fn test_empty_sequence() {
        let a = vec![vec![1.0]];
        let empty: MfccSequence = vec![];
        assert!(dtw_distance_full(&a, &empty).is_infinite());
        assert!(dtw_distance_full(&empty, &a).is_infinite());
    }
}
