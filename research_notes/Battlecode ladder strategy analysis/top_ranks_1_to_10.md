# Strategies of the top 10 teams (ranks 1–10), Cambridge Battlecode 2026a ladder

Scope and method (applies to all sections below):
- Ladder snapshot taken 2026-10-22 ~20:37 local with `cambc ladder --limit 25 --json`. Ratings: 1 Intercontinental Fish 2298, 2 prism best girl 2235 (flagged `isDev: true`, only 150 career matches), 3 procless 2169, 4 policy wonk 2139, 5 new team(4) 2126, 6 Univardisty of Shingtonwash 2118, 7 :3 2117, 8 Pantheon 2113, 9 Sponge(Albert and Bob) 2073, 10 EMPEROR Tung 2049 — [ladder.json](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/ladder.json)
- For each team: the 40 most recent ranked matches (`cambc matches list --team <UUID> --kind ranked --limit 40 --json`). All 400 team-games fall within 2026-10-22 18:20–19:30 UTC, i.e. current submissions only. 235 unique matches; 165 of them are top-10 vs top-10. The match summary of every one of the 235 was downloaded (`cambc matches summary <id>` then `cambc summary` to JSON). So each team has n = 40 games with full per-champion metrics, timelines and event logs — [lists/](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/lists), [sum/](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/sum)
- Derived per-team/per-match records: [records.jsonl](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/records.jsonl). Per-team aggregates: [agg_current.txt](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt) (cited below as AGG). Draft and meta tables: [meta_top10.txt](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/meta_top10.txt) (META) and [drafts_by_team.txt](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/drafts_by_team.txt) (DRAFTS). Analysis scripts are in the same folder (analyse.py, aggregate.py, meta.py, drafts.py).
- Match citations use the 8-character prefix of the match UUID. Each one resolves through `cambc matches show <id>` or all_matches.json in the same folder. Full IDs of the key matches: 2822eea9-b39a-45e2-8792-9d80f4502f44, 52d9249b-151b-4aa9-898d-85e835b47bd5, 9fff921f-a77c-4ecd-8935-474606118a9a, 8ba28be5-09c2-46f2-8daf-d1e3e5961535, 39d1c7a5-c0c6-4195-846b-ecbf834856a1, 23e71e0f-4c84-4c7c-9fc4-3a7ba739c9ff, 74843b98-ff79-4fd0-ad4f-19719aec57b6, 2c93c05f-e5fc-4310-b82b-322dc692a8ae, cd99ee3e-08f2-4397-ad27-9b5e0b1bdea5, 95436af5-4260-472c-bf72-88c6a9d02c09, 5aa61853-5573-473a-8509-5738c278fa7a, 0b870823-2503-4550-95e8-5a134a168ae6, 1ee7d02c-26a1-422f-ac46-fdba47e2406a, f53050cf-d5c4-4adf-9390-6ffe5627aaad, 07efcdf6-3749-4b27-b440-76cba6a5a5b1, c5a7de9a-9cec-49f8-95e1-14f340258b5a, 101c0e7d-1d61-4abf-b966-176899150729, 7eb96ed8-5875-4441-8199-2b2e3bbe704c, c9e03469-3d6c-4094-9a79-d93e96385c71, eb0146b8-74b5-45a3-a825-00b9f9507c12.
- ID mapping: the match lists' `draftPicks` use the SDK `ChampionDraft` enum (0 Ryth, 1 Vesper Vale, 2 Vendetta, 3 Cassi & Issac, 4 Stellar, 5 Prism, 6 Marcella, 7 Willow, 8 Ren, 9 Varg, 10 Tavi, 11 Heart of Ruin). Summary champion names use `ChampionType`, in which Cassi and Issac are separate (3/4) and Heart of Ruin is 12. Item IDs follow the `ItemType` enum. Consumable raw ID 2 is the altar lock and 0 the teleport crystal — [sdk/rust/src/types.rs](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/sdk/rust/src/types.rs)
- Secondary sample: an organiser-provided dump of 10-21 summaries, from which I took the 86 ranked top-10 vs top-10 matches (engine 1.0.2; the current games run on engine 1.0.3). I use it only for "what changed since yesterday" contrasts — [records_dump.jsonl](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/records_dump.jsonl), [agg_dump.txt](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_dump.txt)
- Metric definitions, so the numbers are interpretable. "g@N" is the team's tributed gems at round N (team submittedGems timeline), and "gold@N" is lifetime gold earned at round N; means are over games still running at round N. "First tribute" is the first round with tributed gems above 0. Kills, deaths and assists come from per-champion metrics. "Stolen" means gems collected from champion kills, and "lost" means gems lost on death. "Enemy-half share" is the fraction of events nearer (Chebyshev) to the enemy spawn than to their own; spawn is estimated as the centroid of non-Tavi buy locations. Fuel counts are `fuelExhausted` events.
- Not used: replays (`cambc matches download`, binary .repl26a) were not parsed. Positional and timing claims rely on summary event coordinates and timelines only.

## Q1. What draft does each team use, and is it fixed or adapted per map/opponent?

