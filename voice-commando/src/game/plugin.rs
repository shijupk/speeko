use bevy::prelude::*;

use crate::app_states::AppState;
use crate::commands::types::{GameCommand, GameCommandEvent};
use crate::config_resource::GameConfigRes;

use super::actors::{
    placeholder_color, placeholder_size, brighten, ActorKind, AnimationState, AnimationTimer,
    PlaceholderLabel,
};
use super::difficulty::Difficulty;
use super::lanes::LANE_POSITIONS;
use super::obstacles::{
    spawn_obstacle, FallingObject, Obstacle, ObstacleLane, ObstacleSpawner, PreyInteraction,
};
use super::player::{Player, PlayerSprite};
use super::scoring::Score;
use super::world::scroll_world;

/// Tracks the last recognized voice command (for HUD display).
#[derive(Resource, Default)]
pub struct LastCommand {
    pub text: String,
}

/// Marker for lane background sprites so they can be cleaned up.
#[derive(Component)]
struct LaneMarker;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastCommand>()
            // Idle: listen for "start"
            .add_systems(
                Update,
                idle_command_system.run_if(in_state(AppState::Idle)),
            )
            // Running: full game loop
            .add_systems(OnEnter(AppState::Running), setup_game)
            .add_systems(
                Update,
                (
                    running_command_system,
                    energy_drain_system,
                    obstacle_spawn_system,
                    world_scroll_system,
                    prey_interaction_system,
                    collision_system,
                    difficulty_system,
                    scoring_system,
                    animation_timer_system,
                    player_visual_system,
                    check_death_system,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(OnExit(AppState::Running), cleanup_game)
            // Paused
            .add_systems(
                Update,
                paused_command_system.run_if(in_state(AppState::Paused)),
            )
            // GameOver
            .add_systems(
                Update,
                game_over_command_system.run_if(in_state(AppState::GameOver)),
            );
    }
}

// ---------------------------------------------------------------------------
// State: Idle
// ---------------------------------------------------------------------------

fn idle_command_system(
    mut events: EventReader<GameCommandEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    mut last_cmd: ResMut<LastCommand>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Running);
        return;
    }
    for event in events.read() {
        last_cmd.text = format!("{:?}", event.command);
        if event.command == GameCommand::Start {
            next_state.set(AppState::Running);
        }
    }
}

// ---------------------------------------------------------------------------
// State: Running — setup / cleanup
// ---------------------------------------------------------------------------

fn setup_game(mut commands: Commands, config: Res<GameConfigRes>) {
    commands.insert_resource(Score::default());
    commands.insert_resource(Difficulty::new(
        config.game.initial_scroll_speed,
        config.game.max_scroll_speed,
    ));
    commands.insert_resource(ObstacleSpawner::new(0.15));

    // Spawn player (Komodo dragon) at center lane
    let komodo = ActorKind::Komodo;
    commands
        .spawn((
            Player::default(),
            PlayerSprite,
            komodo,
            AnimationState::Idle,
            Sprite {
                color: placeholder_color(&komodo),
                custom_size: Some(placeholder_size(&komodo)),
                ..default()
            },
            Transform::from_translation(Vec3::new(0.0, -250.0, 1.0)),
        ))
        .with_children(|parent| {
            parent.spawn((
                PlaceholderLabel,
                Text2d::new(komodo.label()),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Transform::from_translation(Vec3::new(0.0, 0.0, 0.1)),
            ));
        });

    // Lane marker backgrounds
    for &x in LANE_POSITIONS.iter() {
        commands.spawn((
            LaneMarker,
            Sprite {
                color: Color::srgba(0.3, 0.3, 0.3, 0.3),
                custom_size: Some(Vec2::new(100.0, 800.0)),
                ..default()
            },
            Transform::from_translation(Vec3::new(x, 0.0, -1.0)),
        ));
    }

    tracing::info!("Game started");
}

fn cleanup_game(
    mut commands: Commands,
    player_query: Query<Entity, With<Player>>,
    obstacle_query: Query<Entity, With<Obstacle>>,
    lane_query: Query<Entity, With<LaneMarker>>,
) {
    for entity in player_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in obstacle_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in lane_query.iter() {
        commands.entity(entity).despawn();
    }
}

