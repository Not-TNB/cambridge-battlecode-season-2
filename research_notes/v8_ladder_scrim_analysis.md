# bot-v8 against the top 100: where we lose and why

*2026-10-25. Data: 485 unranked scrims of bot-v8 against 97 top-100 opponents (24 Oct 00:14 – 25 Oct 11:20), 747 recent ranked games of the other 99 top-100 teams, and 3,642 ranked top-100 games from the 24 Oct match-database dump. Full tables are in [v8_ladder_scrim_tables.md](v8_ladder_scrim_tables.md).*

## Summary

- **v8 won 104 of 485 scrims against the top 100 (21%, 95% CI 18–25%).** We were around rank 94–100 during the run.
- **There are two cliffs, not one.** Against ranks 81–100 we win 40%. Against ranks 31–80 that drops to about 23% and stays flat. Against ranks 1–30 it is 4% (6 wins in 140 games), including 0 of 40 against ranks 21–30.
- **We lose on economy, and the gap shows up early.** In losses we are 82 gold behind at round 100, 411 behind at round 300 and 2,573 behind at round 1000. Across all top-100 ranked games, the team ahead on gold at round 300 wins 74% of the time.
- **Four of our five champions are starved of gold.** Our Vesper Vale out-earns the opponent's Vesper (527 against 462 gold at round 300). Our Marcella, Heart of Ruin and Vendetta earn only 41–65% of what the same champion earns on the other side. v8 funnels camp captures to Vesper. A local A/B (section 4) shows the funnel itself is not the problem: turning it off cost 6% team gold at round 300.
- **We die far more.** We average 13 deaths a game against 8 for our opponents. Our first death comes at a median of round 139, against 172 for theirs. Every one of our champions dies 1.5–2× as often as the same champion on the other side.
- **Our builds are one-dimensional, and the top teams' builds are not.** v8 Vesper and Vendetta take 91–95% Strength emblems, while top-30 teams put 27–39% into Health. In 3,642 ranked games from the match database, Vesper and Vendetta with two or more Health emblems in their first four win 56–57% of the time, against 53% with one (section 3b). That's a modest effect, but it's consistent. Top-30 Marcellas often build Focus, but Focus Marcella builds don't win more, so that change is lower priority.
- **v8 runs out of fuel on about 140 turns a game, against 12–15 for top teams.** That is roughly 10× as many lost turns. 88% of them happen outside combat, so they mostly cost us farming and walking time.
- **Things that turned out not to matter:** our draft is the same core as the top teams' (Vesper, Marcella, Heart of Ruin, Vendetta, Ryth). Teleport crystals don't predict wins among top teams (48% win rate with them, 52% without). Altar locks don't separate winners from losers either, even though we buy twice as many.

## 1. Where we start losing

Ranks are the opponent's rank when the group was queued. Each team got one 5-game group, with maps cycled through the pool and sides alternated.

| Opponent rank | Games | Our wins | Win rate [95% CI] | Gems tributed, us / them | Gold@300, us / them |
|---|---|---|---|---|---|
| 1–10 | 50 | 3 | 6% [2–16%] | 351 / 994 | 1,048 / 1,496 |
| 11–20 | 50 | 3 | 6% [2–16%] | 350 / 1,017 | 1,042 / 1,497 |
| 21–30 | 40 | 0 | 0% [0–9%] | 315 / 1,114 | 983 / 1,671 |
| 31–40 | 45 | 8 | 18% [9–31%] | 472 / 931 | 1,014 / 1,379 |
| 41–50 | 50 | 10 | 20% [11–33%] | 541 / 886 | 1,154 / 1,418 |
| 51–60 | 40 | 10 | 25% [14–40%] | 565 / 903 | 1,136 / 1,298 |
| 61–70 | 50 | 15 | 30% [19–44%] | 681 / 845 | 1,153 / 1,122 |
| 71–80 | 50 | 11 | 22% [13–35%] | 548 / 910 | 1,003 / 1,215 |
| 81–90 | 60 | 27 | 45% [33–58%] | 724 / 771 | 1,216 / 1,272 |
| 91–100 | 50 | 17 | 34% [22–48%] | 632 / 816 | 1,101 / 1,176 |

**What this means:**

