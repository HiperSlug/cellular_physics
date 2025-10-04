use bevy::{math::I8Vec2, prelude::*};
use enum_map::EnumMap;
use ndshape::{ConstPow2Shape2u32, ConstShape};
use rand::{Rng, rng};
use std::{mem::transmute, ptr::NonNull, sync::atomic::Ordering};

use crate::{
    Dir::{self, *},
    OFFSETS,
    cell::{Cell, DynamicCell, MaybeAtomicPackedCell, PackedCell, StaticCell},
};

const BITS: u32 = 6;
pub const LEN: i32 = 1 << BITS;
const AREA: usize = LEN.pow(2) as usize;

const MIN: i32 = 0;
const MAX: i32 = LEN - 1;

type Shape = ConstPow2Shape2u32<BITS, BITS>;

use Bounds::*;
enum Bounds {
    Within,
    Greater,
    Less,
}

pub struct Chunk {
    read: [PackedCell; AREA],
    /// Atomic when:
    /// 1. Parrallel neighbor access
    /// 2. If the cell `is_edge`
    /// 3. If the cell was `None` last `sub_step`
    write: [MaybeAtomicPackedCell; AREA],
    neighbors: EnumMap<Dir, Option<NonNull<Chunk>>>,
}

// Safety: only safe if used to parrallel execution of the same function on a chunk
unsafe impl Send for Chunk {}
unsafe impl Sync for Chunk {}

impl Chunk {
    pub const EMPTY: Self = Self {
        read: [PackedCell::NONE; AREA],
        write: unsafe { transmute([PackedCell::NONE; AREA]) },
        neighbors: EnumMap::from_array([None; 8]),
    };

    pub fn push_writes(&mut self) {
        for (read, write) in self.read.iter_mut().zip(self.write.iter()) {
            // Safety: Atomics are only nececcary when running functions on chunks that use its `neighbors`.
            *read = unsafe { write.plain };
        }
    }