### Takeaway
Every top-10 team drafts deterministically by map: in this sample, no team ever used two different drafts on the same map, whatever the opponent or side. The draft size (3, 4 or 5 champions) is set by the map. Two families dominate. The first is "Heart of Ruin first" (procless, new team(4), :3, Sponge, EMPEROR Tung). The second is "Prism first + Vesper/Marcella" (prism best girl, Pantheon). The rank-1 team uniquely opens Vesper + Vendetta + HoR + Cassi & Issac, and policy wonk uniquely plays Marcella/Vesper/Tavi/Willow/Ryth.

### Cited Findings
- Main drafts in pick order, 40 games each — [DRAFTS](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/drafts_by_team.txt); [lists_table.txt](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/lists_table.txt):
  - r1 Intercontinental Fish: Vesper, Vendetta, HoR, C&I, Ryth (22/40). 4-champion maps: Vesper, Vendetta, HoR, C&I (7). 3-champion maps: Vesper, HoR, Vendetta (8). A Varg variant (Vesper, Vendetta, HoR, Varg, C&I) on 3 five-champion maps (e.g. 00expanse, 00road_of_riches).
  - r2 prism best girl: always Prism first. 5-champion maps: Prism, Vesper, Marcella, Ryth, HoR (12) or Prism, Marcella, Vendetta, HoR, Vesper (8) or Prism, Vesper, Marcella, Vendetta, HoR (5), chosen by map (e.g. 00great_wall, 00eight, 01starfish give the Vendetta version; 01elk_arena, 00contest, 01starvation give the Ryth version). 3-champion maps: Prism, Vesper, HoR. 4-champion maps: Prism, Vesper, Marcella, Ryth or Vendetta.
  - r3 procless: HoR, Vesper, Ryth, Marcella, Vendetta (26). 4 champions: HoR, Vesper, Ryth, Marcella. 3 champions: Vesper, HoR, Vendetta.
  - r4 policy wonk: Marcella, Vesper, Tavi, Willow, Ryth (29). 4 champions: drop Ryth. 3 champions: Marcella, Vesper, Tavi. One exception on 01boss_battle: Prism, Vesper, Tavi, Willow, Ryth.
  - r5 new team(4): HoR, Marcella, Vesper, Ryth, Vendetta (23). 4 champions: drop Vendetta. 3 champions: HoR, Marcella, Vesper.
  - r6 Univardisty of Shingtonwash: Marcella, Varg, HoR, Vendetta, Vesper (17) or Marcella, HoR, Vesper, Ryth, Tavi (6), by map. 3 champions: Marcella, HoR, Vesper, or Marcella, Varg, HoR on 01gold_rush.
  - r7 :3: HoR, Prism, Vendetta, Vesper, Marcella (17) or HoR, Prism, Ryth, Vesper, Marcella (6). 3 champions: Vesper, Vendetta, Tavi. One-offs with Stellar (01concentrated_wealth) and Ren (01islands).
  - r8 Pantheon: Prism, Vesper, Marcella, Ryth, HoR (17) or Prism, Vesper, Marcella, Vendetta, HoR (13). 3 champions: Prism, Vesper, HoR.
  - r9 Sponge: HoR, Ryth, Vendetta, Willow, Marcella (29). Subsets on smaller maps.
  - r10 EMPEROR Tung: HoR, Vesper, Marcella, Ryth, Vendetta (26). 3 champions: Marcella, Vendetta, Tavi.
- Consistency test: maps played more than once per team — 8 to 13 maps per team — with zero maps showing more than one draft, and zero (map, side) combinations with more than one draft. In other words, the draft is a pure function of the map in the sample (computed from the list JSON).
- Example of opponent-independence: prism best girl drafted Prism, Vesper, Marcella, Ryth on 00interlock against three different opponents, from both sides — [DRAFTS](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/drafts_by_team.txt)
- Draft size is fixed per map: 3-champion maps were 00collision, 00fragment, 00packs, 01broken_curve, 01gold_rush and 01the_farm; 4-champion maps were 00gemfight, 00interlock, 00reverse_spiral, 00rows, 00rushbait_v2, 00swirl, 01hooks and 01shortcut; the rest were 5-champion — [META](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/meta_top10.txt)
- Drafts changed sharply since 10-21. In the 10-21 dump, Intercontinental Fish played Stellar, HoR, Ren, C&I, Ryth (22/32). Pantheon played HoR, Marcella, Vesper, Ryth, Stellar, and Sponge played HoR, Vendetta, Ryth, Marcella, Ren. Ren and Stellar were common then (68 and 67 picks in 86 top-vs-top matches) but are almost absent now (1 pick each in 165 matches) — [records_dump.jsonl](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/records_dump.jsonl); [META](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/meta_top10.txt)
- The user's own bot notes describe the engine 3.0.4 balance patch as buffing Vesper, Vendetta, Tavi and Prism and nerfing Stellar, Heart of Ruin and Ren — [bots/bot-v5/src/draft.rs line 12](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/bots/bot-v5/src/draft.rs)

