#!/bin/sh
# Play one match between two bots with fresh random seeds, on a random map.
# Usage: scripts/randmatch.sh <bot A> <bot B> [--map NAME] [--trace on|off] [other `cambc run` options, e.g. --view]
#
#   --trace off  turns the bots' debug prints off (TRACE and the farm-target diagnostics in
#                src/common/trace.rs, or src/common.rs for the Tavi squad); --trace on turns the
#                per-turn TRACE lines on. Without it the bots print whatever their source says.
#
# Each bot is copied to bots/<bot>-r<seed>, its SEED constant (main.rs, see scripts/reseed.sh)
# replaced in the copy, and the copy run, so the originals stay untouched. The copies sit under
# bots/ because Cargo.toml finds the SDK at ../../sdk/rust; they're deleted afterwards. Bots
# without a SEED constant run as they are. The map is picked from maps/ unless --map is given.
# The full seeds are printed, so a game can be replayed by putting them back in main.rs.
set -eu
cd "$(dirname "$0")/.."
[ $# -ge 2 ] || { echo "usage: $0 <bot A> <bot B> [--map NAME] [--trace on|off] [cambc run options]" >&2; exit 2; }
bot_a=$1
bot_b=$2
shift 2

map=""
trace=""
rest=""
while [ $# -gt 0 ]; do
  case $1 in
    --map) map=$2; shift 2 ;;
    --map=*) map=${1#--map=}; shift ;;
    --trace) trace=$2; shift 2 ;;
    --trace=*) trace=${1#--trace=}; shift ;;
    *) rest="$rest $1"; shift ;;
  esac
done
case $trace in ""|on|off) ;; *) echo "--trace takes on or off" >&2; exit 2 ;; esac
if [ -z "$map" ]; then
  map=$(ls maps/*.map26a | sed 's#maps/##; s#\.map26a$##' |
    awk -v seed="$(od -An -N4 -tu4 /dev/urandom)" 'BEGIN { srand(seed) } { m[NR] = $0 } END { print m[int(rand() * NR) + 1] }')
fi

copies=""
cleanup() {
  for dir in $copies; do
    rm -rf "bots/${dir:?}"
  done
}
trap cleanup EXIT INT TERM

# Copy bot $1 with a random seed (and the --trace setting); prints the name to run. A bot with no
# SEED constant is only copied when --trace asks for a change.
seeded() {
  bot=$1
  [ -d "bots/$bot" ] || { echo "no bot at bots/$bot" >&2; exit 1; }
  has_seed=no
  grep -q 'const SEED: u64 = 0x' "bots/$bot/src/main.rs" 2>/dev/null && has_seed=yes
  has_trace=no
  trace_file=""
  for candidate in src/common/trace.rs src/common.rs; do
    if [ -z "$trace_file" ] && grep -q '^pub const TRACE: bool' "bots/$bot/$candidate" 2>/dev/null; then
      trace_file=$candidate
    fi
  done
  [ -n "$trace" ] && [ -n "$trace_file" ] && has_trace=yes
  if [ $has_seed = no ] && [ $has_trace = no ]; then
    echo "$bot"
    return
  fi
  seed=$(od -An -N8 -tx8 /dev/urandom | tr -d ' 
')
  copy="$bot-r$(printf %s "$seed" | cut -c1-6)"
  mkdir "bots/$copy"
  for item in .cargo .gitignore Cargo.lock Cargo.toml rust-toolchain.toml src; do
    [ -e "bots/$bot/$item" ] && cp -r "bots/$bot/$item" "bots/$copy/"
  done
  if [ $has_seed = yes ]; then
    sed -i.bak -E "s/const SEED: u64 = 0x[0-9a-fA-F_]+;/const SEED: u64 = 0x$seed;/" "bots/$copy/src/main.rs"
    rm -f "bots/$copy/src/main.rs.bak"
    echo "$bot seed: 0x$seed" >&2
  fi
  if [ $has_trace = yes ]; then
    file="bots/$copy/$trace_file"
    if [ "$trace" = off ]; then
      sed -i.bak -E 's/^(pub const TRACE: bool = )true;/\1false;/; s/^((pub(\(super\))? )?const FARM_TARGET_DIAGNOSTICS: bool = )true;/\1false;/' "$file"
    else
      sed -i.bak -E 's/^(pub const TRACE: bool = )false;/\1true;/' "$file"
    fi
    rm -f "$file.bak"
  fi
  echo "$copy"
}

run_a=$(seeded "$bot_a")
case $run_a in "$bot_a-r"*) copies="$copies $run_a" ;; esac
run_b=$(seeded "$bot_b")
case $run_b in "$bot_b-r"*) copies="$copies $run_b" ;; esac

echo "map: $map"
echo "team A: $run_a"
echo "team B: $run_b"
[ -n "$trace" ] && echo "debug prints: $trace"
# shellcheck disable=SC2086 # $rest is the extra options, split on purpose
cambc run "bots/$run_a" "bots/$run_b" --map "$map" $rest
