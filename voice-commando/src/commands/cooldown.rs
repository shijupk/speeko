use bevy::prelude::*;
use std::collections::HashMap;
use std::time::Duration;

use super::types::GameCommand;

#[derive(Resource)]
pub struct CooldownState {
    cooldowns: HashMap<GameCommand, Timer>,
}

impl CooldownState {
    pub fn new() -> Self {
        Self {
            cooldowns: HashMap::new(),
        }
    }

    pub fn is_ready(&self, command: GameCommand) -> bool {
        self.cooldowns
            .get(&command)
            .map_or(true, |t| t.finished())
    }

    pub fn start_cooldown(&mut self, command: GameCommand, duration: Duration) {
        self.cooldowns
            .insert(command, Timer::new(duration, TimerMode::Once));
    }

    pub fn tick_all(&mut self, delta: Duration) {
        for timer in self.cooldowns.values_mut() {
            timer.tick(delta);
        }
    }
}
