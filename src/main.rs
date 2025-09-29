use bevy::{math::I8Vec2, prelude::*};
use ndshape::{ConstPow2Shape2u32, ConstShape};
use std::sync::atomic::{AtomicU8, Ordering};

fn main() {}

const MAX_SPEED: I8Vec2 = I8Vec2::splat(3);
const MIN_SPEED: I8Vec2 = I8Vec2::splat(-3);

const MASK_3: u8 = 1 << 3;

const X_SHIFT: u32 = 0;
const X_MASK: u8 = MASK_3 << X_SHIFT;

const Y_SHIFT: u32 = 3;
const Y_MASK: u8 = MASK_3 << Y_SHIFT;

const I3_TO_I8_SHIFT: u32 = 5;
const I8_TO_I3_MASK: u8 = MASK_3;

const MASS_SHIFT: u32 = 6;

const I3_NEG_4: u8 = 0x04;
const INVALID_X: u8 = I3_NEG_4;
const INVALID_Y: u8 = I3_NEG_4 << Y_SHIFT;

struct AtomicCell(AtomicU8);

impl AtomicCell {
    fn store(&self, val: Cell, order: Ordering) {
        self.0.store(val.0, order);
    }

    fn update(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: impl FnMut(Cell) -> Cell,
    ) {
        let _ = self.0.fetch_update(set_order, fetch_order, |inner| {
            Some(f(Cell(inner)).0)
        });
    }

    fn swap(&self, val: Cell, order: Ordering) -> Cell {
        Cell(self.0.swap(val.0, order))
    }
}

#[derive(Clone, Copy)]
struct Cell(u8);

impl Cell {
    const NONE: Cell = Cell(INVALID_X);

    fn is_some(self) -> bool {
        self.0 & X_MASK != INVALID_X
    }

    fn is_dynamic(self) -> bool {
        self.0 & Y_MASK != INVALID_Y
    }

    fn velocity_unchecked(self) -> I8Vec2 {
        let x = (((self.0 & X_MASK) << (I3_TO_I8_SHIFT - X_SHIFT)) as i8) >> I3_TO_I8_SHIFT;
        let y = (((self.0 & Y_MASK) << (I3_TO_I8_SHIFT - Y_SHIFT)) as i8) >> I3_TO_I8_SHIFT;
        I8Vec2::new(x, y)
    }

    fn velocity(self) -> Option<I8Vec2> {
        (self.is_some() & self.is_dynamic()).then_some(self.velocity_unchecked())
    }

    fn mass(self) -> u8 {
        (self.0 >> MASS_SHIFT) + 1
    }

    fn with_velocity(mut self, vel: I8Vec2) -> Self {
        let x = vel.x as u8 & I8_TO_I3_MASK;
        let y = vel.y as u8 & I8_TO_I3_MASK;
        self.0 &= !(X_MASK | Y_MASK);
        self.0 |= x << X_SHIFT | y << Y_SHIFT;
        self
    }
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

struct SubStepGrid {
    write: [AtomicCell; AREA],
    read: [Cell; AREA],
}

impl SubStepGrid {
    fn sync_buffers(&mut self) {
        for (read, write) in self.read.iter_mut().zip(&self.write) {
            *read = write.swap(Cell::NONE, Ordering::Relaxed);
        }
    }

    // n 0..3
    fn sub_step(&self, n: u8) {
        let skip = I8Vec2::splat(n as i8);

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

                let Some(mut adj_vel) = adj_cell.velocity() else {
                    continue;
                };

                let adj_sub_step_delta = (adj_vel - skip).signum(); // - skip doesnt work!!!
                let adj_dst = (adj_pos.as_ivec2() + adj_sub_step_delta.as_ivec2()).as_uvec2();

                if adj_dst == pos {
                    let adj_mass = adj_cell.mass();
                    collision((mass, &mut vel), (adj_mass, &mut adj_vel));
                }
            }

            let sub_step_delta = (vel - skip).signum(); // - skip doesnt work!!!
            let dst = (pos.as_ivec2() + sub_step_delta.as_ivec2()).as_uvec2();
            let dst_i = Shape::linearize(dst.into()) as usize;

            let dst_cell = self.read[dst_i];

            if dst_cell.is_some() {
                let mut dst_vel = dst_cell.velocity_unchecked();
                let dst_mass = dst_cell.mass();
                collision((mass, &mut vel), (dst_mass, &mut dst_vel));

                self.write[i].store(cell.with_velocity(vel), Ordering::Relaxed);
            } else {
                self.write[dst_i].update(Ordering::Acquire, Ordering::Release, |dst_cell| {
                    if dst_cell.is_some() {
                        let mut dst_vel = dst_cell.velocity_unchecked();
                        let dst_mass = dst_cell.mass();
                        collision((mass, &mut vel), (dst_mass, &mut dst_vel));
                        
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

// should be two way compatable
fn collision((mass_1, vel_1): (u8, &mut I8Vec2), (mass_2, vel_2): (u8, &mut I8Vec2)) {
    todo!()
}
