//! Shared game state. Systems live in the modules below.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use voxel_common::{block, chunk_index, Block, World, CHUNKS_X, CHUNKS_Y, CHUNKS_Z};

pub const DEFAULT_SEED: i64 = 0x5EED_C0DE_1234_5678;

pub mod hud;
pub mod mesher;
pub mod net;
pub mod player;
pub mod raycast;

#[derive(Resource)]
pub struct Game {
    pub world: World,
    pub dirty: HashSet<u32>,
    pub dirty_hud: bool,
    pub chunk_entities: HashMap<u32, Entity>,
    pub remote: HashMap<u64, Entity>,
    pub roster: HashMap<u64, String>,
    pub cube: Handle<Mesh>,
    pub white: Handle<StandardMaterial>,
    pub camera: Option<Entity>,
    pub local: LocalPlayer,
    pub my_id: u64,
    pub name: String,
    pub connected: bool,
    pub last_status: String,
    pub last_roster: String,
    pub last_hotbar: String,
}

#[derive(Debug)]
pub struct LocalPlayer {
    pub feet: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
    pub selected: Block,
    pub locked: bool,
    pub last_send: f64,
}

impl Default for Game {
    fn default() -> Self {
        let chunks = CHUNKS_X * CHUNKS_Y * CHUNKS_Z;
        Game {
            world: World::generate(DEFAULT_SEED),
            dirty: (0..chunks).collect(),
            dirty_hud: true,
            chunk_entities: HashMap::new(),
            remote: HashMap::new(),
            roster: HashMap::new(),
            cube: Handle::default(),
            white: Handle::default(),
            camera: None,
            local: LocalPlayer {
                feet: Vec3::ZERO,
                vel: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                grounded: false,
                selected: block::GRASS,
                locked: false,
                last_send: 0.0,
            },
            my_id: 0,
            name: "player".into(),
            connected: false,
            last_status: String::new(),
            last_roster: String::new(),
            last_hotbar: String::new(),
        }
    }
}

pub fn all_chunks() -> u32 {
    CHUNKS_X * CHUNKS_Y * CHUNKS_Z
}

/// Mark the chunk containing a block dirty, plus the neighbouring chunks: face
/// visibility and ambient occlusion both reach across chunk borders.
pub fn mark_dirty(game: &mut Game, x: i32, y: i32, z: i32) {
    game.dirty.insert(chunk_index(x as u32, y as u32, z as u32));
    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
        let (nx, ny, nz) = (x + dx, y + dy, z + dz);
        if nx < 0 || ny < 0 || nz < 0 || nx >= 64 || ny >= 48 || nz >= 64 {
            continue;
        }
        let here = chunk_index(x as u32, y as u32, z as u32);
        let other = chunk_index(nx as u32, ny as u32, nz as u32);
        if other != here {
            game.dirty.insert(other);
        }
    }
}

pub fn run() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "bench-voxel3d".into(),
            canvas: Some("#game".to_string()),
            prevent_default_event_handling: false,
            ..default()
        }),
        ..default()
    }));

    app.add_systems(Startup, setup);
    app.add_systems(Update, (net::connect, net::poll).chain());
    app.add_systems(
        Update,
        (
            player::look,
            player::release,
            player::movement,
            player::hotbar,
            player::click,
            mesher::rebuild,
            net::flush,
            hud::update,
        )
            .chain(),
    );

    app.run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut game = Game::default();
    game.white = materials.add(StandardMaterial::default());
    game.cube = meshes.add(Cuboid::new(0.6, 1.8, 0.6));

    let camera = commands
        .spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: 1.1,
                near: 0.05,
                far: 250.0,
                ..default()
            }),
            Transform::default(),
        ))
        .id();
    game.camera = Some(camera);

    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_x(-50.0)),
    ));

    commands.insert_resource(game);
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn start() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    run();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn send_chat(text: &str) {
    net::send_chat(text);
}
