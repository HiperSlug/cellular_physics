use bevy::{math::I8Vec2, prelude::*};
use ndshape::{ConstPow2Shape2u32, ConstShape};
use nonmax::{NonMaxI8, NonMaxU8};
use std::{
    mem::ManuallyDrop,
    num::{NonZero, NonZeroI8, NonZeroU8},
    sync::atomic::{AtomicU8, Ordering},
};

fn main() {}

const MAX_SPEED: i8 = 3;
const MIN_SPEED: i8 = -3;

const SPLAT_MAX_SPEED: I8Vec2 = I8Vec2::splat(MAX_SPEED);
const SPLAT_MIN_SPEED: I8Vec2 = I8Vec2::splat(MIN_SPEED);

const MASK_3: u8 = 0b111;

const X_SHIFT: u32 = 0;
const X_MASK: u8 = MASK_3 << X_SHIFT;

const Y_SHIFT: u32 = 3;
const Y_MASK: u8 = MASK_3 << Y_SHIFT;

/// sign extension shift
const I3_TO_I8_SHIFT: u32 = u8::BITS - 3;
/// bit mask for lower 3 bits of an i8 casted as a u8
const I8_TO_I3_MASK: u8 = MASK_3;

const MASS_SHIFT: u32 = 6;
const RESTITUTION_SHIFT: u32 = 4;

/// -4 reprented as an i3
const I3_NEG_4: u8 = 0x04;
const INVALID_X: u8 = I3_NEG_4;
const INVALID_Y: u8 = I3_NEG_4 << Y_SHIFT;

const INV_NONE_LOWEST_Y_BIT: u8 = (!INVALID_Y) & (1 << Y_SHIFT);

/// Y bit pattern marking NONE
const NONE_BIT_PATTERN: u8 = INVALID_Y;
/// X bit pattern marking STATIC
const STATIC_X_BIT_PATTERN: u8 = INVALID_X;
/// STATIC bit pattern that also prevents NONE bit pattern
const SOME_STATIC_BIT_PATTERN: u8 = STATIC_X_BIT_PATTERN | INV_NONE_LOWEST_Y_BIT;

struct AtomicPackedCell(AtomicU8);

impl AtomicPackedCell {
    fn store(&self, val: PackedCell, order: Ordering) {
        self.0.store(val.0, order);
    }

    fn update(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: impl FnMut(PackedCell) -> PackedCell,
    ) {
        let _ = self
            .0
            .fetch_update(set_order, fetch_order, |inner| Some(f(PackedCell(inner)).0));
    }

    fn swap(&self, val: PackedCell, order: Ordering) -> PackedCell {
        PackedCell(self.0.swap(val.0, order))
    }
}

#[derive(Clone, Copy)]
struct PackedCell(u8);

impl PackedCell {
    const NONE: Self = Self(NONE_BIT_PATTERN);

    fn pack(cell_opt: Option<Cell>) -> Self {
        if let Some(cell) = cell_opt {
            match cell {
                Cell::Dynamic { mass, velocity } => {
                    debug_assert!(velocity.clamp(SPLAT_MIN_SPEED, SPLAT_MAX_SPEED) == velocity);
                    debug_assert!(mass.clamp(1, 4) == mass);

                    let mass = (mass - 1) << MASS_SHIFT;
                    let y = (velocity.y as u8 & I8_TO_I3_MASK) << Y_SHIFT;
                    let x = (velocity.x as u8 & I8_TO_I3_MASK) << X_SHIFT;

                    Self(mass | y | x)
                }
                Cell::Static { restitution } => {
                    debug_assert!(restitution.clamp(0, 15) == restitution);

                    let restitution = (restitution as u8) << RESTITUTION_SHIFT;

                    Self(restitution | SOME_STATIC_BIT_PATTERN)
                }
            }
        } else {
            Self::NONE
        }
    }

    fn unpack(self) -> Option<Cell> {
        self.is_some().then(|| {
            if self.is_dynamic() {
                Cell::Dynamic {
                    mass: self.mass(),
                    velocity: self.velocity(),
                }
            } else {
                Cell::Static {
                    restitution: self.restitution(),
                }
            }
        })
    }

    fn is_some(self) -> bool {
        self.0 != NONE_BIT_PATTERN
    }

    fn is_dynamic(self) -> bool {
        self.0 & X_MASK != STATIC_X_BIT_PATTERN
    }

    fn velocity(self) -> I8Vec2 {
        let x = (((self.0 & X_MASK) << (I3_TO_I8_SHIFT - X_SHIFT)) as i8) >> I3_TO_I8_SHIFT;
        let y = (((self.0 & Y_MASK) << (I3_TO_I8_SHIFT - Y_SHIFT)) as i8) >> I3_TO_I8_SHIFT;
        I8Vec2::new(x, y)
    }

    fn mass(self) -> u8 {
        (self.0 >> MASS_SHIFT) + 1
    }

    fn restitution(self) -> i8 {
        (self.0 >> RESTITUTION_SHIFT) as i8
    }
}

enum Cell {
    Static { restitution: i8 },
    Dynamic { mass: u8, velocity: I8Vec2 },
}

union MaybeAtomicPackedCell {
    plain: PackedCell,
    atomic: ManuallyDrop<AtomicPackedCell>,
}