### Inferences
- The draft bots appear to read the map, at least the champion count and probably layout features, and ignore the opponent's earlier picks. Mirror picks are allowed (picking a champion the opponent already has is legal per docs/GameRules.md), so there is no denial incentive, and the data fits that. Because the drafts are predictable per map, a counter-draft can be precomputed per (opponent, map).
- The 10-21 to 10-22 shift (Stellar/Ren out; Vesper, Vendetta and Prism in) matches the patch description, so the current meta is a response to the balance patch rather than a stable equilibrium. This is inferred from the timing; I did not verify engine changelogs.
- procless and EMPEROR Tung share an identical 5-champion draft, the same first-buy pattern (Treasure emblems first, then Bruiser) and similar per-champion stat lines (see Q3). They may share code ancestry. This is a guess; nothing proves common authorship.
- prism best girl and Pantheon also share a draft template (Prism, Vesper, Marcella, Ryth/Vendetta, HoR) and Prism gem-hoarding, but their item builds differ (prism best girl stacks Bruiser and Power Health, Pantheon stacks Power Focus/Strength). That points to the same idea but different code.

### Gaps
- I could not see the draft bot's 1024-byte draft memory or its map heuristics. Why a team switches between, e.g., its Ryth and Vendetta fifth pick on particular maps is not measurable from summaries.
- Ten of the 40 maps appeared only once per team, so for those the "map-determined" claim is untested.

## Q2. How do they score gems/gold — timing of first income, peak rates, final totals?

### Takeaway
No top team rushes. All of them farm camps from round ~15–30 and make a first tribute between round ~80 and ~215. Games end by the 1,000-gem race at a median of ~1,000–1,150 rounds (229/235 by gems, 6 by tiebreak). Ranks 1–3 out-earn ranks 8–10 in gold from the very start (394 vs 334 gold at round 100; 2,670 vs 2,367 at round 500) and hold opponents to fewer gems at round 500 (211 vs 253). An early tribute lead by itself is not decisive: Pantheon has the fastest gem curve of all ten and still loses most games.

### Cited Findings
- Per-team economy, 40 games each; medians for first tribute, means otherwise — [AGG](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt):

| Rank / team | Win % | First tribute (median round) | Gems @100 / @300 / @500 | Opp gems @500 | Gold @100 / @300 / @500 | Final lifetime gold (own vs opp) | Median rounds to 1000 in wins |
|---|---|---|---|---|---|---|---|
| 1 Intercontinental Fish | 65 | 96 | 24 / 136 / 301 | 216 | 425 / 1475 / 2860 | 7285 vs 6026 | 980 |
| 2 prism best girl | 65 | 146 | 12 / 89 / 197 | 199 | 411 / 1455 / 2637 | 7247 vs 6680 | 1082 |
| 3 procless | 52 | 158 | 6 / 82 / 216 | 217 | 345 / 1320 / 2513 | 6557 vs 6731 | 1012 |
| 4 policy wonk | 65 | 132 | 22 / 99 / 207 | 201 | 398 / 1557 / 2958 | 9439 vs 7376 | 1118 |
| 5 new team(4) | 55 | 88 | 21 / 122 / 247 | 217 | 357 / 1327 / 2558 | 6272 vs 6783 | 982 |
| 6 Univardisty of Shingtonwash | 42 | 157 | 14 / 108 / 247 | 228 | 326 / 1212 / 2406 | 6981 vs 6165 | 1139 |
| 7 :3 | 52 | 215 | 8 / 75 / 193 | 252 | 336 / 1179 / 2421 | 6737 vs 6919 | 1063 |
| 8 Pantheon | 42 | 80 | 35 / 144 / 267 | 300 | 349 / 1350 / 2581 | 5372 vs 6499 | 1078 |
| 9 Sponge | 52 | 183 | 14 / 91 / 179 | 228 | 326 / 1192 / 2195 | 6710 vs 6439 | 1191 |
| 10 EMPEROR Tung | 40 | 165 | 18 / 88 / 164 | 232 | 327 / 1202 / 2336 | 7016 vs 7552 | 1121 |

