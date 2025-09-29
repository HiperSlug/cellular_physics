use bevy::{math::I8Vec2, prelude::*};
use ndshape::{ConstPow2Shape2u32, ConstShape};

fn main() {}

const MAX_SPEED: i8 = 3;

const X_MASK: u8 = (1 << 3) - 1;
const X_SHL: u32 = 5;

const Y_MASK: u8 = ((1 << 3) - 1) << 3;
const Y_SHL: u32 = 2;

const VEL_SHR: u32 = 5;

const MASS_SHIFT: u32 = 6;

const I3_NEG_4: u8 = 0x04;
const NONE_X_VALUE: u8 = I3_NEG_4;
const STATIC_Y_VALUE: u8 = I3_NEG_4 << 3;

#[derive(Clone, Copy)]
struct Cell(u8);

impl Cell {
    fn is_some(&self) -> bool {
        self.0 & X_MASK != NONE_X_VALUE
    }

    fn is_dynamic(&self) -> bool {
        self.0 & Y_MASK != STATIC_Y_VALUE
    }

    fn velocity(&self) -> I8Vec2 {
        let x = (((self.0 & X_MASK) << X_SHL) as i8) >> VEL_SHR;
        let y = (((self.0 & Y_MASK) << Y_SHL) as i8) >> VEL_SHR;
        I8Vec2::new(x, y)
    }

    fn mass(&self) -> u8 {
        (self.0 >> MASS_SHIFT) + 1
    }
}

const BITS: u32 = 6;
const LEN: usize = 1 << BITS;
const AREA: usize = LEN * LEN;
type Shape = ConstPow2Shape2u32<BITS, BITS>;

// struct SubStepGrid {
//     write: [Mutex<Cell>; AREA],
//     read: [Cell; AREA],
// }

// impl SubStepGrid {
//     fn sync_buffers(&mut self) {
//         for (write, read) in self.write.iter().zip(&mut self.read) {
//             *read = *write.lock().unwrap();
//         }
//     }

//     fn sub_step(&self, n: usize) {
//         for (i, cell) in self.read.iter().enumerate() {
//             let velocity = cell.velocity;
//             let pos = UVec2::from(Shape::delinearize(i as u32));

//             for x in [-1, 1] {
//                 for y in [-1, 1] {
//                     let adj_i = Shape::linearize((pos.as_ivec2() + IVec2::new(x, y)).as_uvec2().into()) as usize;
//                     let adj = &self.read[adj_i];
//                     if adj.velocity.
//                 }
//             }
//         }
//     }
// }

// fn physics_step(&mut self) {
//     self.double_buffer();

//     for (i, cell) in self.last_cells.iter().enumerate() {
//         let mut velocity = cell.velocity;
//         let mut pos = UVec2::from(Shape::delinearize(i as u32));

//         let steps = cell.velocity.x.abs().max(cell.velocity.y.abs());

//         fn collision_check(last_cells: &[Cell; AREA], pos: UVec2) -> Option<u8> {
//             let i = Shape::linearize(pos.into()) as usize;
//             let cell = last_cells[i];
//             cell.is_some.then_some(cell.mass)
//         }

//         for step_by in 0..steps {
//             if velocity.x.abs() > step_by {
//                 let mut next_pos = pos;
//                 next_pos.x = ((pos.x as i32) + velocity.x.signum() as i32) as u32;

//                 if let Some(mass) = collision_check(&self.last_cells, next_pos) {
//                     // TODO use the mass? or drop mass entierly?
//                     velocity.x = -velocity.x.signum() * (velocity.x.abs() - 1)
//                 } else {
//                     pos = next_pos
//                 }
//             }
//             if cell.velocity.y.abs() > step_by {
//                 let mut next_pos = pos;
//                 next_pos.y = ((pos.y as i32) + velocity.y.signum() as i32) as u32;

//                 if let Some(mass) = collision_check(&self.last_cells, next_pos) {
//                     // TODO use the mass? or drop mass entierly?
//                     velocity.y = -velocity.x.signum() * (velocity.x.abs() - 1)
//                 } else {
//                     pos = next_pos
//                 }
//             }
//         }

//         let target_pos = (pos.as_ivec2() + cell.velocity.as_ivec2()).as_uvec2();
//         let target_cell = self.cells[Shape::linearize(target_pos.into()) as usize].lock().unwrap();
//         if target_cell.is_some {

//         }
//     }
// }
