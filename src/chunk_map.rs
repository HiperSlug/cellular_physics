use bevy::{platform::collections::HashMap, prelude::*, tasks::{ComputeTaskPool, ParallelSliceMut}};
use enum_map::{Enum, EnumMap};
use std::array::from_fn;

use crate::{Dir, OFFSETS, chunk::Chunk};

struct ChunkMap {
    map: HashMap<IVec2, Chunk>,
}

impl ChunkMap {
    fn step(&mut self) {
        let mut vec = self.map.values_mut().collect::<Vec<_>>();
        for n in 0..3 {
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
    }

    fn insert(&mut self, k: IVec2, v: Chunk) {
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

    fn remove(&mut self, k: IVec2) {
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
}