- Group averages: ranks 1–3 (n = 120) had gold@100 393.7, gold@500 2,670 and opponents' gems@500 211.0. Ranks 8–10 (n = 120) had 334.1, 2,367 and 252.7. Within top-10-only matches, ranks 1–3 won 59% with gems@500 242 vs 215 for their opponents; ranks 8–10 won 39% with 218 vs 260 — computed from records.jsonl ([records.jsonl](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/records.jsonl))
- End reasons: 229 of 235 games ended by gems and 6 by tiebreak at round 2,000, on 01starvation (4 times), 01elk_arena and 01boss_battle. Example: policy wonk beat a team 790–540 on 01boss_battle with lifetime gold of 40,960 vs 38,713 (dfb781e5) — [all_matches.json](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/all_matches.json)
- Converting a lead at round 500 (wins / games led): policy wonk 14/14, Sponge 10/11, prism best girl 16/19, new team(4) 16/21, :3 10/13, IF 19/26, procless 17/24, UoS 13/18, Pantheon 9/14, EMPEROR Tung 6/10. Comebacks from behind at 500: :3 10/25, Sponge 10/26, policy wonk 7/20, prism best girl 6/16, Pantheon 6/22, EMPEROR Tung 8/26, procless 3/14 — records.jsonl
- Intercontinental Fish's economy runs on one carry. Vesper Vale earns 49% of the team's lifetime gold (3,696 gold per game). It buys about 104 Power Shards of Strength per game, i.e. ~3,100 gold of strength stacking, and collects 437 gems per game. In every other team Vesper's gold share is 19–37% — [AGG](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt); records.jsonl. Vesper's +33% gold passive is documented in [docs/GameRules.md](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/docs/GameRules.md)
- policy wonk's economy runs on Marcella. She earns 4,135 gold and submits 409 gems per game, building Bruiser Focus ×57 and Bruiser Health ×45 shards per game. Marcella gets 2 gold per tributed gem. The team has the highest final gold (9,439 per game) and the highest gold@300 (1,557) — [AGG](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt); passive in [docs/GameRules.md](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/docs/GameRules.md)
- Prism gem-hoarding (Prism gains Focus per carried gem; the SDK constant is 1.5 Focus per gem, while the docs say 1). Time-averaged gems carried by Prism: prism best girl 89.6 (median peak 216), Pantheon 81.8 (median peak 259), :3 57.6 (median peak 156). Other champions average roughly 2–26. Prism tributes only about half of what it collects (prism best girl 159 of 293 per game; Pantheon 112 of 261) — records.jsonl; [sdk/rust/src/constants.rs](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/sdk/rust/src/constants.rs)
- The extreme case: in 2822eea9 (00swirl) Pantheon's Prism carried 737 gems and tributed them in one channel at round 1,252. The team total was already 920, so it finished at 1,657 to 248. Pantheon's Prism also scored 10 kills there — [2822eea9 summary](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/sum/2822eea9.json)
- prism best girl's winning tributes are large: the median final (game-ending) tribute in its wins is 218 gems. IF's is 40, procless's 33 and UoS's 26. prism best girl made 47 single tributes of 150 or more in 40 games (20 by Marcella, 13 by Prism) — records.jsonl
- Monolith farming (a monolith gives 200 gems and 300 gold). Camp-gem collections of 100 or more per team over 40 games: prism best girl 28 (18 of them on 00contest), Intercontinental Fish 16 (5 on 00contest, 5 on 00stampede), new team(4) 11, Sponge 10 (4 on 00monolith, 4 on 01gold_rush), UoS 4, policy wonk 3, procless 2, EMPEROR Tung 1, :3 0, Pantheon 0. The first such capture comes at round ~260–650 — summary gemCollection events (all files in [sum/](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/sum)); monolith values from [docs/GameRules.md](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/docs/GameRules.md)
- Tribute cadence differs. Pantheon makes the most and smallest tributes (27.6 per game, average 31.8 gems), whereas prism best girl makes the fewest and largest (14.9 per game, average 75.3) — [AGG](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt)
- Since 10-21: Intercontinental Fish then had a median first tribute of 93.5 and reached 1,000 in a median 1,135 rounds; now it is 96 and 980 (wins). It is faster at closing out on the new draft — [agg_dump.txt](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_dump.txt); AGG

### Inferences
- Early gold separates ranks 1–3 from ranks 8–10 more than early tributes do. Pantheon has the best gems@100/@300 (35/144) but the second-worst final gold (5,372) and a 42% win rate. Tributing quickly and often in small bags looks like a lower-value plan than building gold/stat leads, or than banking big bags that a Prism or monolith can deliver late.
- Prism hoarding and monolith farming both produce "lumpy" late gem curves, so trailing at round 500 means less for prism best girl and :3 than for other teams. This fits prism best girl's high gems@500 and its very large final tributes.

### Gaps
- Per-camp-type income (wolves vs elk vs clusters) is not broken out; summaries tag the camp group ID, not the camp type. Monolith captures are inferred from collections of 100 or more gems.
- The gold@N figures are lifetime earned gold, so spending curves (and therefore item timing beyond the first buy) are only partly covered.

## Q3. How do they fight — when do they engage, focus fire, retreat, kite, use abilities?

### Takeaway
Fighting is opportunistic and mid-game. Median first kill is round ~100–320. Fewer than 1 kill per game happens before round 100 for every team. Kills lean towards the enemy half (56–81% of kills), and each kill is shared by about 2 assisters. Each team has one or two designated killers: IF's stacked Vesper (171 kills in 40 games), prism best girl's Vesper + Marcella (167 + 163), policy wonk's Marcella (164), Sponge's Heart of Ruin (244). Champions dying to camps is rare (≤2.7 per game).

### Cited Findings
- Per game: kills, deaths, median first kill round, kills before round 300, and percentage of kills in the enemy half. IF 9.1 / 12.9 / 194 / 1.7 / 71%. prism best girl 12.8 / 7.7 / 103 / 2.1 / 62%. procless 5.0 / 8.8 / 317 / 0.8 / 62%. policy wonk 9.2 / 8.9 / 162 / 1.7 / 78%. new team(4) 4.5 / 7.5 / 267 / 0.9 / 69%. UoS 7.6 / 6.5 / 170 / 1.7 / 81%. :3 6.9 / 8.6 / 148 / 1.3 / 65%. Pantheon 5.1 / 5.7 / 208 / 1.2 / 65%. Sponge 13.0 / 9.3 / 132 / 2.1 / 76%. EMPEROR Tung 4.4 / 9.0 / 250 / 0.8 / 56% — [AGG](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt)
- Kills before round 100 average 0.1–0.7 per game for all ten teams — AGG
- Who gets the kills (total over 40 games) — records.jsonl kill events:
  - IF: Vesper 171, Vendetta 88, HoR 40, Cassi 25, Ryth 21.
  - prism best girl: Vesper 167, Marcella 163, Prism 80, HoR 48.
  - procless: HoR 68, Vendetta 50, Vesper 33.
  - policy wonk: Marcella 164, Vesper 83, Tavi 68.
  - new team(4): Marcella 43, HoR 41, Vesper 40, Ryth 40 (evenly spread).
  - UoS: Marcella 90, HoR 89, Vesper 52, Varg 32.
  - :3: Vesper 98, Vendetta 58, Prism 52.
  - Pantheon: Marcella 45, Vesper 45, Prism 43, HoR 38.
  - Sponge: HoR 244, Vendetta 114, Marcella 87.
  - EMPEROR Tung: HoR 54, Marcella 46.
