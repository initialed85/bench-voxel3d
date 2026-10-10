# bench-voxel3d

Multiplayer 3D voxel first-person shooter. Rust + Bevy on the client, WebSockets to an
authoritative Tokio server, and the same client compiled to WASM for the browser.

## Layout

| Path | What it is |
| --- | --- |
| `common/` | Protocol messages, deterministic world generation, chunk indexing. Shared by server and client. |
| `server/` | Authoritative WebSocket server (Tokio + tokio-tungstenite). |
| `client/` | Bevy renderer, player physics, meshing, raycast, HUD. `cdylib` for WASM, binary for native. |
| `web/` | `index.html`, `style.css`, and the generated `voxel_client.js` + `voxel_client_bg.wasm`. |
| `scripts/` | `test.sh`, `build.sh`, `serve.sh`, `dev.sh`. |

## Prerequisites

- `rust-toolchain.toml` pins Rust 1.97.1.
- `rustup target add wasm32-unknown-unknown`
- `wasm-bindgen-cli` 0.2.129 on the PATH (`cargo install wasm-bindgen-cli --version 0.2.129`)
- `entr` for the rebuild-on-change loop (`apt install entr`)
- `python3` for the static file server

## Commands

**Ready-to-commit check** — unit tests, then both client targets:

```sh
./scripts/test.sh
```

Success: `test result: ok` for `voxel-common` (5 tests) and `voxel-client` (4 raycast
tests), `voxel-server` builds, `wrote web/voxel_client.js`, then `ok`.

**Run the server** (listens on `:3001`, world 64x48x64, seed `0x5eedc0de12345678`):

```sh
cargo run -p voxel-server
```

**Serve the browser client** (serves `web/` on `PORT`, default 8080):

```sh
./scripts/serve.sh
```

Open `http://localhost:8080`. The client connects to the host it was served from on
`:3001`. Use `?name=alice` to name yourself and `?pitch=-1.2` to start looking down
(debug aid).

**Native client** (windowed build, connects to `ws://localhost:3001/ws`; override with
`VOXEL_WS`, name with `VOXEL_NAME`). Needs a display: `DISPLAY` or `WAYLAND_DISPLAY`
must be set, otherwise winit fails at startup.

```sh
cargo run -p voxel-client --bin voxel-client
```

**Rebuild the WASM bundle on every source change** (run alongside `serve.sh`, then
refresh the page):

```sh
./scripts/dev.sh
```

## Controls

Click to grab the mouse, then: WASD move, Space jump, Shift sprint, 1-6 pick a block,
left click break, right click place, arrow keys look (useful without a mouse). Escape
releases the mouse so the chat box can be clicked; Enter in the box sends the message.

## How it works

- The world is generated deterministically from the seed, so the initial transfer is a
  seed, not a block array. Edits are broadcast as single cells.
- Meshing writes vertex colours for shading and ambient occlusion into one mesh per
  chunk, drawn with a single white material.
- The server owns the world and player positions; clients send input and render the
  authoritative state.

## Notes

- Bevy is pinned to 0.19.1: 0.20.0's default features reference `bevy_gilrs`, which is
  not published, so native builds fail.
- `bevy` is a top-level dependency in `client/Cargo.toml` (not only target-specific),
  because Bevy's derive macros resolve their crate paths from the package manifest.
- `web/voxel_client.js` and `voxel_client_bg.wasm` are generated output; do not edit.
- `window.send_chat(text)` is exposed in the browser console as a debug aid.
- The WASM canvas is sized from the browser viewport (CSS px x device pixel ratio) at
  startup and on every frame, so the DOM crosshair sits at the rendered centre. Bevy's
  default 1280x720 does not match a typical viewport.
- Headless browser testing renders through SwiftShader and runs at well under 1 fps, so
  input-driven tests need generous waits. Pointer lock blocks DOM clicks, so drive the
  chat box with `send_chat` rather than clicking it. Headless pointer-lock `movementX`
  is reported with the opposite sign to `mouse.move`, so mouse-look direction cannot be
  verified headlessly; it is pinned by unit tests in `client/src/player.rs`.
