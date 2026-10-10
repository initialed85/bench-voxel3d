//! First-person controller: mouse look, voxel collision, block picking.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use voxel_common::{block, ClientMsg, World, WORLD_X, WORLD_Z};

use crate::{hud, mark_dirty, net, raycast, Game, LocalPlayer};

const EYE: f32 = 1.62;
const HALF: f32 = 0.3;
const HEIGHT: f32 = 1.8;
const GRAVITY: f32 = -22.0;
const JUMP: f32 = 7.6;
const SPEED: f32 = 5.5;
const SPRINT: f32 = 8.5;
const REACH: f32 = 6.0;
const SEND_INTERVAL: f64 = 0.1;

fn view_dir(p: &LocalPlayer) -> Vec3 {
    (Quat::from_rotation_y(p.yaw) * Quat::from_rotation_x(p.pitch)) * Vec3::NEG_Z
}

fn collides(world: &World, feet: Vec3) -> bool {
    let x0 = (feet.x - HALF).floor() as i32;
    let x1 = (feet.x + HALF).floor() as i32;
    let y0 = feet.y.floor() as i32;
    let y1 = (feet.y + HEIGHT).floor() as i32;
    let z0 = (feet.z - HALF).floor() as i32;
    let z1 = (feet.z + HALF).floor() as i32;

    for x in x0..=x1 {
        for y in y0..=y1 {
            for z in z0..=z1 {
                if world.solid(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

fn overlaps_player(feet: Vec3, cell: [i32; 3]) -> bool {
    let x0 = (feet.x - HALF).floor() as i32;
    let x1 = (feet.x + HALF).floor() as i32;
    let y0 = feet.y.floor() as i32;
    let y1 = (feet.y + HEIGHT).floor() as i32;
    let z0 = (feet.z - HALF).floor() as i32;
    let z1 = (feet.z + HALF).floor() as i32;
    cell[0] >= x0 && cell[0] <= x1 && cell[1] >= y0 && cell[1] <= y1 && cell[2] >= z0 && cell[2] <= z1
}

fn apply_edit(game: &mut Game, cell: [i32; 3], b: u8) {
    if !game.world.in_bounds(cell[0], cell[1], cell[2]) {
        return;
    }
    game.world.set(cell[0], cell[1], cell[2], b);
    mark_dirty(game, cell[0], cell[1], cell[2]);
    hud::log(&format!(
        "{} at {},{},{}",
        if b == block::AIR { "break" } else { "place" },
        cell[0],
        cell[1],
        cell[2]
    ));
    net::outgoing().lock().unwrap().push_back(ClientMsg::Edit {
        pos: cell,
        block: b,
    });
}

pub fn click(
    mut game: ResMut<Game>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: Query<&mut CursorOptions>,
) {
    if hud::input_focused() {
        return;
    }

    if mouse.just_pressed(MouseButton::Left) && !game.local.locked {
        for mut c in cursor.iter_mut() {
            c.grab_mode = CursorGrabMode::Locked;
            c.visible = false;
        }
        game.local.locked = true;
        return;
    }
    if !game.local.locked {
        return;
    }

    let origin = game.local.feet + Vec3::Y * EYE;
    let dir = view_dir(&game.local);

    if mouse.just_pressed(MouseButton::Left) {
        if let Some((cell, _)) = raycast::raycast(&game.world, origin, dir, REACH) {
            apply_edit(&mut game, cell, block::AIR);
        }
    } else if mouse.just_pressed(MouseButton::Right) {
        if let Some((_, prev)) = raycast::raycast(&game.world, origin, dir, REACH) {
            if !overlaps_player(game.local.feet, prev) {
                let selected = game.local.selected;
                apply_edit(&mut game, prev, selected);
            }
        }
    }
}

pub fn look(
    mut game: ResMut<Game>,
    mut events: MessageReader<MouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if !game.local.locked {
        return;
    }
    for ev in events.read() {
        game.local.yaw += ev.delta.x * 0.0026;
        game.local.pitch = (game.local.pitch - ev.delta.y * 0.0026).clamp(-1.55, 1.55);
    }
    // Arrow keys look too: handy when the mouse is unavailable.
    let step = 0.08;
    if keys.pressed(KeyCode::ArrowUp) {
        game.local.pitch = (game.local.pitch + step).clamp(-1.55, 1.55);
    }
    if keys.pressed(KeyCode::ArrowDown) {
        game.local.pitch = (game.local.pitch - step).clamp(-1.55, 1.55);
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        game.local.yaw += step;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        game.local.yaw -= step;
    }
}

/// Escape releases the mouse so the chat box can be reached.
pub fn release(
    mut game: ResMut<Game>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor: Query<&mut CursorOptions>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    game.local.locked = false;
    for mut c in cursor.iter_mut() {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
    }
}

pub fn hotbar(mut game: ResMut<Game>, keys: Res<ButtonInput<KeyCode>>) {
    for (i, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ]
    .iter()
    .enumerate()
    {
        if keys.just_pressed(*key) {
            game.local.selected = block::HOTBAR[i];
            game.dirty_hud = true;
        }
    }
}

pub fn movement(
    mut game: ResMut<Game>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut transforms: Query<&mut Transform>,
) {
    if hud::input_focused() {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    game.dirty_hud = true;

    let mut forward = 0.0;
    let mut strafe = 0.0;
    if keys.pressed(KeyCode::KeyW) {
        forward += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        forward -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        strafe += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        strafe -= 1.0;
    }

    let yaw = game.local.yaw;
    let fwd = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let wish = fwd * forward + right * strafe;
    let speed = if keys.pressed(KeyCode::ShiftLeft) { SPRINT } else { SPEED };
    if wish.length_squared() > 0.0 {
        game.local.vel.x = wish.normalize().x * speed;
        game.local.vel.z = wish.normalize().z * speed;
    } else {
        game.local.vel.x = 0.0;
        game.local.vel.z = 0.0;
    }

    game.local.vel.y += GRAVITY * dt;
    if game.local.grounded && keys.just_pressed(KeyCode::Space) {
        game.local.vel.y = JUMP;
        game.local.grounded = false;
    }

    let feet = game.local.feet;
    let dx = game.local.vel.x * dt;
    let dy = game.local.vel.y * dt;
    let dz = game.local.vel.z * dt;

    // Horizontal, with a small auto step-up.
    let target = feet + Vec3::new(dx, 0.0, dz);
    if collides(&game.world, target) {
        let stepped = target + Vec3::new(0.0, 0.6, 0.0);
        if game.local.grounded && !collides(&game.world, stepped) {
            game.local.feet = stepped;
        } else {
            game.local.vel.x = 0.0;
            game.local.vel.z = 0.0;
        }
    } else {
        game.local.feet = target;
    }

    // Vertical.
    let target = game.local.feet + Vec3::new(0.0, dy, 0.0);
    if collides(&game.world, target) {
        if dy < 0.0 {
            game.local.grounded = true;
        }
        game.local.vel.y = 0.0;
    } else {
        game.local.feet = target;
        game.local.grounded = false;
    }

    if game.local.feet.y < -4.0 {
        let x = WORLD_X as i32 / 2;
        let z = WORLD_Z as i32 / 2;
        game.local.feet = Vec3::new(x as f32 + 0.5, game.world.surface(x, z) as f32 + 1.0, z as f32 + 0.5);
        game.local.vel = Vec3::ZERO;
    }

    if let Some(camera) = game.camera {
        if let Ok(mut t) = transforms.get_mut(camera) {
            t.translation = game.local.feet + Vec3::Y * EYE;
            t.rotation = Quat::from_rotation_y(game.local.yaw) * Quat::from_rotation_x(game.local.pitch);
        }
    }

    let now = time.elapsed_secs_f64();
    if now - game.local.last_send >= SEND_INTERVAL {
        game.local.last_send = now;
        let p = game.local.feet;
        net::outgoing().lock().unwrap().push_back(ClientMsg::Move {
            pos: [p.x, p.y, p.z],
            yaw: game.local.yaw,
            pitch: game.local.pitch,
        });
    }

}