- Most-killed victims of the top teams: IF kills Vesper 83, HoR 81, Vendetta 58; prism best girl kills Vesper 93, Ryth 83, HoR 74, Vendetta 74 — records.jsonl
- Assists per kill (focus-fire proxy): IF 2.22, new team(4) 2.23, procless 2.15, prism best girl 2.10, policy wonk 2.00, :3 2.00, UoS 1.96, Pantheon 1.85, EMPEROR Tung 1.66, Sponge 1.62 — summary per-champion metrics
- Ryth as stun support: Ryth averages 11.0 assists per game for prism best girl (6.6 for Sponge, 5.7 for IF) but only 1–2 kills. In Ryth's kit, slot 0 is a radius-1 area stun — [AGG](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt); kit in [docs/GameRules.md](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/docs/GameRules.md)
- Healing and shielding: policy wonk's Willow heals 12,532 per game, and Sponge's Willow 7,702. Univardisty of Shingtonwash shields 56,769 per game, 3–5× any other team (Varg's self-shield plus HoR) — AGG
- Teleport crystals bought per game: Sponge 13.7, EMPEROR Tung 10.8, UoS 9.9, :3 9.3, new team(4) 8.2, IF 6.6, policy wonk 3.0, prism best girl 2.4, procless 0.8, Pantheon 0.1. Altar locks per game: new team(4) 8.8, Pantheon 8.7, :3 8.6, Sponge 7.4, EMPEROR Tung 6.5, policy wonk 5.1, IF 5.0, UoS 0.3, prism best girl 0, procless 0 — AGG (buyConsumable events)
- Wards per game: UoS 258, new team(4) 175, :3 179, IF 170, prism best girl 170, policy wonk 140, Sponge 109, procless 95, EMPEROR Tung 93, Pantheon 77 — AGG
- Champion damage dealt per game: prism best girl 47,100 and policy wonk 43,803 are the highest; Pantheon 22,649 and EMPEROR Tung 23,623 the lowest — AGG
- Heart of Ruin is not in docs/GameRules.md. The SDK gives it a passive of up to +50% damage at 100% missing health. The user's bot code describes its kit as a self-shield plus a landing dash (1.5×Focus damage and a one-round stun) — [sdk/rust/src/constants.rs](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/sdk/rust/src/constants.rs); [bots/bot-v5/src/common/attack.rs](file:///Users/tristanbasri/Desktop/Code/cambridge-battlecode-s2/bots/bot-v5/src/common/attack.rs) (secondary, user's notes)

### Inferences
- Since about 2 champions are credited per kill and kills cluster in the enemy half, top bots appear to collapse 2–3 champions onto an enemy caught near its own side, typically an enemy farmer or tributer. I could not separate this from simple pursuit because positions are only recorded at events.
- IF's and prism best girl's high kill share for Vesper suggests a ranged Vesper with stacked Strength and attack resets (slot 0) dominates trades and picks off other teams' Vespers (the most-killed victim of both). This is inferred from kill/victim counts plus the Power Strength stacking.
- Retreat and kiting behaviour cannot be measured from summaries. The low camp-death counts suggest every top bot checks HP before camp fights.

### Gaps
- Ability-cast counts, recall usage and stun counts are not in the summary format. Kiting and retreating are unmeasured.
- Altar-lock *placements* and lock-induced tribute cancellations are not logged; only purchases are.

## Q4. What do they do in the first ~100 rounds (opening) and in the late game?

### Takeaway
Openings are uniform. Champions split up and farm camps solo (first clear at round 14–31; 3.6–5.5 clears by round 100) and buy a full 150-gold emblem on their first shop visit (round 40–84). The first emblem is usually Treasure (+20% gold) or Jungle (−30% camp damage). There are almost no kills before round 100. The late game is decided by Prism/monolith mega-tributes (prism best girl, Pantheon), runaway carries (IF's Vesper, policy wonk's Marcella), or wipe-outs on fight-heavy maps (00contest, 01elk_arena).

### Cited Findings
- Opening metrics: median first camp clear round, mean clears before round 100, median first purchase round. IF 25.5 / 4.1 / 63. prism best girl 27 / 3.6 / 62. procless 31 / 4.2 / 48. policy wonk 22 / 4.3 / 42.5. new team(4) 27 / 5.2 / 48. UoS 23 / 4.2 / 64. :3 14 / 5.1 / 56.5. Pantheon 24.5 / 5.5 / 51. Sponge 17.5 / 5.2 / 83.5. EMPEROR Tung 25.5 / 4.7 / 39.5 — [AGG](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt)
- First purchases are bought 5 shards at a time (`buyItem` value "5", 150 gold), which immediately completes an emblem — summary buyItem events, e.g. [52d9249b](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/sum/52d9249b.json)
- Most common first emblem by team, from the first two buyItem events per champion:
  - Treasure first: procless (all 5 champions), EMPEROR Tung (all), Sponge (HoR Treasure Focus, others Treasure Health), policy wonk (Marcella Treasure Focus, Vesper/Tavi Treasure Strength), IF (Vesper Treasure Strength, Vendetta/HoR Treasure Health; Cassi Power Focus).
  - Jungle first: :3 (HoR Jungle Focus; Vesper, Vendetta and Marcella Jungle Health) and new team(4) (HoR Jungle Focus, Marcella Jungle Strength, Vesper Treasure Strength).
  - Pantheon: Prism Monster Focus, others Jungle.
  - prism best girl is alone in starting with stat/defensive emblems: Prism Durability Health, Vesper Power Strength, Marcella Bruiser Strength, HoR Power Health.
