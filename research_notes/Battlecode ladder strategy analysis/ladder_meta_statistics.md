# Ladder-wide meta statistics: Cambridge Battlecode 2026a (match-summary dump, 2026-10-21 part 1)

**Sources used throughout:**
- [DUMP] = `/private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/matchdb/summaries-2026-10-21-part1.zip`: 23,401 `.camstat` summaries (11,447 ranked and 11,954 unranked), each converted with `cambc summary <file> -o -`.
- [FEAT] = the per-match feature rows I extracted, at `.../scratchpad/ladder-data/matchdb/json/feat_*.jsonl`. The scripts are in `.../scratchpad/ladder-data/matchdb/scripts/` (`extract.py`, `analyze.py`, `analyze2.py`, `ours.py`, `champprod.py`). The raw outputs are `.../matchdb/out/analysis1.txt` and `analysis2.txt`.
- [CSV] = chart-ready tables in `.../scratchpad/ladder-data/matchdb/out/`: `champ_stats.csv`, `champ_team_demeaned.csv`, `pair_stats.csv`, `map_stats.csv`, `champ_map_resid.csv`, `profiles.csv` and `separators.csv`.
- [LADDER] = `cambc ladder --page 1..30 --limit 25 --json`, pulled on 2026-10-22 and saved at `.../matchdb/ladder/all.json`. It gives 724 unique teams. The live ladder shifted while I paged through it (746 rows, 724 unique). 695 of the 724 teams in the dump matched a ladder entry; the rest are "unrated".
- [RULES] = `/Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/docs/GameRules.md`

