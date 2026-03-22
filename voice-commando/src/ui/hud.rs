use bevy::prelude::*;

use crate::game::scoring::Score;

#[derive(Component)]
pub struct HudUI;

#[derive(Component)]
pub struct ScoreText;

#[derive(Component)]
pub struct ComboText;

#[derive(Component)]
pub struct DistanceText;

pub fn setup_hud(mut commands: Commands) {
    commands
        .spawn((
            HudUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(50.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceAround,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|parent| {
            parent.spawn((
                ScoreText,
                Text::new("Score: 0"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                ComboText,
                Text::new("Combo: 0x"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.8, 0.0)),
            ));
            parent.spawn((
                DistanceText,
                Text::new("Distance: 0m"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

pub fn update_hud(
    score: Option<Res<Score>>,
    mut score_text: Query<&mut Text, (With<ScoreText>, Without<ComboText>, Without<DistanceText>)>,
    mut combo_text: Query<&mut Text, (With<ComboText>, Without<ScoreText>, Without<DistanceText>)>,
    mut distance_text: Query<&mut Text, (With<DistanceText>, Without<ScoreText>, Without<ComboText>)>,
) {
    let Some(score) = score else { return };

    for mut text in score_text.iter_mut() {
        **text = format!("Score: {}", score.points);
    }
    for mut text in combo_text.iter_mut() {
        **text = format!("Combo: {}x", score.combo);
    }
    for mut text in distance_text.iter_mut() {
        **text = format!("Distance: {:.0}m", score.distance);
    }
}

pub fn cleanup_hud(mut commands: Commands, query: Query<Entity, With<HudUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
