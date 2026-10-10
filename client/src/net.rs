//! WebSocket transport.
//!
//! The game talks to the network only through two global queues, so the same
//! systems work on wasm (`web_sys::WebSocket`) and on native (`tungstenite` on
//! a background task).

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use bevy::prelude::*;
use voxel_common::{ClientMsg, PlayerInfo, ServerMsg, World};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::{hud, mark_dirty, Game};

static STARTED: AtomicBool = AtomicBool::new(false);
static CONNECTED: AtomicBool = AtomicBool::new(false);

pub fn incoming() -> &'static Arc<Mutex<VecDeque<ServerMsg>>> {
    static Q: OnceLock<Arc<Mutex<VecDeque<ServerMsg>>>> = OnceLock::new();
    Q.get_or_init(Default::default)
}

pub fn outgoing() -> &'static Arc<Mutex<VecDeque<ClientMsg>>> {
    static Q: OnceLock<Arc<Mutex<VecDeque<ClientMsg>>>> = OnceLock::new();
    Q.get_or_init(Default::default)
}

#[cfg(not(target_arch = "wasm32"))]
static TX: OnceLock<tokio::sync::mpsc::UnboundedSender<String>> = OnceLock::new();

pub fn is_connected() -> bool {
    CONNECTED.load(Ordering::Relaxed)
}

fn set_connected(v: bool) {
    CONNECTED.store(v, Ordering::Relaxed);
}

#[cfg(target_arch = "wasm32")]
fn query_param(key: &str) -> String {
    let search = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    for pair in search.trim_start_matches('?').split('&') {
        if let Some(v) = pair.strip_prefix(&format!("{key}=")) {
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    String::new()
}

/// Debug aid: `?pitch=` sets the initial view angle (useful for headless tests).
pub fn debug_pitch() -> f32 {
    #[cfg(target_arch = "wasm32")]
    {
        query_param("pitch").parse().unwrap_or(0.0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0.0
    }
}

pub fn player_name() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let name = query_param("name");
        if name.is_empty() { "player".to_string() } else { name }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("VOXEL_NAME").unwrap_or_else(|_| "player".to_string())
    }
}

fn ws_url() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let target = query_param("server");
        let loc = web_sys::window().map(|w| w.location());
        let https = loc.as_ref().and_then(|l| l.protocol().ok()).as_deref() == Some("https:");
        let proto = if https { "wss:" } else { "ws:" };

        if !target.is_empty() {
            // Explicit override, e.g. ?server=host:3001
            format!("{proto}//{target}/ws")
        } else if https {
            // Behind a TLS proxy the game server is reached through the same origin, so
            // keep the origin's host and port (location.host omits the default 443).
            let host = loc.as_ref().and_then(|l| l.host().ok()).unwrap_or_default();
            format!("{proto}//{host}/ws")
        } else {
            // Plain http dev server: the game server is a separate port.
            let host = loc.as_ref().and_then(|l| l.hostname().ok()).unwrap_or_default();
            let host = if host.is_empty() { "localhost" } else { &host };
            format!("{proto}//{host}:3001/ws")
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("VOXEL_WS").unwrap_or_else(|_| "ws://localhost:3001/ws".to_string())
    }
}

pub fn connect(_game: ResMut<Game>) {
    if STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let url = ws_url();
    hud::set("status", &format!("connecting to {url}"));
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&format!("connecting to {url}").into());

    #[cfg(target_arch = "wasm32")]
    connect_wasm(&url);
    #[cfg(not(target_arch = "wasm32"))]
    connect_native(url);
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SOCK: RefCell<Option<web_sys::WebSocket>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn js_ref<T: ?Sized>(c: &wasm_bindgen::closure::Closure<T>) -> &js_sys::Function {
    c.as_ref().unchecked_ref::<js_sys::Function>()
}

#[cfg(target_arch = "wasm32")]
fn connect_wasm(url: &str) {
    use wasm_bindgen::closure::Closure;

    let ws = match web_sys::WebSocket::new(url) {
        Ok(ws) => ws,
        Err(e) => {
            hud::set("status", &format!("websocket error: {e:?}"));
            return;
        }
    };

    let inc = incoming().clone();

    let on_message = Closure::wrap(Box::new(move |ev: web_sys::MessageEvent| {
        let Some(text) = ev.data().as_string() else { return };
        if let Ok(msg) = serde_json::from_str::<ServerMsg>(&text) {
            inc.lock().unwrap().push_back(msg);
        }
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    ws.set_onmessage(Some(js_ref(&on_message)));
    on_message.forget();

    let on_open = Closure::wrap(Box::new(|| {
        set_connected(true);
        outgoing().lock().unwrap().push_back(ClientMsg::Hello { name: player_name() });
    }) as Box<dyn FnMut()>);
    ws.set_onopen(Some(js_ref(&on_open)));
    on_open.forget();

    let on_close = Closure::wrap(Box::new(|| set_connected(false)) as Box<dyn FnMut()>);
    ws.set_onclose(Some(js_ref(&on_close)));
    on_close.forget();

    SOCK.with(|s| *s.borrow_mut() = Some(ws));
}

#[cfg(not(target_arch = "wasm32"))]
fn connect_native(url: String) {
    use futures_util::sink::SinkExt;
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("runtime: {e}");
                return;
            }
        };

        rt.block_on(async move {
            let socket = match tokio_tungstenite::connect_async(url).await {
                Ok((socket, _)) => socket,
                Err(e) => {
                    eprintln!("websocket error: {e}");
                    return;
                }
            };
            let (mut sink, mut stream) = socket.split();
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let _ = TX.set(tx);

            set_connected(true);
            outgoing().lock().unwrap().push_back(ClientMsg::Hello { name: player_name() });

            loop {
                tokio::select! {
                    frame = rx.recv() => {
                        let Some(frame) = frame else { break };
                        if sink.send(Message::text(frame)).await.is_err() {
                            break;
                        }
                    }
                    msg = stream.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(parsed) = serde_json::from_str::<ServerMsg>(text.as_str()) {
                                    incoming().lock().unwrap().push_back(parsed);
                                }
                            }
                            Some(Ok(_)) => {}
                            Some(Err(e)) => {
                                eprintln!("websocket: {e}");
                                break;
                            }
                            None => break,
                        }
                    }
                }
            }
            set_connected(false);
        });
    });
}

