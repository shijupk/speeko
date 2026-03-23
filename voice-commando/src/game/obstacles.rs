use bevy::prelude::*;
use rand::Rng;

use super::actors::{ActorKind, AnimalSprites, SpriteAnimation, SPRITE_COLUMNS};
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
pub fn spawn_obstacle(
    commands: &mut Commands,
    spawner: &ObstacleSpawner,
    sprites: &AnimalSprites,
) {
    let mut rng = rand::thread_rng();

    let lane = match rng.gen_range(0..3) {
        0 => Lane::Left,
        1 => Lane::Center,
        _ => Lane::Right,
    };

    // ~35% predator, ~65% prey
    let kind = if rng.gen_range(0..100) < 35 {
        FallingObject::Predator
    } else {
        FallingObject::Prey
    };

    let actor = match kind {
        FallingObject::Prey => ActorKind::random_prey(),
        FallingObject::Predator => ActorKind::random_predator(),
    };

    let (image, layout) = sprites
        .map
        .get(&actor)
        .expect("missing sprite for actor");

    commands.spawn((
        Obstacle,
        kind,
        actor,
        ObstacleLane(lane),
        SpriteAnimation::new(0, SPRITE_COLUMNS as usize, 5.0),
        Sprite {
            image: image.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: layout.clone(),
                index: 0,
            }),
            custom_size: Some(actor.display_size()),
            ..default()
        },
        Transform::from_translation(Vec3::new(lane.x(), spawner.spawn_y, 0.0)),
    ));
}
