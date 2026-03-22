use bevy::prelude::*;

use crate::app_states::AppState;

#[derive(Component)]
pub struct SettingsUI;

pub fn setup_settings(mut commands: Commands) {
    commands
        .spawn((
            SettingsUI,
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
            parent.spawn((
                Text::new("SETTINGS"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new("Settings screen — coming soon"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                Node {
                    margin: UiRect::top(Val::Px(20.0)),
                    ..default()
                },
            ));

            parent.spawn((
                Text::new("[ESC] Back to Menu"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.4, 0.4)),
                Node {
                    margin: UiRect::top(Val::Px(40.0)),
                    ..default()
                },
            ));
        });
}

pub fn settings_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::MainMenu);
    }
}

pub fn cleanup_settings(mut commands: Commands, query: Query<Entity, With<SettingsUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
