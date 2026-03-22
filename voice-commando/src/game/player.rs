use bevy::prelude::*;

use super::lanes::Lane;

/// The player — a Komodo dragon that moves between lanes.
#[derive(Component, Debug)]
pub struct Player {
    pub lane: Lane,
    pub energy: f32,
    pub alive: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            lane: Lane::Center,
            energy: 100.0,
            alive: true,
        }
    }
}

impl Player {
    pub fn move_left(&mut self) {
        if let Some(target) = self.lane.left() {
            self.lane = target;
        }
    }

    pub fn move_right(&mut self) {
        if let Some(target) = self.lane.right() {
            self.lane = target;
        }
    }

    pub fn eat(&mut self, energy_gain: f32) {
        self.energy = (self.energy + energy_gain).min(100.0);
    }

    pub fn drain_energy(&mut self, amount: f32) {
        self.energy = (self.energy - amount).max(0.0);
        if self.energy <= 0.0 {
            self.alive = false;
        }
    }

    pub fn die(&mut self) {
        self.alive = false;
    }
}

/// Marker component for the player entity's sprite.
#[derive(Component)]
pub struct PlayerSprite;
