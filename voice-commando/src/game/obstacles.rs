use bevy::prelude::*;
use rand::Rng;

use super::lanes::Lane;

/// The two object types that fall toward the player.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallingObject {
    /// Green — say "yes" to eat and gain energy.
    Prey,
    /// Red — colliding with this kills the player.
    Predator,
}

/// Marker for all falling objects.
#[derive(Component)]
pub struct Obstacle;

/// Tracks which lane a falling object is in.
#[derive(Component)]
pub struct ObstacleLane(pub Lane);

/// When a prey object reaches the player, this timer is added.
/// The player must say "yes" before it expires to eat.
#[derive(Component)]
pub struct PreyInteraction {
    pub timer: Timer,
}

/// Controls spawn timing.
#[derive(Resource)]
pub struct ObstacleSpawner {
    pub spawn_timer: Timer,
    pub spawn_y: f32,
    pub despawn_y: f32,
}

impl ObstacleSpawner {
    pub fn new(rate: f32) -> Self {
        Self {
            spawn_timer: Timer::from_seconds(1.0 / rate, TimerMode::Repeating),
            spawn_y: 400.0,
            despawn_y: -400.0,
        }
    }
}

/// Spawn a random prey or predator in a random lane.
pub fn spawn_obstacle(commands: &mut Commands, spawner: &ObstacleSpawner) {
    let mut rng = rand::thread_rng();

    let lane = match rng.gen_range(0..3) {
        0 => Lane::Left,
        1 => Lane::Center,
        _ => Lane::Right,
    };

    // ~40% predator, ~60% prey
    let kind = if rng.gen_range(0..10) < 4 {
        FallingObject::Predator
    } else {
        FallingObject::Prey
    };

    let color = match kind {
        FallingObject::Prey => Color::srgb(0.2, 0.9, 0.2),
        FallingObject::Predator => Color::srgb(0.9, 0.2, 0.2),
    };

    let size = match kind {
        FallingObject::Prey => Vec2::new(50.0, 50.0),
        FallingObject::Predator => Vec2::new(60.0, 60.0),
    };

    commands.spawn((
        Obstacle,
        kind,
        ObstacleLane(lane),
        Sprite {
            color,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_translation(Vec3::new(lane.x(), spawner.spawn_y, 0.0)),
    ));
}
