# Our bot on the Cambridge Battlecode 2026a ladder: identity, strategy, strengths and faults

Data: 50 most recent ranked matches of `--team me` (2026-10-22 18:00–19:31 UTC, 10 match groups of 5), pulled with `cambc matches list --team me --kind ranked --limit 50 --json`, plus each match's summary (`cambc matches summary <id> --json`). Also 13 of our ranked matches from 2026-10-21 (engine 1.0.2) taken from the bulk dump `matchdb/summaries-2026-10-21-part1.zip`, and 3 local test games. Everything is saved under `/private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/ours/`: `me_ranked.json`, `sum/<id8>.json`, `rows.json` (per-match table), `zip21/`. The analysis scripts are in `.../scratchpad/scripts/` (an.py, an2.py, an3.py, spill.py, fuel.py).

Sources. A source written as "match XXXXXXXX" is `cambc matches summary <full id>` (full ids are in `rows.json`). A source written as a file path is in the repo at `/Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/`. Code citations point at `bots/bot-v5-r175199/` unless stated otherwise, because that tree matches the bundle we believe was submitted (see Q1).

Summary decoding (from the SDK). `name = "Unknown champion (N)"` in a summary uses ChampionType: 0 Ryth, 1 Vesper, 2 Vendetta, 3 Cassi, 4 Issac, 5 Stellar, 6 Prism, 7 Marcella, 8 Willow, 9 Ren, 10 Varg, 11 Tavi, 12 Heart of Ruin. The draft ids in `matches list` use ChampionDraft: 0 Ryth, 1 Vesper, 2 Vendetta, 3 C&I, 4 Stellar, 5 Prism, 6 Marcella, 7 Willow, 8 Ren, 9 Varg, 10 Tavi, 11 HoR. Both come from [sdk/rust/src/types.rs:30-31](sdk/rust/src/types.rs).

## Which team and which bot version is ours, and what's its record recently?

### Takeaway
"me" is **Anime girls against #2 period cramp** (0d643742-c2b9-486f-bc6c-416b76b7cfc3), not "Sydney's Realist". Its drafts carry the fingerprints of our **v5** code (the gem-carry Marcella+Prism draft and the Vesper-first weights). The bundle that matches what plays on the ladder is `bots/bot-v5/man.zip`, which is identical to `bots/bot-v5-r175199/src` apart from the seed. The team went **33-17 (66%)** over the last 50 ranked games, but only **9-16** against teams within about 10 ladder places and **24-1** against weaker ones. Its rating was falling during the sample: #97 at 1764.7, then #108 at 1748.5 a few minutes later.

