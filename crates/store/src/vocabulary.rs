use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

/// Load vocabulary from a text file (one word per line).
/// Ignores blank lines and lines starting with '#'.
/// Warns on duplicates.
pub fn load_vocabulary(path: &Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| {
            format!(
                "vocabulary.txt not found at {:?}. Create it with one word per line.",
                path
            )
        })?;

    let mut words = Vec::new();
    let mut seen = HashSet::new();

    for (line_num, line) in content.lines().enumerate() {
        let word = line.trim().to_lowercase();

        // Skip empty lines and comments.
        if word.is_empty() || word.starts_with('#') {
            continue;
        }

        if seen.contains(&word) {
            log::warn!(
                "Duplicate word '{}' in vocabulary.txt (line {}). Ignoring duplicate.",
                word,
                line_num + 1
            );
            continue;
        }

        seen.insert(word.clone());
        words.push(word);
    }

    log::info!("Loaded vocabulary: {} words", words.len());
    Ok(words)
}

/// Check if a word is in the vocabulary.
pub fn is_in_vocabulary(vocabulary: &[String], word: &str) -> bool {
    vocabulary.iter().any(|w| w == word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_vocabulary() {
        let dir = std::env::temp_dir().join("speeko_test_vocab");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vocabulary.txt");

        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "# Command words").unwrap();
        writeln!(f, "start").unwrap();
        writeln!(f, "stop").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "open").unwrap();
        writeln!(f, "CLOSE").unwrap(); // should be lowercased
        writeln!(f, "start").unwrap(); // duplicate

        let words = load_vocabulary(&path).unwrap();
        assert_eq!(words, vec!["start", "stop", "open", "close"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_is_in_vocabulary() {
        let vocab = vec!["start".to_string(), "stop".to_string()];
        assert!(is_in_vocabulary(&vocab, "start"));
        assert!(!is_in_vocabulary(&vocab, "go"));
    }
}