- Spawn-side play early on: only 10–30% of camp clears before round 300 happen in the enemy half for every team (procless 0.2, Pantheon 0.2, :3 0.2, policy wonk 0.3, IF 0.3) — AGG enemy-half shares
- :3's opening is crippled by fuel. It has 199 `fuelExhausted` events per game, ~190 of them before round 250, spread over all champions (e.g. Vesper 1,686 and HoR 1,691 events across 40 games). Its first tribute is the latest of the ten (median round 215), and it has the lowest gems@100 among the top teams (8.3) — fuel event timestamps in [sum/](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/sum); AGG
- Late game, Prism banking: Pantheon 2822eea9, the 737-gem single tribute at round 1,252 (see Q2). prism best girl's Prism tributed 154 or more gems 13 times in 40 games, and Pantheon's 13 times — records.jsonl subs
- Late game, monolith/contest maps: prism best girl won 00contest 1600–0 against procless (39d1c7a5) and 1000–0 against Trzy Kwaterki (23e71e0f, with 39 kills). IF won 00contest 1000–0 against new team(4) (74843b98, 32 kills to 2). Across the sample, 00contest produced 6 shutouts (loser under 100 gems) and 01elk_arena produced 5 — records.jsonl
- Late game, runaway carry: IF's Vesper ends games with ~104 Power Strength shards (308 in the 00swirl loss 2822eea9). policy wonk's Marcella buys ~102 Bruiser shards per game — AGG; [2822eea9](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/sum/2822eea9.json)
- The six round-2,000 tiebreak games were on 01starvation (4), 01elk_arena (1) and 01boss_battle (1): low-income or stalemate maps where neither side reaches 1,000 — records.jsonl

### Inferences
- Since everyone opens with solo farming and an economy emblem, the opening carries little differentiation at the top. The gaps that matter appear between rounds 100 and 500 (gold@300 and gold@500 gaps, Q2) and in how bags are converted late.
- :3 ranks 7th despite losing roughly the first 250 rounds of compute to fuel exhaustion. That implies its mid/late logic is strong, which fits its 10/25 comeback rate when behind at round 500. Fixing the opening fuel would likely be a large gain for them (inference).

### Gaps
- "Split vs grouped" farming cannot be measured: `campClear` credits a single champion per clear, so co-located farming is invisible. Without replay parsing, I could not get positions over time.

## Q5. Which concrete techniques are measurable in the data?

### Takeaway
Seven techniques can be measured: (1) hyper-carry stat stacking (IF's Vesper, policy wonk's Marcella); (2) Prism gem-banking for Focus followed by one huge tribute (prism best girl, Pantheon, partly :3); (3) monolith farming on 00contest, 00stampede and 00monolith (prism best girl, IF, Sponge); (4) heavy consumable use (altar locks: new team(4), Pantheon, :3, Sponge; teleport crystals: Sponge, EMPEROR Tung, UoS); (5) vision spam (UoS 258 wards per game); (6) sustain comps (Willow for policy wonk and Sponge; shields for UoS); (7) Treasure-first economy builds. Early rushing is essentially absent.

### Cited Findings
- Hyper-carry: IF's Vesper gets ~104 Power Strength shards per game and 49% of team gold. policy wonk's Marcella earns 4,135 gold per game and buys Bruiser Focus ×57 and Bruiser Health ×45 — [AGG](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/agg_current.txt)
- Prism banking: prism best girl's Prism carries 89.6 gems on average with a median peak of 216; Pantheon's 81.8 with a median peak of 259. Largest single tribute: 737 (2822eea9) — records.jsonl and summary `gems` timelines
- Monolith farming: 28 camp collections of 100+ gems for prism best girl and 16 for IF; zero for :3 and Pantheon — summary gemCollection events
- Altar locks bought per game: new team(4) 8.8, Pantheon 8.7, :3 8.6, Sponge 7.4; prism best girl 0 and procless 0. Teleport crystals per game: Sponge 13.7, EMPEROR Tung 10.8, UoS 9.9 — AGG
- Vision: UoS 258 wards per game against Pantheon's 77 — AGG
- Rushing: kills before round 100 are 0.1–0.7 per game for every team, and median first kill is ≥103 for all ten — AGG
- No Tavi "sell" usage: `sellItem` events average 0 for all ten teams, including Tavi users policy wonk and UoS — AGG (sells field)
- Bot faults, resignations and diagnostic codes: zero across all 235 summaries — summary diagnostics

