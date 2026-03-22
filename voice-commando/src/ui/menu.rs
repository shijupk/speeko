use bevy::prelude::*;

use crate::app_states::AppState;
use crate::commands::types::{GameCommand, GameCommandEvent};

#[derive(Component)]
pub struct MenuUI;

pub fn setup_menu(mut commands: Commands) {
    commands
        .spawn((
            MenuUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.15)),
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("VOICE COMMANDO"),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 0.2)),
            ));

            // Subtitle
            parent.spawn((
                Text::new("Command Rush"),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                Node {
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
            ));

            // Instructions
            parent.spawn((
                Text::new("Say \"START\" or press ENTER to play"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                Node {
                    margin: UiRect::top(Val::Px(60.0)),
                    ..default()
                },
            ));

            // Keyboard hint
            parent.spawn((
                Text::new("[C] Calibration  |  [ESC] Quit"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.4, 0.4)),
                Node {
                    margin: UiRect::top(Val::Px(20.0)),
                    ..default()
                },
            ));
        });
}

pub fn menu_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut command_events: EventReader<GameCommandEvent>,
) {
    // Keyboard: Enter to start
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Playing);
        return;
    }
    // Keyboard: C for calibration
    if keyboard.just_pressed(KeyCode::KeyC) {
        next_state.set(AppState::Calibration);
        return;
    }

    // Voice: "start" to play
    for event in command_events.read() {
        if event.command == GameCommand::Resume {
            next_state.set(AppState::Playing);
            return;
        }
    }
}

pub fn cleanup_menu(mut commands: Commands, query: Query<Entity, With<MenuUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
