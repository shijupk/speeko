use bevy::prelude::*;

use crate::app_states::AppState;
use crate::commands::types::{GameCommand, GameCommandEvent};
use crate::config_resource::GameConfigRes;

use super::difficulty::Difficulty;
use super::obstacles::{spawn_obstacle, Obstacle, ObstacleSpawner, ObstacleType};
use super::player::{Player, PlayerSprite};
use super::scoring::Score;
use super::world::scroll_world;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), setup_game)
            .add_systems(OnExit(AppState::Playing), cleanup_game)
            .add_systems(OnEnter(AppState::GameOver), setup_game_over_trigger)
            .add_systems(
                Update,
                (
                    game_command_system,
                    player_tick_system,
                    obstacle_spawn_system,
                    world_scroll_system,
                    collision_system,
                    scoring_system,
                    difficulty_system,
                    player_visual_system,
                )
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            );
    }
}

fn setup_game(mut commands: Commands, config: Res<GameConfigRes>) {
    // Initialize resources
    commands.insert_resource(Score::default());
    commands.insert_resource(Difficulty::new(
        config.game.initial_scroll_speed,
        config.game.max_scroll_speed,
    ));
    commands.insert_resource(ObstacleSpawner::new(0.15));

    // Spawn player with 3-second invincibility grace period
    let mut player = Player::default();
    player.invincible_timer = Some(Timer::from_seconds(3.0, TimerMode::Once));
    commands.spawn((
        player,
        PlayerSprite,
        Sprite {
            color: Color::srgb(0.2, 0.8, 0.2),
            custom_size: Some(Vec2::new(60.0, 60.0)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, -250.0, 1.0)),
    ));

    // Spawn lane markers
    for i in 0..3 {
        let x = super::lanes::LANE_POSITIONS[i];
        commands.spawn((
            Sprite {
                color: Color::srgba(0.3, 0.3, 0.3, 0.3),
                custom_size: Some(Vec2::new(100.0, 800.0)),
                ..default()
            },
            Transform::from_translation(Vec3::new(x, 0.0, -1.0)),
        ));
    }

    tracing::info!("Game setup complete");
}

fn cleanup_game(
    mut commands: Commands,
    player_query: Query<Entity, With<Player>>,
    obstacle_query: Query<Entity, With<Obstacle>>,
) {
    for entity in player_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in obstacle_query.iter() {
        commands.entity(entity).despawn();
    }
}

fn setup_game_over_trigger() {
    tracing::info!("Game Over!");
}

fn game_command_system(
    mut events: EventReader<GameCommandEvent>,
    mut player_query: Query<&mut Player>,
    config: Res<GameConfigRes>,
    mut score: ResMut<Score>,
    mut next_state: ResMut<NextState<AppState>>,
    current_state: Res<State<AppState>>,
) {
    let Ok(mut player) = player_query.get_single_mut() else {
        return;
    };

    for event in events.read() {
        let applied = match event.command {
            GameCommand::Jump => {
                if player.is_grounded() {
                    player.start_jump(config.game.jump_duration_ms);
                    true
                } else {
                    false
                }
            }
            GameCommand::Slide => {
                if player.is_grounded() {
                    player.start_slide(config.game.slide_duration_ms);
                    true
                } else {
                    false
                }
            }
            GameCommand::MoveLeft => {
                if let Some(target) = player.lane.left() {
                    player.start_lane_change(target, config.game.lane_change_duration_ms);
                    true
                } else {
                    false
                }
            }
            GameCommand::MoveRight => {
                if let Some(target) = player.lane.right() {
                    player.start_lane_change(target, config.game.lane_change_duration_ms);
                    true
                } else {
                    false
                }
            }
            GameCommand::Freeze => {
                player.start_freeze(config.game.freeze_duration_ms);
                true
            }
            GameCommand::Resume => {
                if *current_state.get() == AppState::Playing {
                    // Already playing, could pause instead
                    false
                } else {
                    false
                }
            }
            // Gate, shield, choice — simplified for MVP
            GameCommand::OpenGate
            | GameCommand::CloseShield
            | GameCommand::AcceptChoice
            | GameCommand::RejectChoice => {
                tracing::debug!("Command {:?} not yet implemented for gameplay", event.command);
                false
            }
        };

        if applied {
            score.on_successful_command();
        } else {
            score.on_failed_command();
        }
    }
}

