use bevy::prelude::*;

use super::lanes::Lane;
use super::obstacles::{Obstacle, ObstacleType};
use super::player::{Player, PlayerAction};

/// Simple AABB collision check between player and obstacles.
pub fn check_collisions(
    player_query: &Query<(&Player, &Transform)>,
    obstacle_query: &Query<(&Transform, &ObstacleType), With<Obstacle>>,
) -> bool {
    let Ok((player, player_tf)) = player_query.get_single() else {
        return false;
    };

    if !player.is_alive() {
        return false;
    }

    // Player is invincible during freeze
    if player.invincible_timer.is_some() {
        return false;
    }

    let player_x = player.lane.x();
    let player_y = player_tf.translation.y;
    let lane_half_width = 50.0;
    let hit_tolerance_y = 40.0;

    for (transform, obstacle_type) in obstacle_query.iter() {
        let obs_y = transform.translation.y;
        // Only check obstacles near the player's actual y position
        if (obs_y - player_y).abs() > hit_tolerance_y {
            continue;
        }

        let obs_x = transform.translation.x;

        // Check x overlap
        if (player_x - obs_x).abs() > lane_half_width {
            continue;
        }

        // Check if player action avoids the obstacle
        match obstacle_type {
            ObstacleType::LowBarrier => {
                // Jump to avoid
                if player.action != PlayerAction::Jumping {
                    return true;
                }
            }
            ObstacleType::HighBarrier => {
                // Slide to avoid
                if player.action != PlayerAction::Sliding {
                    return true;
                }
            }
            ObstacleType::LaneBlocker { lane } => {
                // Must not be in that lane
                if player.lane == *lane {
                    return true;
                }
            }
        }
    }

    false
}
