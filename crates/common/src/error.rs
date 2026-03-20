use std::fmt;

/// Application-level error types for Speeko.
#[derive(Debug)]
pub enum SpeekError {
    /// No audio input device found.
    NoAudioDevice,
    /// Audio capture failed.
    AudioCapture(String),
    /// No speech detected in recording.
    NoSpeechDetected,
    /// Recording too short to be a valid utterance.
    RecordingTooShort { duration_ms: u32, min_ms: u32 },
    /// Audio appears clipped.
    AudioClipped,
    /// Word not found in vocabulary.
    WordNotInVocabulary(String),
    /// No templates found for any word.
    NoTemplates,
    /// No templates found for a specific word.
    NoTemplatesForWord(String),
    /// Configuration error.
    Config(String),
    /// Vocabulary file error.
    Vocabulary(String),
    /// Template storage error.
    Storage(String),
    /// Feature extraction error.
    FeatureExtraction(String),
    /// User cancelled operation.
    Cancelled,
}

impl fmt::Display for SpeekError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpeekError::NoAudioDevice => {
                write!(f, "No audio input device found. Run `speeko diagnose` to check.")
            }
            SpeekError::AudioCapture(msg) => write!(f, "Audio capture failed: {msg}"),
            SpeekError::NoSpeechDetected => {
                write!(f, "No speech detected. Please speak louder or check mic.")
            }
            SpeekError::RecordingTooShort { duration_ms, min_ms } => {
                write!(
                    f,
                    "Recording too short ({duration_ms}ms). Minimum is {min_ms}ms. Please speak the full word."
                )
            }
            SpeekError::AudioClipped => {
                write!(f, "Audio appears clipped. Move further from mic or reduce volume.")
            }
            SpeekError::WordNotInVocabulary(word) => {
                write!(f, "'{word}' is not in vocabulary.txt.")
            }
            SpeekError::NoTemplates => {
                write!(f, "No templates found. Run `speeko train <word>` first.")
            }
            SpeekError::NoTemplatesForWord(word) => {
                write!(f, "No templates found for word '{word}'.")
            }
            SpeekError::Config(msg) => write!(f, "Configuration error: {msg}"),
            SpeekError::Vocabulary(msg) => write!(f, "Vocabulary error: {msg}"),
            SpeekError::Storage(msg) => write!(f, "Storage error: {msg}"),
            SpeekError::FeatureExtraction(msg) => write!(f, "Feature extraction error: {msg}"),
            SpeekError::Cancelled => write!(f, "Operation cancelled."),
        }
    }
}

impl std::error::Error for SpeekError {}
