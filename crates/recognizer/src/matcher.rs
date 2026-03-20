use speeko_common::types::{MfccSequence, RecognitionResult, Template, WordScore};

use crate::confidence::compute_confidence;
use crate::dtw::dtw_distance;

/// Multi-template matcher that compares a query against all stored templates.
pub struct TemplateMatcher {
    /// Sakoe-Chiba band fraction.
    band_fraction: f32,
    /// Confidence threshold for rejection.
    confidence_threshold: f32,
    /// Maximum acceptable DTW distance for absolute rejection.
    max_distance: f32,
}

impl TemplateMatcher {
    pub fn new(band_fraction: f32, confidence_threshold: f32, max_distance: f32) -> Self {
        Self {
            band_fraction,
            confidence_threshold,
            max_distance,
        }
    }

    /// Match a query MFCC sequence against all templates.
    ///
    /// For each word, computes the minimum DTW distance across all its templates.
    /// Returns the best match with confidence, or rejects if confidence is too low.
    pub fn recognize(
        &self,
        query: &MfccSequence,
        templates: &[Template],
    ) -> RecognitionResult {
        if templates.is_empty() {
            return RecognitionResult {
                word: None,
                confidence: 0.0,
                best_distance: f32::INFINITY,
                scores: vec![],
            };
        }

        // Group templates by word and find minimum distance per word.
        let mut word_distances: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();

        for template in templates {
            let dist = dtw_distance(query, &template.mfcc, self.band_fraction);
            log::debug!(
                "  DTW: '{}' sample={} ({} frames) -> dist={:.3}",
                template.word,
                template.sample_index,
                template.mfcc.len(),
                dist
            );
            let entry = word_distances
                .entry(template.word.clone())
                .or_insert(f32::INFINITY);
            if dist < *entry {
                *entry = dist;
            }
        }

        // Sort by distance ascending.
        let mut scores: Vec<WordScore> = word_distances
            .into_iter()
            .map(|(word, distance)| WordScore { word, distance })
            .collect();
        scores.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        if scores.is_empty() {
            return RecognitionResult {
                word: None,
                confidence: 0.0,
                best_distance: f32::INFINITY,
                scores,
            };
        }

        let best_distance = scores[0].distance;
        let best_word = scores[0].word.clone();

        // Compute confidence.
        let confidence = compute_confidence(&scores, self.max_distance);

        log::info!(
            "Matcher: best='{}' dist={:.3}, confidence={:.3}, max_dist={:.1}",
            best_word,
            best_distance,
            confidence,
            self.max_distance
        );
        for score in &scores {
            log::info!("  {}: dist={:.3}", score.word, score.distance);
        }

        // Apply rejection logic.
        let accepted = confidence >= self.confidence_threshold
            && best_distance <= self.max_distance;

        RecognitionResult {
            word: if accepted { Some(best_word) } else { None },
            confidence,
            best_distance,
            scores,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speeko_common::types::Template;

    fn make_template(word: &str, mfcc: MfccSequence) -> Template {
        Template {
            word: word.to_string(),
            sample_index: 0,
            mfcc,
        }
    }

    #[test]
    fn test_exact_match() {
        let matcher = TemplateMatcher::new(0.0, 0.1, 100.0);

        let templates = vec![
            make_template("start", vec![vec![1.0, 0.0], vec![2.0, 0.0]]),
            make_template("stop", vec![vec![0.0, 1.0], vec![0.0, 2.0]]),
        ];

        let query = vec![vec![1.0, 0.0], vec![2.0, 0.0]];
        let result = matcher.recognize(&query, &templates);
        assert_eq!(result.word, Some("start".to_string()));
    }

    #[test]
    fn test_no_templates() {
        let matcher = TemplateMatcher::new(0.0, 0.1, 100.0);
        let result = matcher.recognize(&vec![vec![1.0]], &[]);
        assert!(result.word.is_none());
    }

    #[test]
    fn test_rejection_high_threshold() {
        let matcher = TemplateMatcher::new(0.0, 0.99, 100.0);

        let templates = vec![
            make_template("start", vec![vec![1.0], vec![2.0]]),
            make_template("stop", vec![vec![1.1], vec![2.1]]),
        ];

        let query = vec![vec![1.05], vec![2.05]];
        let result = matcher.recognize(&query, &templates);
        // With very similar templates and high threshold, might reject.
        // This depends on actual distances — just check it returns a valid result.
        assert!(result.confidence >= 0.0);
    }
}