const BITS: u32 = 6;
const LEN: usize = 1 << BITS;
const AREA: usize = LEN * LEN;
type Shape = ConstPow2Shape2u32<BITS, BITS>;

const ADJ_OFFSETS: [IVec2; 8] = [
    IVec2::new(1, 0),
    IVec2::new(0, 1),
    IVec2::new(-1, 0),
    IVec2::new(0, -1),
    IVec2::new(1, 1),
    IVec2::new(-1, -1),
    IVec2::new(-1, 1),
    IVec2::new(1, -1),
];

fn pos_is_edge(pos: UVec2) -> bool {
    pos.cmpeq(UVec2::ZERO).any() || pos.cmpeq(UVec2::splat(LEN as u32 - 1)).any()
}

struct SubStepGrid {
    write: [MaybeAtomicPackedCell; AREA],
    read: [PackedCell; AREA],
}

impl SubStepGrid {
    fn push_writes(&mut self) {
        for (i, (read_cell, write_cell)) in self.read.iter_mut().zip(&mut self.write).enumerate() {
            let pos = UVec2::from(Shape::delinearize(i as u32));
            if pos_is_edge(pos) {
                let write_cell = unsafe {
                    &write_cell.atomic
                };
                *read_cell = write_cell.swap(PackedCell::NONE, Ordering::Acquire);
            } else {
                let write_cell = unsafe {
                    &mut write_cell.plain
                };
                *read_cell = *write_cell;
                *write_cell = PackedCell::NONE;
            }
        }
    }

    // n 0..3
    fn sub_step(&self, n: u8) {
        // TODO we cannot iterate over every cell unless we have padding or neighbor access.
        for (i, cell) in self.read.iter().enumerate() {
            let Some(mut vel) = cell.velocity() else {
                continue;
            };
            let mass = cell.mass();
            let pos = UVec2::from(Shape::delinearize(i as u32));

            for offset in ADJ_OFFSETS {
                let adj_pos = (pos.as_ivec2() + offset).as_uvec2();
                let adj_i = Shape::linearize(adj_pos.into()) as usize;
                let adj_cell = self.read[adj_i];

                let Some(adj_vel) = adj_cell.velocity() else {
                    continue;
                };

                // TODO finalize consistent spreading out of velocity step
                // cannot % 0 so deal with that if we use %
                let adj_sub_step_delta = (adj_vel % n as i8).signum();
                let adj_dst = (adj_pos.as_ivec2() + adj_sub_step_delta.as_ivec2()).as_uvec2();

                if adj_dst == pos {
                    let adj_mass = adj_cell.mass();
                    if offset.x != 0 {
                        vel.x = dynamic_collision(mass, vel.x, adj_mass, adj_vel.x);
                    }
                    if offset.y != 0 {
                        vel.y = dynamic_collision(mass, vel.y, adj_mass, adj_vel.y);
                    }
                }
            }

            // TODO (above)
            let sub_step_delta = (vel % n as i8).signum();
            let dst = (pos.as_ivec2() + sub_step_delta.as_ivec2()).as_uvec2();
            let dst_i = Shape::linearize(dst.into()) as usize;
            let dst_cell = self.read[dst_i];

            if dst_cell.is_some() {
                let dst_mass = dst_cell.mass();

                if dst_cell.is_dynamic() {
                    let dst_vel = dst_cell.velocity_unchecked();
                    if dst.x != 0 {
                        vel.x = dynamic_collision(mass, vel.x, dst_mass, dst_vel.x);
                    }
                    if dst.y != 0 {
                        vel.y = dynamic_collision(mass, vel.y, dst_mass, dst_vel.y);
                    }
                } else {
                    if dst.x != 0 {
                        vel.x = static_collision(vel.x);
                    }
                    if dst.y != 0 {
                        vel.y = static_collision(vel.y);
                    }
                }

                self.write[i].store(cell.with_velocity(vel), Ordering::Relaxed);
            } else {
                self.write[dst_i].update(Ordering::Release, Ordering::Acquire, |dst_cell| {
                    if dst_cell.is_some() {
                        debug_assert!(dst_cell.is_dynamic());

                        let mut dst_vel = dst_cell.velocity_unchecked();
                        let dst_mass = dst_cell.mass();

                        if dst.x != 0 {
                            vel.x = dynamic_collision(mass, vel.x, dst_mass, dst_vel.x);
                            dst_vel.x = dynamic_collision(dst_mass, dst_vel.x, mass, vel.x);
                        }
                        if dst.y != 0 {
                            vel.y = dynamic_collision(mass, vel.y, dst_mass, dst_vel.y);
                            dst_vel.y = dynamic_collision(dst_mass, dst_vel.y, mass, vel.y);
                        }

                        self.write[i].store(cell.with_velocity(vel), Ordering::Relaxed);

                        dst_cell.with_velocity(dst_vel)
                    } else {
                        cell.with_velocity(vel)
                    }
                });
            }
        }
    }
}

fn dynamic_collision(m1: u8, v1: i8, m2: u8, v2: i8) -> i8 {
    let m1 = m1 as i8;
    let m2 = m2 as i8;

    // AI
    let v1_new = ((m1 - m2) * v1 + 2 * m2 * v2) / (m1 + m2);
    v1_new.clamp(MIN_SPEED, MAX_SPEED)
}

fn static_collision(v: i8) -> i8 {
    -v * 4 / 5
}
