You're picking up work on our Cambridge Battlecode 2026 bot (team "Anime girls against #2 period cramp"), a Rust bot for the CAMBC engine. The repo is this one. Read these first:
- `docs/GameRules.md` (rules);
- `docs/BotAPIRust.md` (SDK);
- `docs/writeup.md` (entries 34–40 cover the latest work);
- `scripts/abtest/README.md` and `scripts/ladder/README.md` (test and analysis tools).

## State

- **Our bots:**
  - `bots/bot-v7` is the newest and not yet uploaded.
  - `bots/bot-v6` (Tristan: faster allocator and fuel savings, meta draft) is the base it was copied from.
  - `bots/bot-v5` is the older main bot.
- **Every behaviour is a `pub const` switch** with its test results in the comment above it. Keep that style: a new idea gets a switch, an A/B test, and its numbers written next to it.
- **Building:** `cambc build bots/bot-v7`.
  - cambc refuses a bot whose `Cargo.toml`, `.cargo/config.toml`, `rust-toolchain.toml` or `.gitignore` differ from its template. Windows line endings count as different; `sed -i 's/\r$//'` fixes it.
- **Testing:** `scripts/abtest/abtest.sh NAME "SEEDS" TESTED "EDITS" OPP "EDITS" [MAPS] [both]`, with named switch flips in `scripts/abtest/edits.sh`.
  - Matches are deterministic, so sample size comes only from seeds × maps.
  - A 40-game run is about ±5 wins of noise; decide on 160-game runs (4 seeds × 40 maps, about 5–10 minutes with 8 jobs).
  - 01boss-battle swings far more: use 16+ games (8 seeds, `both` sides).
- **What decides top ladder games** (7,000 games, entry 34):
  - Early gold first: the team ahead at round 300 wins 74%, and 89% with a 50%+ lead. Then gems collected, emblems built, and captures worth 20+ gems.
  - Heart of Ruin is the best champion on the ladder. No AP beats one AP, 57% to 43%. Exactly one AD carry is best.

## v7 results so far

Against bot-v5 (160 games): v6 101/160 and v7 candidates 111–119/160. Those were partly selection effects: switch values were picked on the same seeds (981–984), and on fresh seeds v7 scored 98/160. **Head to head against bot-v6 is the number to trust: 344/640 (53.8%) over two fresh 320-game seed sets** (writeup entry 40). Lesson: pick on one seed set, confirm on another, and test head to head against the previous bot, not only against an older one.

Kept:
- `safety::SHIELDED_ESCAPE`
- `base::GATE_GUARD` and `GATE_QUEUE`: 01boss-battle 12/16 against 8/16
- `chase::ALONE_CHASE_MARGIN` 2
- `farm::RALLY_DECISION_ROUNDS` 15: timeouts −10%. `STALE_PLAN_ROUNDS` 5 cut timeouts 13% but lost head to head against v6 (76/160 against 97/160 at 3), so it's back at 3
- `gemcarry::GEM_CARRY` off
- `champions::heart_of_ruin::HEART_SHIELD_SPAM`: shield like Varg, 182/320 against v7 without it
- `duel::DUEL_SHIELDS`: ready shields count as health in duels, 168/320

Rejected: gate siege, team-fight bounty, stale-plan skip, longer chases.

## Worth investigating next (most promising first)

1. **Heart of Ruin plays badly in our code.** Shield spam (writeup entry 41) fixed part of it, but it still farms less than Marcella and Vendetta.
   - Before that fix it earned 1,371 gold a game, against 1,941 for Marcella and 1,823 for Vendetta, with the least damage of our five. On 00reverse-spiral it dies up to 13 times a game. Top ladder teams win with it.
   - Look at how it uses its landing dash (`attack::IMPACT_DASH`, 1.5 × Focus damage plus a stun, every 8 turns) on camps and in fights. Also its Focus build, its shield (`champions/heart_of_ruin.rs`), and whether it should draft lower on some maps.
2. **Weak maps against v5:** 00reverse-spiral (29% wins, gold ratio 0.60), 01corridors (29%), 00gemfight (33%), 00quadrants (38%). v5's weighted draft beats the fixed meta order there, so find out why (per-champion gold and deaths per map: see `econ.py --per-map`).
3. **01boss-battle.**
   - The gate siege opens the monolith around round 1,000 instead of 1,950, but v5 then finishes our damage and captures it.
   - Try sieging only while our guard is ready to hold the gate, or only once we can finish the pair before the enemy can.
   - After the gate opens, bags of 1,000+ gems change hands in fights (gems lost up to 2,500 a game). Protect the big bags: bank first, and don't fight with a big bag near the gap.
4. **Timeouts** are still about 130 per 1,000 rounds, against roughly 30 for top teams.
   - Cut-off turns are 74% farm turns, and nearly all rebuild the farm plan: about 30k for the group loop, 24k for target choice, 15k for the rally decision.
   - Reusing the plan's per-group verdicts across rounds while nothing in sight changes is the next idea.
   - Profile with the `TRACE` edit (spill-only traces in each replay's `team-a.stdout`/`team-b.stdout`) and the `ACT` edit (activity log: where turns go).
5. **Idle turns** (7.7%) are mostly capture holding and stale plans after a capture. `farm::STALE_SKIP_AT_GOAL` didn't help, so look for a cheaper fix.

## Ways of working the team expects

- No map-specific hard-coding: rules must be computed from the map or game state.
- Test every change against the previous bot. Report wins together with gold, tributes, deaths, kills and fuel-exhausted turns.
- Put results in the code comments and add a `docs/writeup.md` entry (template at its end).
- Don't commit or upload without asking.
