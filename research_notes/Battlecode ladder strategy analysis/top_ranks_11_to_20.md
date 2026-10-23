# Ladder ranks 11–20 (Cambridge Battlecode 2026a): what their bots do, from recent matches

**Data and method (read first).**
- **Ladder snapshot.** Taken 2026-10-22 about 20:37 with `cambc ladder --limit 100 --page 1/2 --json`. Ratings run from 2043 (#11 Let's LARP again) down to 1988 (#20 ひななー). #10 EMPEROR Tung is at 2049 and #1 Intercontinental Fish at 2298. Ranks 80–110 span ratings 1791–1749 (#92 KKB 1770, #93 Stranger Danger 1770).
- **Match lists.** I pulled the last 100 ranked matches for each team with `cambc matches list --team <uuid> --kind ranked --limit 100 --json`. These cover 2026-10-22 16:20–19:30 UTC, so they are the current submissions. I also pulled 400 older ranked matches per team (back to 03:00 the same day) to check for draft changes.
- **Primary sample.** I downloaded 121 binary summaries with `cambc matches summary` and converted them with `cambc summary`, all on engine 1.0.3. That gives 165 team-sides: the latest three 5-map groups per team, plus one top-10 group where the latest three had none. Teams with no top-10 opponent among their last 100 ranked matches (print dont int, NTU, ひななー) have no top-10 data in this sample.
- **Secondary sample (zip).** The coordinator's dump `matchdb/summaries-2026-10-21-part1.zip` holds Oct 21 data on engines 1.0.1/1.0.2, so these are older submissions. From it I took all 117 matches of these 10 teams against ranks 76–115: 19 ranked and 98 unranked. It is the only data on how these teams play against the ~1770 band. Matchmaking is close to rating, so none of the 1,000 recent ranked matches (and only 36 of the 400 older ones, all Noobot / print dont int) were against ranks 76–115.
- **Where everything is stored.** Under `/private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top11-20/`:
  - `summ/<matchId>.json`: recent summaries
  - `zipband/<matchId>.json`: zip extracts
  - `extract_recent.json` and `extract_zip.json`: per-match metrics
  - `agg_recent.txt`, `agg_zip.txt`, `cross.txt`, `extra.txt` and `lists_report.txt`: per-team aggregates
  - The scripts that produced them are in `scratchpad/scripts/`.
- **How citations work.** Each link below is a match ID (`[3ed01335](cambc:matches/<full id>)`) or an aggregate file. To get the raw data for a match, run `cambc matches summary <id>`; full IDs are in the extract files.
- **ID mappings.** These come from `sdk/rust/src/types.rs`, which the docs do not fully cover.
  - **Draft IDs:** 0 Ryth, 1 Vesper Vale, 2 Vendetta, 3 Cassi & Issac, 4 Stellar, 5 Prism, 6 Marcella, 7 Willow, 8 Ren, 9 Varg, 10 Tavi, 11 **Heart of Ruin**.
    - Heart of Ruin is a melee bruiser with a self-shield. Its passive is up to +50% damage at low HP (`HEART_OF_RUIN_MISSING_HEALTH_DAMAGE_MAX_BONUS = 0.5`, `sdk/rust/src/constants.rs`). It is not in `docs/GameRules.md`.
    - In summaries, "Unknown champion (12)" is Heart of Ruin (ChampionType enum).
  - **Consumables:** raw ID 0 is the teleport crystal and raw ID 2 is the altar lock.
- **What the summaries contain.** Per-champion timelines (earned gold, tributed gems, kills, and so on), event lists (kills/deaths with x,y and round, camp clears, tributes, shop and emblem purchases, wards, `fuelExhausted`, `botFault`) and final metrics.
- **What they don't contain.** Per-round positions and ability casts. So anything about kiting, focus fire or ability use is inferred from damage, kill and position aggregates, not measured directly. I did not parse the binary `.repl26a` replays.
- **Derived metrics.**
  - "Kills in enemy half": share of a team's kills that happen closer, in Chebyshev distance, to the enemy spawn than to its own. Spawn location is estimated from where non-Tavi champions bought items.
  - "Camps": number of `campClear` events.
  - "Fuel events": number of `fuelExhausted` events, i.e. turns cut short by running out of fuel.

---

## Q1. What draft does each team use, and is it fixed or adapted per map/opponent?

### Takeaway
Nine of the ten teams draft a fixed list. The only thing that changes is how many picks the map allows (3, 4 or 5), and the list is cut short to fit. NEETCoders and hello switch lists by map. None of the ten visibly counter-picks the opponent. The band almost always drafts Vesper Vale (92% of sides), Ryth (82%), Marcella (80%), Vendetta (62%) and Heart of Ruin (59%).

### Cited Findings
**Draft per team (last 100 ranked matches each, pick order).** Source: `lists_report.txt` / draft-variability script output (`scratchpad/ladder-data/top11-20/lists/*.json`):
- **#11 Let's LARP again:** Heart-Vesper-Ryth-Marcella-Vendetta (62×), Heart-Vendetta-Ryth-Marcella (22×, 4-pick maps) and Vesper-Marcella-Vendetta (16×, 3-pick maps). The same draft is used on every map (0 of 38 maps had more than one draft). Older 400 matches were the same core.
- **#12 Trzy Kwaterki (TK):** Marcella-Vendetta-C&I-Ryth-Vesper (51×), Marcella-Vendetta-Ryth-Vesper (22×), Marcella-Vendetta-Ryth (15×), and Marcella-Vendetta-Ryth-Vesper-Heart (10×, only 17:30–18:10 UTC).
  - The Heart variant appears only in that time window. It looks like a submission swap, not an adaptation (inferred from timestamps).
  - Marcella is first pick in 98 of 100 matches.
- **#13 Noobot:** Marcella-Ryth-Vesper-Tavi-Willow (73×), Marcella-Ryth-Vesper-Willow (12×) and Marcella-Ryth-Vesper (15×). Fixed on all maps.
- **#14 print dont int (PDI):** Vesper-Heart-Vendetta-Ryth-Marcella (62×), Vesper-Heart-Vendetta-Ryth (22×) and Vesper-Marcella-Heart (12×).
  - It used Tavi as the 5th pick until about 16:20 and switched to Marcella after (submission change).
  - It is the only team that first-picks Vesper every time (100 of 100).
- **#15 Tartarus:** Marcella-Vesper-Tavi-Willow-Ryth (63×), the 4- and 3-pick truncations of it, and Prism-first twice. Fixed.
- **#16 NEETCoders:** draft depends on the map.
  - Heart-Marcella-Ryth-Vendetta-Vesper on most 5-pick maps.
  - Vesper-C&I-Varg-Vendetta-Stellar on great_wall, concentrated_wealth, pilgrimage, expanse, ambush and fracture.
  - Vesper-C&I-Marcella-Vendetta-Stellar on starvation, split, lanes and road_of_riches.
  - Heart-Vendetta-Vesper on 3-pick maps.
  - Each map always gets the same draft (0 of 38 maps had more than one). That points to a per-map table or map-feature rule, not opponent-reactive picking.
- **#17 Zarzorki:** Heart-Vesper-Marcella-Ryth-Vendetta and its truncations, always the same.
- **#18 NTU:** Heart-Ryth-Ren-Marcella-Vesper (64×), Heart-Ryth-Ren-Vesper (19×) and Heart-Ryth-Vesper (17×). It is the only team using Ren (Ren's side win rate is 43%).
- **#19 hello:** two families, chosen by map.
  - Vesper-Ryth-Vendetta-Tavi-Marcella on 5-pick maps.
  - Heart-Vesper-Marcella-Ryth on 4-pick maps.
  - Marcella-Vendetta-Tavi on 3-pick maps.
  - C&I-Heart-Varg-Marcella only on 00rows.
- **#20 ひななー:** Heart-Vesper-Ryth-Willow-Vendetta (62×), Heart-Vesper-Willow-Vendetta (14×), Vesper-Heart-Vendetta (20×), and Heart-Marcella-Ryth-Varg on 4 matches.

**Pick frequency in the band, all 1,000 recent sides.** Source: `cross.txt`
| Champion | Share of sides | Side win rate |
| --- | --- | --- |
| Vesper Vale | 92% | 49% |
| Ryth | 82% | 50% |
| Marcella | 80% | 49% |
| Vendetta | 62% | 51% |
| Heart of Ruin | 59% | 50% |
| Tavi | 26% | 47% |
| Willow | 25% | 50% |
| C&I | 9% | 53% |
| Ren | 8% | 43% |
| Stellar | 3% | 65% (n=31) |
| Varg | 3% | 63% (n=27) |
| Prism | 2 sides | – |

**First picks.** Heart 457 (side win rate 49%), Marcella 312 (47%), Vesper 226 (54%). Source: `cross.txt`

**Top-10 drafts against these teams.** Marcella 153 sides (66% win), Heart 152 (63%), Ryth 141 (65%), Vesper 127 (66%), Vendetta 115 (65%), Willow 62, Tavi 35, Prism 27 (74%). The top teams draft the same core champions as the 11–20 band. Source: `cross.txt`
- Examples: #9 Sponge uses Heart-Ryth-Vendetta-Willow-Marcella; #2 prism best girl uses Prism-Vesper-Marcella-Ryth-Heart; #6 Univardisty of Shingtonwash uses Marcella-Heart-Vesper-Ryth-Tavi. Source: `lists_report.txt`

### Inferences
- Champion choice is not what separates rank 11 from rank 1. Top-10 teams draft the same five-champion meta, and inside the band every core champion wins about 50% of the time.
- Stellar and Varg have higher win rates, but the samples are small and they come almost entirely from NEETCoders' map-specific draft, so the win rate probably reflects those maps.
- Nobody counter-drafts in practice. A per-map draft table, as NEETCoders appears to use, is feasible because the draft bot sees the whole map.

### Gaps
- I can't tell whether NEETCoders' and hello's per-map drafts are hard-coded by map name or computed from map features such as elk count or camp layout. The source code isn't available.
- Reading the 1024-byte draft memory isn't possible from summaries.

---

## Q2. How do they score gems and gold: timing of first income, peak rates, final totals?

### Takeaway
- Almost every team clears its first camp by round 10–30. The first tribute comes at a median of round 80–155, except NTU, whose median first tribute is round 224.
- The median team-side has tributed only 60–260 gems by round 500. Most games run 1,100–1,330 rounds and end when someone reaches 1,000 gems.
- Gold is what most clearly separates winners from losers. Winners earn about 1.5–2.5× the loser's lifetime gold, and the gap is already visible by round 250–500.

### Cited Findings
**Per-team medians, recent sample (engine 1.0.3, n = team-sides).** Source: `agg_recent.txt`, `extract_recent.json`
| Team | n | W-L | vs top10 | gems@100 | @300 | @500 | 1st tribute rnd | gold@250 | gold@500 | lifetime gold | K/match | D/match | camps | wards | altar locks/m | TP crystals/m | fuel-exhaust events/m |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| LARP (#11) | 15 | 8-7 | 4-6 | 4 | 79 | 193 | 91 | 818 | 1862 | 5230 | 6.9 | 11.7 | 50 | 17 | 5.0 | 0 | 0 |
| Trzy Kwaterki (#12) | 15 | 4-11 | 1-9 | 22 | 100 | 220 | 79 | 889 | 2056 | 5537 | 4.6 | 12.3 | 50 | 165 | 0 | 12.3 | 34.1 |
| Noobot (#13) | 15 | 10-5 | 2-3 | 0 | 66 | 187 | 155 | 903 | 2385 | 8528 | 7.1 | 4.6 | 41 | 90 | 9.1 | 7.9 | 1.1 |
| print dont int (#14) | 15 | 11-4 | – | 12 | 152 | 221 | 96 | 920 | 2079 | 7115 | 7.2 | 6.9 | 78 | 111 | 0 | 0 | 3.2 |
| Tartarus (#15) | 15 | 5-10 | 2-3 | 0 | 77 | 165 | 132 | 902 | 1749 | 4589 | 6.7 | 10.9 | 40 | 217 | 4.5 | 1.0 | 9.1 |
| NEETCoders (#16) | 20 | 8-12 | 2-3 | 0 | 78 | 214 | 113 | 978 | 2186 | 6104 | 7.2 | 9.6 | 44 | 98 | 4.3 | 0 | 3.5 |
| Zarzorki (#17) | 20 | 10-10 | 1-4 | 10 | 58 | 144 | 106 | 706 | 1909 | 6418 | 8.3 | 7.3 | 68 | 85 | 7.2 | 0 | 0.5 |
| NTU (#18) | 15 | 7-8 | – | 0 | 65 | 266 | 224 | 1248 | 3157 | 7686 | 8.1 | 8.3 | 61 | 168 | 8.3 | 0 | 4.5 |
| hello (#19) | 20 | 11-9 | 3-2 | 0 | 103 | 228 | 121 | 982 | 2152 | 7074 | 8.4 | 6.8 | 41 | 120 | 5.0 | 0 | 1.5 |
| ひななー (#20) | 15 | 5-10 | – | 26 | 96 | 182 | 82 | 1032 | 2230 | 5988 | 6.2 | 5.5 | 45 | 63 | 0 | 2.5 | 0.7 |

(For Trzy Kwaterki, Cassi and Issac each count a death, so its death totals are inflated.)

**Median first camp clear** (`agg_recent.txt`):
| Team | Median round of first camp clear |
| --- | --- |
| hello | 4–22 |
| NTU | 11 |
| NEET | 10–16 |
| TK | 10–22 |
| LARP | 20 |
| PDI | 17–32 |
| ひななー | 18–26 |
| Zarzorki | 24–29 |
| Tartarus | 30–40 |
| Noobot | 32 |

**Camps cleared by round 100.** Most teams clear about 5–6 camps in the first 100 rounds. Noobot (2.3) and Tartarus (3.6) are slow openers. Source: `agg_recent.txt`

**Tribute style.**
- **Small and frequent:** PDI averages 27 gems per tribute (median 28 tributes per match).
- **Hoarding:** Noobot averages 76 gems per tribute (19 tributes) and Zarzorki 59.5 gems (26.5 tributes). Source: `extra` metrics script output (avg_trib, n_trib)

**Altar locks get bought early.**
| Team | Median lock-purchase round | 25th percentile |
| --- | --- | --- |
| LARP | 167 | 76 |
| hello | 198 | 72 |
| NTU | 328 | 118 |
| Zarzorki | 331 | 136 |
| Tartarus | 370 | 156 |
| Noobot | 486 | 209 |
| NEET | 547 | 400 |

PDI, TK and ひななー buy no altar locks. TK and Noobot buy many teleport crystals instead: 184 and 118 purchases over 15 matches, at median rounds 460 and 650. Source: consumable-timing script output (`scratchpad/scripts`, run on `summ/*.json`)

**Gold separates winners from losers.**
- In matches between two band teams (n=90 sides), winners had median lifetime gold of 8,043 vs 5,036 for losers.
- At round 250 they were nearly level (952 vs 927), but by round 500 they had split (2,371 vs 2,079).
- Against top-10 teams, our teams' losses had our gold@500 of 1,679 vs the opponent's 2,760, and lifetime gold of 4,062 vs 8,645. Source: `cross.txt`

**Stalemate maps with huge gold totals.**
- On 01boss_battle, 00contest and 00stampede both teams often have 0 gems tributed at round 500. Some of these games go to round 1,800–2,000 and are decided late.
- Examples:
  - [06193921](cambc:matches/06193921-7344-470d-b8e8-b5bb83ad8787), NEET vs ひななー on boss_battle: first tribute at round 1,320, final score 1,024-0, lifetime gold 42,661 vs 60,254.
  - [98812711](cambc:matches/98812711-4ef8-46d6-93e1-a38ad73e4e4c), LARP vs Sponge on contest: first tribute at round 1,241, won 1,000-0 with 43-8 kills.
  - [23e71e0f](cambc:matches/23e71e0f-4c84-4c7c-9fc4-3a7ba739c9ff), TK vs prism best girl on contest: TK never tributed, lost 0-1,000 and was out-killed 4-39.
  - [db4d35d5](cambc:matches/db4d35d5-6a04-4a57-a19a-ffbf89283ee8), Zarzorki vs NTU on stampede: went to round 2,000 and finished 320-0 on tributed gems.

**How matches end.** Of the 450 recent matches against teams outside the band: 423 ended on gems, 26 on the gem tiebreak at round 2,000 and 1 on the gold tiebreak. Source: map-level script over `lists/*.json`

### Inferences
- In the opening, everyone is roughly even on gems. Mid-game camp throughput, gold to buy stats, and kills decide matches; gem totals at round 100–300 don't.
- On contest, boss_battle and stampede the altars seem to be contested, locked, or reachable only through monsters. Whoever controls the fight there wins on late tributes or the tiebreak. This is inferred: the summaries show no gems and many kills, but not the altar geometry.
- Our own bot needs a plan for zero-tribute stalemates: when to commit gems, and whether to lock the enemy altar.

### Gaps
- There are no per-camp-type rewards in the summaries. `campClear` names only a group ID, so I can't split gem income into wolves, elk and monolith or Golem.
- Monolith/Golem timing can't be measured without map files.

---

## Q3. How do they fight: when do they engage, focus fire, retreat, kite, use abilities?

### Takeaway
- Fighting is mostly a mid- and late-game activity. Only 8–18% of kills come before round 300.
- The median team's first champion kill is at round 160–440.
- Winners don't just trade better: they get 2–4× more kills and die far less (wins about 4 kills conceded; losses about 10–14).
- Ability-level behaviour (stun chains, focus fire, kiting) can't be measured from summaries. The proxies are damage split, Heart/Varg shield totals, Willow healing and kill positions.

### Cited Findings
**Kill timing** (recent sample; the opponent's kill split is in brackets). Source: `extra.txt`
| Team | Kills ≤ round 300 | Rounds 300–1000 | > round 1000 | Opponent split | Median first kill (round) |
| --- | --- | --- | --- | --- | --- |
| LARP | 15 | 55 | 33 | [27/99/29] | 191 |
| TK | 7 | 48 | 14 | [23/74/43] | 330 |
| Noobot | 8 | 66 | 32 | [5/39/8] | 443 |
| PDI | 12 | 46 | 50 | – | 326 |
| Tartarus | 15 | 60 | 26 | – | 223 |
| NEET | 26 | 78 | 40 | – | 209 |
| Zarzorki | 13 | 85 | 69 | – | 288 |
| NTU | 17 | 68 | 37 | – | 216 |
| hello | 30 | 98 | 40 | – | 159 |
| ひななー | 14 | 53 | 26 | – | 282 |

**Kills and deaths in wins versus losses.** Source: `agg_recent.txt`
| Team | Wins: K-D | Losses: K-D |
| --- | --- | --- |
| LARP | 11.0-4.0 | 3.6-19.4 |
| Zarzorki | 13.5-3.7 | 3.1-10.3 |
| hello | 12.4-3.1 | 3.6-11.3 |
| NTU | 14.3-7.0 | 3.0-11.4 |
| NEET | 10.0-5.6 | 5.3-12.2 |

**Where the kills happen** ("kills in enemy half").
- **Most aggressive:** Zarzorki 0.68 (0.70–0.72 in wins), hello 0.66, Noobot 0.57, ひななー 0.56, Tartarus 0.52.
- **Fights near their own base:** LARP 0.29 (0.45 in wins and only 0.02 in losses), TK 0.39, NEET 0.39.
- Source: `agg_recent.txt` (killsInEnemyHalf); method described in the header.

**Damage spent on champions versus camps** (champion damage ÷ monster damage).
- **Most fight-focused:** LARP 0.39, PDI 0.38, Zarzorki 0.34.
- **Most farm-focused:** Noobot 0.18, ひななー 0.18, NEET 0.20.
- In wins against top-10 teams, our champion damage was 27,005 vs the opponent's 18,910. In losses it was even (27,037 vs 28,643), yet the opponent got about 3× the kills (13.4 vs 4.5).
- Source: `extra` metrics output, `cross.txt`

**Shields and heals used.** Source: `extra` metrics output
- Shielding (Heart of Ruin and Varg): ひななー about 43k per match, PDI 27k, Zarzorki 25k, NTU 18k, LARP 17k.
- Healing (Willow): ひななー 13.7k, Noobot 12.7k, Tartarus 8.1k.
- Tartarus's Willow averages 1 kill, 2 deaths and only 601 gold earned per match. Noobot's Willow earns 132 gold per match, i.e. a pure support that doesn't farm.

**Camp deaths.** Recent sample: 0 camp deaths per match for every team. In the Oct-21 zip, about 1–2 per match across all 117 matches (ours 2.05 vs the mid-band's 3.21 in ranked games). The current bots appear to have fixed camp-fight safety. Source: `agg_recent.txt`, `cross.txt`

**Recalling isn't measurable.** The summaries have no "recall" event. A proxy is the number of matches where a team leads on gems at round 500 and still loses: our teams did this in 13 of 30 losses to top-10 teams and 12 of 45 losses to band teams. Source: `cross.txt`

### Inferences
- Several teams (LARP, TK, NEET) seem to fight only when enemies come into their own half, and lose when they're dragged into the enemy's half. LARP gets 0.02 of its kills in the enemy half in losses vs 0.45 in wins. Zarzorki and hello actively invade.
- Willow drafts (Noobot, Tartarus, ひななー) don't show up as a fight advantage: all three are 5-10 or worse except Noobot at 10-5, and Noobot's Willow contributes almost nothing in gold or kills. The value of a Willow seat is doubtful.
- Heart of Ruin with a big shield and the emblem builds below (Q4) make Heart the band's main frontline killer. Per match, Heart gets 8–13 kills in the zip mid-band games (NEET 13, NTU 12, LARP 10). In recent games against peers it gets 1–4 (`agg_*.txt` per-champ averages).

### Gaps
- No per-turn positions or ability-cast events, so kiting, focus fire (number of attackers per kill), retreat thresholds and stun usage can't be measured. A replay (`.repl26a`) parser would be needed. `cambc matches download` gives replays, but the format isn't documented in the SDK.
- Assist events exist and could stand in for focus fire (assists per kill). I didn't aggregate them.

---

## Q4. What do they do in the opening (~first 100 rounds) and in the late game?

### Takeaway
- **Opening:** all ten teams farm camps nearest-first and rarely fight. Kills before round 150 average 0.1–0.7 per match.
  - Fast openers (TK, ひななー, LARP, PDI) tribute by round 80–95 and lead early on gems at round 100: 22, 26, 4 and 12.
  - Slow openers (Noobot, NTU, Tartarus) have 0 gems at round 100 but bank more gold, which shows up later.
- **Shopping:** every team buys its passive emblems first (Treasure first, then Jungle, Durability or Home), and from about round 400 piles gold into Bruiser or Power emblems.
- **Late game:** the gold lead turns into stats and kills. Long games (over 1,300 rounds) are fights and altar-lock wars.

### Cited Findings
**Opening.**
- Median gems at round 100 range from 0 (Noobot, Tartarus, NEET, NTU, hello) to 26 (ひななー). Median first tribute ranges from round 79 (TK) to 224 (NTU). Source: table in Q2 / `agg_recent.txt`
- Kills before round 150 average 0.1–0.7 per match across the band; Noobot's is 0.13 and hello's 0.7. Source: `agg_recent.txt` (K<=150)

**Emblem order.** First three emblems per champion and the median round the first one completes. Source: emblem-order script output over `summ/*.json` (itemCombination events)
- **Noobot.**
  - Marcella: Treasure(Health) > Jungle(Focus) > Bruiser(Health), first at round 134 (15/15 matches).
  - Ryth: Treasure > Jungle > Bruiser at round 258.
  - Vesper: Treasure(Strength) > Jungle(Strength) at round 368.
  - Willow: no emblem until about round 765.
  - Marcella then stacks about 24 Bruiser-Focus emblems per match (362 over 15 matches). Source: `agg_recent.txt`
- **LARP.** Every champion opens Treasure(Health/Strength) > Jungle > Bruiser. First emblem: Heart at round 144, Marcella 167, Vendetta 199.
- **NTU.**
  - Vesper: Treasure > Jungle > Monster, first emblem at round 112.
  - Heart: Treasure(Health) > Jungle(Focus) > Durability at round 228.
  - Ryth: Durability > Jungle > Treasure at round 192.
- **Zarzorki.**
  - Vesper: Treasure > Power > Durability at round 136.
  - Heart: Power(Focus) > Durability > Treasure at round 409.
  - Marcella: Treasure(Focus) > Durability > Bruiser at round 343.
- **PDI and NEET.**
  - Heart: Treasure(Focus) > Power(Focus) > Home(Health), at round 277 for PDI and 217 for NEET.
  - NEET's Marcella: Monster(Strength) > Jungle > Power at round 191.
  - These are the only teams in the band using the **Home** emblem, which shortens recall.
- **Tartarus.** Opens with Jungle first, not Treasure. Marcella: Jungle(Focus) > Treasure > Monster at round 219; Tavi: Jungle > Treasure > Monster at round 198.
- **hello.**
  - Marcella: Jungle > Durability > Monster.
  - Vesper: Durability > Power > Jungle at round 236.
  - Tavi: Bruiser(Strength) first at round 332.

**Emblem families in total.** Bruiser and Power make up the bulk of what our teams buy over a match: Bruiser-Strength 1,751, Power-Strength 1,268, Bruiser-Health 1,144 and Bruiser-Focus 1,010 over 165 sides. Top-10 opponents also lean on Bruiser-Health (521) and Bruiser-Strength (444). Source: `cross.txt`

**Late game.**
- Matches past round 1,300 are mostly fight-driven stalemates. Examples:
  - [db4d35d5](cambc:matches/db4d35d5-6a04-4a57-a19a-ffbf89283ee8): Zarzorki 24-1 kills over 2,000 rounds.
  - [9a3cce49](cambc:matches/9a3cce49-7f8b-41a8-af0d-fa6702e6fd2a): PDI vs ひななー, 18-21 kills, ended on the gem tiebreak 648-936.
  - [6fba09dc](cambc:matches/6fba09dc-8c8c-4ab0-9fee-485c52baf622): hello vs #23, tied 640-640 on gems at round 2,000 and decided on gold.
- Zarzorki gets 69 of its 167 kills after round 1,000 and PDI 50 of 108. Source: `extra.txt`

### Inferences
- The band's shared build is: Treasure first (more gold from camps), then Jungle (takes less camp damage), then Durability/Monster, and after that put all spare gold into Bruiser or Power stat emblems.
- Teams whose first emblem finishes at round 110–170 (NTU's Vesper, Noobot's Marcella, LARP's Heart) out-earn on gold. NTU has the band's highest gold@500 at 3,157.
- Two dominant ways to play the opening: rush tributes (TK, ひななー) or bank gold and items (Noobot, NTU). The rushers lead on gems at round 100 but have the worst records here: TK 4-11, ひななー 5-10.

### Gaps
- Exact shard purchase timing (not just emblem completion) could be extracted from `buyItem` events. I didn't tabulate it.
- I couldn't measure opening routes: which camps and in what order. `campClear` gives only a group ID and (x,y), and I didn't join these against map files.

---

## Q5. Which concrete techniques are measurable in the data, and what are each team's weaknesses? (per-team profiles)

### Takeaway
Each team's profile is below. The measurable techniques: altar-lock spam (Zarzorki, NTU and Noobot buy 7–9 per match), teleport-crystal use (TK 12 and Noobot 8 per match), Treasure-first emblem economy, Home emblem (PDI and NEET), per-map drafting (NEET and hello), and ward volume (Tartarus, TK and NTU place 165–217 per match vs LARP's 17).
The weaknesses that recur: falling behind on gold by round 500, dying a lot in the enemy half, map-specific collapses (01islands, 00expanse, 00gemfight, 01big_world, 00reverse_spiral), and heavy fuel exhaustion for TK and, less so, Tartarus.

### Cited Findings
**#11 Let's LARP again (2043).** Heart-Vesper-Ryth-Marcella-Vendetta.
- Record 53-47 over the last 100 ranked games; 10-15 against the top 10. Source: `lists_report.txt`
- Recent sample: 8-7.
- Fights on its own side of the map (kills in enemy half 0.29) and places almost no wards (17 per match).
- Buys altar locks early (25th-percentile purchase at round 76).
- Zero fuel exhaustion in the sample.
- Wins are kill-heavy blowouts: [e2fec31e](cambc:matches/e2fec31e-53a6-4306-9047-8c2cea5ab97b) beat #6 1,007-0 with 9-2 kills on gold_rush; [98812711](cambc:matches/98812711-4ef8-46d6-93e1-a38ad73e4e4c) beat #9 1,000-0 on contest.
- **Weakness:** gets run over when it loses fights. It dies 19.4 times per loss, e.g. [3ed01335](cambc:matches/3ed01335-4925-43e7-bea6-f88faa55007f) with 11-32 kills against #6 despite leading 154-0 at round 100, and [fc061570](cambc:matches/fc061570-c6da-43f4-9aba-b5c3e601e912) with 4-29 kills against #9. Source: `agg_recent.txt`
- **Bad maps:** 00split 0-4, 01islands 0-4, 01big_world 0-3, 00packs 0-3. **Good maps:** rushbait_v2 4-0, pilgrimage 3-0, interlock 3-0.

**#12 Trzy Kwaterki (2031).** Marcella-Vendetta-C&I-Ryth-Vesper.
- 46-54 overall, 18-47 against the top 10 (the worst top-10 record in the band) but 28-7 against ranks 11–20. Source: `lists_report.txt`
- Recent sample: 4-11 (1-9 against the top 10).
- Fastest first tribute (round 79). Uses teleport crystals heavily (about 12 per match) and wards a lot (165 per match). Buys no altar locks.
- Kills come late; median first kill is round 330.
- **Fuel problems:** 34 fuel-exhaustion events per match, up to 101 in [858cd1eb](cambc:matches/858cd1eb-48b1-4f9f-91d6-c2f3efdbbde7) and 75 in [e83a31f6](cambc:matches/e83a31f6-64a5-468c-81a9-ce9b0e9ba00d). About 5 per match happen in round 1. Source: `agg_recent.txt`
- **Weakness:** loses fights to top teams badly. In [23e71e0f](cambc:matches/23e71e0f-4c84-4c7c-9fc4-3a7ba739c9ff) it never tributed and was out-killed 4-39 by #2; [d15686cc](cambc:matches/d15686cc-e48b-4e34-8898-81bed9b11896) went 7-21.
- In the Oct-21 unranked games against ranks 104/106 it was a spawn-camping deathball: 31–90 kills against 0–19, and in several 2,000-round games it hardly tributed (0-0, 160-0, 600-0). Example: [054181dd](cambc:matches/054181dd). That was the older engine 1.0.1/1.0.2 build. Source: `agg_zip.txt`

**#13 Noobot (2029).** Marcella-Ryth-Vesper-Tavi-Willow.
- 52-48 overall, 4-11 against the top 10. Recent sample: 10-5.
- Slow opener: 0 gems at round 100, median first tribute round 155, about 2.3 camps by round 100. Hoards gems (76 per tribute).
- Buys the most altar locks (9.1 per match) and also teleport crystals (7.9 per match). Wards 90 per match.
- Marcella carries the team: 4,709 gold, 509 gems and 5 kills per match.
- Beat #10 EMPEROR Tung on gem_fields and rushbait. Lost to #10 when it fell behind on gold: [cba52969](cambc:matches/cba52969-3872-4bef-bdb2-54d769a86888) 12,771 vs 16,597; [acf0c82a](cambc:matches/acf0c82a-098a-4f58-a240-ad00b32d7cf5) 5,283 vs 8,651.
- **Bad maps:** boss_battle 0-3, expanse 0-3. **Strong:** corridors 5-1, great_wall 4-0.
- Against the ~1770 band in the zip it went 20-11, the weakest mid-band record in this group. Its losses include 32-608 against #101 [032a2e8f](cambc:matches/032a2e8f) and 168-1,000 against #91 error404 (6-26 kills, [06b99255](cambc:matches/06b99255)). Source: `cross.txt`, `agg_zip.txt`

**#14 print dont int (2027).** Vesper-Heart-Vendetta-Ryth-Marcella.
- Best record in the band: 61-39, and 9 of its 12 gem-tiebreak games ended in wins. Recent sample: 11-4.
- Played no top-10 team in its last 100 games.
- Gets the most gems early (152 at round 300) and clears the most camps (78 per match). Tributes small and often (27 gems × 28).
- Uses no consumables. Heart builds Treasure > Power > Home.
- Fast-strike example: [4ffc2ec7](cambc:matches/4ffc2ec7-93e5-4e8c-b4aa-77afb404f9bd) had 231 gems at round 100 and won 1,001-678 in 674 rounds.
- **Weakness:** loses when out-killed, e.g. [925ef8ee](cambc:matches/925ef8ee-c7cb-48c4-ae0b-77b04f72cb7d) 1-13 kills against ひななー. **Bad maps:** fragment 0-3, expanse 0-3, hooks 1-3.
- Against the ~1770 band (zip) it went 15-1. Source: `lists_report.txt`, `agg_zip.txt`

**#15 Tartarus (2016).** Marcella-Vesper-Tavi-Willow-Ryth.
- 43-57 overall, 15-25 against the top 10. Recent sample: 5-10.
- Slowest camp start: first camp at round 30–40, 3.6 camps by round 100.
- Most wards in the band (217 per match). Buys altar locks (4.5 per match) and an occasional teleport crystal.
- Lowest lifetime gold in the band (4,589).
- **Fuel problems:** 87 fuel-exhaustion events in [be31afa7](cambc:matches/be31afa7-ee2f-409b-94b6-2e1e03f8f569) and 28 in [6f3a6866](cambc:matches/6f3a6866-ee41-4525-be05-11ac8122e26a).
- **Weakness:** loses 10.9 champions per match and loses on gold (1,749 at round 500). **Bad maps:** reverse_spiral 0-4, shortcut 0-4, gemfight 0-3. Source: `agg_recent.txt`, `lists_report.txt`

**#16 NEETCoders (2011).** Per-map drafts.
- 50-50 overall, 11-14 against the top 10. Recent sample: 8-12.
- The C&I/Stellar variant wins 65% of the time (Stellar sides 65%). Source: `cross.txt`
- Locks altars late (median purchase round 548).
- About 2.25 of its 3.5 fuel-exhaustion events per match happen in round 1, so precomputation runs over budget.
- Wins big: [0f0cf883](cambc:matches/0f0cf883-e7b8-4e65-ae6e-a9e70df44d63) beat #10 1,043-172 with 17-3 kills.
- **Weakness:** loses early fights. In losses it concedes 0.92 kills by round 150 (vs 0.12 in wins) and has 33 opponent gems at round 100. Example: [e0009135](cambc:matches/e0009135-dde9-4578-be11-defaba46f0a7), lost in 544 rounds with 288 gems vs 1,009.
- **Bad maps:** lost_world 0-4, hooks 0-3. **Good:** shortcut 4-0, starvation 5-1.

**#17 Zarzorki (2006).** Heart-Vesper-Marcella-Ryth-Vendetta.
- 47-53 overall; most recent opponents are ranked 21–75 (31-24 against them).
- Most invasive kill pattern in the band: 0.68 of kills in the enemy half. Buys 7.2 altar locks per match and hoards gems (59.5 per tribute).
- Long games: median 1,334 rounds; 7 of its wins left the opponent under 300 gems.
- Big example: [0f1a30a8](cambc:matches/0f1a30a8-4bc5-4dd9-990a-4ff921ce17e6), 38-3 kills and 1,012-0 against NTU.
- **Weakness:** slow gold (706 at round 250, the lowest in the band) and losses to Sponge #9 by kill margins of 4-18 and 0-11 ([30a2ad04](cambc:matches/30a2ad04-b29f-4d8e-b7c8-b581467912c0), [a0330ded](cambc:matches/a0330ded-4983-41cf-b0f2-640b63bc80e9)). **Bad maps:** islands 0-3, packs 0-3, big_world 0-3.
- Against the ~1770 band it went 21-0 (zip, unranked), with 87 kills before round 300 across 21 matches. Source: `extra.txt`

**#18 NTU (1996).** Heart-Ryth-Ren-Marcella-Vesper.
- 49-51 overall. Recent sample: 7-8. Played no top-10 team in its last 100 games.
- Banks first: median first tribute round 224 and 0 gems at round 100, but the highest gold in the band (1,248 at round 250, 3,157 at round 500). Its Vesper finishes a Treasure emblem by round 112.
- 8.3 altar locks per match; wards 168 per match.
- **Weakness:** concedes early gems (opponents have 30 at round 100) and loses kill races (3.0-11.4 in losses). It is 0 of 3 recent games against Zarzorki's invasions ([0f1a30a8](cambc:matches/0f1a30a8-4bc5-4dd9-990a-4ff921ce17e6) 3-38, [db4d35d5](cambc:matches/db4d35d5-6a04-4a57-a19a-ffbf89283ee8) 1-24).
- **Bad maps:** concentrated_wealth 0-5, hooks, elkring, stampede and monolith each 0-3.

**#19 hello (1996).** Per-map drafts.
- 49-51 overall but 6-4 against the top 10 (the best top-10 record in the band). Recent sample: 11-9, including 3-2 against #4 policy wonk.
- Earliest first camp (median round 4–22) and most early kills (30 kills before round 300 across 20 matches). Invades (0.66 of kills in enemy half).
- Buys altar locks early (25th-percentile purchase at round 72).
- Example win against #4: [402de614](cambc:matches/402de614-bec0-419d-bb93-c57577c2a2fd), 858 gems by round 500 and won in 656 rounds.
- **Weakness:** an all-or-nothing gold curve. In losses it has 4,274 lifetime gold vs the opponent's 7,310, e.g. [17c2309b](cambc:matches/17c2309b-25ab-4347-9580-7b45c0320f2f), 11-25 kills and 3,335 vs 15,206 gold against #4.
- **Bad maps:** pilgrimage 0-5, rushbait_v2 0-5, great_wall 0-3.

**#20 ひななー (1988).** Heart-Vesper-Ryth-Willow-Vendetta.
- 48-52 overall; played no top-10 team in its last 100 games. Recent sample: 5-10.
- Earliest gems in the band (26 at round 100, first tribute round 82). Biggest shield (43k per match) and heal (13.7k) totals. Uses some teleport crystals (2.5 per match) and no altar locks. Fewest wards (63 per match).
- **Weakness:** falls behind on camps (45 vs the opponent's 65). In losses, lifetime gold is 5,269 vs 7,924.
- **Bad maps:** the_farm 0-4, gemfight 0-3, corners 1-4. **Good:** hooks 4-0, ambush 3-0, elk_arena 3-0.

**Crash/fuel status across the band.**
- Zero `botFaults` in all 165 recent sides and all 117 zip sides.
- Fuel exhaustion is a real problem only for TK (34 per match) and Tartarus (9.1 per match). PDI, NEET and NTU have occasional round-1 overruns of 1–2 per match.
- Source: `agg_recent.txt` and `agg_zip.txt` (faults, fuel lines)

### Inferences
- Altar-lock users (Zarzorki, NTU, Noobot, LARP, hello) don't clearly beat non-users (PDI, TK, ひななー). PDI has the best record without any locks, so locks are not decisive at this level.
- TK's teleport-crystal-heavy bot, with fuel problems on top, is the weakest against strong fighters. Its strength is the fast tribute rush, which works against weaker or similar teams.
- Every team has 3–5 maps it goes 0-3 or worse on. A map-aware strategy (for example per-map drafts, as NEET and hello use) could recover many games.

### Gaps
- I couldn't tell from summaries whether altar locks are placed on enemy altars or used defensively; there are no placement events.
- Exact ward positions and their timing relative to fights weren't analysed. Ward events do have x,y if needed.

---

## Q6. Cross-team patterns: how do these teams lose to the top 10, and how do they beat the ~1770 band?

### Takeaway
- **Against the top 10, the band loses 2:1** (15-30 in the recent sample; 65-120 by list record across the seven teams that played them in their last 100 games: LARP 10-15, TK 18-47, Noobot 4-11, Tartarus 15-25, NEET 11-14, Zarzorki 1-4, hello 6-4).
- **Gems aren't the reason.** In 13 of the 30 losses our team led on gems at round 500.
- **What top-10 teams do better:**
  - Out-earn on gold from round 250 (1,078 vs 795), and are far ahead by round 500 (2,760 vs 1,679).
  - Clear more camps (57 vs 42).
  - Turn that into about 3× the kills (13.4 vs 4.5 per match), mostly in our half of the map.
  - Tribute bigger bags later.
- **Against the ~1770 band, these teams win about 90%** (13 of 19 ranked and 89 of 98 unranked, Oct-21 data). They do it by doubling gold and kills, not by early gems.

### Cited Findings
**Losses to top-10 teams** (n=30 sides, recent sample).
| Metric | Our team | Top-10 opponent |
| --- | --- | --- |
| Gold @ round 250 | 795 | 1,078 |
| Gold @ round 500 | 1,679 | 2,760 |
| Lifetime gold | 4,062 | 8,645 |
| Camps | 42 | 57 |
| Kills | 4.53 | 13.37 |
| Gems @ round 500 | 128 | 190 |
| First tribute (round) | 114 | 192 |
| Average tribute size | 51 | 77 |
| Altar locks | 4.3 | 5.1 |
| Final gems | 446 | 1,022 |

- 69% of our deaths happened in our own half.
- Top-10 opponents bought 461 teleport crystals and 242 altar locks over these 45 matches.
- Source: `cross.txt`

**Wins against top-10 teams** (n=15). The pattern reverses: our gold@500 was 2,394 vs 1,536, and our kills 11.9 vs 3.5. Source: `cross.txt`

**Which top-10 opponents the band loses to.**
- #9 Sponge (Heart-Ryth-Vendetta-Willow-Marcella) beat LARP 3-2, TK 4-1, Tartarus 3-2 and Zarzorki 4-1 in the sampled groups. It won through kill margins of 17-29 per game, e.g. [fc061570](cambc:matches/fc061570-c6da-43f4-9aba-b5c3e601e912) 29-4 and [32334dea](cambc:matches/32334dea-9b5a-4140-8334-9720f6ff8d7b) 17-2.
- #2 prism best girl (Prism-first) beat TK 5-0, out-killing it 39-4 and 21-7.
- Source: `agg_recent.txt`

**Map weaknesses against teams outside the band** (450 matches). Source: map-level script over `lists/*.json`
- **Worst maps:**
  | Map | W/n | vs top 10 |
  | --- | --- | --- |
  | 01islands | 0/12 | 0/2 |
  | 00expanse | 2/10 | 0/3 |
  | 00gemfight | 2/9 | 0/4 |
  | 01big_world | 3/13 | 0/4 |
  | 00reverse_spiral | 4/13 | 0/5 |
  | 00pilgrimage | 3/8 | – |
  | 01starfish | – | 0/3 |
  | 00corners | – | 0/4 |
  | 01starvation | – | 0/4 |
- **Best maps:** great_wall 6/7, lanes 7/9, quadrants 6/8 (5/5 vs top 10), elk_arena 6/8, concentrated_wealth 8/11.

**Beating the ~1770 band** (zip, Oct 21, engines 1.0.1/1.0.2).
| Metric | Ranked (n=19, 13 wins) | Unranked (n=98, 89 wins) |
| --- | --- | --- |
| Final gems (ours–theirs) | 1,003–722 | 1,006–228 |
| Gold @ round 500 | 1,871–1,350 | 2,757–1,569 |
| Lifetime gold | 6,360–5,520 | 8,584–3,320 |
| Kills | 8.0–5.1 | 23.7–4.0 |
| Camp deaths | 2.05–3.21 | 1.85–3.04 |

- Camps cleared by round 100 were similar (3.5 vs 2.8). Source: `cross.txt`

**Records against the ~1770 band, by team** (zip). Source: `agg_zip.txt`
| Team | W-L |
| --- | --- |
| Trzy Kwaterki | 10-0 |
| Zarzorki | 21-0 |
| print dont int | 15-1 |
| Tartarus | 6-0 |
| NTU | 6-0 |
| hello | 5-0 |
| LARP | 4-0 |
| NEET | 5-1 |
| ひななー | 10-2 |
| Noobot | 20-11 (weakest) |

**What the mid-band losses looked like.**
- Ranked: [0323d6ba](cambc:matches/0323d6ba), ひななー 158-1,016 with 1-20 kills against #107 Danger Bille.
- Unranked: [06b99255](cambc:matches/06b99255), Noobot 168-1,000 with 6-26 kills against #91 error404.
- Source: `cross.txt`

### Inferences
- **What separates ranks 11–20 from the top 10:**
  1. Faster gold in rounds 250–500, from Treasure-first builds and more camps.
  2. Converting that gold into stat emblems that win fights decisively in the opponent's half.
  3. Not overcommitting to early tributes.

  Our teams frequently out-tribute top teams early and still lose (13 of 30), so gem-rushing alone doesn't beat a gold-and-kill snowball.
- **Against ~1770 teams, the band wins by:**
  - Farming more safely (fewer camp deaths).
  - Out-earning 1.5–2.5× on gold.
  - Out-killing the opponent: a modest 8-5 in ranked games, 24-4 in unranked ones, where the mid-band bots look much weaker in fights.
- **For a team at about 1770** (inferred guidance from the patterns above):
  - **Biggest lever:** close the gold@500 gap with an early Treasure (then Jungle) emblem on the main farmer, and avoid camp deaths.
  - **Second:** avoid fights in the enemy half unless ahead on items.
  - **Third:** have a plan for the stalemate maps (contest, boss_battle, stampede) and the weak maps above.
- **Caveat:** the mid-band evidence is from the previous day's submissions on older engines, and 98 of 117 games were unranked, which may include test bots (e.g. TK's 2,000-round 0-0 games).

### Gaps
- None of these teams has a current (Oct 22, engine 1.0.3) ranked game against ranks 76–115. The mid-band comparison rests on Oct 21 data.
- I didn't download any of the ~1770 teams' own matches against each other, so there is no within-band baseline for them.
- The top-10 analysis covers only opponents met in the sampled groups: #2, #4, #6, #9 and #10. The other top-10 teams (#1, #3, #5, #7, #8) weren't sampled.
