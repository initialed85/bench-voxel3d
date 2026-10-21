//! Authoritative voxel server.
//!
//! Holds the canonical world, hands out spawns, and relays edits, movement and
//! chat. Each connection runs as its own task behind a shared state lock.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use voxel_common::{block, ClientMsg, PlayerInfo, ServerMsg, World, SEED, WORLD_X, WORLD_Y, WORLD_Z};

const DEFAULT_PORT: u16 = 3001;

/// The browser client and the WebSocket sim share this one port.
const MIME: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("mjs", "text/javascript; charset=utf-8"),
    ("wasm", "application/wasm"),
    ("css", "text/css; charset=utf-8"),
    ("json", "application/json"),
    ("png", "image/png"),
    ("svg", "image/svg+xml"),
    ("ico", "image/x-icon"),
];

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

fn send_all(state: &State, msg: &ServerMsg) {
    let text = serde_json::to_string(msg).unwrap();
    for client in state.clients.values() {
        let _ = client.tx.send(Message::text(text.clone()));
    }
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
    let port: u16 = std::env::var("VOXEL_PORT")
        .or_else(|_| std::env::var("PORT"))
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let web_dir = Arc::new(std::env::var("VOXEL_WEB").unwrap_or_else(|_| "web".to_string()));

    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    println!("voxel server serving {} on http://0.0.0.0:{port} (ws at /ws)", web_dir);
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
        tokio::spawn(handle(stream, addr, state.clone(), web_dir.clone()));
    }
}

async fn handle(
    mut stream: tokio::net::TcpStream,
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    web_dir: Arc<String>,
) {
    // One port, two jobs: peek the request head and only hand the socket to
    // accept_async when it is a WebSocket upgrade. Anything else is a file from
    // web/, served until the client closes or goes quiet.
    loop {
        let mut buf = [0u8; 4096];
        let got = match tokio::time::timeout(Duration::from_secs(30), stream.peek(&mut buf)).await {
            Ok(Ok(n)) => n,
            _ => return,
        };
        if got == 0 {
            return;
        }
        let head = String::from_utf8_lossy(&buf[..got]).to_string();
        let lower = head.to_lowercase();
        let request_line = head.lines().next().unwrap_or("").trim().to_string();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("GET").to_string();
        let path = parts.next().unwrap_or("/").to_string();
        let route = path.split('?').next().unwrap_or("/").to_string();
        let header_lines: Vec<&str> = lower.lines().skip(1).map(|l| l.trim()).collect();
        let upgrading = header_lines.iter().any(|l| l.starts_with("upgrade:"))
            && header_lines.iter().any(|l| l.contains("websocket"));

        if route == "/ws" || route.starts_with("/ws?") {
            if !upgrading {
                return;
            }
            run_ws(stream, addr, state).await;
            return;
        }

        if method != "GET" && method != "HEAD" {
            respond(&mut stream, 405, "text/plain; charset=utf-8", b"method not allowed", "", true)
                .await
                .ok();
            return;
        }

        // Consume the head we peeked, otherwise the next request on this
        // connection is read as garbage and the browser gets a reset.
        let end = head.find("\r\n\r\n").map(|i| i + 4).unwrap_or(head.len());
        let mut consumed = vec![0u8; end];
        if stream.read_exact(&mut consumed).await.is_err() {
            return;
        }

        let gzip = header_lines
            .iter()
            .any(|l| l.starts_with("accept-encoding:") && l.contains("gzip"));
        let head_only = method == "HEAD";
        if let Err(e) = serve_file(&mut stream, &web_dir, &path, gzip, head_only).await {
            eprintln!("http {addr}: {e}");
            return;
        }

        if header_lines
            .iter()
            .any(|l| l.starts_with("connection:") && l.contains("close"))
        {
            return;
        }
    }
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    MIME.iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, m)| *m)
        .unwrap_or("application/octet-stream")
}

fn static_path(root: &str, request_path: &str) -> Option<PathBuf> {
    let path = request_path
        .split('?')
        .next()
        .unwrap_or("/")
        .split('#')
        .next()
        .unwrap_or("/");
    let rel = if path == "/" || path.is_empty() {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };
    let candidate = Path::new(rel);
    if candidate.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::RootDir
        )
    }) {
        return None;
    }
    Some(Path::new(root).join(candidate))
}

async fn serve_file(
    stream: &mut tokio::net::TcpStream,
    root: &str,
    request_path: &str,
    accept_gzip: bool,
    head_only: bool,
) -> std::io::Result<()> {
    let file = match static_path(root, request_path) {
        Some(p) => p,
        None => {
            return respond(stream, 400, "text/plain; charset=utf-8", b"bad request", "", head_only).await;
        }
    };

    // scripts/build.sh leaves a .gz beside the wasm blob; hand that over when the
    // browser will take it, because the raw bundle is huge.
    if accept_gzip {
        let mut gz = file.clone();
        gz.set_extension(format!(
            "{}.gz",
            file.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        if let Ok(body) = std::fs::read(&gz) {
            return respond(
                stream,
                200,
                mime_for(&file.to_string_lossy()),
                &body,
                "content-encoding: gzip\r\ncache-control: no-store\r\n",
                head_only,
            )
            .await;
        }
    }

    match std::fs::read(&file) {
        Ok(body) => {
            respond(
                stream,
                200,
                mime_for(&file.to_string_lossy()),
                &body,
                "cache-control: no-store\r\n",
                head_only,
            )
            .await
        }
        Err(e) => {
            let msg = format!("not found: {request_path} ({e})");
            respond(stream, 404, "text/plain; charset=utf-8", msg.as_bytes(), "", head_only).await
        }
    }
}

async fn respond(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    extra_headers: &str,
    head_only: bool,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n{extra_headers}connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    if !head_only {
        stream.write_all(body).await?;
    }
    stream.flush().await
}

async fn run_ws(stream: tokio::net::TcpStream, addr: SocketAddr, state: Arc<Mutex<State>>) {
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
            // The name is server-assigned (player-{id}); a client-supplied name in Hello
            // or Chat must not overwrite it, otherwise joiners disagree on each other's
            // display name.
            ClientMsg::Hello { .. } => {}
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
            ClientMsg::Chat { text, .. } => {
                // Everyone, including the sender, so the sender sees their own message.
                let out = ServerMsg::Chat {
                    name: state.lock().unwrap().clients[&id].name.clone(),
                    text,
                };
                send_all(&state.lock().unwrap(), &out);
            }
        }
    }

    println!("- #{id}");
    state.lock().unwrap().clients.remove(&id);
}