    pub fn sub_step(&mut self, n: u8) {
        for i in 0..AREA {
            let Some(Cell::Dynamic(original_cell)) = self.read[i].unpack() else {
                continue;
            };
            let mut cell = original_cell;
            let pos = delinearize(i);

            // pull collisions
            for (_, offset) in OFFSETS {
                let adj_pos = pos + offset;
                let adj_i = wrapping_linearize(adj_pos);

                // Safety: `read` is shared
                let nn_read = |nn: NonNull<Chunk>| unsafe { (*nn.as_ptr()).read[adj_i].unpack() };

                let Some(Cell::Dynamic(adj_cell)) = (match bounds(adj_pos) {
                    [Within, Within] => self.read[adj_i].unpack(),
                    [Less, Within] => self.neighbors[Left].and_then(nn_read),
                    [Greater, Within] => self.neighbors[Right].and_then(nn_read),
                    [Within, Less] => self.neighbors[Down].and_then(nn_read),
                    [Within, Greater] => self.neighbors[Up].and_then(nn_read),
                    [Less, Less] => self.neighbors[DownLeft].and_then(nn_read),
                    [Greater, Less] => self.neighbors[DownRight].and_then(nn_read),
                    [Less, Greater] => self.neighbors[UpLeft].and_then(nn_read),
                    [Greater, Greater] => self.neighbors[UpRight].and_then(nn_read),
                }) else {
                    continue;
                };

                let sub_step_delta = adj_cell.sub_step_delta(n);

                if sub_step_delta.x != 0 && sub_step_delta.y != 0 {
                    let x_pos = adj_pos + ivec2(sub_step_delta.x, 0);
                    let x_i = wrapping_linearize(x_pos);

                    let nn_read = |nn: NonNull<Chunk>| unsafe { (*nn.as_ptr()).read[x_i].unpack() };

                    let x_cell = match bounds(x_pos) {
                        [Within, Within] => self.read[x_i].unpack(),
                        [Less, Within] => self.neighbors[Left].and_then(nn_read),
                        [Greater, Within] => self.neighbors[Right].and_then(nn_read),
                        [Within, Less] => self.neighbors[Down].and_then(nn_read),
                        [Within, Greater] => self.neighbors[Up].and_then(nn_read),
                        [Less, Less] => self.neighbors[DownLeft].and_then(nn_read),
                        [Greater, Less] => self.neighbors[DownRight].and_then(nn_read),
                        [Less, Greater] => self.neighbors[UpLeft].and_then(nn_read),
                        [Greater, Greater] => self.neighbors[UpRight].and_then(nn_read),
                    };

                    let y_pos = adj_pos + ivec2(0, sub_step_delta.y);
                    let y_i = wrapping_linearize(y_pos);

                    let nn_read = |nn: NonNull<Chunk>| unsafe { (*nn.as_ptr()).read[y_i].unpack() };
                    
                    let y_cell = match bounds(y_pos) {
                        [Within, Within] => self.read[y_i].unpack(),
                        [Less, Within] => self.neighbors[Left].and_then(nn_read),
                        [Greater, Within] => self.neighbors[Right].and_then(nn_read),
                        [Within, Less] => self.neighbors[Down].and_then(nn_read),
                        [Within, Greater] => self.neighbors[Up].and_then(nn_read),
                        [Less, Less] => self.neighbors[DownLeft].and_then(nn_read),
                        [Greater, Less] => self.neighbors[DownRight].and_then(nn_read),
                        [Less, Greater] => self.neighbors[UpLeft].and_then(nn_read),
                        [Greater, Greater] => self.neighbors[UpRight].and_then(nn_read),
                    };

                    if !x_cell.is_some() && !y_cell.is_some() {
                        cell.dynamic_collision(&adj_cell, sub_step_delta);
                    }
                } else {
                    let dst = pos + sub_step_delta;
                    if dst == pos {
                        cell.dynamic_collision(&adj_cell, sub_step_delta);
                    }
                }
            }

            // push collision
            let delta = cell.sub_step_delta(n);
            if delta == IVec2::ZERO {
                if cell != original_cell {
                    // `self.write[i].plain` can be freely written to b/c we know that no other threads will attempt to mutate a cell which was `Some` last frame
                    self.write[i].plain = cell.pack();
                }
                continue;
            }

            if delta.x != 0 && delta.y != 0 {
                let x_pos = pos + ivec2(delta.x, 0);
                let x_i = wrapping_linearize(x_pos);

                let nn_read = |nn: NonNull<Chunk>| unsafe { (*nn.as_ptr()).read[x_i].unpack() };

                let x_cell = match bounds(x_pos) {
                    [Within, Within] => self.read[x_i].unpack(),
                    [Less, Within] => self.neighbors[Left].and_then(nn_read),
                    [Greater, Within] => self.neighbors[Right].and_then(nn_read),
                    [Within, Less] => self.neighbors[Down].and_then(nn_read),
                    [Within, Greater] => self.neighbors[Up].and_then(nn_read),
                    [Less, Less] => self.neighbors[DownLeft].and_then(nn_read),
                    [Greater, Less] => self.neighbors[DownRight].and_then(nn_read),
                    [Less, Greater] => self.neighbors[UpLeft].and_then(nn_read),
                    [Greater, Greater] => self.neighbors[UpRight].and_then(nn_read),
                };

                let y_pos = pos + ivec2(0, delta.y);
                let y_i = wrapping_linearize(y_pos);

                let nn_read = |nn: NonNull<Chunk>| unsafe { (*nn.as_ptr()).read[y_i].unpack() };
                
                let y_cell = match bounds(y_pos) {
                    [Within, Within] => self.read[y_i].unpack(),
                    [Less, Within] => self.neighbors[Left].and_then(nn_read),
                    [Greater, Within] => self.neighbors[Right].and_then(nn_read),
                    [Within, Less] => self.neighbors[Down].and_then(nn_read),
                    [Within, Greater] => self.neighbors[Up].and_then(nn_read),
                    [Less, Less] => self.neighbors[DownLeft].and_then(nn_read),
                    [Greater, Less] => self.neighbors[DownRight].and_then(nn_read),
                    [Less, Greater] => self.neighbors[UpLeft].and_then(nn_read),
                    [Greater, Greater] => self.neighbors[UpRight].and_then(nn_read),
                };
                
                match (x_cell, y_cell) {
                    (None, None) => {
                        let dst = pos + delta;
                        let dst_i = wrapping_linearize(dst);

                        fn ptr_to_ref<'a>(nn: NonNull<Chunk>) -> &'a Chunk {
                            unsafe { &*nn.as_ptr() }
                        }

                        // Safety: Only used to read shared state or mutate shared atomics
                        let Some(chunk) = (match bounds(dst) {
                            [Within, Within] => Some(&*self),
                            [Less, Within] => self.neighbors[Left].map(ptr_to_ref),
                            [Greater, Within] => self.neighbors[Right].map(ptr_to_ref),
                            [Within, Less] => self.neighbors[Down].map(ptr_to_ref),
                            [Within, Greater] => self.neighbors[Up].map(ptr_to_ref),
                            [Less, Less] => self.neighbors[DownLeft].map(ptr_to_ref),
                            [Greater, Less] => self.neighbors[DownRight].map(ptr_to_ref),
                            [Less, Greater] => self.neighbors[UpLeft].map(ptr_to_ref),
                            [Greater, Greater] => self.neighbors[UpRight].map(ptr_to_ref),
                        }) else {
                            cell.static_collision(&StaticCell { restitution: 15 }, delta);
                            self.write[i].plain = cell.pack();
                            continue;
                        };

                        if let Some(dst_cell) = chunk.read[dst_i].unpack() {
                            match dst_cell {
                                Cell::Dynamic(dst_cell) => cell.dynamic_collision(&dst_cell, delta),
                                Cell::Static(dst_cell) => cell.static_collision(&dst_cell, delta),
                            }

                            self.write[i].plain = cell.pack();
                        } else {
                            if is_edge_or_ob(dst) {
                                let mut replacement = PackedCell::NONE;

                                // Safety: Atomic b/c `is_edge`
                                let atomic = unsafe { &chunk.write[dst_i].atomic };

                                atomic.update(Ordering::AcqRel, Ordering::Acquire, |dst_cell| {
                                    if let Some(dst_cell) = dst_cell.unpack() {
                                        if let Cell::Dynamic(mut dst_cell) = dst_cell {
                                            cell.two_way_dynamic_collision(&mut dst_cell, delta);

                                            replacement = cell.pack();

                                            dst_cell.pack()
                                        } else {
                                            unreachable!()
                                        }
                                    } else {
                                        replacement = PackedCell::NONE;

                                        cell.pack()
                                    }
                                });

                                self.write[i].plain = replacement;
                            } else {
                                // this branch only occurs when `chunk == self`

                                // Safety: `!is_edge` no other threads can mutate `dst`
                                let plain = unsafe { &mut self.write[dst_i].plain };

                                if let Some(dst_cell) = plain.unpack() {
                                    if let Cell::Dynamic(mut dst_cell) = dst_cell {
                                        cell.two_way_dynamic_collision(&mut dst_cell, delta);

                                        *plain = dst_cell.pack();
                                        self.write[i].plain = cell.pack();
                                    } else {
                                        unreachable!()
                                    }
                                } else {
                                    *plain = cell.pack();
                                    self.write[i].plain = PackedCell::NONE;
                                };
                            }
                        }
                    },
                    (x_cell, y_cell) => {
                        if let Some(x_cell) = x_cell {
                            match x_cell {
                                Cell::Dynamic(x_cell) => cell.dynamic_collision_x(&x_cell),
                                Cell::Static(x_cell) => cell.static_collision_x(&x_cell),
                            }
                        }
                        if let Some(y_cell) = y_cell {
                            match y_cell {
                                Cell::Dynamic(y_cell) => cell.dynamic_collision_y(&y_cell),
                                Cell::Static(y_cell) => cell.static_collision_y(&y_cell),
                            }
                        }

                        self.write[i].plain = cell.pack();
                    },
                }
            } else {
                let dst = pos + delta;
                let dst_i = wrapping_linearize(dst);

                fn ptr_to_ref<'a>(nn: NonNull<Chunk>) -> &'a Chunk {
                    unsafe { &*nn.as_ptr() }
                }

                // Safety: Only used to read shared state or mutate shared atomics
                let Some(chunk) = (match bounds(dst) {
                    [Within, Within] => Some(&*self),
                    [Less, Within] => self.neighbors[Left].map(ptr_to_ref),
                    [Greater, Within] => self.neighbors[Right].map(ptr_to_ref),
                    [Within, Less] => self.neighbors[Down].map(ptr_to_ref),
                    [Within, Greater] => self.neighbors[Up].map(ptr_to_ref),
                    [Less, Less] => self.neighbors[DownLeft].map(ptr_to_ref),
                    [Greater, Less] => self.neighbors[DownRight].map(ptr_to_ref),
                    [Less, Greater] => self.neighbors[UpLeft].map(ptr_to_ref),
                    [Greater, Greater] => self.neighbors[UpRight].map(ptr_to_ref),
                }) else {
                    cell.static_collision(&StaticCell { restitution: 15 }, delta);
                    self.write[i].plain = cell.pack();
                    continue;
                };

                if let Some(dst_cell) = chunk.read[dst_i].unpack() {
                    match dst_cell {
                        Cell::Dynamic(dst_cell) => cell.dynamic_collision(&dst_cell, delta),
                        Cell::Static(dst_cell) => cell.static_collision(&dst_cell, delta),
                    }

                    self.write[i].plain = cell.pack();
                } else {
                    if is_edge_or_ob(dst) {
                        let mut replacement = PackedCell::NONE;

                        // Safety: Atomic b/c `is_edge`
                        let atomic = unsafe { &chunk.write[dst_i].atomic };

                        atomic.update(Ordering::AcqRel, Ordering::Acquire, |dst_cell| {
                            if let Some(dst_cell) = dst_cell.unpack() {
                                if let Cell::Dynamic(mut dst_cell) = dst_cell {
                                    cell.two_way_dynamic_collision(&mut dst_cell, delta);

                                    replacement = cell.pack();

                                    dst_cell.pack()
                                } else {
                                    unreachable!()
                                }
                            } else {
                                replacement = PackedCell::NONE;

                                cell.pack()
                            }
                        });

                        self.write[i].plain = replacement;
                    } else {
                        // this branch only occurs when `chunk == self`

                        // Safety: `!is_edge` no other threads can mutate `dst`
                        let plain = unsafe { &mut self.write[dst_i].plain };

                        if let Some(dst_cell) = plain.unpack() {
                            if let Cell::Dynamic(mut dst_cell) = dst_cell {
                                cell.two_way_dynamic_collision(&mut dst_cell, delta);

                                *plain = dst_cell.pack();
                                self.write[i].plain = cell.pack();
                            } else {
                                unreachable!()
                            }
                        } else {
                            *plain = cell.pack();
                            self.write[i].plain = PackedCell::NONE;
                        };
                    }
                }
            }
        }
    }

