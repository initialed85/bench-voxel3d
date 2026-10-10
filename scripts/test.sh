#!/usr/bin/env bash
# Canonical "ready to commit" check: unit tests, then both client targets.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo test -p voxel-common -p voxel-client
cargo build -p voxel-server
./scripts/build.sh

echo "ok"