- **Ranks 81–100 are our peers.** Games against them are close: gems tributed are within about 10–20%, and gold at round 300 is within 5%.
- **Around rank 80 we lose about half our win rate.** From there to rank 31 the win rate is flat at about 20–30%, and the gold gap at round 300 grows to 200–400.
- **Above rank 30 the top teams beat us in about 96% of games.** By round 300 they are 450–700 gold ahead and end with three times our tributed gems. Gains in the 31–80 band are realistic for the next version; the top 30 needs a much stronger economy.
- **A logistic fit on rating** puts our 50% point at about rating 1742, which is our own level, and our 30% point at about rating 1853 (≈ rank 74).

## 2. Both sides of the same games: what the opponent does that we don't

Averages over our 381 losses. The opponent's numbers are from the same games.

| Metric | v8 | Opponent | Gap |
|---|---|---|---|
| Gold earned @100 | 285 | 367 | −82 |
| Gold earned @300 | 1,009 | 1,420 | −411 |
| Gold earned @1000 | 4,281 | 6,854 | −2,573 |
| Gems tributed @1000 | 288 | 582 | −294 |
| Camp clears | 53 | 63 | −10 |
| Kills / deaths | 3.9 / 15 | 14 / 5.2 | |
| Gems per tribute | 31 | 52 | −21 |
| Wards placed | 142 | 200 | −58 |
| Altar locks bought | 8.4 | 4.1 | +4.3 |
| Teleport crystals bought | 0 | 5.5 | −5.5 |
| Fuel-exhausted turns | 140 | 13 | +127 |
| Unspent gold at end | 222 | 769 | −547 |

In our 104 wins the picture flips: we lead on gold from round 100 (373 against 319), clear more camps (80 against 52), and get 16 kills to their 6. **The games are decided by who farms better in the first 300 rounds, and the fights follow from that.**

### 2a. The funnel starves four of our five champions

Same champion on both sides, in scrims where both teams drafted it:

| Champion | Games | v8 gold@300 | Opp gold@300 | v8 gold@1000 | Opp gold@1000 | v8 as a share of opp @1000 |
|---|---|---|---|---|---|---|
| Vesper Vale | 427 | 527 | 462 | 2,386 | 2,093 | 114% |
| Marcella | 419 | 157 | 375 | 731 | 1,779 | 41% |
| Heart of Ruin | 353 | 145 | 257 | 642 | 1,149 | 56% |
| Vendetta | 301 | 171 | 259 | 824 | 1,277 | 65% |
| Ryth | 226 | 153 | 184 | 679 | 892 | 76% |

- **Vesper gains about 65 gold at round 300; the other four lose 30–220 each.** Overall the team ends up about 400 behind at round 300, which matches the deficit in our losses.
- **This comes from the capture funnel in `common/camps.rs`.** With `FUNNEL_MELEE` on (`camps.rs:241`), Bruisers (Marcella, Heart of Ruin) and Assassins (Vendetta) leave a cleared camp's capture to Vesper, the gold-bonus carry. `TANK_FUNNEL` (`camps.rs:10`) does the same for Ryth, a Tank. Those are exactly the starved champions: the three `FUNNEL_MELEE` classes are at 41–65%, and Ryth is at 76%. (The Group opening's `GROUP_FUNNEL` in `opening.rs` is inactive in v8, because `GROUP_MODE = false`.) Concentrating gold on Vesper should pay off because of her +53% gold passive with Treasure. In practice the team as a whole farms less: in losses we clear 10 fewer camps, and the others have far fewer items. A 2026-10-24 local test against our own bot had `FUNNEL_MELEE` off at 109/200 against a 97/200 control, within noise (`common/rally.rs:721`).
- **The others end up with far fewer emblems.** Our Marcella completes 6.4 emblems a game against 12 for top-30 Marcellas. Our Heart of Ruin completes 5.2 against 7.9, and our Vendetta 6.5 against 9.4. Our Vesper completes 20 against 14.

### 2b. We die about 1.6× as often, and every champion is affected

| | v8 | Scrim opponents | Top-100 ranked | Ranks 1–30 ranked |
|---|---|---|---|---|
| Deaths per game | 13 | 8.0 | 9.3 | 9.3 |
| Killed by champions | 12 | 6.7 | 8.1 | 8.4 |
| Deaths before round 300 | 2.0 | 1.5 | 1.8 | 1.9 |
| Median first death (round) | 139 | 172 | 160 | 164 |

| v8 champion | Deaths per game | Same champion on the opponent's side |
|---|---|---|
| Vendetta | 3.5 | 1.9 |
| Heart of Ruin | 3.0 | 1.9 |
| Marcella | 3.0 | 1.5 |
| Ryth | 2.8 | 1.9 |
| Vesper Vale | 2.2 (52 gems lost per game) | 1.4 |

