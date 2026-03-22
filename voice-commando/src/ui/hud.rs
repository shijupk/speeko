use bevy::prelude::*;

use crate::app_states::AppState;
use crate::game::player::Player;
use crate::game::plugin::LastCommand;
use crate::game::scoring::Score;

#[derive(Component)]
pub struct HudUI;

#[derive(Component)]
pub struct EnergyText;

#[derive(Component)]
pub struct ScoreText;

#[derive(Component)]
pub struct StateText;

#[derive(Component)]
pub struct LastCommandText;

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
                EnergyText,
                Text::new("Energy: 100"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 1.0, 0.2)),
            ));
            parent.spawn((
                ScoreText,
                Text::new("Score: 0"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                StateText,
                Text::new("Running"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.2)),
            ));
            parent.spawn((
                LastCommandText,
                Text::new("Last: --"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
            ));
        });
}

pub fn update_hud(
    score: Option<Res<Score>>,
    player_query: Query<&Player>,
    last_cmd: Option<Res<LastCommand>>,
    state: Res<State<AppState>>,
    mut energy_text: Query<
        (&mut Text, &mut TextColor),
        (
            With<EnergyText>,
            Without<ScoreText>,
            Without<StateText>,
            Without<LastCommandText>,
        ),
    >,
    mut score_text: Query<
        &mut Text,
        (
            With<ScoreText>,
            Without<EnergyText>,
            Without<StateText>,
            Without<LastCommandText>,
        ),
    >,
    mut state_text: Query<
        &mut Text,
        (
            With<StateText>,
            Without<EnergyText>,
            Without<ScoreText>,
            Without<LastCommandText>,
        ),
    >,
    mut cmd_text: Query<
        &mut Text,
        (
            With<LastCommandText>,
            Without<EnergyText>,
            Without<ScoreText>,
            Without<StateText>,
        ),
    >,
) {
    // Energy
    if let Ok(player) = player_query.get_single() {
        for (mut text, mut color) in energy_text.iter_mut() {
            **text = format!("Energy: {:.0}", player.energy);
            let t = (player.energy / 100.0).clamp(0.0, 1.0);
            *color = TextColor(Color::srgb(1.0 - t, t, 0.2));
        }
    }

    // Score
    if let Some(ref score) = score {
        for mut text in score_text.iter_mut() {
            **text = format!("Score: {} ({} eaten)", score.total_points(), score.prey_eaten);
        }
    }

    // State
    for mut text in state_text.iter_mut() {
        **text = format!("{:?}", state.get());
    }

    // Last command
    if let Some(ref last_cmd) = last_cmd {
        for mut text in cmd_text.iter_mut() {
            if last_cmd.text.is_empty() {
                **text = "Last: --".to_string();
            } else {
                **text = format!("Last: {}", last_cmd.text);
            }
        }
    }
}

pub fn cleanup_hud(mut commands: Commands, query: Query<Entity, With<HudUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
