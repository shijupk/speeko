use speeko_common::types::WordScore;

/// Compute confidence score from sorted word scores.
///
/// Multi-word: confidence = 1.0 - (best / second_best), measuring how much
/// better the best match is compared to the runner-up.
///
/// Single-word (or when only one word is trained): confidence is based on
/// absolute distance — `1.0 - (distance / max_distance)`. This ensures that
/// dissimilar utterances get low confidence and are rejected.
pub fn compute_confidence(scores: &[WordScore], max_distance: f32) -> f32 {
    if scores.is_empty() {
        return 0.0;
    }

    let best = scores[0].distance;

    if scores.len() == 1 {
        // Single word trained — use absolute distance for confidence.
        // Close to 0 distance → confidence ~1.0, close to max_distance → ~0.0.
        if max_distance <= 0.0 {
            return 0.0;
        }
        return (1.0 - best / max_distance).clamp(0.0, 1.0);
    }

    let second = scores[1].distance;

    if second <= 0.0 {
        return 0.0;
    }

    // Relative confidence: log-ratio is more sensitive to small distance gaps
    // than a simple ratio, especially for similar-sounding words.
    let ratio = second / best; // > 1.0 when best is better
    let relative = ((ratio.ln() / 0.5_f32.ln().abs()) ).clamp(0.0, 1.0);

    // Absolute confidence: how good the best match is in absolute terms.
    let absolute = if max_distance > 0.0 {
        (1.0 - best / max_distance).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Blend: weight absolute heavily (0.7) because with CMN + deltas + mean
    // templates, a low absolute distance is a strong match signal. Similar-
    // sounding words (start/stop) may have small relative gaps but the
    // absolute distance gate already rejects garbage.
    let blended = 0.3 * relative + 0.7 * absolute;
    blended.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(word: &str, distance: f32) -> WordScore {
        WordScore {
            word: word.to_string(),
            distance,
        }
    }

    #[test]
    fn test_high_confidence() {
        let scores = vec![ws("start", 10.0), ws("stop", 50.0)];
        // ratio = 5.0, relative = ln(5)/ln(2) = 2.32 -> clamped to 1.0
        // absolute = 0.9, blended = 0.3*1.0 + 0.7*0.9 = 0.93
        let c = compute_confidence(&scores, 100.0);
        assert!((c - 0.93).abs() < 1e-6);
    }

    #[test]
    fn test_low_confidence() {
        let scores = vec![ws("start", 48.0), ws("stop", 50.0)];
        // ratio = 50/48 = 1.0417, relative = ln(1.0417)/0.693 = 0.059
        // absolute = 0.52, blended = 0.3*0.059 + 0.7*0.52 = 0.0177 + 0.364 = 0.3817
        let c = compute_confidence(&scores, 100.0);
        assert!((c - 0.3817).abs() < 0.001);
    }

    #[test]
    fn test_single_word_close() {
        // distance=10, max=40 → 1.0 - 10/40 = 0.75
        let scores = vec![ws("start", 10.0)];
        let c = compute_confidence(&scores, 40.0);
        assert!((c - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_single_word_far() {
        // distance=35, max=40 → 1.0 - 35/40 = 0.125
        let scores = vec![ws("start", 35.0)];
        let c = compute_confidence(&scores, 40.0);
        assert!((c - 0.125).abs() < 1e-6);
    }

    #[test]
    fn test_empty() {
        let c = compute_confidence(&[], 40.0);
        assert_eq!(c, 0.0);
    }

    #[test]
    fn test_identical_distances() {
        let scores = vec![ws("start", 10.0), ws("stop", 10.0)];
        // ratio = 1.0, relative = ln(1)/0.693 = 0.0
        // absolute = 0.9, blended = 0.3*0.0 + 0.7*0.9 = 0.63
        let c = compute_confidence(&scores, 100.0);
        assert!((c - 0.63).abs() < 1e-6);
    }
}