- **Who kills us:** opponent Marcella gets 27% of our deaths, Vesper 21%, Heart of Ruin 15% and Vendetta 15%. Camps cause only 8%. We lose fights against the same comp we draft.
- **Vesper loses the most gems on death,** 52 a game. She carries the funnelled gems, and each death hands them to the killer.

### 2c. Builds: we stack one stat, the top teams mix

Purchase order of the first three emblems, and the share of all completed emblems by stat. v8 builds are fixed, so its rows are 100% one order.

| Champion | v8 first three | Most common top-30 first three | Stat mix, v8 | Stat mix, ranks 1–30 |
|---|---|---|---|---|
| Vesper Vale | Treasure Str → Bruiser Str → Durability HP | Treasure Str → Jungle Str → Bruiser **HP** (31%) | Str 95%, HP 5% | Str 73%, HP 27% |
| Marcella | Treasure HP → Jungle HP → Bruiser Str | Treasure HP → Jungle Str → Bruiser HP (20%); several **Focus** lines | Str 65%, HP 35%, **Focus 0%** | **Focus 41%**, HP 35%, Str 25% |
| Vendetta | Treasure Str → Jungle Str → Bruiser Str | Treasure **HP** → Jungle Str → Bruiser **HP** (39%) | Str 91%, HP 9% | Str 61%, HP 39% |
| Heart of Ruin | Treasure Focus → Jungle Focus → Bruiser Focus | Treasure **HP** → Jungle Focus → Bruiser **HP** (24%) | Focus 89%, HP 11% | Focus 62%, HP 37% |
| Ryth | Treasure HP → Jungle HP → Bruiser HP | Treasure HP → Jungle **Str** → Bruiser HP (44%) | HP 100% | HP 55%, Str 44% |

- **Marcella builds differ, but it may not matter.** Her slot-1 spell deals 2 × Focus, and Focus grows +15 per shard against +5 Strength (`docs/GameRules.md`, Marcella). Top teams build her Focus-first in many games, while v8 builds no Focus at all (`bots/bot-v8/src/champions/marcella.rs:16`). However, in the match-database sample Focus Marcella builds don't win more than other builds (section 3b).
- **Vesper and Vendetta are glass cannons.** Both builds repeat Bruiser Strength once the list runs out (`vesper_vale.rs:13`, `vendetta.rs:13`; `bot.rs:158` repeats the last entry). That fits their 1.5–1.8× death rate. Top teams take a Health emblem early, most often a third-emblem Bruiser Health.
- **Ryth goes all-Health;** top teams mix in Strength (Jungle Strength second in 44% of top-30 games).

### 2d. Fuel: about 10× more lost turns

| | Per game | During combat | Rounds 0–100 | 100–500 | 500+ |
|---|---|---|---|---|---|
| v8 | 140 | 17 (12%) | 20 | 48 | 72 |
| Scrim opponents | 13 | 1.1 | 4.3 | 3.3 | 5.3 |
| Ranks 1–30 ranked | 12 | 0.9 | 7.8 | 2.1 | 1.7 |

- **Every v8 champion stalls; it isn't one bad script.** Per game: Vendetta 39, Heart of Ruin 35, Ryth 33, Marcella 28, Vesper 22.
- **When fuel runs out, the champion's turn is suspended and resumes next round** (`docs/GameRules.md`, Fuel). About 2% of our champion-turns are lost, mostly while farming or walking. That is a small but steady drag on economy.
- **Within our own games, fuel stalls barely track the result.** We win 26% with under 50 stalls and 18–22% with more. Fixing fuel is worth doing, but on its own it won't close the gap.

### 2e. Maps

Each map has 11–14 games, so treat these as hints. We are 0–1 wins on **00interlock, 01shortcut, 01elk_arena, 00expanse, 01gold_rush and 00lost_world**. We are 43–50% on **00contest, 00gemfight, 00fragment, 00elkring and 00rushbait_v2**. Side barely matters: 23% on side A, 20% on side B.

## 3. What the top 100 do (747 recent ranked games)

