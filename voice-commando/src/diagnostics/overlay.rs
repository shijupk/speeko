use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};

use crate::game::scoring::Score;

use super::counters::LatencyTracker;

#[derive(Component)]
pub struct DebugOverlay;

#[derive(Component)]
pub struct DebugOverlayText;

#[derive(Resource)]
pub struct DebugOverlayVisible(pub bool);

pub fn toggle_debug_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<DebugOverlayVisible>,
    mut query: Query<&mut Visibility, With<DebugOverlay>>,
) {
    if keyboard.just_pressed(KeyCode::F3) {
        visible.0 = !visible.0;
        for mut vis in query.iter_mut() {
            *vis = if visible.0 {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub fn setup_debug_overlay(mut commands: Commands) {
    commands
        .spawn((
            DebugOverlay,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(60.0),
                right: Val::Px(10.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                DebugOverlayText,
                Text::new("Debug info loading..."),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.0, 1.0, 0.0)),
            ));
        });
}

pub fn update_debug_overlay(
    visible: Res<DebugOverlayVisible>,
    diagnostics: Res<DiagnosticsStore>,
    score: Option<Res<Score>>,
    latency: Res<LatencyTracker>,
    mut text_query: Query<&mut Text, With<DebugOverlayText>>,
) {
    if !visible.0 {
        return;
    }

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    let frame_time = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    let score_info = score
        .map(|s| format!("Score: {} | Eaten: {} | Time: {:.0}s", s.total_points(), s.prey_eaten, s.time_survived))
        .unwrap_or_else(|| "No game active".to_string());

    let avg_latency = latency.average().as_millis();

    for mut text in text_query.iter_mut() {
        **text = format!(
            "FPS: {:.0}  Frame: {:.1}ms\nLatency avg: {}ms ({} samples)\n{}",
            fps, frame_time * 1000.0, avg_latency, latency.count(), score_info
        );
    }
}
