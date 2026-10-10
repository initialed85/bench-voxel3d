#!/usr/bin/env bash
# Serve web/ for local testing.
set -euo pipefail
cd "$(dirname "$0")/.."

PORT="${PORT:-8080}"
echo "serving web/ on http://localhost:${PORT}"
python3 -m http.server "$PORT" --directory web
