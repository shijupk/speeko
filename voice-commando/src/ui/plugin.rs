use bevy::prelude::*;

use crate::app_states::AppState;

use super::game_over::{cleanup_game_over_ui, setup_game_over_ui};
use super::hud::{cleanup_hud, setup_hud, update_hud};
use super::menu::{cleanup_menu, setup_menu};
use super::subtitles::{cleanup_subtitles, setup_subtitles, update_subtitles};

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app
            // Idle: title screen
            .add_systems(OnEnter(AppState::Idle), setup_menu)
            .add_systems(OnExit(AppState::Idle), cleanup_menu)
            // Running: HUD + subtitles
            .add_systems(OnEnter(AppState::Running), (setup_hud, setup_subtitles))
            .add_systems(
                Update,
                (update_hud, update_subtitles).run_if(in_state(AppState::Running)),
            )
            .add_systems(OnExit(AppState::Running), (cleanup_hud, cleanup_subtitles))
            // GameOver: overlay
            .add_systems(OnEnter(AppState::GameOver), setup_game_over_ui)
            .add_systems(OnExit(AppState::GameOver), cleanup_game_over_ui);
    }
}