// ---------------------------------------------------------------------------
// State: Running — systems
// ---------------------------------------------------------------------------

fn running_command_system(
    mut events: EventReader<GameCommandEvent>,
    mut player_query: Query<(Entity, &mut Player, &mut AnimationState)>,
    mut next_state: ResMut<NextState<AppState>>,
    mut last_cmd: ResMut<LastCommand>,
    config: Res<GameConfigRes>,
    mut score: ResMut<Score>,
    prey_query: Query<(Entity, &ObstacleLane, &PreyInteraction)>,
    mut commands: Commands,
) {
    let Ok((player_entity, mut player, mut anim_state)) = player_query.get_single_mut() else {
        return;
    };

    for event in events.read() {
        last_cmd.text = format!("{:?}", event.command);
        match event.command {
            GameCommand::MoveLeft => {
                player.move_left();
                *anim_state = AnimationState::WalkLeft;
                commands.entity(player_entity).insert(AnimationTimer {
                    timer: Timer::from_seconds(0.3, TimerMode::Once),
                    return_to: AnimationState::Idle,
                });
            }
            GameCommand::MoveRight => {
                player.move_right();
                *anim_state = AnimationState::WalkRight;
                commands.entity(player_entity).insert(AnimationTimer {
                    timer: Timer::from_seconds(0.3, TimerMode::Once),
                    return_to: AnimationState::Idle,
                });
            }
            GameCommand::Stop => next_state.set(AppState::Paused),
            GameCommand::Close => {
                player.die();
                next_state.set(AppState::Idle);
            }
            GameCommand::EatPrey => {
                for (entity, lane, interaction) in prey_query.iter() {
                    if lane.0 == player.lane && !interaction.timer.finished() {
                        player.eat(config.game.energy_gain_per_prey);
                        score.on_eat();
                        commands.entity(entity).despawn();
                        *anim_state = AnimationState::Eat;
                        commands.entity(player_entity).insert(AnimationTimer {
                            timer: Timer::from_seconds(0.5, TimerMode::Once),
                            return_to: AnimationState::Idle,
                        });
                        break;
                    }
                }
            }
            GameCommand::Start => { /* already running */ }
        }
    }
}

fn energy_drain_system(
    time: Res<Time>,
    config: Res<GameConfigRes>,
    mut player_query: Query<&mut Player>,
) {
    let Ok(mut player) = player_query.get_single_mut() else {
        return;
    };
    player.drain_energy(config.game.energy_drain_per_sec * time.delta_secs());
}

fn obstacle_spawn_system(
    mut commands: Commands,
    time: Res<Time>,
    mut spawner: ResMut<ObstacleSpawner>,
    difficulty: Res<Difficulty>,
) {
    let interval = 1.0 / difficulty.spawn_rate;
    spawner
        .spawn_timer
        .set_duration(std::time::Duration::from_secs_f32(interval));
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
    scroll_world(
        &time,
        &difficulty,
        &spawner,
        &mut obstacle_query,
        &mut commands,
    );
}

fn prey_interaction_system(
    time: Res<Time>,
    config: Res<GameConfigRes>,
    mut commands: Commands,
    mut prey_query: Query<
        (
            Entity,
            &FallingObject,
            &Transform,
            Option<&mut PreyInteraction>,
        ),
        With<Obstacle>,
    >,
) {
    let player_y = -250.0;
    // Large zone so the interaction timer starts early — gives time for
    // the ~2s voice recognition latency before the prey passes.
    let interaction_zone = 200.0;

    for (entity, kind, transform, interaction) in prey_query.iter_mut() {
        if *kind != FallingObject::Prey {
            continue;
        }

        let y = transform.translation.y;
        let in_zone = (y - player_y).abs() < interaction_zone;

        match interaction {
            Some(mut inter) => {
                inter.timer.tick(time.delta());
                if inter.timer.finished() {
                    // Prey window expired — despawn
                    commands.entity(entity).despawn();
                }
            }
            None => {
                if in_zone {
                    commands.entity(entity).insert(PreyInteraction {
                        timer: Timer::from_seconds(
                            config.game.prey_interaction_secs,
                            TimerMode::Once,
                        ),
                    });
                }
            }
        }
    }
}

