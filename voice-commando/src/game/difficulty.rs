use bevy::prelude::*;

#[derive(Resource)]
pub struct Difficulty {
    pub scroll_speed: f32,
    pub spawn_rate: f32,
    pub time_elapsed: f32,
    pub initial_speed: f32,
    pub max_speed: f32,
}

impl Difficulty {
    pub fn new(initial_speed: f32, max_speed: f32) -> Self {
        Self {
            scroll_speed: initial_speed,
            spawn_rate: 0.25,
            time_elapsed: 0.0,
            initial_speed,
            max_speed,
        }
    }

    pub fn update(&mut self, delta: f32) {
        self.time_elapsed += delta;
        let t = (self.time_elapsed / 180.0).min(1.0);
        self.scroll_speed = self.initial_speed + t * (self.max_speed - self.initial_speed);
        self.spawn_rate = 0.25 + t * 0.35; // 0.25 → 0.6 over 3 minutes
    }
}