**Sampling caveats (apply to every number below):**
- All matches come from a single day, 2026-10-21, and this is part 1 of 19 parts.
- Two engine versions are mixed: 1.0.2 (n=17,838) and 1.0.1 (n=5,563).
- Ratings and ranks are current ladder values from 2026-10-22, not the values at match time.
- Buckets are by current rank: top20 (20 teams, 6,459 team-games), 21–50 (29 teams, 5,437), 51–150 (96 teams, 10,629), 151–300 (142 teams, 8,413), 301+ (408 teams, 14,789), and unrated (29 teams, 1,019).
- "Unranked" matches are scrimmages. In them a lower-rated challenger is almost always Team A and the invited, higher-rated team is Team B (for example, #1 appears 684 times as B). This biases raw side splits (see Q2).
- 95% intervals are Wilson intervals unless I say otherwise.
- "Elo residual" = mean of (win − Elo-expected win) using current ratings. Expected = 1/(1+10^(−Δ/400)). This adjusts for team strength. Calibration check: when Team A's expected win was 0.8–0.9 it actually won 74.2% (n=629), and at 0.0–0.1 it won 5.8% (n=1,363). So Elo is slightly overconfident at the extremes.
- "Team-demeaned" = mean of (win − that team's own win rate in the dump), using teams with ≥20 games. This compares a champion against the same team's games without it.

## Q1. What does a match summary contain? (schema)

### Takeaway
A `.camstat` file converts to roughly 3.3 MB of JSON. It holds end-of-game per-champion metrics, cumulative run-length-encoded per-round timelines for 17 metrics per champion, timestamped event lists (kills, deaths, camp clears, tributes, purchases, fuel exhaustion, bot faults, resignation), the draft order and match metadata. It does not contain team names or UUIDs; those come only from the filename.

### Cited Findings
- **Filename:** `<ranked|unranked>_<matchUUID>_<teamA UUID>_vs_<teamB UUID>.camstat`. This is the only place the team identities appear — [DUMP].
- **Top-level keys:** `match`, `teams`, `roster`, `champions`, `championTimelines`, `teamTimelines`, `events`, `heatmaps`, `diagnostics`, `formatVersion`, `jsonFormatVersion` (both "1.0") — [DUMP].
- **`match`:**
  - `outcome` ("teamA"/"teamB")
  - `roundsCompleted`, `roundsStarted`, `finalRoundPartial`, `finalInput`, `finalSubstep`
  - `mapHash` (SHA-256; there are no map names), `width`, `height`
  - `seed`, `engineVersion`, `engineBuildHash`, `metricSemanticsVersion`

  Source: [DUMP].
- **`roster`:**
  - `draft`: ordered list of `{team, champion}`, alternating A then B. Cassi & Issac is a single pick.
  - `champions`: `{id, name, team, draftPick}`, where `draftPick` is the global draft index. Cassi and Issac share one index.

  Source: [DUMP].
- **`champions[]`:**
  - final `items` and `consumables`; `itemsBought` and `consumablesBought` with quantities
  - `metrics`: assists, championDamageDone/Taken, collectedGems, deaths, earnedGold, gems (carried), gold, healingDone/Received, kills, monsterDamageDone/Taken, saleRefunds, shieldingDone, spentGold, submittedGems

  Source: [DUMP].
- **`championTimelines[]`:**
  - One entry per champion × metric (17 metrics, the same list as above).
  - Each entry is `{metric, owner, sampleCount: rounds+1, runs:[{duration, value}]}`. The value at round r is found by walking the cumulative durations.
  - `teamTimelines[]` has the team `submittedGems` timeline in the same format.

  Source: [DUMP].
- **`events[]`:**
  - Kinds, per champion: death, kill, assist, wardPlacement, buyItem, buyConsumable, sellItem, itemCombination, campClear, gemSubmission, gemCollection, gemLoss, fuelExhausted, botFault, resignation.
  - Each entry has `timestamp{round, phase, substep, input, ordinal}`, `x`, `y`, `target`, `amount`, `value` and `context`.
  - `heatmaps[]` (death, kill, wardPlacement) are run-length-encoded tile grids.

  Source: [DUMP].
- **`diagnostics`:**
  - `botFaults` gives a count per team.
  - `codes` was empty in every file.
  - `detailedCaptureComplete`.
  - botFault and fuelExhausted events are counted per turn. A champion that keeps faulting logs one event every turn; for example, 26 team-games had exactly 10,001 events.

  Source: [DUMP].
- **The dump contains:**
  - 40 distinct maps, 35 of them with about 560–750 games each. Sizes range from 30×30 to 100×100.
  - Draft size depends on the map: 3, 4 or 5 picks per team (14,880 games with 5 picks, 4,547 with 4, 3,917 with 3).
  - 28 games with an empty draft. In each, Team A (a 301+ team) resigned at round 0 and B was awarded the win.

  Source: [DUMP].

### Inferences
- The timelines are dense enough to rebuild per-round gold, gems, kill and death curves for every champion. Any metric "at round N" can be computed without the replay file.
- There is no per-match rating field. Rating at match time would have to be recovered from ladder history.

### Gaps
- No map names: maps can only be identified by `mapHash` and size. Map names need a separate lookup, for example from replays.
- Ward events are numerous and their meaning is unclear (some teams log over 1,000 per game). I did not try to interpret them in depth.

## Q2. Champion pick and win rates, pairs/cores, draft order, and Team-A vs Team-B bias

### Takeaway
Top teams converge on one core: Heart of Ruin (85% pick rate in the top 20), Ryth, Marcella and Vesper Vale/Vendetta. After controlling for team strength, those are also the champions that over-perform:
- Heart of Ruin: +5.9pp team-demeaned
- Marcella: +4.4pp
- Vesper Vale: +3.2pp
- Vendetta: +2.4pp

The mid and low ladder over-picks Stellar (−4.4pp), Varg (−3.0pp), Cassi & Issac (−2.8pp) and Ren (−1.8pp). There is no real Team-A/Team-B bias in ranked play (B wins 49.8%). The apparent 56% Team-B win rate overall comes from how scrimmage sides are assigned.

### Cited Findings

**Side bias**
- Ranked: Team B won 49.8% [48.9–50.8] (n=11,419). B's mean residual against Elo was −0.05pp (n=10,627). Among games between teams rated within 50 points of each other, B won 51.2% [50.0–52.3] (n=7,355) — [FEAT]/[CSV].
- Unranked: Team B won 62.4% [61.5–63.3] (n=11,954). B is higher rated in 66% of unranked games (mean rB−rA = +96). The higher B win rate is a selection effect, not a structural advantage — [FEAT].
- The rules' tie-break ("if everything ties, Team B wins") decided only 76/23,373 games (0.3%) — [FEAT]; rule in [RULES].

**Pick rate and win rate, all teams** (team-games n=46,746; win rate excludes mirror games; resid = Elo residual; team-demeaned ±95% CI). Source: [CSV] `champ_stats.csv` and `champ_team_demeaned.csv`.

| Champion | Pick % (all) | WR (no mirror) [95% CI], n | Elo resid | Team-demeaned |
|---|---|---|---|---|
| Ryth | 62.5 | 52.1% [51.1–53.2], 9,281 | +0.3 | +1.8 ±1.0 |
| Stellar | 58.5 | 44.6% [43.6–45.6], 9,422 | −4.1 | −4.4 ±1.0 |
| Ren | 50.1 | 48.0% [47.0–48.9], 10,237 | −1.9 | −1.8 ±0.9 |
| Varg | 49.7 | 42.7% [41.7–43.7], 9,691 | −1.3 | −3.0 ±0.9 |
| Heart of Ruin | 43.8 | 59.0% [57.9–60.0], 8,465 | −0.2 | +5.9 ±1.0 |
| Cassi & Issac | 42.1 | 45.4% [44.4–46.3], 10,210 | −1.8 | −2.8 ±0.9 |
| Marcella | 32.2 | 57.4% [56.3–58.4], 8,743 | +2.5 | +4.4 ±1.0 |
| Vesper Vale | 31.3 | 55.0% [54.0–56.1], 8,569 | +3.0 | +3.2 ±1.0 |
| Vendetta | 31.0 | 52.6% [51.5–53.6], 8,957 | +3.1 | +2.4 ±1.0 |
| Tavi | 26.6 | 48.0% [46.9–49.0], 8,259 | +0.8 | −0.7 ±1.0 |
| Willow | 10.1 | 48.5% [47.0–50.1], 4,120 | +2.0 | +0.0 ±1.5 |
| Prism | 8.5 | 46.3% [44.6–47.9], 3,455 | −0.8 | −1.9 ±1.6 |

**Pick rate (%) by rating bucket.** Source: [CSV] `champ_stats.csv`.

| Champion | top20 | 21–50 | 51–150 | 151–300 | 301+ |
|---|---|---|---|---|---|
| Heart of Ruin | 85.4 | 73.0 | 61.7 | 33.5 | 10.3 |
| Ryth | 71.3 | 70.5 | 64.4 | 60.1 | 56.4 |
| Marcella | 59.2 | 44.6 | 30.6 | 27.0 | 21.0 |
| Vesper Vale | 44.2 | 35.9 | 38.0 | 21.9 | 25.7 |
| Stellar | 44.0 | 51.3 | 56.3 | 69.3 | 61.8 |
| Ren | 43.3 | 56.5 | 58.1 | 58.7 | 40.6 |
| Vendetta | 35.3 | 28.7 | 34.3 | 20.6 | 34.1 |
| Cassi & Issac | 22.7 | 32.9 | 27.6 | 44.6 | 61.1 |
| Varg | 19.1 | 33.3 | 43.3 | 57.4 | 67.8 |
| Tavi | 10.8 | 12.5 | 19.5 | 31.5 | 40.2 |
| Prism | 6.5 | 2.5 | 6.5 | 7.9 | 12.8 |
| Willow | 4.7 | 5.1 | 8.4 | 14.8 | 12.8 |

**Team-demeaned effect within the top 20** (pp, ±95%): Heart of Ruin +9.1 ±1.7 (n=1,632), Marcella +6.5 ±2.0, Vendetta +5.6 ±2.3, Vesper Vale +5.6 ±2.0, Tavi +2.8 ±3.7, Ryth +0.7 ±2.4, Ren −1.5 ±2.4, Cassi & Issac −2.6 ±2.5, Varg −3.9 ±3.6, Prism −6.8 ±4.7, Stellar −7.3 ±2.7 (n=1,004) — [CSV].

**Team-demeaned effect within 51–150** (pp): Heart of Ruin +6.3 ±1.8, Marcella +5.1 ±2.3, Vendetta +4.7 ±2.0, Vesper Vale +4.1 ±2.0, Ryth +2.4 ±2.2, Ren −3.1 ±1.9, Varg −3.3 ±1.9, Cassi & Issac −3.9 ±2.2, Stellar −5.1 ±2.0 — [CSV].

**Pairs** (same team, n≥400, sorted by Elo residual). Source: [CSV] `pair_stats.csv`.
- Best pairs:
  - Vesper Vale + Willow: +3.6pp (n=1,841)
  - Vendetta + Vesper Vale: +2.6pp (n=7,534, WR 53.7%)
  - Heart of Ruin + Vendetta: +2.3pp (n=6,243)
  - Marcella + Vesper Vale: +2.3pp (n=7,877, WR 56.3%)
  - Heart of Ruin + Marcella: +2.2pp (n=8,571, WR 56.8%)
- Worst pairs:
  - Prism + Vendetta: −2.9pp (n=876)
  - Cassi & Issac + Heart of Ruin: −2.8pp (n=3,366)
  - Heart of Ruin + Stellar: −2.5pp (n=10,227)
  - Heart of Ruin + Varg: −2.1pp (n=5,596)
  - Ren + Stellar: −1.7pp (n=17,183)
  - Cassi & Issac + Stellar: −1.7pp (n=13,889)

**Full 5-pick comps.** Source: [FEAT].
- Most common overall: Cassi & Issac/Ren/Ryth/Stellar/Varg (n=3,556, WR 43.6% [42.0–45.3], resid −1.6pp; only 1% of these games were played by top-20 teams).
- Most common among the top 20:
  - HoR/Marcella/Ryth/Vendetta/Vesper Vale: n=775, WR 75.9% [72.7–78.8]. Ladder-wide it has n=2,155, WR 57.5%, resid +1.8pp.
  - Cassi & Issac/HoR/Ren/Ryth/Stellar: n=506, WR 91.1% [88.3–93.3]. Ladder-wide resid is −4.0pp: it wins because the teams playing it are strong, not because the comp is.
  - HoR/Ren/Ryth/Stellar/Varg: n=390, WR 66.9%.
- Marcella/Ryth/Tavi/Vendetta/Vesper Vale had the best residual among frequent comps: +8.8pp (n=295, WR 58.0%).

**Draft order.** Source: [FEAT].
- First pick is Heart of Ruin in 28.0% (A) and 30.4% (B) of drafts. Next come Cassi & Issac (about 16%), Varg (12–15%) and Stellar (10–12%).
- Elo residual by first pick: Heart of Ruin −0.0pp (n=13,641), Marcella +2.4pp (n=2,258), Vendetta +1.4pp, Varg +1.1pp, Stellar −1.4pp, Ren −4.0pp (n=3,105).
- Mean pick position (0 = first) for Heart of Ruin: top20 0.53, 21–50 0.43, 51–150 0.60, 301+ 1.10. Cassi & Issac is picked later by top teams (2.23) than by 301+ teams (1.05).

**Head-to-head** (Elo residual of champion X when the opponent has Y, n≥600). Source: [FEAT].
- Vesper Vale vs Heart of Ruin: +6.1pp (n=1,245)
- Heart of Ruin vs Stellar: +5.7pp (n=2,123)
- Vendetta vs Varg: +5.6pp (n=2,121)
- Vesper Vale vs Stellar: +5.4pp (n=4,551)
- Marcella vs Stellar: +5.2pp (n=3,479)
- Vendetta vs Ren: +4.8pp (n=3,591)

### Inferences
- The strength-adjusted signal (team-demeaned and Elo residual) agrees with what the top teams pick: Heart of Ruin, Marcella, Vesper Vale and Vendetta are the meta, and Stellar, Varg, Cassi & Issac and Ren are traps.
  - Stellar is the clearest case. It is the 2nd most-picked champion overall, yet it has the worst adjusted effect in every bucket from top20 through 151–300.
  - Raw win rates mislead here. Heart of Ruin's 59% raw rate is partly team strength, yet even within the same team it adds about +6pp.
- Vesper Vale and Vendetta's good results against Heart of Ruin and Stellar suggest the B-side "answer" to a Heart of Ruin first pick is Vesper Vale or Vendetta.
  - Picks are revealed simultaneously each round, so this works only after the earlier round's pick is visible.
- The draft is effectively symmetric, so side choice is not exploitable in ranked play.

### Gaps
- Effects of 1–6pp are robust at these sample sizes, but champion choices are confounded with bot quality and map (teams draft by map).
  - The team-demeaned metric partly controls this, but teams that switch comps also switch strategies.
  - Proper causal estimates would need regression with team fixed effects plus a map term.
- The balance-patch status per engine version (1.0.1 vs 1.0.2) was not separated out. [RULES] says the patch is "not yet in the engine".

## Q3. Map-level effects: win conditions, game lengths, champion over-performance by map

### Takeaway
Overall:
- 73.2% of games end by a team reaching 1,000 tributed gems.
- 22.8% reach round 2,000 and are decided on tributed gems.
- 3.5% are decided on the gold tie-break, 0.3% on the all-tie Team-B default, and 0.2% end early in other ways.

Maps differ enormously:
- Gem-race wins range from 26% (49807e4756ec 60×50, 0b8e85603a85 80×80) to 97–98% (4621e92590be 30×30, 566697c98eac 35×31).
- On a few maps the gold tie-break matters a lot: 53086e249294 at 52% and 554240a5a793 at 30%. On those maps both teams often finish with 0 gems.
- Map area barely predicts any of this (r = −0.12).

### Cited Findings
- **Win types** (n=23,373):

  | Win type | Share | Team B win rate |
  |---|---|---|
  | gems ≥1,000 | 73.2% [72.7–73.8] | 58.3% |
  | round limit, gems | 22.8% [22.2–23.3] | 49.7% |
  | round limit, gold | 3.5% [3.2–3.7] | 53.3% |
  | all-tie Team-B default | 0.3% (n=76) | 100% |
  | other early ends | 0.2% (n=48) | — |

  Source: [FEAT].
- **Round-limit games** (n=6,210): 13.2% end with 0 tributed gems for both teams; the winner's median tributed gems is 600 — [FEAT].
- **Selected maps** (full table in [CSV] `map_stats.csv`):

  | Map | Size | Picks | n | Gems-1000 % | RL-gems % | RL-gold % | Both 0 gems % | Median length of gem wins | Mean total kills |
  |---|---|---|---|---|---|---|---|---|---|
  | 49807e4756ec | 60×50 | 5 | 633 | 25.6 | 56.6 | 17.7 | 13.1 | 1,739 | 25.3 |
  | 0b8e85603a85 | 80×80 | 5 | 557 | 26.2 | 70.4 | 1.4 | 3.1 | 1,795 | 26.7 |
  | 53086e249294 | 50×41 | 5 | 624 | 32.2 | 15.4 | 52.1 | 52.4 | 1,362 | 6.4 |
  | 7d7ddb9c37c7 | 50×65 | 5 | 621 | 35.7 | 61.7 | 2.6 | 2.6 | 1,716 | 22.0 |
  | 554240a5a793 | 50×50 | 5 | 752 | 43.0 | 27.0 | 29.7 | 27.0 | 1,527 | 51.3 |
  | 6e564fdf09fd | 31×31 | 3 | 659 | 48.3 | 51.3 | 0.2 | 0.2 | 1,612 | 21.5 |
  | d97cf04b7383 | 60×60 | 5 | 683 | 80.1 | 16.0 | 2.8 | 4.0 | 1,190 | 9.6 |
  | 97906cc38f50 | 71×71 | 5 | 725 | 93.0 | 6.3 | 0 | 0.7 | 1,115 | 16.0 |
  | 261fe6c133f0 | 45×45 | 5 | 604 | 96.7 | 2.6 | 0.2 | 0.7 | 710 | 6.5 |
  | 4621e92590be | 30×30 | 5 | 737 | 97.2 | 2.3 | 0 | 0.5 | 945 | 19.0 |

  Source: [CSV].
- **Fastest maps:** the median gem-win length is 710 rounds on 261fe6c133f0, 729 on 566697c98eac (n=105) and 841 on 80af89f07723 (n=143) — [CSV].
- **Most kill-heavy maps:** 554240a5a793 (51.3 total kills per game), 72b8e03bc437 (48.1), e6f860670947 (47.2). The least: f1ecb7264728 100×100 (2.1) and 6a4203ae1bfe (4.6) — [CSV].
- **Ranked Team-B win rate per map** ranges from 40.5% (d97cf04b7383, n=333) to 57.0% (f1ecb7264728, n=337). With about 300 games per map the ±5.5pp CI makes most of these consistent with 50%, but d97cf04b7383 at 40.5% is about 3.5 SE from 50% — [CSV].
- **Champion × map Elo residual:** 359 cells with n≥150; 34 have |z|>3, about 1 expected by chance. Source: [CSV] `champ_map_resid.csv`.
  - Largest positive cells:
    - Vesper Vale on 6e564fdf09fd (31×31, 3-pick): +16.3pp ±6.2 (n=189)
    - Vesper Vale on 1763423b171f (40×40): +14.5 ±5.8
    - Marcella on 8ca26ed2ea65 (90×30): +13.5 ±5.6
    - Varg on af4236f408f8 (80×80): +13.3 ±5.6
    - Vendetta on 8ca26ed2ea65: +11.6 ±5.4
    - Cassi & Issac on 6a4203ae1bfe: +11.3 ±5.3
  - Largest negative cells:
    - Stellar on 1763423b171f: −16.9 ±5.4
    - Heart of Ruin on 6a4203ae1bfe: −16.1 ±6.2
    - Stellar on 8ca26ed2ea65: −12.8
    - Stellar on 6e564fdf09fd: −12.0
    - Heart of Ruin on f1ecb7264728 (100×100): −11.9
- **Pick rate by draft size** (all teams): Ryth 24% on 3-pick maps vs 74% on 5-pick maps; Stellar 50% vs 62%; Heart of Ruin about 41–45% regardless — [FEAT].

### Inferences
- A bot needs a credible round-2,000 plan on about 10 of the 40 maps. On those maps, tributed gems at round 2,000 and then lifetime gold earned decide the game, so banking gems early and farming gold safely beats risky late tributes.
- On 53086e249294 and 554240a5a793, simply tributing anything, or out-earning on gold, wins outright a large share of the time.
- Stellar's negative map cells line up with its overall weakness. The Vesper Vale, Marcella and Vendetta positive cells are concentrated on narrow or small maps (90×30, 31×31, 40×40). Map-specific draft tables are plausible but should be validated, because the per-cell CIs are ±5–7pp.

### Gaps
- Map names and layouts were not resolved from the hashes.
- Why some maps produce 0–0 gem games (probably a strong Monolith or altar control, or contested chokepoints) cannot be read from summaries alone.

## Q4. Economy and combat profile: top 20 vs mid-ladder vs our team

### Takeaway
The top 20 earn more and die far less:
- Gold earned: 8,140 per game vs 6,560 for ranks 51–150.
- Deaths: 7.2 vs 15.2.
- Gold at round 300: 1,320 vs 1,020.
- Reach the round limit in 6% of games vs 15%.

Even in ranked games between similar teams, top-20 teams average about 10 deaths vs about 15 for 51–150 teams. Top teams also buy Treasure (gold-gain) emblems more often.

Sydney's Realist (#91, 1,773) plays the ladder's most common low-tier comp (Varg/Stellar/Cassi & Issac/Tavi/Ren) and wins exactly its Elo expectation (30 of 64 vs 30.2 expected). Its losses are kill-driven: 23.7 deaths in losses vs 9.8 in wins.

### Cited Findings
**Per team-game means.** All games unless marked "ranked". SR = Sydney's Realist (c6a4a0d8…), LT = Latent Tactics (d35fcb89…), AG = "Anime girls against #2 period cramp" (0d643742…; n=13, ranked only). Sources: [CSV] `profiles.csv`; [FEAT] via `ours.py`.

| Metric | top20 | 21–50 | 51–150 | 151–300 | 301+ | SR | LT | AG |
|---|---|---|---|---|---|---|---|---|
| n (team-games) | 6,459 | 5,437 | 10,629 | 8,413 | 14,789 | 64 | 63 | 13 |
| Win rate | 72.3% | 52.4% | 45.1% | 43.9% | 46.6% | 46.9% | 44.4% | 69.2% |
| Wins by reaching 1,000 gems | 68.4% | 46.6% | 37.9% | 32.0% | 21.3% | 39.1% | 31.7% | 53.8% |
| Game goes to round 2,000 | 6.4% | 11.1% | 14.5% | 23.7% | 50.7% | 17.2% | 22.2% | 23.1% |
| Median length of own gem wins | 1,130 | 1,220 | 1,330 | 1,410 | 1,530 | 1,420 | 1,300 | 1,360 |
| Gems tributed | 862 | 725 | 666 | 618 | 508 | 733 | 644 | 793 |
| Gems collected | 1,010 | 888 | 838 | 776 | 635 | 829 | 748 | 1,000 |
| Gold earned | 8,140 | 7,110 | 6,560 | 6,110 | 5,170 | 6,680 | 5,050 | 7,670 |
| Kills | 11.6 | 10.1 | 9.5 | 10.4 | 9.1 | 10.4 | 9.6 | 13.5 |
| Deaths | 7.2 | 12.5 | 15.2 | 19.1 | 26.3 | 17.2 | 18.2 | 10.4 |
| Kill differential | +6.3 | +1.0 | −1.3 | −1.6 | −1.2 | −2.6 | −4.9 | +5.7 |
| Camp clears | 59.6 | 57.8 | 55.4 | 54.4 | 52.6 | 67.0 | 33.8 | 78.8 |
| Gems per tribute | 32.8 | 30.6 | 28.1 | 24.7 | 18.2 | 23.9 | 35.5 | 30.9 |
| Median first kill (round) | 188 | 192 | 207 | 206 | 239 | 164 | 265 | 332 |
| Median first tribute (round) | 118 | 115 | 119 | 118 | 118 | 101 | 123 | 216 |
| Gold earned by R100 | 353 | 312 | 292 | 256 | 200 | 314 | 285 | 351 |
| Gold earned by R300 | 1,320 | 1,110 | 1,020 | 878 | 669 | 943 | 890 | 1,090 |
| Gold earned by R500 | 2,550 | 2,060 | 1,860 | 1,580 | 1,200 | 1,650 | 1,540 | 1,970 |
| Gold earned by R1000 | 6,090 | 4,830 | 4,260 | 3,620 | 2,720 | 3,960 | 3,150 | 4,580 |
| Gems tributed by R500 | 220 | 179 | 155 | 136 | 98 | 140 | 152 | 167 |
| Gems tributed by R1000 | 621 | 483 | 416 | 358 | 255 | 401 | 382 | 502 |
| Any fuel-exhausted turn | 37.9% | 37.0% | 54.4% | 56.7% | 55.7% | 12.5% | 0% | 100% |
| Crash (botFault) rate | 0% | 0% | 0.01% | 0.05% | 0.76% | 0% | 0% | 0% |
| Healing done | 530 | 728 | 997 | 1,560 | 736 | 0 | 8,850 | 1,100 |

**Ranked only, which strips scrimmage selection.**
- top20 (n=647): WR 55.0%, deaths 9.9, gold 6,920, gold by R300 1,170, gems by R1000 545.
- 51–150 (n=2,920): WR 50.2%, deaths 15.3, gold 6,840, gold by R300 1,000, gems by R1000 426.

Source: [CSV].

**Matched opponents.** Source: [CSV].
- Top20 vs 51–150 (n=1,518): top-20 team wins 87.4%, gold 8,700 vs 4,310, kills 14.0 vs 3.7.
- Top20 vs top20 (n=2,262): deaths 9.6, gold 7,110, game length about 1,190.

**Per-champion production, top20 vs 51–150** (kills / deaths / gold / gems tributed per game). Source: [FEAT] via `champprod.py`.

| Champion | top20 | 51–150 |
|---|---|---|
| Heart of Ruin | 3.9 / 1.4 / 2,171 / 236 | 3.0 / 2.6 / 1,712 / 184 |
| Stellar | 3.3 / 1.7 / 1,846 / 194 | 2.6 / 3.4 / 1,234 / 114 |
| Vendetta | 2.8 / 1.4 / 2,027 / 233 | 2.3 / 2.7 / 1,645 / 182 |
| Marcella | 2.9 / 1.2 / 2,238 / 214 | 2.5 / 2.4 / 1,789 / 167 |
| Cassi | — | deaths 5.4, vs 2.4 for top20 |

Cassi & Issac die together, so each death counts for both — [RULES].

**Final items.** Source: [FEAT].
- Top-20 champions end with Treasure Emblem of Health 22% and Treasure Emblem of Focus 20% of the time. Treasure (+20% gold) does not appear in the 51–150 top-10 item list, so it is under 13% there.
- Durability Emblem of Health: 67% (top20) vs 56% (51–150). Bruiser Emblem of Health: 51% vs 37%.

**Sydney's Realist.** Source: [FEAT].
- 64 games: 39 ranked (19 wins, 18.9 expected) and 25 unranked (11 wins, 11.3 expected). Most opponents were ranked 51–150.
- Comp: Varg/Stellar/Cassi & Issac/Tavi/Ren in 39 games, Varg/Ryth/Stellar/Ren in 14. Never Heart of Ruin, Marcella, Vesper Vale or Vendetta.
- Wins: gold diff +243 at R300, kill diff +7.2, 9.8 deaths. Losses: gold diff −248 at R300 and −559 at R500, kill diff −11.3, 23.7 deaths.
- It led on gold at R300 in 48% of games.
- Highest camp-clear count of any group (67) and earliest first tribute (median R101). But only 52 item purchases per game against a ladder average of 107, and 0 healing.

**Latent Tactics.** Source: [FEAT].
- 63 games, 28 wins against 26.3 expected.
- Plays Heart of Ruin/Ryth/Ren/Willow/Marcella (17 games) and Marcella/Vesper Vale/Tavi/Willow(/Ryth) (24).
- Low gold earned (5,050) and only 33.8 camp clears. Losses average −1,065 gold at R500.

### Inferences
- The gap between the top 20 and mid-ladder is mostly survival plus early economy, not kill volume.
  - Top-20 teams get about the same kills as mid-ladder teams (11.6 vs 9.5) but die half as often.
  - They are already about 300 gold ahead by R300, and they finish gem races about 200 rounds faster.
- For Sydney's Realist the levers are:
  1. Avoid death spirals: deaths in losses are 2.4× those in wins.
  2. Move off the Stellar/Varg/Cassi & Issac/Ren core toward Heart of Ruin/Marcella/Vesper Vale/Vendetta.
  3. Spend gold; its item purchase count is half the ladder average. Its unusually low purchase-event count may also reflect buying patterns, such as consumables (11.7 per game against 4.8 average), and is worth checking against its own code.
- AG's 100% fuel-exhaustion rate (median 636 exhausted turns per game) together with a 69% win rate (n=13) is too small a sample to conclude anything.

### Gaps
- Which of the three candidate teams is the user's was not verified from the dump. Their sample sizes are small (64, 63 and 13 games), so their per-team rates carry wide intervals; for example, a 46.9% win rate on n=64 has a CI of about [35–59%].
- Item-purchase event counts may not equal the number of items bought (gold spent per buyItem event varies). I did not validate this.

## Q5. Crash rates and how often crashes decide games

### Takeaway
Crashes are negligible above rank 300:
- 0 of 11,896 top-50 team-games.
- 1 of 10,629 in 51–150 and 4 of 8,413 in 151–300.
- 112 of 14,789 (0.76%) in 301+.

Crashed teams win 33.1% [25.2–42.0] (n=118), against 36.6% expected from Elo. Crashes plausibly decided at most 77 of 23,373 games (0.33%). Our candidate teams had 0 crashes (n=64, 63 and 13). Fuel exhaustion is common (51% of team-games) but barely hurts win rates, except for very heavy exhaustion among mid-tier teams.

### Cited Findings
- **Crashes (any botFault event):** 118 team-games (0.25%). Of these, 101 started faulting before round 100. Faulting champions, by count: Varg 81, Issac 58, Vendetta 43, Ren 43 — [FEAT].
- **Crash-decided games:** a crashed loser against a clean winner happened in 77 of 23,373 games. There were also 28 draft-phase failures (empty draft, Team A resigned at round 0), all from 301+ teams — [FEAT].
- **Fuel exhaustion in 51–150 teams, by number of exhausted turns per game:**
  - 0: Elo resid −1.3pp, n=4,847
  - 4–50: resid −3.3pp, n=3,573
  - 51–500: resid −4.9pp, n=1,114
  - over 500: resid −10.3pp, n=75
  - 151–300 teams with over 500 exhausted turns: −12.6pp (n=168)
  - Top-20 teams with over 500 exhausted turns had +11.5pp (n=149), presumably deliberately heavy compute.

  Source: [FEAT].
- **Our candidates' fuel exhaustion:** SR had at least one exhausted turn in 12.5% of games (mean 0.7 turns); LT never exhausted fuel — [FEAT].
- **Rule context:** an out-of-fuel bot is suspended and resumes the next round. A draft bot that fails gets random picks — [RULES].

### Inferences
- Crash-proofing is no longer a differentiator for mid-ladder teams. Reducing chronic fuel exhaustion may be worth about 3–5pp for mid-tier bots that exhaust fuel tens to hundreds of times per game, although this is correlational.

### Gaps
- `diagnostics.codes` was always empty, so fault types (panic vs out-of-memory, etc.) cannot be distinguished.

## Q6. What statistically separates winners from losers?

### Takeaway
Early gold earned is the strongest early predictor:
- The leader in gold earned wins 64.9% of games at R100, 73.9% at R300 and 78.1% at R500.
- A lead of 400 or more gold at R300 wins 86% (n=10,993). A deficit of more than 300 wins only 16%.

Later, the tributed-gems lead dominates: the R1000 leader wins 86.4%. A kill lead is a weaker signal (73% at R500), and having fewer deaths is weaker still (69%). Winners out-earn losers on gold in 85.9% of games.

### Cited Findings
**P(leader at round R wins)**, counting only games still running at R and excluding ties. Source: [CSV] `separators.csv`.

| Metric | R100 | R200 | R300 | R500 | R750 | R1000 | R1500 |
|---|---|---|---|---|---|---|---|
| Gold earned | 64.9% (n=21,648) | 70.4 | 73.9 | 78.1 | 81.2 | 82.1 | 80.8 |
| Gems tributed | 57.8% (12,626; 10,703 ties) | 64.4 | 68.5 | 75.9 | 82.4 | 86.4 | 91.1 |
| Gems collected | 61.8 | 67.3 | 70.6 | 76.8 | 82.4 | 85.5 | 87.6 |
| Kills | 67.3% (7,478; 15,851 ties) | 68.9 | 70.9 | 73.3 | 75.1 | 75.6 | 72.6 |
| Fewer deaths | 61.9 | 64.0 | 65.8 | 68.6 | 70.3 | 70.6 | 66.4 |

All CIs are ±1pp or less.

**Win rate by gold-earned difference at R300** (team perspective). Source: [FEAT].

| Gold difference | Win rate | n |
|---|---|---|
| < −300 | 16% | 13,289 |
| −300 to −200 | 33% | 2,901 |
| −200 to −100 | 39% | 3,329 |
| −100 to 0 | 46% | 3,487 |
| 0 to +100 | 53% | 3,895 |
| +100 to +200 | 61% | 3,359 |
| +200 to +300 | 67% | 2,928 |
| +300 to +400 | 73% | 2,469 |
| ≥ +400 | 86% | 10,993 |

**Whole-game winner vs loser means** (n=23,373). Source: [FEAT].

| Metric | Winner | Loser | Winner higher (% of games) |
|---|---|---|---|
| Gems tributed | 892 | 386 | — |
| Gold earned | 8,242 | 4,353 | 85.9% |
| Monster damage | 136k | 84k | 84.6% |
| Item buys | 136 | 78 | 70.5% |
| Kills | 14.4 | 5.4 | 69.0% |
| Deaths | 12.8 | 23.5 | 23.8% (lower in 71.2%) |
| Camp clears | 65.9 | 44.5 | 75.5% |
| Tributes | 33.6 | 17.7 | — |

Wards (193 vs 172) and consumables (5.2 vs 4.4) barely differ.

**Gems per tribute** rises with tier: 18.2 (301+) to 32.8 (top20) — [CSV].

### Inferences
- The game is decided by economy tempo. A gold-earned deficit at R300 is already hard to recover from, and fewer than about 1 in 6 teams come back from 300+ behind.
- A practical diagnostic for the user's bot is to compare gold by R100 and R300 against the bucket table in Q4. Top 20 average 353 and 1,320; Sydney's Realist averages 314 and 943.
- Kills matter mainly because they swing gold (100 per kill) and steal carried gems [RULES], not as a goal in themselves.
- These are correlations, and the leader is usually also the better bot. Use them as targets and diagnostics rather than causal levers.

### Gaps
- I did not fit a multivariate model (for example, logistic regression of the win on gold, gem and kill differences at R300 with map fixed effects). It would give the marginal value of each metric and is a natural next step using `json/feat_*.jsonl`.
- Only 1 of 19 daily parts was analysed. Repeating this on the other 18 parts would tighten the per-map, per-pair and per-team estimates.