### Cited Findings
- **Team identity:** "Anime girls against #2 period cramp" (0d643742…) appears in all 50 of 50 `--team me` matches. No opponent appears more than 5 times (Latent Tactics, Zkey Farming, FF15, 5m, will think later, Preyas, Turbo Stunners, Memory?, patisserie du pont neuf, hold up: 5 each) — [me_ranked.json](scratchpad/ladder-data/ours/me_ranked.json)
- "Sydney's Realist" (c6a4a0d8…) never appears in our matches. Its own recent matches show a fixed draft: HoR, Ryth, Marcella, Vendetta, Varg (draft ids 11,0,6,2,9) in all 10. Our drafts vary from game to game — [sydney.json](scratchpad/ladder-data/ours/sydney.json)
- **Ladder position:** the first ladder read showed us at #97, rating 1764.66, with Sydney's Realist at #94 (1769.5). A ladder JSON fetched minutes later showed us at #108, 1748.51, after 469 matches, and Sydney's Realist at #91 (1772.5) — `cambc ladder --page 4`; [ladder_p*.json](scratchpad/ladder-data/ours/)
- **Draft matches the v5 source.** In all 8 games on gem-carry maps we drafted Marcella then Prism first: 00gem_fields b6f381da, 00lost_world c54e03d3, 00great_wall e17af25e/3d1e7e1f/bca18825, 00road_of_riches 6558fff1/bf78490d, 00quadrants 1b234e9f. That is `gemcarry::draft_pick` ([bots/bot-v5-r175199/src/common/gemcarry.rs:23,61](bots/bot-v5-r175199/src/common/gemcarry.rs); [draft.rs:248](bots/bot-v5-r175199/src/draft.rs)). On the other maps Vesper (draft 1) is in 47 of 50 rosters, which fits `small_team_weight` (Vesper 1.0, AD ×5 when we have none; [draft.rs:114-185](bots/bot-v5-r175199/src/draft.rs)). bot-v4 has neither gemcarry.rs nor SMALL_TEAM_MAX=5 (`bots/bot-v4/src/draft.rs:103` has 4) — [bots/bot-v5-r175199/src/common/gemcarry.rs](bots/bot-v5-r175199/src/common/gemcarry.rs)
- **Which bundle:** `bots/bot-v5/man.zip` (dated 10-22 02:26) has the same source as `bots/bot-v5-r175199/src`; only `main.rs` (SEED) differs. The working-tree `bots/bot-v5/src` adds `press.rs` (PRESS=false) and a jackpot delay in farm.rs/opening.rs, 65 changed lines. `bots/bot-v4/some.zip` equals `bots/bot-v4/src` exactly. Checked with `diff -r --strip-trailing-cr`. Commit ac8351a ("version 5", 2026-10-22 16:30) added both v5 trees — git log; [bots/bot-v5/man.zip](bots/bot-v5/man.zip)
- **Record:** 33-17 overall; 17-8 as side A, 16-9 as side B — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **By opponent** (rank and rating from the later ladder read):

  | Opponent | Rank | Rating | Our record |
  |---|---|---|---|
  | Latent Tactics | #93 | 1770 | 2-3 |
  | Zkey Farming | #94 | 1768 | 2-3 |
  | will think later | #97 | 1765 | 1-4 |
  | hold up | #113 | 1743 | 2-3 |
  | 5m | #115 | 1740 | 4-1 |
  | FF15 | #123 | 1729 | 5-0 |
  | Turbo Stunners | #133 | 1713 | 5-0 |
  | patisserie du pont neuf | #140 | 1707 | 3-2 |
  | Memory? | #143 | 1703 | 4-1 |
  | Preyas | #159 | 1684 | 5-0 |

  — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Trend in the sample:** the last two groups (19:20 against Zkey, 19:30 against Latent) went 4-6 — [me_ranked.json](scratchpad/ladder-data/ours/me_ranked.json)
- **Previous day:** 13 of our ranked games on 2026-10-21 (engine 1.0.2, older bot) went 9-4 — [zip21/](scratchpad/ladder-data/ours/zip21/)