- **Comps have settled on one core,** the same one we draft. Vesper Vale is in 84–90% of comps, Marcella 81–88%, Heart of Ruin 70–81% and Vendetta 62–70%; Ryth is 47–76%. The most common top-30 comp is Heart of Ruin/Marcella/Ryth/Vendetta/Vesper (116 team-games, 51% win rate). **The draft is not where we lose.**
- **Gold decides games.** In top-100 games the gold leader wins 61% at round 100, 74% at round 300, 77% at round 500 and 85% at round 1000. Winners have more gold than the loser at round 1000 in 85% of games. Kills, camp clears and gems per tribute are next (66–74%).
- **Winners die less:** 5.5 deaths a game against 13 for losers.
- **Top teams barely run out of fuel:** 12–15 stalls a game, mostly in the first 100 rounds, likely during setup.
- **Teleport crystals:** 29% of top-100 team-games buy at least one (4.7 per team-game in ranks 1–30), mostly Marcella and Vesper, at a median of round 533. They don't predict winning (48% win rate with, 52% without), so they're low priority.
- **Altar locks:** top teams buy 4–5 a game against our 8–9. Winners and losers buy the same number.
- **Builds:** top-30 teams spread emblems across Bruiser Strength, Bruiser Health and Bruiser Focus (15 / 11 / 5 per team-game for ranks 1–10). Our builds put almost everything into one stat per champion (see 2c).

## 3b. Match-database check: 3,642 ranked games from 24 Oct

This section uses `summaries-2026-10-24-part1.zip` and `part2.zip` from the site's match database: 49,535 matches, of which 3,642 are ranked games involving a top-100 team. That is 4.9× the sample in section 3, run through the same feature code. The full tables are in `~/Desktop/Code/battlecode-scrim-2026-10-25/matchdb/report_db.md` and `deep_db.md`.

**Everything in section 3 replicates.**

| | `cambc` sample (747 games) | Match database (3,642 games) |
|---|---|---|
| P(gold leader wins) @300 / @1000 | 74% / 85% | 73% / 83% |
| Winner deaths / loser deaths | 5.5 / 13 | 5.7 / 14 |
| Fuel-exhausted turns, ranks 1–30 | 12 | 10 |
| Deaths per game, ranks 1–30 | 9.3 | 8.8 |
| Most common top-30 comp | HoR/Marcella/Ryth/Vendetta/Vesper (51%) | same (441 team-games, 53%) |
| Emblem stat mix, top-30 Vesper / Vendetta | Str 73/61%, HP 27/39% | Str 73/62%, HP 27/38% |
| Crystals: win rate with / without | 48% / 52% | 52% / 50% |

**Do the top teams' build choices actually win more?** This compares top-100 team-games by how many of a champion's first four emblems are of one stat. It only covers champions that completed at least three emblems, which is why all the rates are above 50%.

| Champion (stat counted) | 0 of that stat | 1 | 2 or more |
|---|---|---|---|
| Vesper Vale (Health) | 54% (n=169) | 53% (n=2,215) [51–55] | **56%** (n=2,458) [54–58] |
| Vendetta (Health) | 46% (n=50) | 53% (n=975) [50–56] | **57%** (n=2,525) [55–59] |
| Heart of Ruin (Health) | 62% (n=116) | 52% (n=1,000) [49–55] | 56% (n=2,740) [55–58] |
| Ryth (Strength) | 56% (n=409) | 56% (n=1,542) [54–59] | 59% (n=1,062) [56–62] |
| Marcella (Focus) | 57% (n=1,986) [55–59] | 53% (n=731) [49–57] | 56% (n=1,995) [54–58] |

- **Health on Vesper and Vendetta has the clearest link to winning, but it's modest:** about +3–4 points over one Health emblem, with confidence intervals that barely overlap. Part 1 alone showed +6; adding part 2 shrank it. v8 builds take zero or one Health emblem among the first four.
- **Heart of Ruin** shows a similar +4 for two or more Health emblems over one.
- **Marcella Focus shows no benefit.** That lowers its priority in section 4.
- **Ryth's mixed Strength build** trends slightly upward. None of these build effects is large; the economy gap (section 2a) is far bigger.
- **This is still correlation:** builds also reflect the rest of each bot. Treat it as evidence for which A/B tests to run first.

## 4. What to change, in order

The first two candidates were A/B tested locally on 2026-10-25; neither won. 312-game A/B noise is about ±20 wins.

### Local A/B results (2026-10-25)

Each variant played 624 games against bot-v8 (52 maps × 2 sides × 6 seeds), paired with a v8-vs-v8 control on the same seeds (`tools/local/bench.py`, `compare.py`; results in `replays/bench/ab25-*`, `ab25b-*`).

| Variant | Wins / 624 (control 312) | p | Team gold @300 | Deaths | Gems lost |
|---|---|---|---|---|---|
| `FUNNEL_MELEE` off | 301 | 0.57 | −6% | −5% | +4% |
| Health builds (Vesper, Vendetta) | 331 | 0.22 | −1% | −9% | −9% |
| Both | 299 | 0.49 | −7% | −9% | +4% |
| Health builds, fresh seeds 6–11 | 314 | 0.95 | −0% | −3% | +37% |

