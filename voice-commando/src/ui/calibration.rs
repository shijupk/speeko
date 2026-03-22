use bevy::prelude::*;

use crate::app_states::AppState;
use crate::speech::events::RecognizedWordEvent;

#[derive(Component)]
pub struct CalibrationUI;

#[derive(Component)]
pub struct CalibrationLastWord;

#[derive(Resource, Default)]
pub struct CalibrationData {
    pub word_counts: std::collections::HashMap<String, (u32, u32, f32)>, // (tested, accepted, total_confidence)
}

pub fn setup_calibration(mut commands: Commands) {
    commands.insert_resource(CalibrationData::default());

    commands
        .spawn((
            CalibrationUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.2)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("CALIBRATION"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new("Speak any command word to test recognition"),
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
                CalibrationLastWord,
                Text::new("Waiting for speech..."),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.2)),
                Node {
                    margin: UiRect::top(Val::Px(40.0)),
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
                    margin: UiRect::top(Val::Px(60.0)),
                    ..default()
                },
            ));
        });
}

pub fn calibration_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut recognized_events: EventReader<RecognizedWordEvent>,
    mut data: ResMut<CalibrationData>,
    mut last_word_text: Query<(&mut Text, &mut TextColor), With<CalibrationLastWord>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::MainMenu);
        return;
    }

    for event in recognized_events.read() {
        let entry = data.word_counts.entry(event.word.clone()).or_insert((0, 0, 0.0));
        entry.0 += 1; // tested
        entry.1 += 1; // accepted
        entry.2 += event.confidence; // total confidence

        for (mut text, mut color) in last_word_text.iter_mut() {
            **text = format!(
                "Heard: \"{}\"  Confidence: {:.0}%  ✓",
                event.word,
                event.confidence * 100.0
            );
            *color = TextColor(Color::srgb(0.2, 1.0, 0.2));
        }
    }
}

pub fn cleanup_calibration(mut commands: Commands, query: Query<Entity, With<CalibrationUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