fn player_tick_system(time: Res<Time>, mut player_query: Query<&mut Player>) {
    for mut player in player_query.iter_mut() {
        player.tick(time.delta());
    }
}

fn obstacle_spawn_system(
    mut commands: Commands,
    time: Res<Time>,
    mut spawner: ResMut<ObstacleSpawner>,
    difficulty: Res<Difficulty>,
) {
    // Dynamically adjust spawn interval based on difficulty
    let interval = 1.0 / difficulty.obstacle_rate;
    spawner.spawn_timer.set_duration(std::time::Duration::from_secs_f32(interval));

    spawner.spawn_timer.tick(time.delta());
    if spawner.spawn_timer.just_finished() {
        spawn_obstacle(&mut commands, &spawner);
    }
}

fn world_scroll_system(
    time: Res<Time>,
    difficulty: Res<Difficulty>,
    spawner: Res<ObstacleSpawner>,
    mut obstacle_query: Query<(Entity, &mut Transform), With<Obstacle>>,
    mut commands: Commands,
) {
    scroll_world(&time, &difficulty, &spawner, &mut obstacle_query, &mut commands);
}

fn collision_system(
    mut player_query: Query<&mut Player>,
    obstacle_query: Query<(&Transform, &ObstacleType), With<Obstacle>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Ok(mut player) = player_query.get_single_mut() else {
        return;
    };

    if !player.is_alive() || player.invincible_timer.is_some() {
        return;
    }

    let player_x = player.lane.x();
    let lane_half_width = 50.0;

    for (transform, obstacle_type) in obstacle_query.iter() {
        let obs_y = transform.translation.y;
        // Check obstacles near the player's y position (-250); collision zone is ±40px
        let player_y = -250.0;
        if (obs_y - player_y).abs() > 40.0 {
            continue;
        }
        let obs_x = transform.translation.x;
        if (player_x - obs_x).abs() > lane_half_width {
            continue;
        }
        let hit = match obstacle_type {
            ObstacleType::LowBarrier => {
                player.action != super::player::PlayerAction::Jumping
            }
            ObstacleType::HighBarrier => {
                player.action != super::player::PlayerAction::Sliding
            }
            ObstacleType::LaneBlocker { lane } => player.lane == *lane,
        };
        if hit {
            player.die();
            next_state.set(AppState::GameOver);
            return;
        }
    }
}

fn scoring_system(time: Res<Time>, difficulty: Res<Difficulty>, mut score: ResMut<Score>) {
    score.add_distance(difficulty.scroll_speed * time.delta_secs());
}

fn difficulty_system(time: Res<Time>, mut difficulty: ResMut<Difficulty>) {
    difficulty.update(time.delta_secs());
}

fn player_visual_system(
    player_query: Query<&Player>,
    mut sprite_query: Query<&mut Transform, With<PlayerSprite>>,
) {
    let Ok(player) = player_query.get_single() else {
        return;
    };
    let Ok(mut transform) = sprite_query.get_single_mut() else {
        return;
    };

    // Update x position based on lane
    let target_x = player.lane.x();
    transform.translation.x = target_x;

    // Visual feedback for actions
    match player.action {
        super::player::PlayerAction::Jumping => {
            transform.translation.y = -220.0; // Jump up
        }
        super::player::PlayerAction::Sliding => {
            transform.translation.y = -270.0; // Slide down
            transform.scale.y = 0.5; // Squish
        }
        _ => {
            transform.translation.y = -250.0;
            transform.scale.y = 1.0;
        }
    }
}
