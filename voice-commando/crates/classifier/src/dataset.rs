use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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
