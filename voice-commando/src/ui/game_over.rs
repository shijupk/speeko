use bevy::prelude::*;

use crate::app_states::AppState;
use crate::commands::types::{GameCommand, GameCommandEvent};
use crate::game::scoring::Score;

#[derive(Component)]
pub struct GameOverUI;

pub fn setup_game_over_ui(mut commands: Commands, score: Option<Res<Score>>) {
    let (points, distance, max_combo) = score
        .map(|s| (s.points, s.distance, s.max_combo))
        .unwrap_or((0, 0.0, 0));

    commands
        .spawn((
            GameOverUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("GAME OVER"),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.2, 0.2)),
            ));

            parent.spawn((
                Text::new(format!("Score: {}  |  Distance: {:.0}m  |  Max Combo: {}x", points, distance, max_combo)),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::top(Val::Px(30.0)),
                    ..default()
                },
            ));

            parent.spawn((
                Text::new("Say \"START\" or press ENTER to play again"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                Node {
                    margin: UiRect::top(Val::Px(40.0)),
                    ..default()
                },
            ));

            parent.spawn((
                Text::new("[ESC] Main Menu"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.4, 0.4)),
                Node {
                    margin: UiRect::top(Val::Px(15.0)),
                    ..default()
                },
            ));
        });
}

pub fn game_over_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut command_events: EventReader<GameCommandEvent>,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Playing);
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::MainMenu);
        return;
    }

    for event in command_events.read() {
        if event.command == GameCommand::Resume {
            next_state.set(AppState::Playing);
            return;
        }
    }
}

pub fn cleanup_game_over_ui(mut commands: Commands, query: Query<Entity, With<GameOverUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
