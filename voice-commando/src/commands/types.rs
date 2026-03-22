use bevy::prelude::*;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameCommand {
    Start,
    Stop,
    Close,
    MoveLeft,
    MoveRight,
    EatPrey,
}

#[derive(Event, Debug, Clone)]
pub struct GameCommandEvent {
    pub command: GameCommand,
    pub source_word: String,
    pub confidence: f32,
    pub issued_at: Instant,
}
