use bevy::prelude::*;

use super::difficulty::Difficulty;
use super::obstacles::{Obstacle, ObstacleSpawner};

/// Scroll all obstacles downward and despawn when off-screen.
pub fn scroll_world(
    time: &Res<Time>,
    difficulty: &Res<Difficulty>,
    spawner: &Res<ObstacleSpawner>,
    obstacle_query: &mut Query<(Entity, &mut Transform), With<Obstacle>>,
    commands: &mut Commands,
) {
    let scroll_delta = difficulty.scroll_speed * time.delta_secs() * 100.0;

    for (entity, mut transform) in obstacle_query.iter_mut() {
        transform.translation.y -= scroll_delta;
        if transform.translation.y < spawner.despawn_y {
            commands.entity(entity).despawn();
        }
    }
}
