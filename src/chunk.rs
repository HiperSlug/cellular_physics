use bevy::prelude::*;
use ndshape::ConstShape;
use std::sync::atomic::Ordering;

use crate::cell::{AtomicPackedCell, PackedCell};

mod pad {
    use ndshape::ConstPow2Shape2u32;

    const BITS: u32 = 6;

    pub const LEN: u32 = 1 << BITS;
    pub const AREA: u32 = LEN * LEN;

    pub type Shape = ConstPow2Shape2u32<BITS, BITS>;
}

mod unpad {
    use ndshape::ConstShape2u32;

    use super::pad;

    pub const LEN: u32 = pad::LEN - 2;
    pub const AREA: u32 = LEN * LEN;

    pub type Shape = ConstShape2u32<LEN, LEN>;
}

type SharedWrite = (
    [[AtomicPackedCell; pad::LEN as usize]; 2],
    [[AtomicPackedCell; unpad::LEN as usize]; 2],
);
type Write = [PackedCell; unpad::AREA as usize];
type Read = [PackedCell; pad::AREA as usize];

struct Chunk {
    shared_write: SharedWrite,
    write: Write,
    read: Read,
}

impl Chunk {
    fn push_writes(&mut self) {
        for y in 1..unpad::LEN {
            for x in 1..unpad::LEN {
                let unpad_i = unpad::Shape::linearize([x, y]) as usize;
                let pad_i = pad::Shape::linearize([x, y]) as usize;
                self.read[pad_i] = self.write[unpad_i];
            }
        }

        for top_or_bottom in 0..2 {
            let y = top_or_bottom as u32 * pad::LEN;
            for x in 0..pad::LEN {
                let pad_i = pad::Shape::linearize([x, y]) as usize;
                let i = x as usize;
                self.read[pad_i] = self.shared_write.0[top_or_bottom][i].load(Ordering::Acquire);
            }
        }

        for left_or_right in 0..2 {
            let x = left_or_right as u32 * pad::LEN;
            for y in 1..unpad::LEN {
                let pad_i = pad::Shape::linearize([x, y]) as usize;
                let i = x as usize;
                self.read[pad_i] = self.shared_write.1[left_or_right][i].load(Ordering::Acquire);
            }
        }
    }

    fn split(&mut self) -> (&mut Write, &SharedWrite, &Read) {
        (&mut self.write, &self.shared_write, &self.read)
    }

    // n 0..3
    fn sub_step(&self, n: u8) {
        // TODO we cannot iterate over every cell unless we have padding or neighbor access.
        for (i, mut cell) in self
            .read
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.unpack().and_then(|c| c.dynamic_opt().map(|d| (i, d))))
        {
            let pos = UVec2::from(Shape::delinearize(i as u32));

            for offset in ADJ_OFFSETS {
                let adj_pos = (pos.as_ivec2() + offset).as_uvec2();
                let adj_i = Shape::linearize(adj_pos.into()) as usize;
                let Some(Cell::Dynamic(adj_cell)) = self.read[adj_i].unpack() else {
                    continue;
                };

                let adj_dst = (adj_pos.as_ivec2() + adj_cell.sub_step_delta(n)).as_uvec2();

                if adj_dst == pos {
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
            let dst_cell_opt = self.read[dst_i].unpack();

            if let Some(dst_cell) = dst_cell_opt {
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

                self.write[i].store(Cell::Dynamic(cell).pack(), Ordering::Relaxed);
            } else {
                self.write[dst_i].update(Ordering::Release, Ordering::Acquire, |dst_cell| {
                    if let Some(dst_cell) = dst_cell.unpack() {
                        if let Cell::Dynamic(mut dst_cell) = dst_cell {
                            if sub_step_delta.x != 0 {
                                cell.dynamic_collision_x(&dst_cell);
                                dst_cell.dynamic_collision_x(&cell);
                            }
                            if sub_step_delta.y != 0 {
                                cell.dynamic_collision_y(&dst_cell);
                                dst_cell.dynamic_collision_y(&cell);
                            }

                            self.write[i].store(Cell::Dynamic(cell).pack(), Ordering::Relaxed);

                            Cell::Dynamic(dst_cell).pack()
                        } else {
                            unreachable!()
                        }
                    } else {
                        Cell::Dynamic(cell).pack()
                    }
                });
            }
        }
    }
}
