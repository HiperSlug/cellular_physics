use bevy::{math::I8Vec2, prelude::*};
use std::sync::atomic::{AtomicU8, Ordering};

const MAX_SPEED: i8 = 3;

const MAX_VELOCITY: I8Vec2 = I8Vec2::splat(MAX_SPEED);
const MIN_VELOCITY: I8Vec2 = I8Vec2::splat(-MAX_SPEED);

const MASK_3: u8 = 0b111;

const X_SHIFT: u32 = 0;
const X_MASK: u8 = MASK_3 << X_SHIFT;

const Y_SHIFT: u32 = 3;
const Y_MASK: u8 = MASK_3 << Y_SHIFT;

/// sign extension shift
const I3_TO_I8_SHIFT: u32 = u8::BITS - 3;
/// bit mask for relevent i8 bits
const I8_TO_I3_MASK: u8 = MASK_3;

const MASS_SHIFT: u32 = 6;
const RESTITUTION_SHIFT: u32 = 4;

const MAX_RESTITUTION: i8 = (1 << (u8::BITS - RESTITUTION_SHIFT)) - 1;

/// -4 reprented as an i3
const I3_NEG_4: u8 = 0x04;
const INVALID_X: u8 = I3_NEG_4 << X_SHIFT;
const INVALID_Y: u8 = I3_NEG_4 << Y_SHIFT;

const INV_NONE_LOWEST_Y_BIT: u8 = (!INVALID_Y) & (1 << Y_SHIFT);

/// Y bit pattern marking NONE
const NONE_BIT_PATTERN: u8 = INVALID_Y;
/// X bit pattern marking STATIC
const STATIC_X_BIT_PATTERN: u8 = INVALID_X;
/// STATIC bit pattern that also uses a set bottom Y bit to ensure SOMEness.
const SOME_STATIC_BIT_PATTERN: u8 = STATIC_X_BIT_PATTERN | INV_NONE_LOWEST_Y_BIT;

#[derive(Clone, Copy)]
pub struct PackedCell(u8);

impl PackedCell {
    pub const NONE: Self = Self(NONE_BIT_PATTERN);

    pub fn pack(cell_opt: Option<Cell>) -> Self {
        if let Some(cell) = cell_opt {
            cell.pack()
        } else {
            Self::NONE
        }
    }

    pub fn unpack(self) -> Option<Cell> {
        self.is_some().then(|| {
            if self.is_dynamic() {
                Cell::Dynamic(DynamicCell {
                    mass: self.mass(),
                    velocity: self.velocity(),
                })
            } else {
                Cell::Static(StaticCell {
                    restitution: self.restitution(),
                })
            }
        })
    }

    pub fn is_some(self) -> bool {
        self.0 != NONE_BIT_PATTERN
    }

    pub fn is_dynamic(self) -> bool {
        self.0 & X_MASK != STATIC_X_BIT_PATTERN
    }

    fn velocity(self) -> I8Vec2 {
        let x = (((self.0 & X_MASK) << (I3_TO_I8_SHIFT - X_SHIFT)) as i8) >> I3_TO_I8_SHIFT;
        let y = (((self.0 & Y_MASK) << (I3_TO_I8_SHIFT - Y_SHIFT)) as i8) >> I3_TO_I8_SHIFT;
        I8Vec2::new(x, y)
    }

    fn mass(self) -> i8 {
        (self.0 >> MASS_SHIFT) as i8 + 1
    }

    fn restitution(self) -> i8 {
        (self.0 >> RESTITUTION_SHIFT) as i8
    }
}

pub enum Cell {
    Static(StaticCell),
    Dynamic(DynamicCell),
}

impl Cell {
    pub fn pack(self) -> PackedCell {
        match self {
            Cell::Dynamic(c) => c.pack(),
            Cell::Static(c) => c.pack(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct StaticCell {
    pub restitution: i8,
}

impl StaticCell {
    pub fn pack(self) -> PackedCell {
        debug_assert!((0..16).contains(&self.restitution));

        let restitution = (self.restitution as u8) << RESTITUTION_SHIFT;

        PackedCell(restitution | SOME_STATIC_BIT_PATTERN)
    }
}

#[derive(Clone, Copy)]
pub struct DynamicCell {
    pub mass: i8,
    pub velocity: I8Vec2,
}

impl DynamicCell {
    pub fn pack(self) -> PackedCell {
        debug_assert!(
            self.velocity.cmpge(MIN_VELOCITY).all() && self.velocity.cmple(MAX_VELOCITY).all()
        );
        debug_assert!((1..5).contains(&self.mass));

        let mass = (self.mass as u8 - 1) << MASS_SHIFT;
        let y = (self.velocity.y as u8 & I8_TO_I3_MASK) << Y_SHIFT;
        let x = (self.velocity.x as u8 & I8_TO_I3_MASK) << X_SHIFT;

        PackedCell(mass | y | x)
    }

    pub fn sub_step_delta(&self, n: u8) -> IVec2 {
        debug_assert!((n as i8) < MAX_SPEED);
        self.velocity
            .map(|x| {
                let remaining = x.abs() - n as i8;
                if remaining > 0 { x.signum() } else { 0 }
            })
            .as_ivec2()
    }

    pub fn dynamic_collision_x(&mut self, other: &DynamicCell) {
        self.velocity.x =
            dynamic_collision(self.velocity.x, self.mass, other.velocity.x, other.mass)
                .clamp(-MAX_SPEED, MAX_SPEED);
    }

    pub fn dynamic_collision_y(&mut self, other: &DynamicCell) {
        self.velocity.y =
            dynamic_collision(self.velocity.y, self.mass, other.velocity.y, other.mass)
                .clamp(-MAX_SPEED, MAX_SPEED);
    }

    pub fn static_collision_x(&mut self, other: &StaticCell) {
        self.velocity.x = static_collision(self.velocity.x, other.restitution);
    }

    pub fn static_collision_y(&mut self, other: &StaticCell) {
        self.velocity.y = static_collision(self.velocity.y, other.restitution);
    }
}

fn dynamic_collision(v1: i8, m1: i8, v2: i8, m2: i8) -> i8 {
    ((m1 - m2) * v1 + 2 * m2 * v2) / (m1 + m2)
}

fn static_collision(v: i8, r: i8) -> i8 {
    v * r / MAX_RESTITUTION
}

pub struct AtomicPackedCell(AtomicU8);

impl AtomicPackedCell {
    pub fn store(&self, val: PackedCell, order: Ordering) {
        self.0.store(val.0, order);
    }

    pub fn update(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: impl FnMut(PackedCell) -> Option<PackedCell>,
    ) -> Result<PackedCell, PackedCell> {
        self.0
            .fetch_update(set_order, fetch_order, |inner| {
                f(PackedCell(inner)).map(|p| p.0)
            })
            .map(|c| PackedCell(c))
            .map_err(|c| PackedCell(c))
    }
}
