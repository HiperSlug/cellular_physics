use bevy::{
    platform::collections::HashMap,
    prelude::*,
    tasks::{ComputeTaskPool, ParallelSliceMut},
};
use enum_map::{Enum, EnumMap};
use std::array::from_fn;

use crate::{
    Dir, OFFSETS,
    chunk::{Chunk, LEN},
};

#[derive(Resource, Default)]
pub struct ChunkMap {
    map: HashMap<IVec2, Chunk>,
}

impl ChunkMap {
    pub fn sub_step(&mut self, n: u8) {
        let mut vec = self.map.values_mut().collect::<Vec<_>>();

        if n == 0 {
            vec.par_splat_map_mut(ComputeTaskPool::get(), None, |_, slice| {
                for c in slice {
                    c.gravity();
                }
            });
        }

        vec.par_splat_map_mut(ComputeTaskPool::get(), None, |_, slice| {
            for c in slice {
                c.sub_step(n);
            }
        });
        vec.par_splat_map_mut(ComputeTaskPool::get(), None, |_, slice| {
            for c in slice {
                c.push_writes();
            }
        });
    }

    pub fn insert(&mut self, k: IVec2, v: Chunk) {
        self.map.insert(k, v);

        let ks: [_; 9] = from_fn(|i| {
            if i < Dir::LENGTH {
                OFFSETS[Dir::from_usize(i)] + k
            } else {
                k
            }
        });
        let ref_ks: [_; 9] = from_fn(|i| &ks[i]);

        // get_many_mut is the shittiest function ever
        let [a, b, c, d, e, f, g, h, middle] = self.map.get_many_mut(ref_ks);

        let chunk_opts: EnumMap<Dir, Option<&mut Chunk>> =
            EnumMap::from_array([a, b, c, d, e, f, g, h]);
        let middle = middle.unwrap();

        for (dir, chunk_opt) in chunk_opts {
            if let Some(chunk) = chunk_opt {
                middle.add_neighbor(chunk, dir);
                chunk.add_neighbor(middle, dir.inverse());
            }
        }
    }

    pub fn remove(&mut self, k: IVec2) {
        let ks = OFFSETS.map(|_, o| k + o);
        let ref_ks = from_fn(|i| &ks[Dir::from_usize(i)]);

        let chunk_opts: EnumMap<Dir, _> = EnumMap::from_array(self.map.get_many_mut(ref_ks));

        for (dir, chunk_opt) in chunk_opts {
            if let Some(chunk) = chunk_opt {
                chunk.remove_neighbor(dir.inverse());
            }
        }

        self.map.remove(&k);
    }

    pub fn iter_some(&self) -> impl Iterator<Item = IVec2> {
        self.map
            .iter()
            .flat_map(|(p, c)| c.iter_some().map(|s| s + (*p * LEN)))
    }

    pub fn set_dynamic(&mut self, cell_pos: IVec2) {
        let chunk_pos = cell_pos.div_euclid(IVec2::splat(LEN));
        if let Some(chunk) = self.map.get_mut(&chunk_pos) {
            let local_cell_pos = cell_pos.rem_euclid(IVec2::splat(LEN)).as_uvec2();
            chunk.set_dynamic(local_cell_pos)
        }
    }

    pub fn set_static(&mut self, cell_pos: IVec2) {
        let chunk_pos = cell_pos.div_euclid(IVec2::splat(LEN));
        if let Some(chunk) = self.map.get_mut(&chunk_pos) {
            let local_cell_pos = cell_pos.rem_euclid(IVec2::splat(LEN)).as_uvec2();
            chunk.set_static(local_cell_pos)
        }
    }

    pub fn set_none(&mut self, cell_pos: IVec2) {
        let chunk_pos = cell_pos.div_euclid(IVec2::splat(LEN));
        if let Some(chunk) = self.map.get_mut(&chunk_pos) {
            let local_cell_pos = cell_pos.rem_euclid(IVec2::splat(LEN)).as_uvec2();
            chunk.set_none(local_cell_pos)
        }
    }
}
