//! Wire protocol. JSON keeps it debuggable; the shapes are small enough that
//! the overhead is not worth binary framing for a benchmark.

use serde::{Deserialize, Serialize};

pub mod block {
    pub type Block = u8;

    pub const AIR: Block = 0;
    pub const GRASS: Block = 1;
    pub const DIRT: Block = 2;
    pub const STONE: Block = 3;
    pub const SAND: Block = 4;
    pub const WOOD: Block = 5;
    pub const LEAF: Block = 6;
    pub const BRICK: Block = 7;
    pub const SNOW: Block = 8;

    pub const HOTBAR: [Block; 6] = [GRASS, DIRT, STONE, SAND, WOOD, BRICK];

    pub fn name(b: Block) -> &'static str {
        match b {
            AIR => "air",
            GRASS => "grass",
            DIRT => "dirt",
            STONE => "stone",
            SAND => "sand",
            WOOD => "wood",
            LEAF => "leaf",
            BRICK => "brick",
            SNOW => "snow",
            _ => "?",
        }
    }

    pub fn color(b: Block) -> [f32; 3] {
        match b {
            GRASS => [0.36, 0.62, 0.31],
            DIRT => [0.52, 0.40, 0.30],
            STONE => [0.55, 0.56, 0.58],
            SAND => [0.82, 0.76, 0.55],
            WOOD => [0.44, 0.33, 0.22],
            LEAF => [0.28, 0.52, 0.28],
            BRICK => [0.62, 0.35, 0.31],
            SNOW => [0.92, 0.94, 0.96],
            _ => [0.0, 0.0, 0.0],
        }
    }
}

pub use block::Block;

pub type Pos = [i32; 3];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: u64,
    pub name: String,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    Hello { name: String },
    Move { pos: [f32; 3], yaw: f32, pitch: f32 },
    Edit { pos: Pos, block: Block },
    Chat { name: String, text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome {
        id: u64,
        name: String,
        seed: i64,
        spawn: [f32; 3],
        players: Vec<PlayerInfo>,
    },
    Spawn {
        player: PlayerInfo,
    },
    Despawn {
        id: u64,
        name: String,
    },
    Move {
        id: u64,
        pos: [f32; 3],
        yaw: f32,
        pitch: f32,
    },
    Block {
        pos: Pos,
        block: Block,
    },
    Chat {
        name: String,
        text: String,
    },
    Error {
        message: String,
    },
}

pub const SEED: i64 = 0x5EED_C0DE_1234_5678;

pub fn spawn_for(id: u64) -> [f32; 3] {
    let h = crate::world::hash2(id as i64, 11, 0x9E3779B9);
    let x = (8 + h.abs() % 48) as f32;
    let z = (8 + (h / 48).abs() % 48) as f32;
    [x, 26.0, z]
}

pub use crate::world::*;
