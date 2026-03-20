use anyhow::{bail, Result};
use burn::prelude::*;
use burn::tensor::activation::softmax;

use speeko_common::types::{MfccSequence, RecognitionResult, WordScore};

use crate::dataset::VocabMap;
use crate::model::{pad_or_truncate, KeywordCnn};
use crate::training::{load_model, InferBackend};

/// CNN-based recognizer for inference.
pub struct CnnRecognizer {
    model: KeywordCnn<InferBackend>,
    vocab: VocabMap,
    max_frames: usize,
    confidence_threshold: f32,
}

impl CnnRecognizer {
    /// Load a trained CNN model and vocabulary from disk.
    pub fn load(
        model_dir: &std::path::Path,
        confidence_threshold: f32,
        max_frames: usize,
    ) -> Result<Self> {
        let vocab_path = model_dir.join("cnn_vocab.json");
        let vocab = VocabMap::load(&vocab_path)?;

        if vocab.num_classes() < 2 {
            bail!("Model vocabulary has fewer than 2 classes");
        }

        // Determine input channels from vocab metadata or use default.
        // With CMN + deltas, we expect 39 channels.
        let num_coeffs = 39;

        let model = load_model(model_dir, vocab.num_classes(), num_coeffs)?;

        Ok(Self {
            model,
            vocab,
            max_frames,
            confidence_threshold,
        })
    }

    /// Recognize a word from an MFCC sequence.
    pub fn predict(&self, mfcc: &MfccSequence) -> RecognitionResult {
        if mfcc.is_empty() {
            return RecognitionResult {
                word: None,
                confidence: 0.0,
                best_distance: f32::INFINITY,
                scores: vec![],
            };
        }

        let device = Default::default();
        let num_coeffs = mfcc[0].len();

        // Pad/truncate and create tensor.
        let flat = pad_or_truncate(mfcc, self.max_frames);
        let input = Tensor::<InferBackend, 1>::from_floats(flat.as_slice(), &device)
            .reshape([1, num_coeffs, self.max_frames]);

        // Forward pass -> softmax probabilities.
        let logits = self.model.forward(input);
        let probs = softmax(logits, 1);

        // Extract probabilities.
        let probs_data: Vec<f32> = probs.to_data().to_vec().unwrap();

        // Build scores sorted by probability descending.
        let mut scores: Vec<WordScore> = probs_data
            .iter()
            .enumerate()
            .map(|(i, &prob)| WordScore {
                word: self
                    .vocab
                    .idx_to_word
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("class_{}", i)),
                distance: 1.0 - prob, // distance = 1 - probability for compatibility
            })
            .collect();
        scores.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        let best_prob = probs_data
            .iter()
            .cloned()
            .fold(0.0f32, f32::max);
        let best_idx = probs_data
            .iter()
            .position(|&p| (p - best_prob).abs() < 1e-8)
            .unwrap_or(0);
        let best_word = self
            .vocab
            .idx_to_word
            .get(best_idx)
            .cloned()
            .unwrap_or_default();

        let confidence = best_prob;
        let accepted = confidence >= self.confidence_threshold;

        log::info!(
            "CNN: best='{}' prob={:.3}, confidence={:.3}",
            best_word,
            best_prob,
            confidence,
        );
        for (i, prob) in probs_data.iter().enumerate() {
            if let Some(word) = self.vocab.idx_to_word.get(i) {
                log::info!("  {}: prob={:.3}", word, prob);
            }
        }

        RecognitionResult {
            word: if accepted { Some(best_word) } else { None },
            confidence,
            best_distance: 1.0 - best_prob,
            scores,
        }
    }
}