#[cfg(target_arch = "wasm32")]
fn send(text: &str) {
    SOCK.with(|s| {
        if let Some(ws) = &*s.borrow() {
            let _ = ws.send_with_str(text);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn send(text: &str) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(text.to_string());
    }
}

pub fn send_chat(text: &str) {
    outgoing().lock().unwrap().push_back(ClientMsg::Chat {
        name: player_name(),
        text: text.to_string(),
    });
}

pub fn flush(_game: Res<Game>) {
    let q = outgoing();
    let mut drained = Vec::new();
    if let Ok(mut q) = q.lock() {
        while let Some(msg) = q.pop_front() {
            drained.push(msg);
        }
    }
    for msg in drained {
        if let Ok(text) = serde_json::to_string(&msg) {
            send(&text);
        }
    }
}

pub fn poll(
    mut game: ResMut<Game>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut transforms: Query<&mut Transform>,
) {
    let q = incoming();
    let mut drained = Vec::new();
    if let Ok(mut q) = q.lock() {
        while let Some(msg) = q.pop_front() {
            drained.push(msg);
        }
    }

    if drained.is_empty() {
        return;
    }
    game.dirty_hud = true;

    for msg in drained {
        match msg {
            ServerMsg::Welcome {
                id,
                name,
                seed,
                spawn,
                players,
            } => {
                game.my_id = id;
                game.name = name.clone();
                hud::log(&format!("connected as {name} seed {seed} spawn {:.0} {:.0} {:.0}", spawn[0], spawn[1], spawn[2]));
                game.roster.insert(id, name);
                game.world = World::generate(seed);
                game.dirty = (0..crate::all_chunks()).collect();
                game.local.feet = Vec3::new(spawn[0], spawn[1], spawn[2]);
                game.local.pitch = debug_pitch();
                for p in players {
                    game.roster.insert(p.id, p.name.clone());
                    if p.id != game.my_id {
                        spawn_avatar(&mut game, &mut commands, &mut materials, &p);
                    }
                }
            }
            ServerMsg::Spawn { player } => {
                hud::log(&format!("{} joined", player.name));
                game.roster.insert(player.id, player.name.clone());
                if player.id != game.my_id {
                    spawn_avatar(&mut game, &mut commands, &mut materials, &player);
                }
            }
            ServerMsg::Despawn { id, name } => {
                game.roster.remove(&id);
                if let Some(entity) = game.remote.remove(&id) {
                    commands.entity(entity).despawn();
                }
                hud::log(&format!("{name} left"));
            }
            ServerMsg::Move { id, pos, yaw, pitch } => {
                if id == game.my_id {
                    continue;
                }
                if let Some(entity) = game.remote.get(&id) {
                    if let Ok(mut t) = transforms.get_mut(*entity) {
                        t.translation = Vec3::new(pos[0], pos[1] + 0.9, pos[2]);
                        t.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
                    }
                }
            }
            ServerMsg::Block { pos, block: b } => {
                hud::log(&format!("block {} {} {} -> {}", pos[0], pos[1], pos[2], b));
                if game.world.set(pos[0], pos[1], pos[2], b) {
                    mark_dirty(&mut game, pos[0], pos[1], pos[2]);
                }
            }
            ServerMsg::Chat { name, text } => hud::log(&format!("{name}: {text}")),
            ServerMsg::Error { message } => hud::log(&format!("server: {message}")),
        }
    }
}

fn spawn_avatar(
    game: &mut Game,
    commands: &mut Commands,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    p: &PlayerInfo,
) {
    let h = voxel_common::hash2(p.id as i64, 7, 0xB16B00B5);
    let color = Color::srgb(
        0.35 + (h % 256) as f32 / 400.0,
        0.35 + ((h >> 8) % 256) as f32 / 400.0,
        0.35 + ((h >> 16) % 256) as f32 / 400.0,
    );
    let entity = commands
        .spawn((
            Mesh3d(game.cube.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                ..default()
            })),
            Transform::from_xyz(p.pos[0], p.pos[1] + 0.9, p.pos[2]),
        ))
        .id();
    game.remote.insert(p.id, entity);
}