### Inferences
- Treat it as likely, not certain, that man.zip (= bot-v5-r175199) is the live submission. The CLI has no command to list submissions. The only behaviour that separates the two v5 trees is the jackpot delay, which applies only on maps where gold clusters make up ≥0.7 of the camp gold (01elk-arena), and it is too subtle to see in summaries. We won both 01elk_arena games (91f5380d, 4f9b7afa).
- Our rank is falling (#97 → #108) because we lose to peers at our rating. The 33-17 record is inflated by a matchmaking stretch against weaker teams.

### Gaps
- No CLI command shows which submission is active or when it was uploaded, so the live version is inferred from drafts.
- Ratings come from ladder reads taken after the matches, not the ratings at match time.

## What does our bot do well, and where does it lose (maps, opponent types, phase of the game)?

### Takeaway
We win by **fighting and snowballing**. In wins we average 23 kills to 6 and 12.3k gold earned to 5.3k, and we usually end the game by gems (1003 to 400). We lose **low-contact farming races**: in losses we get 2 kills to 7, and 7 of the 17 losses had 0 kills by us. Opponents out-farm us from the opening onwards: we are behind in gold by round 300 (982 to 1199) and the gap grows (4.0k to 5.9k by round 1000). The weakest maps are the **gem-carry maps, 2-6**, and **01islands, 1-2**. Every peer that beat us drafts Marcella + Vesper and gets far more gold out of Marcella and Ryth than we do.

### Cited Findings
- **Wins (33)** average: gems 1003 to 400, gold earned 12,314 to 5,297, kills 23 to 6, deaths 7 to 29, camp clears 88 to 45, game length 1581 rounds — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Losses (17)** average: gems 468 to 1005, gold 5,512 to 7,820, kills 2 to 7, deaths 9 to 4, gems collected 559 to 1086, game length 1223 rounds. The opponent ends the game early by reaching 1000 gems — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **0-kill losses (7):** bb3b6b4b, 71b8dc63, eb0d2eed, 61563931, edcc9d98, 946c9409, bca18825. Several were 0-0 on kills, so pure economy (e.g. 71b8dc63 00split: 426 to 1012 gems by round 716) — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Phase:** in losses team gold at rounds 300/600/1000 was 982/2194/4004 for us against 1199/2989/5937 for them. In wins it was 1195/2973/6449 against 790/1764/3125. Tributed gems at round 1000 in losses: 327 to 730 — [rows.json + teamTimelines](scratchpad/ladder-data/ours/sum/)
- **Gem-carry maps** (00gem_fields, 00quadrants, 00lost_world, 00great_wall, 00road_of_riches): 2-6; 00great_wall alone is 0-3 (e17af25e, 3d1e7e1f, bca18825). All other maps: 31-11 — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Gem-carry losses** averaged 358 to 820 tributed gems by round 1000. In b6f381da (00gem_fields) we collected 335 gems to 1107. Our Prism earned 40 gold and Ryth 40 gold in 864 rounds — [match b6f381da](scratchpad/ladder-data/ours/sum/b6f381da.json)
- **Prism**, drafted on every gem-carry map, has the worst survival on our team: 2.8 deaths/game, 7 wins in 17 games. Marcella: 1.1 deaths, 20/29. Vesper: 0.9 deaths, 30/47 — [an2.py output](scratchpad/scripts/an2.py)
- **The code's own test** of gem-carry (comment at [gemcarry.rs:12-19](bots/bot-v5-r175199/src/common/gemcarry.rs)): 43-53 or 49-47 in its first tests, 8/12 on one seed afterwards. It was switched on "at the user's request (2026-10-22: the top ladder team plays it)" — [bots/bot-v5-r175199/src/common/gemcarry.rs:12-23](bots/bot-v5-r175199/src/common/gemcarry.rs)
- **Other weak maps:** 01islands 1-2 (1ec4d876 L, c7a67204 L, 779bea95 W) and 00expanse 0-2 (61563931, edcc9d98: both 0-0 on kills, about 500 to 1030 gems). Also single losses on 01big_world, 00rows, 00stampede, 01fracture and 01yellow_brick_road — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Strong maps:** 00reverse_spiral 3-0, 01boss_battle 3-0, and 2-0 on 01shortcut, 01broken_curve, 00rushbait_v2, 00packs, 01hooks, 01starfish and 01elk_arena — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Blowouts against weaker teams:** e44f7fbe 00swirl (vs FF15) 1009-0 with 50 kills to 0; c42d88a7 01hooks (vs 5m) 1012-0 with 49-0; 8c3aff21 00reverse_spiral (vs patisserie) 1013-0 with 34-1 — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Opponents who beat us:**
  - Latent Tactics and Zkey Farming both drafted Marcella, Vesper, Tavi, Willow, Ryth (draft 6,1,10,7,0). Their Marcella and Ryth were the main earners: Zkey 71b8dc63 Marcella 3,088 gold / 442 gems and Ryth 2,114 / 359; Latent 1ec4d876 Marcella 4,688 / 604 and Ryth 1,770 / 336. In those games our Ryth earned 180, 40 and 1,120 gold — [matches 71b8dc63, 1ec4d876, eb0d2eed, b6f381da](scratchpad/ladder-data/ours/rows.json)
  - will think later: Vesper, Vendetta, HoR, Marcella, Prism. hold up: HoR, Vesper, Marcella, Ryth, Vendetta. 5m: HoR, Vesper, Ryth, Marcella, Vendetta. Every team that beat us in this sample had Marcella and/or Vesper — [me_ranked.json](scratchpad/ladder-data/ours/me_ranked.json)
- **Unspent gold in our losses:** opponents averaged 621 gold unspent at the end against our 157 (e.g. 1,977 for will think later in edcc9d98, 1,691 for Zkey in eb0d2eed) — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Opening:** the opening planner is off (`OPENING: bool = false`, [opening.rs:35](bots/bot-v5-r175199/src/opening.rs)), so we open with nearest-camp farming. Fight engagement is capped at 7 tiles ([fight.rs:21](bots/bot-v5-r175199/src/common/fight.rs)), and enemies standing on their own spawn are ignored as targets ([fight.rs:17](bots/bot-v5-r175199/src/common/fight.rs)) — [bots/bot-v5-r175199/src/opening.rs](bots/bot-v5-r175199/src/opening.rs)

### Inferences
- Peers who beat us outrace us on economy: they reach 1000 gems before our fights matter. The deficit already exists at round 300, so their opening and farming route are better than ours (our opening planner is off). The leftover gold suggests they turned gold into gems efficiently and didn't need to keep shopping.
- The Marcella/Prism gem-carry plan looks net-negative on the ladder (2-6, Prism dies a lot, fewer gems collected). It went live on weak evidence, as its own test notes in the source say.
- Our combat edge decides games against weaker opponents. Against peers who avoid fights, it never gets to matter.

### Gaps
- There are no position heatmaps or replays (only death, kill and ward heatmaps), so map control can't be measured directly. Who farms which half of the map is only inferred from gem and gold totals.
- I did not download replays (`cambc matches download`) to watch the losses frame by frame.

## What faults show up in the data (crashes, timeouts, idle units, bad fights, economy stalls), and what code causes them?

### Takeaway
We found **no crashes**: 0 bot faults in all 63 platform summaries, and v5 does not show the "SDK initialization failed" crash. The biggest fault is **fuel exhaustion on about 8.6% of our champion turns**, against about 0.5% for opponents. That works out to roughly 570 lost turns per match. It comes from farm planning (`farm_plan`) and the per-turn sight rebuild, and it reproduces locally. Other faults found:
- **01boss_battle never tributes, or tributes very late**: in 3bca78ca we carried about 2,390 gems to round 2000 and tributed 0.
- **Idle or stuck champions on 01islands**: non-dashers such as Tavi, Stellar and Ren collected 0 gems in 1,100+ rounds.
- **00stampede collapse**: 32 deaths, 3 of 5 champions with 0 gems.

### Cited Findings
- **Crashes:** `diagnostics.botFaults` is 0 for both teams in all 50 recent summaries and all 13 from 10-21. The fault code lists are empty, and there are no `resignation` or `botFault` events. Platform engine is 1.0.3 (10-21: 1.0.2) — [sum/*.json](scratchpad/ladder-data/ours/sum/), [zip21/](scratchpad/ladder-data/ours/zip21/)
- **Local check:** bot-v5-r175199 ran against itself on 00great-wall with cambc 3.0.4 (engine 1.0.2), release and debug builds, both to 1812/1480 rounds, with 0 crashes — [local/gw_rel.json, gw_dbg.json](scratchpad/local/)
- **Fuel exhaustion:**
  - Rate: `fuelExhausted` events per champion-turn average 0.086 for us against 0.005 for opponents (wins 0.080, losses 0.099). That is about 570 per match, from 2,480 in 3bca78ca down to 57 in 0ed35c2b. The older 10-21 bot had the same rate (0.079 against 0.007) — [an3.py output](scratchpad/scripts/an3.py)
  - Which champions: all of them. Mean exhaustions per champion-game: HoR 191, Vendetta 162, Ryth 150, Varg 139, Ren 138, Marcella 134, Prism 123, Tavi 122, Vesper 105 — [an2.py output](scratchpad/scripts/an2.py)
  - Timing and pattern: it happens throughout the game, most in rounds 0–700 (about 2,000–2,700 per 100 rounds across the 50 games). It falls off after round 1100 partly because many games have ended by then. 22,643 of 28,536 are isolated turns, 2,398 are pairs, and runs of 5 or more almost never happen. Exhaustions often come on a regular beat: every 3 rounds for Ryth in 3bca78ca (12, 15, 18, 21…), and every 2 rounds for Tavi in a0668720 (10, 12, 14…) — [fuel streak analysis](scratchpad/scripts/an3.py)
  - Cost: GameRules says a bot that runs out of fuel "is suspended and the next bot … gets to go … the next round, this bot's turn will resume" — [docs/GameRules.md:335-345](docs/GameRules.md). The team's own notes call this effectively a lost turn — [docs/RustFuelPitfalls.md](docs/RustFuelPitfalls.md)
  - It reproduces locally: release build 8.3% / 7.3% (team A / B), debug 7.5% / 9.6% — [local/gw_rel.json](scratchpad/local/gw_rel.json)
  - The code already knows: "turns reached the farm step with ~82k fuel and a plan cost 50-90k … 8.5% of all turns ran out". FARM_PLAN_MIN_FUEL went up to 85k and only cut exhaustion by 17% — [bots/bot-v5-r175199/src/common/farm.rs:2150-2156](bots/bot-v5-r175199/src/common/farm.rs). The writeup's older figures of "0 timed-out turns" and "4.8%→3.1%" no longer hold — [docs/writeup.md:199,576](docs/writeup.md)
- **Where the fuel goes** (local 600-round 00great-wall mirror with TRACE on in a scratchpad copy; 625 spill or heavy turns traced, 531 spills):
  - The fuel ran out right after these checkpoints: `plan-damaged` 170, `plan-target` 49, `path-blocked` 41, `goals-not-goals` 35, `camp-fight-context` 35, `path-dash` 33, `farm-wait-outside` 30, `sight-visible` 28, `plan-enemies` 26, `plan-groups` 23.
  - Mean cost per step: `plan-groups` 19k (578 occurrences), `camp-fight-context` 14k, `plan-target` 13k, `plan-rally-decision` 11k. Round 2's `sight-camps` cost 58k, and `plan-enemies` 54k.
  - These are the farm planner at [farm.rs:835](bots/bot-v5-r175199/src/common/farm.rs) (`farm_plan`), with checkpoints at 868, 954, 1644, 1821, 1935 and 1975; the camp-fight context at [collect.rs:341](bots/bot-v5-r175199/src/common/collect.rs); and the sight rebuild at [world.rs:47-75](bots/bot-v5-r175199/src/common/world.rs).
  - Source: [spill.py output](scratchpad/scripts/spill.py); trace log at `scratchpad/local/gw_trace/x/team-a.stdout`.
- **Fuel guards:** `MIN_FUEL_FOR_PATHING = 40_000` ([bot.rs:8](bots/bot-v5-r175199/src/bot.rs)), `FARM_PLAN_MIN_FUEL = 85_000` with `STALE_PLAN_ROUNDS = 3` ([farm.rs:2156-2159](bots/bot-v5-r175199/src/common/farm.rs)), and `FARM_ENEMY_DUEL_FUEL = 70_000` ([farm.rs:231](bots/bot-v5-r175199/src/common/farm.rs)) — [bots/bot-v5-r175199/src](bots/bot-v5-r175199/src)
- **01boss_battle tribute failure:**
  - 3bca78ca (vs 5m, side B) reached round 2000 at 0-0 tributed gems and was won on the gold tiebreak, 26,552 to 14,777. At the end our champions held Vesper 798, Marcella 554, HoR 467, Tavi 317 and Ryth 257 gems, about 2,390 in total, and there were no gemSubmission events at all.
  - In 30f05991 Vesper tributed 1,230 gems in a single submission at round 1473. In 0f536bf1 Vesper tributed 1,226 at round 1460.
  - The source documents this map's only altar as surrounded by walls, holes and a monolith tile (`OPEN_ALTARS_ONLY`, [base.rs:76-81](bots/bot-v5-r175199/src/common/base.rs)), and says enemy locks cancel tributes there ([base.rs:69-74](bots/bot-v5-r175199/src/common/base.rs)).
  - Sources: [matches 3bca78ca, 30f05991, 0f536bf1](scratchpad/ladder-data/ours/sum/)
- **Idle champions on 01islands:**
  - 1ec4d876: Stellar ended with 120 gold and 0 gems, Tavi 160 gold and 0 gems, over 1,127 rounds. Vesper did most of the work (2,416 gold, 211 gems).
  - c7a67204: Tavi ended with 40 gold and 0 gems, and his last gold came at round 426. Ren ended with 180 gold and 0 gems.
  - Even the 779bea95 win had Stellar at 0 gems in 1,553 rounds and Tavi at 0 gems until round 1120.
  - A code comment notes "champions without a dash were hitting camps across holes (01islands, 01big-world…)" — [collect.rs:1178](bots/bot-v5-r175199/src/common/collect.rs); region reachability is handled in [regions.rs:47-57,219-250](bots/bot-v5-r175199/src/regions.rs)
  - Sources: [matches 1ec4d876, c7a67204, 779bea95](scratchpad/ladder-data/ours/sum/)
- **Bad fights on 00stampede** (59cf7d77, vs will think later): we had 6 kills and 32 deaths (Ren 11, HoR 7, Vendetta 7). Vesper, Ren and HoR collected 0 gems all game. Tavi collected 160 gems at round 1544 and never tributed them. Their Marcella got 16 kills — [match 59cf7d77](scratchpad/ladder-data/ours/sum/59cf7d77.json)
- **Bad fights on 00swirl** (30639d37, vs hold up): 11 kills to 36, 37 deaths, 174 to 1009 gems. Their HoR got 21 kills — [match 30639d37](scratchpad/ladder-data/ours/sum/30639d37.json)
- **Healer earning nothing:** Willow had 0 gold and 0 gems in 61563931. Her longest stretch without collecting a gem was 1,091 rounds — [match 61563931](scratchpad/ladder-data/ours/sum/61563931.json)
- **Slow wins:**
  - 00contest (085fd7c3): 400-0 at round 2000, so the win came on the tiebreak.
  - 00pilgrimage (da0669ad): 943 to 409 at round 2000, with 14.5k gold earned to 5.3k.
  - Both won, but neither reached 1000 gems despite a large economic lead.
  - Sources: [rows.json](scratchpad/ladder-data/ours/rows.json)

### Inferences
- Losing about 9% of turns slows every part of the game: farming, banking and fighting. Opponents lose almost none, so this is a structural handicap in the close farming races we lose. The beat-like pattern (period 2–3) matches a plan that is cut off, finished the next turn, and then rebuilt (the farm.rs:2150 comment describes the same thing).
- On 01boss_battle the fix needs to be about the altar access or lock logic, not farming. Carrying 2,000+ gems to the end is a near-loss that the gold tiebreak happened to save.
- On 01islands, champions that cannot dash or crystal-teleport off the start area likely get stuck targeting unreachable camps, or do nothing. This is inferred from flat gold and gem timelines; the summaries have no positions.

### Gaps
- Summaries have no per-turn positions, so "stuck" and "idle" are inferred from flat earnedGold/collectedGems timelines, which can mislead. For example, on 00contest gems come from monoliths and the collectedGems metric stays flat.
- The fuel profiling covers one map (00great-wall) and a debug build with TRACE on. Spill locations may differ on other maps and in the platform's release build.
- I did not look into why 01boss_battle tributes fail in some games and work late in others (side, enemy locks, monolith timing). That needs a replay.

## How do our economy/kill numbers compare to our opponents' in the same matches?

### Takeaway
Across all 50 games we earn more gold and get far more kills, but this splits sharply. In **wins** we have 2.3× their gold and 4× their kills. In **losses** we have 0.7× their gold, collect half their gems, and get fewer kills. We also clear more camps even in losses (56 to 48). So our farming is less valuable per action: low-value camps, gems lost or carried rather than tributed, and fewer gold-efficient carries (their Marcella and Ryth).

### Cited Findings
- **Wins (n=33)** average: gold earned 12,314 to 5,297; spent 11,854 to 5,077; gems collected 1,210 to 712; kills 23 to 6; deaths 7 to 29; camp clears 88 to 45; ward placements 156 to 196; gem-loss events 7 to 29 — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Losses (n=17)** average: gold 5,512 to 7,820; spent 5,356 to 7,199; gems collected 559 to 1,086; tributed 468 to 1,005; kills 2 to 7; deaths 9 to 4; camp clears 56 to 48; wards 154 to 230; gold left unspent 157 to 621 — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Gold per round in losses:** 1.4 vs 3.5 (b6f381da), 3.2 vs 6.3 (1ec4d876), 6.4 vs 9.1 (71b8dc63), 4.8 vs 14.3 (a0668720), 3.7 vs 9.7 (30639d37) — [an2.py output](scratchpad/scripts/an2.py)
- **Exception:** in two losses we had more gold but still lost the gem race: bb3b6b4b (01big_world) 8,063 to 7,160 gold, 738 to 1,019 gems; 946c9409 (01fracture) 9,513 to 8,194 gold, 636 to 1,011 gems — [rows.json](scratchpad/ladder-data/ours/rows.json)
- **Previous day (13 matches):** similar profile, e.g. 0bd85570 lost 0 to 1008 gems with 7,356 to 7,156 gold, and 070f3041 lost 188 to 926 with 12 kills to 31 — [zip21/](scratchpad/ladder-data/ours/zip21/)

### Inferences
- In the losses where we had more gold (bb3b6b4b, 946c9409), we had the gold but did not turn it into gems fast enough. That matches the code's own note about 00lanes ("3,000 more gold and 17 kills to 3, and still lost the gem race") — [bots/bot-v5/src/common/press.rs:1-6](bots/bot-v5/src/common/press.rs). The fix it proposes (PRESS) is off.
- Fewer wards in losses (154 to 230) may mean less vision and map control against stronger teams. This is weak evidence.

### Gaps
- The kills and deaths here don't say who started each fight, so I could not measure targeting quality.
- I could not measure "wasted economy" (items bought but useless) beyond the spend and bank totals. The item ids in summaries are just numbers and I did not map them to names.
