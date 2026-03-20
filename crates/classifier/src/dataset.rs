use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use speeko_common::types::MfccSequence;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::augment;
use crate::model::pad_or_truncate;

/// Vocabulary mapping: word <-> class index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabMap {
    /// word -> index (sorted alphabetically).
    pub word_to_idx: HashMap<String, usize>,
    /// index -> word.
    pub idx_to_word: Vec<String>,
}

impl VocabMap {
    /// Build vocabulary from a list of word names (sorted alphabetically).
    pub fn from_words(words: &[String]) -> Self {
        let mut sorted = words.to_vec();
        sorted.sort();
        sorted.dedup();
        let word_to_idx: HashMap<String, usize> = sorted
            .iter()
            .enumerate()
            .map(|(i, w)| (w.clone(), i))
            .collect();
        Self {
            word_to_idx,
            idx_to_word: sorted,
        }
    }

    pub fn num_classes(&self) -> usize {
        self.idx_to_word.len()
    }

    /// Save vocabulary map to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load vocabulary map from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read vocab file {:?}", path))?;
        let vocab: Self = serde_json::from_str(&json)?;
        Ok(vocab)
    }
}

/// A single labeled sample: MFCC features + class label.
#[derive(Debug, Clone)]
pub struct LabeledSample {
    /// MFCC sequence (variable length).
    pub mfcc: MfccSequence,
    /// Class index.
    pub label: usize,
    /// Word string (for diagnostics).
    pub word: String,
}

/// Load all MFCC samples from a WAV directory structure.
///
/// Expects: `base_dir/<word>/sample_*.wav`
///
/// The `process_wav` callback converts a WAV path into an MfccSequence
/// (applies preprocess → VAD → MFCC → CMN → Δ/ΔΔ).
pub fn load_samples_from_dir<F>(
    base_dir: &Path,
    mut process_wav: F,
) -> Result<(Vec<LabeledSample>, VocabMap)>
where
    F: FnMut(&Path) -> Result<Option<MfccSequence>>,
{
    if !base_dir.is_dir() {
        bail!("Directory not found: {:?}", base_dir);
    }

    // Discover word directories.
    let mut words: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(base_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                words.push(name.to_string());
            }
        }
    }
    words.sort();

    if words.is_empty() {
        bail!("No word directories found in {:?}", base_dir);
    }

    let vocab = VocabMap::from_words(&words);
    let mut samples = Vec::new();

    for word in &words {
        let word_dir = base_dir.join(word);
        let label = vocab.word_to_idx[word];

        let mut wav_files: Vec<PathBuf> = Vec::new();
        for entry in std::fs::read_dir(&word_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "wav") {
                wav_files.push(path);
            }
        }
        wav_files.sort();

        for wav_path in &wav_files {
            match process_wav(wav_path) {
                Ok(Some(mfcc)) => {
                    samples.push(LabeledSample {
                        mfcc,
                        label,
                        word: word.clone(),
                    });
                }
                Ok(None) => {
                    log::warn!("No speech detected in {:?}, skipping", wav_path);
                }
                Err(e) => {
                    log::warn!("Failed to process {:?}: {}", wav_path, e);
                }
            }
        }
    }

    log::info!(
        "Loaded {} samples across {} words from {:?}",
        samples.len(),
        vocab.num_classes(),
        base_dir
    );

    Ok((samples, vocab))
}

/// Split samples into train and validation sets (stratified by class).
pub fn train_val_split(
    samples: &[LabeledSample],
    val_fraction: f64,
) -> (Vec<LabeledSample>, Vec<LabeledSample>) {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    // Group by label.
    let mut by_label: HashMap<usize, Vec<&LabeledSample>> = HashMap::new();
    for s in samples {
        by_label.entry(s.label).or_default().push(s);
    }

    let mut train = Vec::new();
    let mut val = Vec::new();

    for (_label, mut group) in by_label {
        group.shuffle(&mut rng);
        let val_count = (group.len() as f64 * val_fraction).ceil() as usize;
        let val_count = val_count.min(group.len()).max(1.min(group.len()));
        for (i, s) in group.iter().enumerate() {
            if i < val_count {
                val.push((*s).clone());
            } else {
                train.push((*s).clone());
            }
        }
    }

    (train, val)
}

/// Convert labeled samples to padded float vectors for burn tensors.
/// Applies data augmentation to training samples.
pub fn prepare_batch(
    samples: &[LabeledSample],
    max_frames: usize,
    augment: bool,
    num_augments: usize,
) -> (Vec<Vec<f32>>, Vec<usize>) {
    let mut features = Vec::new();
    let mut labels = Vec::new();

    for sample in samples {
        if augment {
            let variants = augment::augment_sample(&sample.mfcc, num_augments);
            for variant in &variants {
                features.push(pad_or_truncate(variant, max_frames));
                labels.push(sample.label);
            }
        } else {
            features.push(pad_or_truncate(&sample.mfcc, max_frames));
            labels.push(sample.label);
        }
    }

    (features, labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vocab_map() {
        let words = vec!["stop".to_string(), "start".to_string(), "yes".to_string()];
        let vocab = VocabMap::from_words(&words);
        assert_eq!(vocab.num_classes(), 3);
        assert_eq!(vocab.idx_to_word, vec!["start", "stop", "yes"]);
        assert_eq!(vocab.word_to_idx["start"], 0);
        assert_eq!(vocab.word_to_idx["stop"], 1);
        assert_eq!(vocab.word_to_idx["yes"], 2);
    }

    #[test]
    fn test_train_val_split() {
        let samples: Vec<LabeledSample> = (0..10)
            .map(|i| LabeledSample {
                mfcc: vec![vec![i as f32]],
                label: (i % 2) as usize,
                word: if i % 2 == 0 {
                    "a".to_string()
                } else {
                    "b".to_string()
                },
            })
            .collect();
        let (train, val) = train_val_split(&samples, 0.2);
        assert!(!val.is_empty());
        assert!(!train.is_empty());
        assert_eq!(train.len() + val.len(), 10);
    }
}
