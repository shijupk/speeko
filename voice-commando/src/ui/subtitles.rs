use bevy::prelude::*;

use crate::speech::events::{RecognizedWordEvent, RejectedWordEvent};

#[derive(Component)]
pub struct SubtitleUI;

#[derive(Component)]
pub struct SubtitleText;

#[derive(Resource)]
pub struct SubtitleTimer(pub Timer);

pub fn setup_subtitles(mut commands: Commands) {
    commands.insert_resource(SubtitleTimer(Timer::from_seconds(2.0, TimerMode::Once)));

    commands
        .spawn((
            SubtitleUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                position_type: PositionType::Absolute,
                bottom: Val::Px(20.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                SubtitleText,
                Text::new(""),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
            ));
        });
}

pub fn update_subtitles(
    time: Res<Time>,
    mut timer: ResMut<SubtitleTimer>,
    mut recognized_events: EventReader<RecognizedWordEvent>,
    mut rejected_events: EventReader<RejectedWordEvent>,
    mut subtitle_text: Query<(&mut Text, &mut TextColor), With<SubtitleText>>,
) {
    timer.0.tick(time.delta());

    for event in recognized_events.read() {
        for (mut text, mut color) in subtitle_text.iter_mut() {
            **text = format!("Heard: \"{}\" ({:.0}%) ✓", event.word, event.confidence * 100.0);
            *color = TextColor(Color::srgb(0.2, 1.0, 0.2));
        }
        timer.0.reset();
    }

    for event in rejected_events.read() {
        let guess = event
            .best_guess
            .as_deref()
            .unwrap_or("???");
        for (mut text, mut color) in subtitle_text.iter_mut() {
            **text = format!("Heard: \"{}\" ({:.0}%) ✗", guess, event.confidence * 100.0);
            *color = TextColor(Color::srgb(1.0, 0.3, 0.3));
        }
        timer.0.reset();
    }

    // Fade out after timer
    if timer.0.finished() {
        for (mut text, _) in subtitle_text.iter_mut() {
            **text = String::new();
        }
    }
}

pub fn cleanup_subtitles(mut commands: Commands, query: Query<Entity, With<SubtitleUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
