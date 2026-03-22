use bevy::prelude::*;
use std::time::Instant;

use crate::config_resource::GameConfigRes;
use crate::speech::events::RecognizedWordEvent;

use super::cooldown::CooldownState;
use super::debounce::DebounceState;
use super::mapping::map_word_to_command;
use super::types::GameCommandEvent;

pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GameCommandEvent>()
            .add_systems(Startup, setup_command_resources)
            .add_systems(
                Update,
                (cooldown_tick_system, command_mapping_system).chain(),
            );
    }
}

fn setup_command_resources(mut commands: Commands, config: Res<GameConfigRes>) {
    commands.insert_resource(DebounceState::new(config.game.debounce_ms));
    commands.insert_resource(CooldownState::new());
}

fn cooldown_tick_system(time: Res<Time>, mut cooldown: ResMut<CooldownState>) {
    cooldown.tick_all(time.delta());
}

fn command_mapping_system(
    mut events: EventReader<RecognizedWordEvent>,
    mut debounce: ResMut<DebounceState>,
    cooldown: Res<CooldownState>,
    mut command_events: EventWriter<GameCommandEvent>,
) {
    let now = Instant::now();

    for event in events.read() {
        let Some(command) = map_word_to_command(&event.word) else {
            tracing::debug!("No command mapping for word: '{}'", event.word);
            continue;
        };

        if !debounce.can_accept(command, now) {
            tracing::debug!("Command {:?} debounced", command);
            continue;
        }

        if !cooldown.is_ready(command) {
            tracing::debug!("Command {:?} on cooldown", command);
            continue;
        }

        debounce.record_accept(command, now);

        command_events.send(GameCommandEvent {
            command,
            source_word: event.word.clone(),
            confidence: event.confidence,
            issued_at: now,
        });

        tracing::info!(
            "Command issued: {:?} (word='{}', confidence={:.2})",
            command,
            event.word,
            event.confidence
        );
    }
}
