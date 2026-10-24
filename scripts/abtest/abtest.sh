#!/usr/bin/env bash
# Head-to-head test: abtest.sh NAME "SEEDS" TESTED_BOT "TESTED_EDITS" OPP_BOT "OPP_EDITS" [MAPS] [both]
#   abtest.sh SE_ON "1 2 3 4" bot-v7 "" bot-v6 ""            # 4 seeds x 40 maps = 160 games
#   abtest.sh SE_OFF "1 2 3 4" bot-v7 "SEOFF" bot-v6 ""      # same, with a switch flipped
#   abtest.sh BOSS "1 2 3 4 5 6 7 8" bot-v7 "" bot-v6 "" "01boss-battle" both
# For each seed k it builds t<NAME>-k<k> (tested, seed k) and o<NAME>-k<k> (opponent, seed k+500)
# from bots/<bot> with the named edits (variables in edits.sh), plays one game per map with sides
# alternating map to map (or both sides of every map with "both"), then prints econ.py's totals.
# Matches are deterministic: more seeds and maps are the only way to a bigger sample (a 40-game
# run is about +-5 wins of noise). Output: replays/abtest/<NAME>/ (*.json summaries, *.repl26a).
set -u
S=$(cd "$(dirname "$0")" && pwd)
R=$(git -C "$S" rev-parse --show-toplevel)
. "$S/maps40.sh"
. "$S/edits.sh"
name=$1; seeds=$2; tsrc=$3; tedits=$4; osrc=$5; oedits=$6; maps=${7:-$MAPS40}; sides=${8:-alternate}
JOBS=${JOBS:-8}
expand() { local out=(); for e in $1; do out+=("${!e}"); done; [ ${#out[@]} -eq 0 ] || printf '%s\n' "${out[@]}"; }
out="$R/replays/abtest/$name"
[ -e "$out" ] && { echo "batch $name exists (delete $out to rerun)" >&2; exit 1; }
mkdir -p "$out"; manifest="$out.txt"; : > "$manifest"
cd "$R"
for k in $seeds; do
  mapfile -t te < <(expand "$tedits"); mapfile -t oe < <(expand "$oedits")
  "$S/copybot.sh" "$tsrc" "t$name-k$k" "$k" "${te[@]}" | grep -v "^built" || true
  "$S/copybot.sh" "$osrc" "o$name-k$k" "$((k+500))" "${oe[@]}" | grep -v "^built" || true
  n=0
  for m in $maps; do
    t="$R/bots/t$name-k$k"; o="$R/bots/o$name-k$k"; map="$R/maps/$m.map26a"
    if [ "$sides" = both ] || [ $((n % 2)) -eq 0 ]; then
      echo "\"$t\" \"$o\" \"$map\" \"$out/$m-$k-A.repl26a\" --statistics \"$out/$m-$k-A.json\" --json" >> "$manifest"
    fi
    if [ "$sides" = both ] || [ $((n % 2)) -eq 1 ]; then
      echo "\"$o\" \"$t\" \"$map\" \"$out/$m-$k-B.repl26a\" --statistics \"$out/$m-$k-B.json\" --json" >> "$manifest"
    fi
    n=$((n + 1))
  done
done
cambc run-batch "$manifest" --jobs "$JOBS" --log-limit 50000000 > "$out.log" 2>&1
for k in $seeds; do rm -rf "$R/bots/t$name-k$k" "$R/bots/o$name-k$k"; done
echo "== $name: $tsrc[$tedits] vs $osrc[$oedits]"
python -I "$S/econ.py" "$out" | head -3
