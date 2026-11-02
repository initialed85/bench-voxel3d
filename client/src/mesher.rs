//! Chunk meshing: one mesh per chunk, one quad per exposed face, with baked
//! ambient occlusion in the vertex colours.

use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use voxel_common::{block, World, CHUNK, CHUNKS_X, CHUNKS_Z};

use crate::{all_chunks, Game};

struct Face {
    n: [i32; 3],
    u: [i32; 3],
    v: [i32; 3],
    shade: f32,
}

// u x v == n, so the corner order (0,0) (1,0) (1,1) (0,1) is counter-clockwise
// when viewed from outside the block.
const FACES: [Face; 6] = [
    Face {
        n: [1, 0, 0],
        u: [0, 1, 0],
        v: [0, 0, 1],
        shade: 0.72,
    },
    Face {
        n: [-1, 0, 0],
        u: [0, 0, 1],
        v: [0, 1, 0],
        shade: 0.72,
    },
    Face {
        n: [0, 1, 0],
        u: [0, 0, 1],
        v: [1, 0, 0],
        shade: 1.00,
    },
    Face {
        n: [0, -1, 0],
        u: [1, 0, 0],
        v: [0, 0, 1],
        shade: 0.45,
    },
    Face {
        n: [0, 0, 1],
        u: [1, 0, 0],
        v: [0, 1, 0],
        shade: 0.86,
    },
    Face {
        n: [0, 0, -1],
        u: [0, 1, 0],
        v: [1, 0, 0],
        shade: 0.86,
    },
];

fn chunk_origin(chunk: u32) -> [u32; 3] {
    [
        (chunk % CHUNKS_X) * CHUNK,
        (chunk / (CHUNKS_X * CHUNKS_Z)) * CHUNK,
        ((chunk / CHUNKS_X) % CHUNKS_Z) * CHUNK,
    ]
}

pub fn build_chunk(world: &World, chunk: u32) -> Option<Mesh> {
    let [ox, oy, oz] = chunk_origin(chunk);

    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut norm: Vec<[f32; 3]> = Vec::new();
    let mut col: Vec<[f32; 4]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    for y in 0..CHUNK {
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let bx = ox as i32 + x as i32;
                let by = oy as i32 + y as i32;
                let bz = oz as i32 + z as i32;
                let b = world.get(bx, by, bz);
                if b == block::AIR {
                    continue;
                }
                let base_color = block::color(b);

                for face in &FACES {
                    if world.solid(bx + face.n[0], by + face.n[1], bz + face.n[2]) {
                        continue;
                    }

                    // Corner of the face plane, then walk the four corners.
                    let fo = [
                        bx + face.n[0].max(0),
                        by + face.n[1].max(0),
                        bz + face.n[2].max(0),
                    ];
                    let base = pos.len() as u32;

                    for (i, j) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                        let p = [
                            fo[0] + i * face.u[0] + j * face.v[0],
                            fo[1] + i * face.u[1] + j * face.v[1],
                            fo[2] + i * face.u[2] + j * face.v[2],
                        ];
                        pos.push([p[0] as f32, p[1] as f32, p[2] as f32]);
                        norm.push([face.n[0] as f32, face.n[1] as f32, face.n[2] as f32]);

                        let a = [
                            if i == 1 { face.u[0] } else { -face.u[0] },
                            if i == 1 { face.u[1] } else { -face.u[1] },
                            if i == 1 { face.u[2] } else { -face.u[2] },
                        ];
                        let b2 = [
                            if j == 1 { face.v[0] } else { -face.v[0] },
                            if j == 1 { face.v[1] } else { -face.v[1] },
                            if j == 1 { face.v[2] } else { -face.v[2] },
                        ];
                        let blockers = world.solid(fo[0] + a[0], fo[1] + a[1], fo[2] + a[2]) as u32
                            + world.solid(fo[0] + b2[0], fo[1] + b2[1], fo[2] + b2[2]) as u32
                            + world.solid(
                                fo[0] + a[0] + b2[0],
                                fo[1] + a[1] + b2[1],
                                fo[2] + a[2] + b2[2],
                            ) as u32;
                        let ao = 1.0 - 0.21 * blockers as f32;
                        let shade = face.shade * ao;
                        col.push([
                            base_color[0] * shade,
                            base_color[1] * shade,
                            base_color[2] * shade,
                            1.0,
                        ]);
                    }

                    idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                }
            }
        }
    }

    if pos.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, norm);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    Some(mesh)
}

pub fn rebuild(
    mut game: ResMut<Game>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    mut mesh_handles: Query<&mut Mesh3d>,
) {
    if game.dirty.is_empty() {
        return;
    }

    let chunks: Vec<u32> = game.dirty.iter().copied().collect();
    for chunk in chunks {
        game.dirty.remove(&chunk);
        let Some(mesh) = build_chunk(&game.world, chunk) else {
            if let Some(entity) = game.chunk_entities.remove(&chunk) {
                commands.entity(entity).despawn();
            }
            continue;
        };

        let handle = meshes.add(mesh);
        match game.chunk_entities.get(&chunk) {
            Some(entity) => {
                if let Ok(mut existing) = mesh_handles.get_mut(*entity) {
                    existing.0 = handle;
                }
            }
            None => {
                let entity = commands
                    .spawn((Mesh3d(handle), MeshMaterial3d(game.white.clone())))
                    .id();
                game.chunk_entities.insert(chunk, entity);
            }
        }
    }
}

pub fn mark_all(game: &mut Game) {
    game.dirty = (0..all_chunks()).collect();
}