    pub fn add_neighbor(&mut self, neighbor: &mut Self, dir: Dir) {
        self.neighbors[dir] = Some(NonNull::new(neighbor as *mut _).unwrap());
    }

    pub fn remove_neighbor(&mut self, dir: Dir) {
        self.neighbors[dir] = None;
    }

    pub fn iter_some(&self) -> impl Iterator<Item = IVec2> {
        self.read.iter().enumerate().filter_map(|(i, c)| {
            if let Some(_) = c.unpack() {
                Some(delinearize(i))
            } else {
                None
            }
        })
    }

    pub fn set_dynamic(&mut self, cell_pos: UVec2) {
        let i = linearize(cell_pos);
        let mass = rng().random_range(1..=4);
        let vel_x = rng().random_range(-3..=3);
        let vel_y = rng().random_range(1..=3);
        let velocity = I8Vec2::new(vel_x, vel_y);
        let p = DynamicCell { mass, velocity }.pack();
        self.write[i].plain = p;
        self.read[i] = p;
    }

    pub fn set_static(&mut self, cell_pos: UVec2) {
        let i = linearize(cell_pos);
        let p = StaticCell { restitution: 15 }.pack();
        self.write[i].plain = p;
        self.read[i] = p;
    }

    pub fn set_none(&mut self, cell_pos: UVec2) {
        let i = linearize(cell_pos);
        let p = PackedCell::NONE;
        self.write[i].plain = p;
        self.read[i] = p;
    }

