//! Deterministic voxel world. Both server and client generate the same world
//! from the same seed, so the initial transfer is a seed, not a block array.

use crate::protocol::block;

pub const WORLD_X: u32 = 64;
pub const WORLD_Y: u32 = 48;
pub const WORLD_Z: u32 = 64;

pub const CHUNK: u32 = 16;
pub const CHUNKS_X: u32 = WORLD_X / CHUNK;
pub const CHUNKS_Y: u32 = WORLD_Y / CHUNK;
pub const CHUNKS_Z: u32 = WORLD_Z / CHUNK;

pub fn chunk_index(x: u32, y: u32, z: u32) -> u32 {
    (x / CHUNK) + (z / CHUNK) * CHUNKS_X + (y / CHUNK) * CHUNKS_X * CHUNKS_Z
}

pub fn hash2(x: i64, y: i64, seed: i64) -> i64 {
    let mut h = (x as u64).wrapping_mul(0x9E3779B97F4A7C15)
        ^ (y as u64).wrapping_mul(0xC2B2AE3D27D4EB4F)
        ^ (seed as u64).wrapping_mul(0x165667B19E3779F9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h as i64
}

fn idx(x: u32, y: u32, z: u32) -> usize {
    ((y * WORLD_Z + z) * WORLD_X + x) as usize
}

#[derive(Debug, Clone)]
pub struct World {
    pub blocks: Vec<u8>,
    pub seed: i64,
}

impl World {
    pub fn generate(seed: i64) -> World {
        let mut blocks = vec![0u8; (WORLD_X * WORLD_Y * WORLD_Z) as usize];

        for x in 0..WORLD_X {
            for z in 0..WORLD_Z {
                let h = hash2(x as i64, z as i64, seed);
                let n = (h & 0xFFFF) as f64 / 65535.0;
                let mut height = 10.0 + n * 14.0;
                height += (x as f64 * 0.11).sin() * 2.0 + (z as f64 * 0.13).cos() * 2.0;
                let height = (height.max(1.0)) as u32;

                for y in 0..=height {
                    let b = if y == height {
                        if height <= 11 {
                            block::SAND
                        } else if height >= 24 {
                            block::SNOW
                        } else {
                            block::GRASS
                        }
                    } else if y >= height.saturating_sub(3) {
                        block::DIRT
                    } else {
                        block::STONE
                    };
                    blocks[idx(x, y, z)] = b;
                }
            }
        }

        for x in 1..WORLD_X - 1 {
            for z in 1..WORLD_Z - 1 {
                let h = hash2(x as i64, z as i64, seed ^ 0x5BD1E995);
                if (h & 0xFF) > 225 {
                    let ground = 10.0
                        + ((h & 0xFFFF) as f64 / 65535.0) * 14.0
                        + (x as f64 * 0.11).sin() * 2.0
                        + (z as f64 * 0.13).cos() * 2.0;
                    let y = (ground.max(1.0)) as u32;
                    if y + 5 >= WORLD_Y {
                        continue;
                    }
                    for dy in 1..=4 {
                        blocks[idx(x, y + dy, z)] = block::WOOD;
                    }
                    for dx in -2i32..=2 {
                        for dz in -2i32..=2 {
                            for dy in 4..=6 {
                                let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                                if nx < 0 || nz < 0 || nx >= WORLD_X as i32 || nz >= WORLD_Z as i32
                                {
                                    continue;
                                }
                                let i = idx(nx as u32, dy, nz as u32);
                                if blocks[i] == 0 {
                                    blocks[i] = block::LEAF;
                                }
                            }
                        }
                    }
                }
            }
        }

        World { blocks, seed }
    }

    pub fn in_bounds(&self, x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < WORLD_X as i32 && y < WORLD_Y as i32 && z < WORLD_Z as i32
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if !self.in_bounds(x, y, z) {
            return block::AIR;
        }
        self.blocks[idx(x as u32, y as u32, z as u32)]
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, b: u8) -> bool {
        if !self.in_bounds(x, y, z) {
            return false;
        }
        let i = idx(x as u32, y as u32, z as u32);
        if self.blocks[i] == b {
            return false;
        }
        self.blocks[i] = b;
        true
    }

    pub fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.get(x, y, z) != block::AIR
    }

    pub fn surface(&self, x: i32, z: i32) -> i32 {
        for y in (0..WORLD_Y as i32).rev() {
            if self.solid(x, y, z) {
                return y;
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_world() {
        let a = World::generate(0x5eed);
        let b = World::generate(0x5eed);
        assert_eq!(a.blocks, b.blocks);
    }

    #[test]
    fn different_seed_gives_different_world() {
        let a = World::generate(1);
        let b = World::generate(2);
        assert_ne!(a.blocks, b.blocks);
    }

    #[test]
    fn set_get_roundtrip() {
        let mut w = World::generate(7);
        w.set(5, 30, 5, block::STONE);
        assert_eq!(w.get(5, 30, 5), block::STONE);
        assert!(w.solid(5, 30, 5));
        w.set(5, 30, 5, block::AIR);
        assert!(!w.solid(5, 30, 5));
    }

    #[test]
    fn out_of_bounds_is_air() {
        let w = World::generate(7);
        assert_eq!(w.get(-1, 0, 0), block::AIR);
        assert!(!w.in_bounds(WORLD_X as i32, 0, 0));
    }

    #[test]
    fn chunk_index_matches_dimensions() {
        assert_eq!(chunk_index(0, 0, 0), 0);
        assert_eq!(chunk_index(CHUNK, 0, 0), 1);
        assert_eq!(chunk_index(0, 0, CHUNK), CHUNKS_X);
        assert_eq!(chunk_index(0, CHUNK, 0), CHUNKS_X * CHUNKS_Z);
    }
}
