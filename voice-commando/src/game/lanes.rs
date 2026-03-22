pub const LANE_WIDTH: f32 = 120.0;
pub const LANE_POSITIONS: [f32; 3] = [-LANE_WIDTH, 0.0, LANE_WIDTH];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    Left = 0,
    Center = 1,
    Right = 2,
}

impl Lane {
    pub fn x(&self) -> f32 {
        LANE_POSITIONS[*self as usize]
    }

    pub fn left(&self) -> Option<Lane> {
        match self {
            Lane::Left => None,
            Lane::Center => Some(Lane::Left),
            Lane::Right => Some(Lane::Center),
        }
    }

    pub fn right(&self) -> Option<Lane> {
        match self {
            Lane::Left => Some(Lane::Center),
            Lane::Center => Some(Lane::Right),
            Lane::Right => None,
        }
    }
}

impl Default for Lane {
    fn default() -> Self {
        Lane::Center
    }
}
