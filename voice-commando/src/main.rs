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

use app_states::AppState;
use audio_bridge::plugin::AudioBridgePlugin;
use commands::plugin::CommandPlugin;
use config_resource::GameConfigRes;
use diagnostics::plugin::DiagnosticsPlugin;
use effects::plugin::EffectsPlugin;
use game::plugin::GamePlugin;
use speech::plugin::SpeechPlugin;
use ui::plugin::UIPlugin;

#[derive(Parser, Debug)]
#[command(name = "voice-commando", about = "Voice-controlled 2D lane survival game")]
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

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = app_config::load_config(Some(&cli.config));
    let resolution = config.game.display.resolution;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Commando Komodo - Voice Survival".into(),
                resolution: (resolution[0] as f32, resolution[1] as f32).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .insert_resource(GameConfigRes::from(config))
        .add_plugins(AudioBridgePlugin)
        .add_plugins(SpeechPlugin)
        .add_plugins(CommandPlugin)
        .add_plugins(GamePlugin)
        .add_plugins(UIPlugin)
        .add_plugins(EffectsPlugin)
        .add_plugins(DiagnosticsPlugin)
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
