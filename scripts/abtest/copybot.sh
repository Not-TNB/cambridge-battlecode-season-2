#!/usr/bin/env bash
# Seeded copy of a bot with switch edits: copybot.sh <source bot dir> <copy dir> <seed k> [FILE=SEDEXPR ...]
# The copy's SEED (src/main.rs) is the original xor k*7919 (k=0 keeps it), so each seed gives the
# bot different random choices. Each FILE=SEDEXPR is applied to src/FILE of the copy and must change
# it (a typo in a switch name fails loudly instead of testing nothing).
set -eu
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
cd "$R"
name=$2; src=bots/$1; d=bots/$2; k=$3; shift 3
rm -rf "$d"; mkdir "$d"
cp -r "$src"/{.cargo,.gitignore,Cargo.lock,Cargo.toml,rust-toolchain.toml,src} "$d"/
# cambc refuses Windows line endings in the files it manages (they must match its template).
sed -i 's/\r$//' "$d"/Cargo.toml "$d"/.cargo/config.toml "$d"/rust-toolchain.toml "$d"/.gitignore
if [ "$k" != 0 ]; then
  seed=$(grep -o 'const SEED: u64 = 0x[0-9a-f]*;' "$d/src/main.rs" | grep -o '0x[0-9a-f]*')
  sed -i "s/const SEED: u64 = $seed;/const SEED: u64 = $seed ^ $((k * 7919));/" "$d/src/main.rs"
  grep -q "\^ $((k * 7919))" "$d/src/main.rs" || { echo "seed not applied in $d" >&2; exit 1; }
fi
for pair in "$@"; do
  file=${pair%%=*}; expr=${pair#*=}
  before=$(md5sum "$d/src/$file")
  sed -i "$expr" "$d/src/$file"
  [ "$before" != "$(md5sum "$d/src/$file")" ] || { echo "no change from $expr in $file" >&2; exit 1; }
done
cambc build "$d" 2>&1 | grep -E "^error" -A6 || true
echo "built $name (k=$k, $# edits)"