- **The funnel is not the cause of the gold gap.** With it off, team gold falls 8% at round 100 and 6% at round 300: gold routed through Vesper's +53% bonus is worth more than the same captures split up. The per-champion gap in section 2a is about how gold is shared, not why we're behind in total. Leave `FUNNEL_MELEE` on.
- **The Health builds show no reliable effect.** +19 on the first seed set did not repeat (+2 on fresh seeds); pooled over 1,248 games it's +21 (p = 0.33).
- **Caveat:** these are mirror matches against v8, which shares our weaknesses. Ladder opponents differ, so a neutral local result doesn't rule a change out, but there is no local evidence to ship either.

### Revised order

1. **Find where the team-gold gap actually comes from.** We are about 400 gold behind at round 300 in losses with the funnel on, so the cause is upstream of how captures are shared: fewer camp clears (53 against 63 in losses), more deaths and lost turns. Compare camp-clear timing and walking time against a top-30 team's replays.
2. **Health builds (optional ladder test).** No local gain; if tried, judge it by a ladder scrim run (`queue.py` against ranks 31–80, baseline 23%).
   *Original suggestion:* **Add Health to Vesper and Vendetta.** Make the third emblem Bruiser Health, the most common top-30 choice, and stop repeating Bruiser Strength forever. For example, alternate Strength and Health after the first three (`vesper_vale.rs:13`, `vendetta.rs:13`). This is the build change with the strongest support (section 3b: 56–57% against 53%, modest). Test whether deaths per game fall from 13 towards 9.
3. **Cut fuel use.** The gap is large (about 140 lost turns a game against about 12), but a 2026-10-22 local test that cut fuel stalls by 21% didn't win more games; savings that keep decisions identical did (bot-v6 allocator, 300/416). Profile where the per-turn 100k budget goes with the local fuel profiler (see `docs/RustFuelPitfalls.md`). Fuel stalls grow over the game (72 after round 500), so look at anything whose cost grows as the game goes on.
4. **Mix Strength into Ryth's build** (Treasure Health → Jungle Strength → Bruiser Health is the top-30 line, in 41% of games). Lower priority, since the win-rate trend is small.
5. **Lower priority:** a Focus build for Marcella (common in the top 30 but not linked to winning; section 3b), buy fewer altar locks (half the gold back), and look at the 0% maps (00interlock, 01shortcut, 01elk_arena). Crystals can wait.

## 5. Caveats

- **Two parts of the match database were used:** 24 Oct, parts 1–2 of the 12 available so far (section 3b), ranked games only. The 19.5k unranked games in those parts weren't processed. Sections 1–3 use each team's 15 most recent ranked games via `cambc matches summary`. The older 21 Oct dump analysis is in `research_notes/Battlecode ladder strategy analysis/`.
- **Ranks moved during the 11-hour run.** Bands use the rank at queue time. A few top-100 teams were never scheduled, because of persistent 429s, a team with no eligible bot, or teams moving into or out of the top 100.
- **Five games per opponent is too few to judge any single opponent.** Use the rank bands, not per-team records.
- **Same-champion and build comparisons show correlation only.** Better players also farm better, and better farming buys more items. The A/B tests in section 4 are what establish cause.

## Data and scripts

Everything is in `~/Desktop/Code/battlecode-scrim-2026-10-25/`. It was moved out of `/tmp` so a reboot won't delete it.

| File | What it holds |
|---|---|
| `scrim/groups.jsonl` | the 97 queued groups (opponent, rank, maps, sides) |
| `scrim/rows_scrim.jsonl`, `scrim/rows_top.jsonl` | per-game feature rows (485 scrims, 747 ranked games) |
| `scrim/deep_scrim.jsonl`, `scrim/deep_top.jsonl` | event-level features (fuel, deaths, emblem order, crystals) |
| `scrim/sum/*.camstat` | raw match summaries |
| `matchdb/summaries-2026-10-24-part{1,2}.zip`, `matchdb/rows_db.jsonl`, `matchdb/deep_db.jsonl` | match-database dump and its extracted rows (`scrim/matchdb.py`) |
| `scrim/queue.py`, `collect.py`, `feat.py`, `analyze.py`, `deep.py`, `deep_report.py` | the scheduler, collector, feature extraction and reports |
