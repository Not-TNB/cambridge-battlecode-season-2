# abtest: head-to-head bot tests

Scripts used to test bot-v5 to bot-v7 (2026-10-22/23). They run from any checkout: paths come from `git rev-parse`, and output goes to `replays/abtest/<NAME>/`, which git ignores.

| File | What it does |
|---|---|
| `abtest.sh` | `abtest.sh NAME "SEEDS" TESTED_BOT "EDITS" OPP_BOT "EDITS" [MAPS] [both]`: builds a seeded copy of each bot per seed, plays one game per map with sides alternating (or `both` sides), then prints `econ.py` totals |
| `copybot.sh` | Seeded copy of a bot with switch edits applied (used by `abtest.sh`) |
| `edits.sh` | Named switch edits for bot-v7, each flipping one default (`SEOFF`, `GGOFF`, `STALE5`, `TRACE`, ...) |
| `maps40.sh` | The 40-map pool (`MAPS40`) |
| `econ.py` | Totals per side: wins, gold a round, gems collected/tributed/lost, clears, deaths, kills, fuel-exhausted turns per 1,000 rounds (`--per-map` too) |
| `bossstats.py` | 01boss-battle detail per game: tributes, first tribute, monolith clears, captures, camp deaths, gems lost |
| `capsize.py` | Captures by gem size (0–10, 10–20, 20–40, 40+) per side, by round 400 and whole game |

Examples:

```sh
scripts/abtest/abtest.sh V7_V6 "1 2 3 4" bot-v7 "" bot-v6 ""                   # 160 games
scripts/abtest/abtest.sh NO_GUARD "1 2 3 4 5 6 7 8" bot-v7 "GGOFF" bot-v5 "" "01boss-battle" both
python -I scripts/abtest/econ.py replays/abtest/V7_V6 --per-map
python -I scripts/abtest/bossstats.py replays/abtest/NO_GUARD
```

Notes:
- **Determinism:** the same bots, map, sides and seed always give the same game. More seeds and maps are the only way to a bigger sample.
- **Noise:** a 40-game run is about ±5 wins and a 160-game run about ±6 (one standard deviation is √(n/4)).
- **01boss-battle** swings far more than other maps (the same bot went 4/8 and 7/8). Use 16+ games.
- **Speed:** a 160-game run takes about 5–10 minutes with 8 jobs (`JOBS=8`). Boss-battle runs take longer, because games last all 2,000 rounds.
- **Name clashes:** a batch name that already exists aborts the run. Delete `replays/abtest/<NAME>` to rerun.
- **Line endings:** `copybot.sh` strips Windows line endings from `Cargo.toml` and the other template files, because cambc refuses a bot whose managed files differ from its template.
