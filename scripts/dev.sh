#!/usr/bin/env bash
# Rebuild the wasm bundle whenever client or common sources change.
# Requires entr. Run scripts/serve.sh in another shell.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v entr >/dev/null; then
    echo "entr is required: apt install entr" >&2
    exit 1
fi

find client/src common/src client/Cargo.toml common/Cargo.toml | entr -c sh -c 'scripts/build.sh'
