use bevy::prelude::*;
use ndshape::{ConstPow2Shape2u32, ConstShape};
use std::{ptr::NonNull, sync::atomic::Ordering};

use crate::cell::{Cell, MaybeAtomicPackedCell, PackedCell};

const BITS: u32 = 6;

pub const LEN: u32 = 1 << BITS;
pub const AREA: usize = LEN.pow(2) as usize;

pub type Shape = ConstPow2Shape2u32<BITS, BITS>;

const OFFSETS: [IVec2; 8] = [
    ivec2(-1, 0),  // left
    ivec2(1, 0),   // right
    ivec2(-1, -1), // down_left
    ivec2(0, -1),  // down_middle
    ivec2(1, -1),  // down_right
    ivec2(-1, 1),  // up_left
    ivec2(0, 1),   // up_middle
    ivec2(1, 1),   // up_right
];

fn is_edge(pos: UVec2) -> bool {
    pos.cmpeq(uvec2(0, 0)).any() || pos.cmpeq(uvec2(LEN - 1, LEN - 1)).any()
}

use Bounds::*;
enum Bounds {
    Inside,
    Greater,
    Less,
}

fn bounds(pos: IVec2) -> [Bounds; 2] {
    pos.to_array().map(|x| {
        if x >= LEN as i32 {
            Greater
        } else if x < 0 {
            Less
        } else {
            Inside
        }
    })
}

struct Chunk {
    read: [PackedCell; AREA],
    write: [MaybeAtomicPackedCell; AREA],
    neighbors: [Option<NonNull<Chunk>>; 8],
}

impl Chunk {
    fn push_writes(&mut self) {
        for (read, write) in self.read.iter_mut().zip(self.write.iter()) {
            // doesnt need to be atomic b/c this occurs after all concurrent operations
            *read = unsafe { write.plain };
        }
    }

