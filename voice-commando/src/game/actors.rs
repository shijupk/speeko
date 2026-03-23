use std::collections::HashMap;

use bevy::prelude::*;
use rand::Rng;

// ---------------------------------------------------------------------------
// Actor identity — visual flavor on top of the gameplay FallingObject role
// ---------------------------------------------------------------------------

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    Komodo,
    Rabbit,
    Chicken,
    Goat,
    Bull,
    Tiger,
    Crocodile,
}

/// Sprite-sheet columns for every animal.
pub const SPRITE_COLUMNS: u32 = 4;
/// Tile size in the generated sprite sheets.
pub const SPRITE_TILE: u32 = 64;

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

    /// Number of rows in the sprite-sheet for this actor.
    pub fn sprite_rows(&self) -> u32 {
        match self {
            Self::Komodo => 6, // idle, walk_l, walk_r, eat, hurt, death
            _ => 1,            // idle only
        }
    }

    /// Asset file name under `assets/sprites/`.
    pub fn sprite_asset(&self) -> &'static str {
        match self {
            Self::Komodo => "sprites/komodo.png",
            Self::Rabbit => "sprites/rabbit.png",
            Self::Chicken => "sprites/chicken.png",
            Self::Goat => "sprites/goat.png",
            Self::Bull => "sprites/bull.png",
            Self::Tiger => "sprites/tiger.png",
            Self::Crocodile => "sprites/crocodile.png",
        }
    }

    /// Display size when rendered in the game world.
    pub fn display_size(&self) -> Vec2 {
        match self {
            Self::Komodo => Vec2::new(70.0, 40.0),
            Self::Rabbit => Vec2::new(35.0, 35.0),
            Self::Chicken => Vec2::new(30.0, 35.0),
            Self::Goat => Vec2::new(45.0, 45.0),
            Self::Bull => Vec2::new(60.0, 55.0),
            Self::Tiger => Vec2::new(55.0, 50.0),
            Self::Crocodile => Vec2::new(65.0, 35.0),
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

impl AnimationState {
    /// Row index in the Komodo sprite-sheet for this state.
    pub fn row_index(&self) -> usize {
        match self {
            Self::Idle => 0,
            Self::WalkLeft => 1,
            Self::WalkRight => 2,
            Self::Eat => 3,
            Self::Hurt => 4,
            Self::Death => 5,
        }
    }
}

/// Transient timer that auto-returns to a resting animation state.
#[derive(Component)]
pub struct AnimationTimer {
    pub timer: Timer,
    pub return_to: AnimationState,
}

// ---------------------------------------------------------------------------
// Sprite-sheet animation component
// ---------------------------------------------------------------------------

/// Drives frame-by-frame sprite-sheet animation.
#[derive(Component)]
pub struct SpriteAnimation {
    pub first_frame: usize,
    pub last_frame: usize,
    pub frame_timer: Timer,
}

impl SpriteAnimation {
    /// Create a new animation cycling through `columns` frames on a single
    /// row starting at `first_frame`, at `fps` frames per second.
    pub fn new(first_frame: usize, columns: usize, fps: f32) -> Self {
        Self {
            first_frame,
            last_frame: first_frame + columns - 1,
            frame_timer: Timer::from_seconds(1.0 / fps, TimerMode::Repeating),
        }
    }
}

// ---------------------------------------------------------------------------
// AnimalSprites resource — preloaded handles for every actor
// ---------------------------------------------------------------------------

/// Holds the `Image` handle and `TextureAtlasLayout` handle for each animal.
#[derive(Resource)]
pub struct AnimalSprites {
    pub map: HashMap<ActorKind, (Handle<Image>, Handle<TextureAtlasLayout>)>,
}

/// Startup system — loads all 7 sprite sheets and builds atlas layouts.
pub fn load_animal_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let kinds = [
        ActorKind::Komodo,
        ActorKind::Rabbit,
        ActorKind::Chicken,
        ActorKind::Goat,
        ActorKind::Bull,
        ActorKind::Tiger,
        ActorKind::Crocodile,
    ];

    let mut map = HashMap::new();
    for kind in kinds {
        let image: Handle<Image> = asset_server.load(kind.sprite_asset());
        let layout = TextureAtlasLayout::from_grid(
            UVec2::splat(SPRITE_TILE),
            SPRITE_COLUMNS,
            kind.sprite_rows(),
            None,
            None,
        );
        let layout_handle = layouts.add(layout);
        map.insert(kind, (image, layout_handle));
    }

    commands.insert_resource(AnimalSprites { map });
    tracing::info!("Loaded {} animal sprite sheets", kinds.len());
}
