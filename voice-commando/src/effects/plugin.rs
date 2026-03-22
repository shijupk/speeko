use bevy::prelude::*;

use super::sound_fx::GameSounds;
use super::visual_fx::flash_system;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameSounds>()
            .add_systems(Update, flash_system);
    }
}
