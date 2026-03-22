use bevy::prelude::*;

use crate::app_states::AppState;

use super::game_over::{cleanup_game_over_ui, setup_game_over_ui, game_over_input_system};
use super::hud::{cleanup_hud, setup_hud, update_hud};
use super::menu::{cleanup_menu, menu_input_system, setup_menu};
use super::subtitles::{cleanup_subtitles, setup_subtitles, update_subtitles};

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app
            // Main menu
            .add_systems(OnEnter(AppState::MainMenu), setup_menu)
            .add_systems(Update, menu_input_system.run_if(in_state(AppState::MainMenu)))
            .add_systems(OnExit(AppState::MainMenu), cleanup_menu)
            // HUD
            .add_systems(OnEnter(AppState::Playing), (setup_hud, setup_subtitles))
            .add_systems(
                Update,
                (update_hud, update_subtitles).run_if(in_state(AppState::Playing)),
            )
            .add_systems(OnExit(AppState::Playing), (cleanup_hud, cleanup_subtitles))
            // Game over
            .add_systems(OnEnter(AppState::GameOver), setup_game_over_ui)
            .add_systems(Update, game_over_input_system.run_if(in_state(AppState::GameOver)))
            .add_systems(OnExit(AppState::GameOver), cleanup_game_over_ui);
    }
}