    fn sub_step(&mut self, n: u8) {
        // neighbor access is compeltely local
        for i in (0..AREA).filter(|i| !is_edge(Shape::delinearize((*i) as u32).into())) {
            let Some(Cell::Dynamic(mut cell)) = self.read[i].unpack() else {
                continue;
            };
            let pos: UVec2 = Shape::delinearize(i as u32).into();

            // pull adj collisions
            for offset in OFFSETS {
                let adj_pos = (pos.as_ivec2() + offset).as_uvec2();
                let adj_i = Shape::linearize(adj_pos.into()) as usize;
                let Some(Cell::Dynamic(adj_cell)) = self.read[adj_i].unpack() else {
                    continue;
                };

                let dst = (adj_pos.as_ivec2() + adj_cell.sub_step_delta(n)).as_uvec2();

                if dst == pos {
                    if offset.x != 0 {
                        cell.dynamic_collision_x(&adj_cell);
                    }
                    if offset.y != 0 {
                        cell.dynamic_collision_y(&adj_cell);
                    }
                }
            }

            let sub_step_delta = cell.sub_step_delta(n);
            let dst = (pos.as_ivec2() + sub_step_delta).as_uvec2();
            let dst_i = Shape::linearize(dst.into()) as usize;

            if let Some(dst_cell) = self.read[dst_i].unpack() {
                // if our destination is blocked handle collision but dont move
                match dst_cell {
                    Cell::Dynamic(dst_cell) => {
                        if sub_step_delta.x != 0 {
                            cell.dynamic_collision_x(&dst_cell);
                        }
                        if sub_step_delta.y != 0 {
                            cell.dynamic_collision_y(&dst_cell);
                        }
                    }
                    Cell::Static(dst_cell) => {
                        if sub_step_delta.x != 0 {
                            cell.static_collision_x(&dst_cell);
                        }
                        if sub_step_delta.y != 0 {
                            cell.static_collision_y(&dst_cell);
                        }
                    }
                }

                // in this iteration all `cells` are plain (not neccecarily adj/dst cells though)
                unsafe { self.write[i].plain = cell.pack() };
            } else {
                if is_edge(dst) {
                    unsafe { &self.write[dst_i].atomic.fetch_update(Ordering::AcqRel, Ordering::Acquire, |dst_cell| {
                            if let Some(dst_cell) = dst_cell.unpack() {
                                // if there was a collision (multiple cells tried to move to the same cell this sub_step) then change the velocities of both cells and dont move
                                if let Cell::Dynamic(mut dst_cell) = dst_cell {
                                    if sub_step_delta.x != 0 {
                                        cell.dynamic_collision_x(&dst_cell);
                                        dst_cell.dynamic_collision_x(&cell);
                                    }
                                    if sub_step_delta.y != 0 {
                                        cell.dynamic_collision_y(&dst_cell);
                                        dst_cell.dynamic_collision_y(&cell);
                                    }

                                    unsafe { self.write[i].plain = cell.pack() }; // borrow problems!

                                    Some(dst_cell.pack())
                                } else {
                                    // a non dynamic cell could not have moved positions
                                    unreachable!()
                                }
                            } else {
                                // move this cell to the new location

                                unsafe { self.write[i].plain = cell.pack() };

                                Some(cell.pack())
                            }
                        });
                    }
                } else {
                    let dst_cell = unsafe { &mut self.write[dst_i].plain };
                    *dst_cell = if let Some(dst_cell) = dst_cell.unpack() {
                        // if there was a collision (multiple cells tried to move to the same cell this sub_step) then change the velocities of both cells and dont move
                        if let Cell::Dynamic(mut dst_cell) = dst_cell {
                            if sub_step_delta.x != 0 {
                                cell.dynamic_collision_x(&dst_cell);
                                dst_cell.dynamic_collision_x(&cell);
                            }
                            if sub_step_delta.y != 0 {
                                cell.dynamic_collision_y(&dst_cell);
                                dst_cell.dynamic_collision_y(&cell);
                            }

                            unsafe { self.write[i].plain = cell.pack() };

                            dst_cell.pack()
                        } else {
                            // a non dynamic cell could not have moved positions
                            unreachable!()
                        }
                    } else {
                        // move this cell to the new location

                        unsafe { self.write[i].plain = cell.pack() };

                        cell.pack()
                    }
                }
            }
        }
        // neighbor access
        for i in (0..AREA).filter(|i| is_edge(Shape::delinearize((*i) as u32).into())) {
            let Some(Cell::Dynamic(mut cell)) = self.read[i].unpack() else {
                continue;
            };
            let pos: IVec2 = UVec2::from(Shape::delinearize(i as u32)).as_ivec2();

            // pull adj collisions
            for offset in OFFSETS {
                let adj_pos = pos + offset;
                let wrapped_adj_pos: UVec2 =
                    adj_pos.to_array().map(|i32| (i32 as u32) & LEN).into();
                let adj_i = Shape::linearize(wrapped_adj_pos.into()) as usize;
                let bounds = bounds(adj_pos);

                let Some(Cell::Dynamic(adj_cell)) = (match bounds {
                    [Inside, Inside] => self.read[adj_i].unpack(),
                    [Greater, Inside] => unsafe {
                        self.neighbors[0].and_then(|ptr| (*ptr.as_ptr()).read[adj_i].unpack())
                    },
                    [Less, Inside] => unsafe {
                        self.neighbors[1].and_then(|ptr| (*ptr.as_ptr()).read[adj_i].unpack())
                    },
                    [Inside, Greater] => unsafe {
                        self.neighbors[2].and_then(|ptr| (*ptr.as_ptr()).read[adj_i].unpack())
                    },
                    [Inside, Less] => unsafe {
                        self.neighbors[3].and_then(|ptr| (*ptr.as_ptr()).read[adj_i].unpack())
                    },
                    [Less, Less] => unsafe {
                        self.neighbors[4].and_then(|ptr| (*ptr.as_ptr()).read[adj_i].unpack())
                    },
                    [Less, Greater] => unsafe {
                        self.neighbors[5].and_then(|ptr| (*ptr.as_ptr()).read[adj_i].unpack())
                    },
                    [Greater, Less] => unsafe {
                        self.neighbors[6].and_then(|ptr| (*ptr.as_ptr()).read[adj_i].unpack())
                    },
                    [Greater, Greater] => unsafe {
                        self.neighbors[7].and_then(|ptr| (*ptr.as_ptr()).read[adj_i].unpack())
                    },
                }) else {
                    continue;
                };

                let dst = adj_pos + adj_cell.sub_step_delta(n);

                if dst == pos {
                    if offset.x != 0 {
                        cell.dynamic_collision_x(&adj_cell);
                    }
                    if offset.y != 0 {
                        cell.dynamic_collision_y(&adj_cell);
                    }
                }
            }
        }

        // for (i, mut cell) in self.read.iter().copied().enumerate() {
        //     let pos = UVec2::from(Shape::delinearize(i as u32)).as_ivec2();

        //     for offset in ADJ_OFFSETS {
        //         let adj_pos = (pos.as_ivec2() + offset).as_uvec2();
        //         let adj_i = Shape::linearize(adj_pos.into()) as usize;
        //         let Some(Cell::Dynamic(adj_cell)) = self.read[adj_i].unpack() else {
        //             continue;
        //         };

        //         let adj_dst = (adj_pos.as_ivec2() + adj_cell.sub_step_delta(n)).as_uvec2();

        //         if adj_dst == pos {
        //             if offset.x != 0 {
        //                 cell.dynamic_collision_x(&adj_cell);
        //             }
        //             if offset.y != 0 {
        //                 cell.dynamic_collision_y(&adj_cell);
        //             }
        //         }
        //     }

        //     let sub_step_delta = cell.sub_step_delta(n);
        //     let dst = (pos.as_ivec2() + sub_step_delta).as_uvec2();
        //     let dst_i = Shape::linearize(dst.into()) as usize;
        //     let dst_cell_opt = self.read[dst_i].unpack();

        //     if let Some(dst_cell) = dst_cell_opt {
        //         match dst_cell {
        //             Cell::Dynamic(dst_cell) => {
        //                 if sub_step_delta.x != 0 {
        //                     cell.dynamic_collision_x(&dst_cell);
        //                 }
        //                 if sub_step_delta.y != 0 {
        //                     cell.dynamic_collision_y(&dst_cell);
        //                 }
        //             }
        //             Cell::Static(dst_cell) => {
        //                 if sub_step_delta.x != 0 {
        //                     cell.static_collision_x(&dst_cell);
        //                 }
        //                 if sub_step_delta.y != 0 {
        //                     cell.static_collision_y(&dst_cell);
        //                 }
        //             }
        //         }

        //         self.write[i].store(Cell::Dynamic(cell).pack(), Ordering::Relaxed);
        //     } else {
        //         self.write[dst_i].update(Ordering::Release, Ordering::Acquire, |dst_cell| {
        //             if let Some(dst_cell) = dst_cell.unpack() {
        //                 if let Cell::Dynamic(mut dst_cell) = dst_cell {
        //                     if sub_step_delta.x != 0 {
        //                         cell.dynamic_collision_x(&dst_cell);
        //                         dst_cell.dynamic_collision_x(&cell);
        //                     }
        //                     if sub_step_delta.y != 0 {
        //                         cell.dynamic_collision_y(&dst_cell);
        //                         dst_cell.dynamic_collision_y(&cell);
        //                     }

        //                     self.write[i].store(Cell::Dynamic(cell).pack(), Ordering::Relaxed);

        //                     Cell::Dynamic(dst_cell).pack()
        //                 } else {
        //                     unreachable!()
        //                 }
        //             } else {
        //                 Cell::Dynamic(cell).pack()
        //             }
        //         });
        //     }
        // }
    }
}
