# bench-voxel3d

Running [Niko1221/Strata](https://github.com/Niko1221/Strata)

```shell
./Strata/engine/strata --serve --pack ./Strata-data/packs/swift-iq2_xs --native ./Strata-data/models/swift-IQ2_XS/Swift-Qwen3.8-Flash-Next-GSQ-RCO-IQ2_XS-00001-of-00002.gguf --ple-gguf ./Strata-data/models/swift-IQ2_XS/Swift-Qwen3.8-Flash-Next-GSQ-RCO-IQ2_XS-00001-of-00002.gguf --expert-profile ./Strata/data/expert-profile.bin --expert-cache auto --prefill auto --spec 4 --spec-min-p 0.5 --mtp ./Strata-data/mtp/rt --max-context 131072 --kv int8
```

- GPU: AMD Radeon RX 9070 XT, 16 GB
- CPU: AMD Ryzen 5 5600X 6-Core Processor, 12 threads
- RAM: 48GB
  - 2 x 16GB 2133 MT/s (Corsair)
  - 2 x 8GB 2133 MT/s (GEIL)
- SSD: EDILOCA EN680E 1TB NVMe

## Prompts

```
hi mate

can you whip me up a quick rust bevy wasm websocket multiplayer 3d voxel-based first-person shooter?
```

---

```
nice, nice- any chance you can re-run it on 0.0.0.0 so i can sniff it from an external machine? as you've pointed out, no display at the moment locally
```

---

```
just been testing it out from another machine- it's working fairly well off the bat

some things to fix: horizontal mouse movements are inverted (vertical is good) and the blocks are too high to jump on to
```

---

```
correction, blocks are not too high to jump on to- they're goo
```

---

```
crosshair isn't central btw; it's like a quarter to bottom right
```

---

```
i've exposed it to the internet via my kube cluster- thing is its trying to connect to wss://voxel3d.dev.initialed85.cc:3001/ws; can we have some logic that's like "if protocol is https, use the port from the origin, don't use 3001"
```

---

```
also, i'm finding the crosshair and the "hit zone" for breaking blocks don't agree- hit zone seems to be halfway between crosshair and bottom of the viewport
```

---

```
the bundle the web client downloads is huuuuuge; like 200+ MB; how can we get that down?
```

```
the bundle the web client downloads is huuuuuge; like 200+ MB; how can we get that down?
```

---

```
i've joined from two browser sessions on the same machine, but it's like \"who is in the game\" is not consistent; one has \"player-70 (you), player-71\" and the other has \"player, player-71 (you)\" but they don't seem to be able to physically see each other
```

---

```
belay that, i can in fact see the other player- just some oddities with the player list i think. also the chat doesn't seem to work
```

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
- `wasm-opt` (binaryen) to shrink the release bundle. `cargo install wasm-opt` fails to
  link on this box; grab the prebuilt tarball from the binaryen releases and put
  `wasm-opt` on the PATH. `build.sh` skips it if absent.
- `entr` for the rebuild-on-change loop (`apt install entr`)
- `python3` for the static file server

## Commands

**Ready-to-commit check** — unit tests, then both client targets:

```sh
./scripts/test.sh
```

Success: `test result: ok` for `voxel-common` (5 tests) and `voxel-client` (10 tests),
`voxel-server` builds, `wrote web/voxel_client_bg.wasm (18M)`, then `ok`.

**Run the server** (listens on `:3001`, world 64x48x64, seed `0x5eedc0de12345678`):

```sh
cargo run -p voxel-server
```

**Bundle size** — `build.sh` builds `--release` by default. Measured on this project:

| Build | wasm | gzipped |
| --- | --- | --- |
| debug | 218 MB | — |
| release, `opt-level = 3` | 30 MB | 7.5 MB |
| release, `opt-level = "z"` | 23 MB | 7.4 MB |
| release, `opt-level = "z"` + `wasm-opt -Oz` | 18 MB | 5.9 MB |

The code section is ~19 MB of Bevy; embedded strings are only ~1 MB, so trimming Bevy
features does little. Serve with `Content-Encoding: gzip` (or brotli) so the browser
transfers ~6 MB, not 18. `python3 -m http.server` does not compress; your ingress
should. Use `scripts/build.sh --debug` for a fast iteration build when size does not
matter.

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
left click break, right click place, arrow keys look (useful without a mouse). E breaks
and Q places without the mouse, so editing works before the first click. Escape
releases the mouse so the chat box can be clicked; Enter in the box sends the message.

A small magenta sphere marks the exact point the crosshair is aiming at, so the hit zone
is visible in the world.

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
- The voxel DDA returns the distance to the face it entered, so the aim marker sits on
  the crosshair ray. `t_max` for a negative axis is `origin - boundary`, not
  `boundary - origin`; getting this sign backwards yields a negative distance and puts
  the aim point behind the camera.
- The WASM canvas is sized from the browser viewport (CSS px x device pixel ratio) at
  startup and on every frame, so the DOM crosshair sits at the rendered centre. Bevy's
  default 1280x720 does not match a typical viewport.
- Headless browser testing renders through SwiftShader and runs at well under 1 fps, so
  input-driven tests need generous waits. Pointer lock blocks DOM clicks, so drive the
  chat box with `send_chat` rather than clicking it. Headless pointer-lock `movementX`
  is reported with the opposite sign to `mouse.move`, so mouse-look direction cannot be
  verified headlessly; it is pinned by unit tests in `client/src/player.rs`.
