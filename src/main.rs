// use bevy::{math::I8Vec2, prelude::*};
// use ndshape::{ConstPow2Shape2u32, ConstShape};
// use std::{
//     mem::ManuallyDrop,
//     sync::atomic::{AtomicU8, Ordering},
// };

// fn main() {}

mod cell;

// const BITS: u32 = 6;
// const LEN: usize = 1 << BITS;
// const AREA: usize = LEN * LEN;
// type Shape = ConstPow2Shape2u32<BITS, BITS>;

// const ADJ_OFFSETS: [IVec2; 8] = [
//     IVec2::new(1, 0),
//     IVec2::new(0, 1),
//     IVec2::new(-1, 0),
//     IVec2::new(0, -1),
//     IVec2::new(1, 1),
//     IVec2::new(-1, -1),
//     IVec2::new(-1, 1),
//     IVec2::new(1, -1),
// ];

// struct SubStepGrid {
//     // currently always assumes atomic for simplicity
//     write: [AtomicPackedCell; AREA],
//     read: [PackedCell; AREA],
// }

// impl SubStepGrid {
//     fn push_writes(&mut self) {
//         for (read_cell, write_cell) in self.read.iter_mut().zip(&mut self.write) {
//             *read_cell = write_cell.swap(PackedCell::NONE, Ordering::Acquire);
//         }
//     }

//     // n 0..3
//     fn sub_step(&self, n: u8) {
//         // TODO we cannot iterate over every cell unless we have padding or neighbor access.
//         for (i, mut cell) in self
//             .read
//             .iter()
//             .enumerate()
//             .filter_map(|(i, p)| p.unpack().and_then(|c| c.dynamic_opt().map(|d| (i, d))))
//         {
//             let pos = UVec2::from(Shape::delinearize(i as u32));

//             for offset in ADJ_OFFSETS {
//                 let adj_pos = (pos.as_ivec2() + offset).as_uvec2();
//                 let adj_i = Shape::linearize(adj_pos.into()) as usize;
//                 let Some(Cell::Dynamic(adj_cell)) = self.read[adj_i].unpack() else {
//                     continue;
//                 };

//                 let adj_dst = (adj_pos.as_ivec2() + adj_cell.sub_step_delta(n)).as_uvec2();

//                 if adj_dst == pos {
//                     if offset.x != 0 {
//                         cell.dynamic_collision_x(&adj_cell);
//                     }
//                     if offset.y != 0 {
//                         cell.dynamic_collision_y(&adj_cell);
//                     }
//                 }
//             }

//             let sub_step_delta = cell.sub_step_delta(n);
//             let dst = (pos.as_ivec2() + sub_step_delta).as_uvec2();
//             let dst_i = Shape::linearize(dst.into()) as usize;
//             let dst_cell_opt = self.read[dst_i].unpack();

//             if let Some(dst_cell) = dst_cell_opt {
//                 match dst_cell {
//                     Cell::Dynamic(dst_cell) => {
//                         if sub_step_delta.x != 0 {
//                             cell.dynamic_collision_x(&dst_cell);
//                         }
//                         if sub_step_delta.y != 0 {
//                             cell.dynamic_collision_y(&dst_cell);
//                         }
//                     }
//                     Cell::Static(dst_cell) => {
//                         if sub_step_delta.x != 0 {
//                             cell.static_collision_x(&dst_cell);
//                         }
//                         if sub_step_delta.y != 0 {
//                             cell.static_collision_y(&dst_cell);
//                         }
//                     }
//                 }

//                 self.write[i].store(Cell::Dynamic(cell).pack(), Ordering::Relaxed);
//             } else {
//                 self.write[dst_i].update(Ordering::Release, Ordering::Acquire, |dst_cell| {
//                     if let Some(dst_cell) = dst_cell.unpack() {
//                         if let Cell::Dynamic(mut dst_cell) = dst_cell {
//                             if sub_step_delta.x != 0 {
//                                 cell.dynamic_collision_x(&dst_cell);
//                                 dst_cell.dynamic_collision_x(&cell);
//                             }
//                             if sub_step_delta.y != 0 {
//                                 cell.dynamic_collision_y(&dst_cell);
//                                 dst_cell.dynamic_collision_y(&cell);
//                             }

//                             self.write[i].store(Cell::Dynamic(cell).pack(), Ordering::Relaxed);

//                             Cell::Dynamic(dst_cell).pack()
//                         } else {
//                             unreachable!()
//                         }
//                     } else {
//                         Cell::Dynamic(cell).pack()
//                     }
//                 });
//             }
//         }
//     }
// }

// fn pos_is_edge(pos: UVec2) -> bool {
//     pos.cmpeq(UVec2::ZERO).any() || pos.cmpeq(UVec2::splat(LEN as u32 - 1)).any()
// }