fn collision_system(
    mut player_query: Query<(&mut Player, &mut AnimationState)>,
    obstacle_query: Query<(&Transform, &FallingObject, &ObstacleLane), With<Obstacle>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Ok((mut player, mut anim_state)) = player_query.get_single_mut() else {
        return;
    };
    if !player.alive {
        return;
    }

    let player_y = -250.0;
    let hit_tolerance_y = 30.0;

    for (transform, kind, lane) in obstacle_query.iter() {
        if *kind != FallingObject::Predator {
            continue;
        }
        if lane.0 != player.lane {
            continue;
        }
        if (transform.translation.y - player_y).abs() < hit_tolerance_y {
            *anim_state = AnimationState::Hurt;
            player.die();
            next_state.set(AppState::GameOver);
            return;
        }
    }
}

fn difficulty_system(time: Res<Time>, mut difficulty: ResMut<Difficulty>) {
    difficulty.update(time.delta_secs());
}

fn scoring_system(time: Res<Time>, mut score: ResMut<Score>) {
    score.tick(time.delta_secs());
}

fn player_visual_system(
    mut query: Query<
        (&Player, &AnimationState, &ActorKind, &mut Transform, &mut Sprite),
        With<PlayerSprite>,
    >,
) {
    let Ok((player, anim_state, kind, mut transform, mut sprite)) = query.get_single_mut() else {
        return;
    };

    // Snap to lane
    transform.translation.x = player.lane.x();
    transform.translation.y = -250.0;

    // Base color from animation state
    let base = match anim_state {
        AnimationState::Idle => placeholder_color(kind),
        AnimationState::WalkLeft | AnimationState::WalkRight => {
            brighten(placeholder_color(kind), 0.15)
        }
        AnimationState::Eat => Color::srgb(0.2, 1.0, 0.3),
        AnimationState::Hurt => Color::srgb(1.0, 0.2, 0.2),
        AnimationState::Death => Color::srgb(0.3, 0.3, 0.3),
    };

    // Tint toward red as energy drops
    let energy_t = (player.energy / 100.0).clamp(0.0, 1.0);
    let b = base.to_srgba();
    sprite.color = Color::srgb(
        b.red * energy_t + (1.0 - energy_t),
        b.green * energy_t,
        b.blue * energy_t * 0.5,
    );
}

fn check_death_system(
    player_query: Query<&Player>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Ok(player) = player_query.get_single() else {
        return;
    };
    if !player.alive {
        next_state.set(AppState::GameOver);
    }
}

// ---------------------------------------------------------------------------
// Animation timer — ticks transient animations and returns to resting state
// ---------------------------------------------------------------------------

fn animation_timer_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut AnimationTimer, &mut AnimationState)>,
) {
    for (entity, mut anim_timer, mut state) in query.iter_mut() {
        anim_timer.timer.tick(time.delta());
        if anim_timer.timer.finished() {
            *state = anim_timer.return_to;
            commands.entity(entity).remove::<AnimationTimer>();
        }
    }
}

// ---------------------------------------------------------------------------
// State: Paused
// ---------------------------------------------------------------------------

fn paused_command_system(
    mut events: EventReader<GameCommandEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    mut last_cmd: ResMut<LastCommand>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Running);
        return;
    }
    for event in events.read() {
        last_cmd.text = format!("{:?}", event.command);
        match event.command {
            GameCommand::Start => next_state.set(AppState::Running),
            GameCommand::Close => next_state.set(AppState::Idle),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// State: GameOver
// ---------------------------------------------------------------------------

fn game_over_command_system(
    mut events: EventReader<GameCommandEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    mut last_cmd: ResMut<LastCommand>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Running);
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Idle);
        return;
    }
    for event in events.read() {
        last_cmd.text = format!("{:?}", event.command);
        match event.command {
            GameCommand::Start => next_state.set(AppState::Running),
            GameCommand::Close => next_state.set(AppState::Idle),
            _ => {}
        }
    }
}
