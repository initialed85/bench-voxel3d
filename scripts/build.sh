#!/usr/bin/env bash
# Compile the client for wasm and emit the JS glue into web/.
# Usage: scripts/build.sh [--release]
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --target wasm32-unknown-unknown -p voxel-client "$@"

profile=debug
for a in "$@"; do
    [[ "$a" == "--release" ]] && profile=release
done

wasm-bindgen --target web --out-dir web "target/wasm32-unknown-unknown/$profile/voxel_client.wasm"
echo "wrote web/voxel_client.js"
