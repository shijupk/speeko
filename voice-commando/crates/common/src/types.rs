use serde::{Deserialize, Serialize};

/// Raw audio samples as f32 in [-1.0, 1.0] range.
pub type AudioBuffer = Vec<f32>;

/// A single frame of audio samples (after windowing).
pub type AudioFrame = Vec<f32>;

/// MFCC feature vector for a single frame.
pub type MfccFrame = Vec<f32>;

/// Sequence of MFCC frames representing an utterance.
/// Shape: [num_frames][num_coefficients]
pub type MfccSequence = Vec<MfccFrame>;

/// A stored template for a trained word.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// The word this template represents.
    pub word: String,
    /// Sample index (0-based) for this word.
    pub sample_index: usize,
    /// MFCC feature sequence.
    pub mfcc: MfccSequence,
}

/// Result of a recognition attempt.
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    /// The predicted word, or None if rejected.
    pub word: Option<String>,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f32,
    /// Distance of best match (1 - probability for CNN).
    pub best_distance: f32,
    /// All per-word scores, sorted by distance ascending.
    pub scores: Vec<WordScore>,
}

/// Score for a single word.
#[derive(Debug, Clone)]
pub struct WordScore {
    pub word: String,
    pub distance: f32,
}

/// VAD result indicating the active speech region.
#[derive(Debug, Clone, Copy)]
pub struct VadRegion {
    /// Start sample index (inclusive).
    pub start: usize,
    /// End sample index (exclusive).
    pub end: usize,
}

impl VadRegion {
    pub fn duration_samples(&self) -> usize {
        self.end - self.start
    }

    pub fn duration_secs(&self, sample_rate: u32) -> f32 {
        self.duration_samples() as f32 / sample_rate as f32
    }
}
