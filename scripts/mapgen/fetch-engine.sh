#!/bin/sh
# Download the map editor's engine wasm (used by mapgen.mjs) from a local `cambc mapeditor`.
# Usage: scripts/mapgen/fetch-engine.sh
set -eu
cd "$(dirname "$0")"
log=$(mktemp)
cambc mapeditor --no-browser > "$log" 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null; rm -f "$log"' EXIT
for _ in 1 2 3 4 5 6 7 8 9 10; do grep -q http "$log" && break; sleep 1; done
base=$(grep -o 'http://[0-9.:]*' "$log" | head -1)
js=$(curl -s "$base/?cambc-cli=1" | grep -o 'assets/index-[^"]*\.js' | head -1)
wasm=$(curl -s "$base/$js" | grep -o 'v[0-9_]*-[A-Za-z0-9_-]*\.wasm' | head -1)
curl -s "$base/assets/$wasm" -o engine.wasm
echo "saved engine.wasm ($wasm)"