    pub fn gravity(&mut self) {
        for (r, w) in self.read.iter_mut().zip(&mut self.write) {
            if let Some(Cell::Dynamic(mut cell)) = r.unpack() {
                cell.gravity();
                let p = cell.pack();
                *r = p;
                w.plain = p;
            }
        }
    }
}

fn bounds(pos: IVec2) -> [Bounds; 2] {
    pos.to_array().map(|x| {
        if x >= MIN && x <= MAX {
            Within
        } else if x < MIN {
            Less
        } else {
            Greater
        }
    })
}

fn is_edge_or_ob(pos: IVec2) -> bool {
    pos.cmple(IVec2::splat(MIN)).any() || pos.cmpge(IVec2::splat(MAX)).any()
}

fn wrapped(pos: IVec2) -> UVec2 {
    pos.to_array().map(|x| (x & MAX) as u32).into()
}

fn wrapping_linearize(pos: IVec2) -> usize {
    linearize(wrapped(pos))
}

fn linearize(pos: UVec2) -> usize {
    Shape::linearize(pos.into()) as usize
}

fn delinearize(i: usize) -> IVec2 {
    UVec2::from(Shape::delinearize(i as u32)).as_ivec2()
}

fn is_diagonal(delta: IVec2) -> bool {
    delta.x != 0 && delta.y != 0
}
