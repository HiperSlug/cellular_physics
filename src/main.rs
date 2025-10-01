mod cell;
mod chunk;
mod chunk_map;

use bevy::prelude::*;
use enum_map::{Enum, EnumMap};

const OFFSETS: EnumMap<Dir, IVec2> = EnumMap::from_array([
    ivec2(-1, 0),  // left
    ivec2(1, 0),   // right
    ivec2(-1, -1), // down_left
    ivec2(0, -1),  // down
    ivec2(1, -1),  // down_right
    ivec2(-1, 1),  // up_left
    ivec2(0, 1),   // up
    ivec2(1, 1),   // up_right
]);

#[derive(Enum, Clone, Copy)]
pub enum Dir {
    Left,
    Right,
    DownLeft,
    Down,
    DownRight,
    UpLeft,
    Up,
    UpRight,
}

impl Dir {
    fn inverse(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::DownLeft => Self::UpRight,
            Self::UpRight => Self::DownLeft,
            Self::DownRight => Self::UpLeft,
            Self::UpLeft => Self::DownRight,
        }
    }
}

fn main() {}
