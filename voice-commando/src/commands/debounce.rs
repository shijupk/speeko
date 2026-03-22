use bevy::prelude::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::types::GameCommand;

#[derive(Resource)]
pub struct DebounceState {
    last_accepted: HashMap<GameCommand, Instant>,
    pub debounce_duration: Duration,
}

impl DebounceState {
    pub fn new(debounce_ms: u32) -> Self {
        Self {
            last_accepted: HashMap::new(),
            debounce_duration: Duration::from_millis(debounce_ms as u64),
        }
    }

    pub fn can_accept(&self, command: GameCommand, now: Instant) -> bool {
        match self.last_accepted.get(&command) {
            Some(&last) => now.duration_since(last) >= self.debounce_duration,
            None => true,
        }
    }

    pub fn record_accept(&mut self, command: GameCommand, now: Instant) {
        self.last_accepted.insert(command, now);
    }
}
