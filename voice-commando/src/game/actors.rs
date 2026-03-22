use bevy::prelude::*;
use rand::Rng;

// ---------------------------------------------------------------------------
// Actor identity — visual flavor on top of the gameplay FallingObject role
// ---------------------------------------------------------------------------

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Komodo,
    Rabbit,
    Chicken,
    Goat,
    Bull,
    Tiger,
    Crocodile,
}

impl ActorKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Komodo => "KOMODO",
            Self::Rabbit => "RABBIT",
            Self::Chicken => "CHICKEN",
            Self::Goat => "GOAT",
            Self::Bull => "BULL",
            Self::Tiger => "TIGER",
            Self::Crocodile => "CROC",
        }
    }

    pub fn random_prey() -> Self {
        match rand::thread_rng().gen_range(0..3) {
            0 => Self::Rabbit,
            1 => Self::Chicken,
            _ => Self::Goat,
        }
    }

    pub fn random_predator() -> Self {
        match rand::thread_rng().gen_range(0..3) {
            0 => Self::Bull,
            1 => Self::Tiger,
            _ => Self::Crocodile,
        }
    }
}

// ---------------------------------------------------------------------------
// Animation state — used by the Komodo player; obstacles stay Idle
// ---------------------------------------------------------------------------

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationState {
    #[default]
    Idle,
    WalkLeft,
    WalkRight,
    Eat,
    Hurt,
    Death,
}

/// Transient timer that auto-returns to a resting animation state.
#[derive(Component)]
pub struct AnimationTimer {
    pub timer: Timer,
    pub return_to: AnimationState,
}

/// Marker on child text labels so they can be replaced with real sprites later.
#[derive(Component)]
pub struct PlaceholderLabel;

// ---------------------------------------------------------------------------
// Placeholder rendering helpers (swap this section for real sprite sheets)
// ---------------------------------------------------------------------------

pub fn placeholder_color(kind: &ActorKind) -> Color {
    match kind {
        ActorKind::Komodo => Color::srgb(0.1, 0.5, 0.1),
        ActorKind::Rabbit => Color::srgb(0.7, 0.6, 0.4),
        ActorKind::Chicken => Color::srgb(0.9, 0.8, 0.2),
        ActorKind::Goat => Color::srgb(0.6, 0.6, 0.6),
        ActorKind::Bull => Color::srgb(0.6, 0.15, 0.1),
        ActorKind::Tiger => Color::srgb(0.9, 0.5, 0.1),
        ActorKind::Crocodile => Color::srgb(0.3, 0.45, 0.2),
    }
}

pub fn placeholder_size(kind: &ActorKind) -> Vec2 {
    match kind {
        ActorKind::Komodo => Vec2::new(70.0, 40.0),
        ActorKind::Rabbit => Vec2::new(35.0, 35.0),
        ActorKind::Chicken => Vec2::new(30.0, 35.0),
        ActorKind::Goat => Vec2::new(45.0, 45.0),
        ActorKind::Bull => Vec2::new(60.0, 55.0),
        ActorKind::Tiger => Vec2::new(55.0, 50.0),
        ActorKind::Crocodile => Vec2::new(65.0, 35.0),
    }
}

pub fn brighten(color: Color, amount: f32) -> Color {
    let c = color.to_srgba();
    Color::srgb(
        (c.red + amount).min(1.0),
        (c.green + amount).min(1.0),
        (c.blue + amount).min(1.0),
    )
}
