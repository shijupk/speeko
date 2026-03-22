use bevy::prelude::*;

/// Screen flash effect component.
#[derive(Component)]
pub struct ScreenFlash {
    pub timer: Timer,
}

/// Spawn a brief screen flash overlay.
pub fn spawn_flash(commands: &mut Commands, color: Color, duration_secs: f32) {
    commands.spawn((
        ScreenFlash {
            timer: Timer::from_seconds(duration_secs, TimerMode::Once),
        },
        Sprite {
            color,
            custom_size: Some(Vec2::new(2000.0, 2000.0)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 100.0)),
    ));
}

/// Tick and despawn flash effects.
pub fn flash_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ScreenFlash, &mut Sprite)>,
) {
    for (entity, mut flash, mut sprite) in query.iter_mut() {
        flash.timer.tick(time.delta());
        // Fade out
        let alpha = 1.0 - flash.timer.fraction();
        sprite.color = sprite.color.with_alpha(alpha * 0.3);
        if flash.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}
