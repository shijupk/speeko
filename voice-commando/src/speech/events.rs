use bevy::prelude::*;
use std::time::Instant;

#[derive(Event, Debug, Clone)]
pub struct RecognizedWordEvent {
    pub word: String,
    pub confidence: f32,
    pub all_scores: Vec<(String, f32)>,
    pub recognized_at: Instant,
}

#[derive(Event, Debug, Clone)]
pub struct RejectedWordEvent {
    pub best_guess: Option<String>,
    pub confidence: f32,
    pub reason: RejectionReason,
    pub recognized_at: Instant,
}

#[derive(Debug, Clone)]
pub enum RejectionReason {
    LowConfidence,
    NoSpeechDetected,
}
