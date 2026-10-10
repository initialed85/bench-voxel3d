//! Authoritative voxel server.
//!
//! Holds the canonical world, hands out spawns, and relays edits, movement and
//! chat. Each connection runs as its own task behind a shared state lock.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use voxel_common::{block, ClientMsg, PlayerInfo, ServerMsg, World, SEED, WORLD_X, WORLD_Y, WORLD_Z};

const PORT: u16 = 3001;

#[derive(Clone)]
struct Client {
    id: u64,
    name: String,
    pos: [f32; 3],
    yaw: f32,
    pitch: f32,
    tx: tokio::sync::mpsc::UnboundedSender<Message>,
}

struct State {
    world: World,
    clients: HashMap<u64, Client>,
    next_id: u64,
}

fn send(state: &State, msg: &ServerMsg, skip: u64) {
    let text = serde_json::to_string(msg).unwrap();
    for client in state.clients.values() {
        if client.id == skip {
            continue;
        }
        let _ = client.tx.send(Message::text(text.clone()));
    }
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind(("0.0.0.0", PORT)).await.unwrap();
    println!("voxel server listening on :{PORT}");
    println!(
        "world {}x{}x{} seed {SEED:#x}",
        WORLD_X, WORLD_Y, WORLD_Z
    );

    let state = Arc::new(Mutex::new(State {
        world: World::generate(SEED),
        clients: HashMap::new(),
        next_id: 1,
    }));

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle(stream, addr, state.clone()));
    }
}

async fn handle(stream: tokio::net::TcpStream, addr: SocketAddr, state: Arc<Mutex<State>>) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("websocket handshake failed for {addr}: {e}");
            return;
        }
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let (mut sink, mut stream) = ws.split();

    tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if sink.send(frame).await.is_err() {
                break;
            }
        }
    });

    let (id, name, spawn, roster) = {
        let mut s = state.lock().unwrap();
        let id = s.next_id;
        s.next_id += 1;
        let spawn = voxel_common::spawn_for(id);
        s.clients.insert(
            id,
            Client {
                id,
                name: format!("player-{id}"),
                pos: spawn,
                yaw: 0.0,
                pitch: 0.0,
                tx: tx.clone(),
            },
        );
        let roster = s
            .clients
            .values()
            .map(|c| PlayerInfo {
                id: c.id,
                name: c.name.clone(),
                pos: c.pos,
                yaw: c.yaw,
                pitch: c.pitch,
            })
            .collect();
        (id, format!("player-{id}"), spawn, roster)
    };

    println!("+ {addr} as #{id}");

    let _ = tx.send(Message::text(
        serde_json::to_string(&ServerMsg::Welcome {
            id,
            name: name.clone(),
            seed: SEED,
            spawn,
            players: roster,
        })
        .unwrap(),
    ));

    send(
        &state.lock().unwrap(),
        &ServerMsg::Spawn {
            player: PlayerInfo {
                id,
                name: name.clone(),
                pos: spawn,
                yaw: 0.0,
                pitch: 0.0,
            },
        },
        id,
    );

    while let Some(msg) = stream.next().await {
        let text = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => {
                eprintln!("#{id}: {e}");
                break;
            }
        };

        let parsed: ClientMsg = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(Message::text(
                    serde_json::to_string(&ServerMsg::Error {
                        message: format!("bad message: {e}"),
                    })
                    .unwrap(),
                ));
                continue;
            }
        };

        match parsed {
            ClientMsg::Hello { name } => {
                state.lock().unwrap().clients.get_mut(&id).unwrap().name = name;
            }
            ClientMsg::Move { pos, yaw, pitch } => {
                state.lock().unwrap().clients.get_mut(&id).unwrap().pos = pos;
                let out = ServerMsg::Move { id, pos, yaw, pitch };
                send(&state.lock().unwrap(), &out, id);
            }
            ClientMsg::Edit { pos, block: b } => {
                if !state.lock().unwrap().world.in_bounds(pos[0], pos[1], pos[2]) {
                    continue;
                }
                if b > block::SNOW {
                    let _ = tx.send(Message::text(
                        serde_json::to_string(&ServerMsg::Error {
                            message: format!("unknown block {b}"),
                        })
                        .unwrap(),
                    ));
                    continue;
                }
                let out = ServerMsg::Block { pos, block: b };
                send(&state.lock().unwrap(), &out, id);
            }
            ClientMsg::Chat { name, text } => {
                state.lock().unwrap().clients.get_mut(&id).unwrap().name = name;
                let out = ServerMsg::Chat {
                    name: state.lock().unwrap().clients[&id].name.clone(),
                    text,
                };
                send(&state.lock().unwrap(), &out, id);
            }
        }
    }

    println!("- #{id}");
    state.lock().unwrap().clients.remove(&id);
}
