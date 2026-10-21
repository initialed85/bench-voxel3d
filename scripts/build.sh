#!/usr/bin/env bash
# Compile the client for wasm and emit the JS glue into web/.
# Default is the optimized build; pass --debug for a fast iteration build.
# wasm-opt (binaryen) shrinks the release bundle further; install it from
# https://github.com/WebAssembly/binaryen/releases if scripts/wasmopt is missing.
set -euo pipefail
cd "$(dirname "$0")/.."

profile=release
args=(--release)
if [[ "${1:-}" == "--debug" ]]; then
    profile=debug
    args=()
fi

cargo build --target wasm32-unknown-unknown -p voxel-client "${args[@]}"

wasm-bindgen --target web --out-dir web "target/wasm32-unknown-unknown/$profile/voxel_client.wasm"

if [[ "$profile" == release ]] && command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-bulk-memory --enable-sign-ext --enable-simd \
        --enable-nontrapping-float-to-int --enable-mutable-globals \
        web/voxel_client_bg.wasm -o web/voxel_client_bg.wasm
    echo "wrote web/voxel_client_bg.wasm ($(du -h web/voxel_client_bg.wasm | cut -f1))"
fi
gzip -9 -k -f web/voxel_client_bg.wasm
echo "wrote web/voxel_client.js"
