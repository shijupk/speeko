mod app_config;
mod app_states;
mod audio_bridge;
mod commands;
pub mod config_resource;
mod diagnostics;
mod effects;
mod game;
mod speech;
mod ui;

use bevy::prelude::*;
use clap::Parser;

use config_resource::GameConfigRes;

use app_states::AppState;
use audio_bridge::plugin::AudioBridgePlugin;
use commands::plugin::CommandPlugin;
use diagnostics::plugin::DiagnosticsPlugin;
use effects::plugin::EffectsPlugin;
use game::plugin::GamePlugin;
use speech::plugin::SpeechPlugin;
use ui::plugin::UIPlugin;

#[derive(Parser, Debug)]
#[command(name = "voice-commando", about = "Voice-controlled 2D runner game")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "config/voice_commando.toml")]
    config: String,

    /// Enable debug overlay on startup
    #[arg(long)]
    debug: bool,
}

fn main() {
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load config
    let config = app_config::load_config(Some(&cli.config));

    let resolution = config.game.display.resolution;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Voice Commando — Command Rush".into(),
                resolution: (resolution[0] as f32, resolution[1] as f32).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .insert_resource(GameConfigRes::from(config))
        // Core plugins
        .add_plugins(AudioBridgePlugin)
        .add_plugins(SpeechPlugin)
        .add_plugins(CommandPlugin)
        .add_plugins(GamePlugin)
        .add_plugins(UIPlugin)
        .add_plugins(EffectsPlugin)
        .add_plugins(DiagnosticsPlugin)
        // Transition from Loading → MainMenu on startup
        .add_systems(OnEnter(AppState::Loading), transition_to_menu)
        // Camera
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn transition_to_menu(mut next_state: ResMut<NextState<AppState>>) {
    tracing::info!("Loading complete, transitioning to MainMenu");
    next_state.set(AppState::MainMenu);
}