### Inferences
- Altar locks do not separate the top. The two teams that never buy them (prism best girl, procless) are ranks 2 and 3, while heavy lock users fill ranks 5, 7, 8 and 9. Locks can delay but not prevent a tribute (5 attacks clear one), which may explain the low return.
- Vision spam does not translate into wins either (UoS 42% with the most wards). Ward counts may partly reflect ward-cooldown-driven placement code rather than deliberate map control.

### Gaps
- I cannot tell whether altar locks were actually placed on enemy altars, or how often they cancelled tributes, from the summary format.

## Q6. What are their weaknesses (maps or opponents they lose to and why)?

### Takeaway
Losses follow two patterns. Blowouts (finishing under 300 gems) come from team-fight wipe-outs on fight-heavy maps such as 01elk_arena, 00contest and 00eight. Close losses (700+ gems) are economy races lost by one or two tributes. IF loses mostly close races, but it collapses on 01elk_arena (0/2, including 0–1008 with 66 deaths against policy wonk). The bottom of the top 10 (EMPEROR Tung, UoS, Pantheon, :3) suffers the most blowouts.

### Cited Findings
- Losses per team (blowouts / close): IF 14 (4 / 8), prism best girl 14 (3 / 5), procless 19 (7 / 7), policy wonk 14 (4 / 1), new team(4) 18 (7 / 6), UoS 23 (8 / 7), :3 19 (8 / 5), Pantheon 23 (8 / 7), Sponge 19 (6 / 3), EMPEROR Tung 24 (10 / 5) — records.jsonl
- Per-team weak spots:
  - IF: 01elk_arena 0–2, including 9fff921f (0–1008, 3 kills to 66 deaths vs policy wonk) and 8ba28be5 (72–1044, 80 deaths vs prism best girl). Also lost 00swirl 248–1657 to Pantheon's Prism bank (2822eea9). Head-to-head: 2–3 vs prism best girl, 2–3 vs policy wonk, 5–5 vs procless. Undefeated maps: 01shortcut 3–0, 00fragment 3–0 — records.jsonl; [META](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/meta_top10.txt)
  - prism best girl: weak on 3-champion maps (2–4). Blown out on 01starvation by Pantheon (cc190268), on 00fragment by UoS (669bad49) and on 01concentrated_wealth by UoS (7322fa1e). Unbeaten on 01elk_arena 3–0, 00contest 3–0 and 00interlock 3–0 — records.jsonl
  - procless: 00eight 0–3, 01shortcut 0–2, 01starvation 0–2. Its losses have very low early gems: gems@100 2.2 and gems@300 57 in losses vs 9.0 and 106 in wins, and gold@500 2,143 vs 2,848. Only 3/14 comebacks when behind at round 500. First kill comes late (median 317) — AGG
  - policy wonk: 00great_wall 0–2, 00elkring 0–2, 00swirl 0–2. Against prism best girl it went 2–3, losing every game in which it fell behind at round 500 (7/20 comebacks overall) — records.jsonl
  - new team(4): 9–14 on 5-champion maps but 13–4 on 3- and 4-champion maps. Blowouts include 00contest 0–1000 vs IF (74843b98) and 01elk_arena 36–1044 vs policy wonk (c9e03469). 1–4 vs IF, 1–4 vs policy wonk and 1–4 vs Sponge; 5–0 vs EMPEROR Tung — META; records.jsonl
  - UoS: 01the_farm 0–4, 00quadrants 0–3, 2–7 on 3-champion maps. Ends games carrying 106 gems per team on average (173 in losses vs 16 in wins), i.e. it dies or the game ends with unbanked gems. 309 fuel events in 40 games (34 games affected). 0–5 vs IF — AGG; META
  - :3: 00split 0–2, 01broken_curve 0–2. Opening fuel exhaustion, about 190 events per game before round 250 (Q4). Loses 143.7 gems per game on death, the second-highest — AGG
  - Pantheon: 2–8 vs IF. In losses it falls far behind on gold (4,519 vs 7,335), even though its gems@100 are still high (30.8): a fast-tribute start that does not convert. Its 0.1 teleport crystals and 77 wards per game are the fewest in the top 10. Prism-centric draft on every map — AGG
  - Sponge: 00monolith 0–3, 00corners 0–2, 01yellow_brick_road 0–2. In losses it averages 12.4 deaths and loses 106 gems on death, with lifetime gold of 4,435 vs 9,185 — AGG
  - EMPEROR Tung: 1–9 on 4-champion maps. 00rushbait_v2 0–3, 01shortcut 0–2. 0–5 vs new team(4), 1–4 vs :3. 10 blowouts. Slowest gem curve of the top 10 (gems@500 164) — META; AGG
- The worst maps by shutouts: 00contest (6) and 01elk_arena (5) — records.jsonl

