#!/bin/sh
# Give each bot's random_seed() a fresh SEED constant. Usage: scripts/reseed.sh [bot ...] (default: all Rust bots)
set -eu
cd "$(dirname "$0")/.."
[ $# -eq 0 ] && set -- $(ls bots)
for bot in "$@"; do
  file="bots/$bot/src/main.rs"
  grep -q 'const SEED: u64 = 0x' "$file" 2>/dev/null || { echo "skip $bot: no SEED constant"; continue; }
  seed=$(od -An -N8 -tx8 /dev/urandom | tr -d ' \n')
  sed -i.bak -E "s/const SEED: u64 = 0x[0-9a-fA-F_]+;/const SEED: u64 = 0x$seed;/" "$file" && rm "$file.bak"
  echo "$bot: SEED = 0x$seed"
done
