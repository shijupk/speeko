use bevy::prelude::*;

#[derive(Resource, Debug)]
pub struct Score {
    pub prey_eaten: u32,
    pub time_survived: f32,
}

impl Default for Score {
    fn default() -> Self {
        Self {
            prey_eaten: 0,
            time_survived: 0.0,
        }
    }
}

impl Score {
    pub fn on_eat(&mut self) {
        self.prey_eaten += 1;
    }

    pub fn tick(&mut self, delta: f32) {
        self.time_survived += delta;
    }

    pub fn total_points(&self) -> u32 {
        self.prey_eaten * 100 + self.time_survived as u32
    }
}
