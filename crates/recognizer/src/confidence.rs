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

    // Relative confidence: how much better the best match is vs the runner-up.
    let relative = (1.0 - (best / second)).clamp(0.0, 1.0);

    // Absolute confidence: how good the best match is in absolute terms.
    let absolute = if max_distance > 0.0 {
        (1.0 - best / max_distance).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Blend: weight absolute confidence more (0.6) because similar-sounding
    // words (start/stop) have inherently low relative separation, but a good
    // absolute match still indicates high quality. The max_distance hard gate
    // in the matcher rejects truly bad matches regardless.
    let blended = 0.4 * relative + 0.6 * absolute;
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
        // relative = 0.8, absolute = 0.9, blended = 0.4*0.8 + 0.6*0.9 = 0.86
        let c = compute_confidence(&scores, 100.0);
        assert!((c - 0.86).abs() < 1e-6);
    }

    #[test]
    fn test_low_confidence() {
        let scores = vec![ws("start", 48.0), ws("stop", 50.0)];
        // relative = 0.04, absolute = 0.52, blended = 0.4*0.04 + 0.6*0.52 = 0.328
        let c = compute_confidence(&scores, 100.0);
        assert!((c - 0.328).abs() < 1e-6);
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
        // relative = 0.0, absolute = 0.9, blended = 0.4*0.0 + 0.6*0.9 = 0.54
        let c = compute_confidence(&scores, 100.0);
        assert!((c - 0.54).abs() < 1e-6);
    }
}