### Inferences
- The most exploitable top-team weakness is IF on 01elk_arena: comps with more burst and sustain (policy wonk's Marcella/Willow/Tavi, prism best girl's Prism/Marcella/Ryth) wipe it repeatedly there. On other maps IF's losses are close races, so it is hard to beat by out-farming.
- Teams that bank big (UoS's Marcella ends games still carrying 57 gems on average, with a time-averaged carry of 27; Prism users) leak gems when they die. Gem-stealing counter-play, killing the bag-carrier, has a measurable upside against them: UoS loses 85 gems per game and Pantheon 108.

### Gaps
- The causes of individual losses (e.g. which fight swung a close race) would need replay-level position data; I did not parse the .repl26a replays.
- Head-to-head cells are small (typically 5 games), so per-pair win rates are noisy.

## Q7. Cross-team patterns: the meta at the top, and what separates ranks 1–3 from 8–10

### Takeaway
The top-10 meta is Vesper Vale (91% of team-games), Heart of Ruin (85%), Marcella (78%), Vendetta (64%) and Ryth (56%), with Prism (31%) as the main alternative core. Win rates per champion are near 50% because mirrors are so common. Among picks that only one side had, Cassi & Issac (69%, n = 32; all IF), Vesper (66%, n = 29) and Tavi (55%) do best, and Marcella (41%), HoR (43%) and Varg (42%) worst. Ranks 1–3 separate from ranks 8–10 through higher early and mid gold (+18% at round 100, +13% at round 500), suppressing opponents' gems (211 vs 253 at round 500), more wards, and a clear carry plan. Lock spam, faster first tributes and more kills do not separate them.

### Cited Findings
- Pick rates and win rates in 165 top-vs-top matches (330 team-games): Vesper 301 picks (91%, win 51.5%), HoR 282 (85%, 48.9%), Marcella 257 (78%, 48.2%), Vendetta 210 (64%, 49.5%), Ryth 186 (56%, 48.9%), Prism 102 (31%, 51.0%), Tavi 42 (13%, 54.8%), Willow 42 (13%, 50.0%), C&I 32 (10%, 68.8%), Varg 24 (7%, 41.7%), Ren 1, Stellar 1 — [META](file:///private/tmp/claude-501/-Users-tristanbasri-Desktop-Code-cambridge-battlecode-s2/b3f4cb6b-e46a-405f-957c-42c44c05c8dc/scratchpad/ladder-data/top1-10/meta_top10.txt)
- Win rate when only one side had the champion: Vesper 66% (n = 29), C&I 69% (n = 32), Tavi 55% (n = 40), Prism 51% (n = 76), Willow 50% (n = 32), Vendetta 49% (n = 84), Ryth 47% (n = 66), HoR 43% (n = 42), Varg 42% (n = 24), Marcella 41% (n = 51) — META
- First picks: HoR 135 (46% wins), Prism 75 (51%), Marcella 68 (46%), Vesper 52 (65%) — META
- Head-to-head records in top-vs-top games: IF 26–14 (8–2 vs Pantheon, 5–0 vs UoS, 4–1 vs new team(4)); prism best girl 21–14; procless 21–19; policy wonk 18–12; new team(4) 17–18 (5–0 vs EMPEROR Tung); UoS 13–22; :3 18–17; Pantheon 17–23; Sponge 9–11; EMPEROR Tung 5–15 — META
- Side bias: side a won 124 and side b 111 of the 235 matches, a small first-pick/side-A edge — all_matches.json
- Group comparison (ranks 1–3 vs ranks 8–10, n = 120 each): gold@100 393.7 vs 334.1; gold@300 1,416 vs 1,248; gold@500 2,670 vs 2,367; opponents' gems@500 211 vs 253; wards 145 vs 93; altar locks 1.7 vs 7.5 per game; teleport crystals 3.2 vs 8.2; kills 9.0 vs 7.5; deaths 9.8 vs 8.0; median first tribute round 138 vs 126; fuel events 0.1 vs 6.6 per game — records.jsonl
- Robustness: ranks 1–3 have zero fuel events except procless (13 in 40 games). Ranks 6–10 all have some, from 4.6 per game (Pantheon) up to 199 per game (:3). No bot faults or crashes anywhere in the sample — summary events/diagnostics
- Distinct plans at the very top:
  - Rank 1 IF: Vesper hyper-carry plus Vendetta/HoR frontline plus C&I. Fast first tribute (median round 96), fastest close-out (median 980 rounds in wins).
  - Rank 2 prism best girl: Prism bank plus Marcella/Vesper killers plus Ryth stuns. Most kills (12.8 per game), most champion damage (47k), big late tributes, monolith farming on contest maps.
  - Rank 3 procless: pure farming. Fewest kills among ranks 1–3 (5.0) and most camp clears (54 per game); Treasure-first builds; no locks.
  - AGG; records.jsonl

### Inferences
- Ranks 1–3 do not share a single strategy (carry, Prism bank, pure farm). What they share is economic efficiency by round 300–500 and few self-inflicted losses: no fuel problems and few blowouts relative to ranks 8–10. The edge looks like execution quality (farming routes, safe tributing, trade selection) rather than a secret technique.
- Mid-ladder habits that do not show up at the top are lock and crystal spam and very early small tributes. These may be resource sinks: 10-gold consumables bought 7–14 times per game cost about 100–150 gold that top teams spend on stats instead.
- The surplus of Marcella and HoR picks with sub-50% one-sided win rates suggests they are over-picked "safe" bruisers. Vesper, C&I (IF only) and Tavi look under-picked relative to their results, but C&I's 69% mostly reflects IF's overall strength.

### Gaps
- Win rates by champion are confounded by team identity: each champion's results come mostly from a few teams. Ten teams and 165 matches are too few to isolate champion strength.
- The 10-21 dump comes from an older engine (1.0.2) and different submissions, so I do not compare it to current numbers beyond the draft shifts noted in Q1–Q2.
