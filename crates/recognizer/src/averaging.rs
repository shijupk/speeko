use speeko_common::types::{MfccSequence, Template};
use std::collections::HashMap;

/// Linearly interpolate an MFCC sequence to a target number of frames.
///
/// Each coefficient is independently interpolated along the time axis.
fn interpolate_sequence(seq: &MfccSequence, target_len: usize) -> MfccSequence {
    if seq.is_empty() || target_len == 0 {
        return vec![];
    }
    if seq.len() == target_len {
        return seq.clone();
    }

    let src_len = seq.len();
    let num_coeffs = seq[0].len();
    let mut result = Vec::with_capacity(target_len);

    for t in 0..target_len {
        // Map target frame index to source (floating point).
        let src_pos = t as f64 * (src_len - 1) as f64 / (target_len - 1).max(1) as f64;
        let lo = (src_pos.floor() as usize).min(src_len - 1);
        let hi = (lo + 1).min(src_len - 1);
        let frac = src_pos - lo as f64;

        let frame: Vec<f32> = (0..num_coeffs)
            .map(|c| {
                let v = seq[lo][c] as f64 * (1.0 - frac) + seq[hi][c] as f64 * frac;
                v as f32
            })
            .collect();
        result.push(frame);
    }

    result
}

/// Compute a mean (averaged) template for each word.
///
/// Steps:
/// 1. Group templates by word.
/// 2. For each word, compute the median frame count.
/// 3. Interpolate each template to the median length.
/// 4. Average the interpolated templates element-wise.
///
/// Returns one `Template` per word (with `sample_index = 0`).
pub fn compute_mean_templates(templates: &[Template]) -> Vec<Template> {
    // Group by word.
    let mut groups: HashMap<String, Vec<&MfccSequence>> = HashMap::new();
    for t in templates {
        groups.entry(t.word.clone()).or_default().push(&t.mfcc);
    }

    let mut result = Vec::new();

    for (word, sequences) in &groups {
        if sequences.is_empty() {
            continue;
        }
        if sequences.len() == 1 {
            result.push(Template {
                word: word.clone(),
                sample_index: 0,
                mfcc: sequences[0].clone(),
            });
            continue;
        }

        // Median frame count.
        let mut lengths: Vec<usize> = sequences.iter().map(|s| s.len()).collect();
        lengths.sort();
        let median_len = lengths[lengths.len() / 2];

        if median_len == 0 {
            continue;
        }

        let num_coeffs = sequences[0][0].len();

        // Interpolate all sequences to median length, then average.
        let mut sum = vec![vec![0.0f64; num_coeffs]; median_len];
        let count = sequences.len() as f64;

        for seq in sequences {
            let interp = interpolate_sequence(seq, median_len);
            for (t, frame) in interp.iter().enumerate() {
                for (c, &val) in frame.iter().enumerate() {
                    sum[t][c] += val as f64;
                }
            }
        }

        // Convert sum to mean.
        let mean_mfcc: MfccSequence = sum
            .iter()
            .map(|frame| frame.iter().map(|&v| (v / count) as f32).collect())
            .collect();

        log::debug!(
            "Mean template for '{}': {} samples -> {} frames x {} coeffs",
            word,
            sequences.len(),
            mean_mfcc.len(),
            num_coeffs
        );

        result.push(Template {
            word: word.clone(),
            sample_index: 0,
            mfcc: mean_mfcc,
        });
    }

    result.sort_by(|a, b| a.word.cmp(&b.word));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_identity() {
        let seq: MfccSequence = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
        let result = interpolate_sequence(&seq, 3);
        assert_eq!(result.len(), 3);
        for (a, b) in result.iter().zip(seq.iter()) {
            for (x, y) in a.iter().zip(b.iter()) {
                assert!((x - y).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn test_interpolate_upsample() {
        let seq: MfccSequence = vec![vec![0.0], vec![10.0]];
        let result = interpolate_sequence(&seq, 3);
        assert_eq!(result.len(), 3);
        assert!((result[0][0] - 0.0).abs() < 1e-5);
        assert!((result[1][0] - 5.0).abs() < 1e-5);
        assert!((result[2][0] - 10.0).abs() < 1e-5);
    }

    #[test]
    fn test_interpolate_downsample() {
        let seq: MfccSequence = vec![vec![0.0], vec![5.0], vec![10.0]];
        let result = interpolate_sequence(&seq, 2);
        assert_eq!(result.len(), 2);
        assert!((result[0][0] - 0.0).abs() < 1e-5);
        assert!((result[1][0] - 10.0).abs() < 1e-5);
    }

    #[test]
    fn test_mean_template_single() {
        let templates = vec![Template {
            word: "start".to_string(),
            sample_index: 0,
            mfcc: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
        }];
        let mean = compute_mean_templates(&templates);
        assert_eq!(mean.len(), 1);
        assert_eq!(mean[0].mfcc, templates[0].mfcc);
    }

    #[test]
    fn test_mean_template_averaging() {
        let templates = vec![
            Template {
                word: "start".to_string(),
                sample_index: 0,
                mfcc: vec![vec![2.0], vec![4.0]],
            },
            Template {
                word: "start".to_string(),
                sample_index: 1,
                mfcc: vec![vec![4.0], vec![6.0]],
            },
        ];
        let mean = compute_mean_templates(&templates);
        assert_eq!(mean.len(), 1);
        assert_eq!(mean[0].word, "start");
        // Average: [3.0], [5.0]
        assert!((mean[0].mfcc[0][0] - 3.0).abs() < 1e-5);
        assert!((mean[0].mfcc[1][0] - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_mean_template_different_lengths() {
        let templates = vec![
            Template {
                word: "start".to_string(),
                sample_index: 0,
                mfcc: vec![vec![0.0], vec![10.0]],
            },
            Template {
                word: "start".to_string(),
                sample_index: 1,
                mfcc: vec![vec![0.0], vec![5.0], vec![10.0]],
            },
            Template {
                word: "start".to_string(),
                sample_index: 2,
                mfcc: vec![vec![0.0], vec![10.0]],
            },
        ];
        let mean = compute_mean_templates(&templates);
        assert_eq!(mean.len(), 1);
        // Median length is 2 (sorted lengths: [2, 2, 3], median index 1 = 2).
        assert_eq!(mean[0].mfcc.len(), 2);
    }

    #[test]
    fn test_mean_template_multiple_words() {
        let templates = vec![
            Template {
                word: "start".to_string(),
                sample_index: 0,
                mfcc: vec![vec![1.0]],
            },
            Template {
                word: "stop".to_string(),
                sample_index: 0,
                mfcc: vec![vec![2.0]],
            },
        ];
        let mean = compute_mean_templates(&templates);
        assert_eq!(mean.len(), 2);
    }
}
