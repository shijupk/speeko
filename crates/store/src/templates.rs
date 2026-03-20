use anyhow::{Context, Result};
use speeko_common::types::Template;
use std::path::{Path, PathBuf};

/// Manages persistence of word templates on disk.
///
/// Directory structure:
/// ```text
/// templates_dir/
///   start/
///     template_000.bin
///     template_001.bin
///   stop/
///     template_000.bin
/// ```
pub struct TemplateStore {
    base_dir: PathBuf,
}

impl TemplateStore {
    /// Create a new template store rooted at `base_dir`.
    /// Creates the directory if it doesn't exist.
    pub fn new(base_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(base_dir)
            .with_context(|| format!("Failed to create templates directory {:?}", base_dir))?;
        Ok(Self {
            base_dir: base_dir.to_path_buf(),
        })
    }

    /// Save a template for a word.
    pub fn save_template(&self, template: &Template) -> Result<PathBuf> {
        let word_dir = self.base_dir.join(&template.word);
        std::fs::create_dir_all(&word_dir)
            .with_context(|| format!("Failed to create word directory {:?}", word_dir))?;

        let filename = format!("template_{:03}.bin", template.sample_index);
        let path = word_dir.join(&filename);

        let data = bincode::serialize(template)
            .context("Failed to serialize template")?;
        std::fs::write(&path, &data)
            .with_context(|| format!("Failed to write template {:?}", path))?;

        log::info!(
            "Saved template: {:?} ({} bytes, {} frames)",
            path,
            data.len(),
            template.mfcc.len()
        );
        Ok(path)
    }

    /// Load all templates for a specific word.
    pub fn load_word_templates(&self, word: &str) -> Result<Vec<Template>> {
        let word_dir = self.base_dir.join(word);
        if !word_dir.exists() {
            return Ok(vec![]);
        }

        let mut templates = Vec::new();
        let entries = std::fs::read_dir(&word_dir)
            .with_context(|| format!("Failed to read word directory {:?}", word_dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "bin") {
                match self.load_template(&path) {
                    Ok(template) => templates.push(template),
                    Err(e) => {
                        log::warn!("Failed to load template {:?}: {}. Skipping.", path, e);
                    }
                }
            }
        }

        templates.sort_by_key(|t| t.sample_index);
        log::debug!("Loaded {} templates for '{}'", templates.len(), word);
        Ok(templates)
    }

    /// Load all templates for all words.
    pub fn load_all_templates(&self) -> Result<Vec<Template>> {
        let mut all_templates = Vec::new();

        if !self.base_dir.exists() {
            return Ok(all_templates);
        }

        let entries = std::fs::read_dir(&self.base_dir)
            .with_context(|| format!("Failed to read templates directory {:?}", self.base_dir))?;

        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(word) = entry.file_name().to_str() {
                    let word_templates = self.load_word_templates(word)?;
                    all_templates.extend(word_templates);
                }
            }
        }

        log::info!("Loaded {} total templates", all_templates.len());
        Ok(all_templates)
    }

    /// Get the count of templates per word.
    pub fn template_counts(&self) -> Result<Vec<(String, usize)>> {
        let mut counts = Vec::new();

        if !self.base_dir.exists() {
            return Ok(counts);
        }

        let entries = std::fs::read_dir(&self.base_dir)?;
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(word) = entry.file_name().to_str() {
                    let word_dir = entry.path();
                    let count = std::fs::read_dir(&word_dir)?
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            e.path().extension().map_or(false, |ext| ext == "bin")
                        })
                        .count();
                    counts.push((word.to_string(), count));
                }
            }
        }

        counts.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(counts)
    }

    /// Get the next available sample index for a word.
    pub fn next_sample_index(&self, word: &str) -> Result<usize> {
        let templates = self.load_word_templates(word)?;
        Ok(templates.last().map_or(0, |t| t.sample_index + 1))
    }

    /// Delete all templates for a word.
    pub fn delete_word(&self, word: &str) -> Result<()> {
        let word_dir = self.base_dir.join(word);
        if word_dir.exists() {
            std::fs::remove_dir_all(&word_dir)
                .with_context(|| format!("Failed to delete templates for '{}'", word))?;
            log::info!("Deleted all templates for '{}'", word);
        }
        Ok(())
    }

    /// Delete all templates.
    pub fn delete_all(&self) -> Result<()> {
        if self.base_dir.exists() {
            std::fs::remove_dir_all(&self.base_dir)?;
            std::fs::create_dir_all(&self.base_dir)?;
            log::info!("Deleted all templates");
        }
        Ok(())
    }

    fn load_template(&self, path: &Path) -> Result<Template> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read template file {:?}", path))?;
        let template: Template = bincode::deserialize(&data)
            .with_context(|| format!("Failed to deserialize template {:?}", path))?;
        Ok(template)
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speeko_common::types::Template;

    #[test]
    fn test_template_roundtrip() {
        let dir = std::env::temp_dir().join("speeko_test_templates");
        let _ = std::fs::remove_dir_all(&dir);

        let store = TemplateStore::new(&dir).unwrap();

        let template = Template {
            word: "start".to_string(),
            sample_index: 0,
            mfcc: vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
        };

        store.save_template(&template).unwrap();

        let loaded = store.load_word_templates("start").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].word, "start");
        assert_eq!(loaded[0].mfcc.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_template_counts() {
        let dir = std::env::temp_dir().join("speeko_test_counts");
        let _ = std::fs::remove_dir_all(&dir);

        let store = TemplateStore::new(&dir).unwrap();

        for i in 0..3 {
            let t = Template {
                word: "hello".to_string(),
                sample_index: i,
                mfcc: vec![vec![1.0]],
            };
            store.save_template(&t).unwrap();
        }

        let counts = store.template_counts().unwrap();
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0], ("hello".to_string(), 3));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
