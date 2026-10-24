# my-bot writeup log

A running log of every feature and change to `bots/my-bot`, written as we go so we can turn it into the final writeup. Add the newest entry at the bottom using the template at the end. Where we have them, include the commit, why we made the change, and how we measured it.

## Code layout

| File | What lives there |
|---|---|
| `src/main.rs` | Entry point: draft vs play dispatch, `random_number` / `random_seed` (with the per-build `SEED`) |
| `src/draft.rs`, `src/constants.rs` | Draft logic and the champion class table |
| `src/regions.rs` | Computed in the draft, read in play from the draft memory: which camp groups we can walk, dash or not get to |
| `src/common/rally.rs` | Elk rallies through the team's shared memory, on maps with enough elk |
| `src/bot.rs` | The turn skeleton shared by every champion: stat choice, recall, shopping, then the champion's hooks |
| `src/common/mod.rs` | Shared helpers, one file per topic; `mod.rs` re-exports what the rest of the crate uses (`crate::common::...`) |
| `src/common/world.rs`, `movement.rs` | Per-turn caches (`sight`, `blocked_tiles`), camp groups, distances; walking, dashing and `WalkGrid` |
| `src/common/attack.rs`, `threats.rs`, `duel.rs` | Attacks and ability casting; what could damage us (`Threats`); 1v1 estimates (`plan_duel`) and the per-turn `FightView` |
| `src/common/fight.rs`, `chase.rs` | Fighting visible enemies (standoffs, backing off, kiting); picking, dashing at and following a chase target |
| `src/common/base.rs`, `camps.rs`, `farm.rs`, `ward.rs` | Banking gems and the go-home pull; fighting and capturing one camp group; the farm plan (`farm_plan`); placing wards |
| `src/common/trace.rs` | The `TRACE` debug log and `checkpoint` fuel profiler |
| `src/common/safety.rs` | Enemy sightings memory, recalls that wait for a safe channel, retreating from losing fights |
| `src/common/collect.rs` | Capture holding, escorting (tanks and Willow only chase near a damage dealer), camp-fight estimates |
| `src/champions/mod.rs` | The `ChampionAi` trait (decision hooks with shared defaults) and `for_champion` dispatch |
| `src/champions/<name>.rs` | One file per champion (12, with Cassi and Issac separate) with its kit summary and any overrides |

Each turn, `bot.rs` runs, in order: skip if unable to act, then shop (`shops_anywhere`), then `support()`, then recall when below 35% health, then `fight()`, then the go-home recall, then `bank_gems()` when `should_bank()`, otherwise `farm()`; with fuel to spare it then records enemy sightings and places a ward. To give a champion its own behaviour, override the hook in its file and leave the rest to `common/`.

---

## 1. Starting point: Rust starter bot
**Commit:** `9ebbc70`

`my-bot` began as a copy of `starter-rust`:
- **Draft:** random pick.
- **Shopping:** buys power shards of one randomly chosen stat.
- **Recall:** below 35% health.
- **Combat:** attacks enemies in basic-attack range.
- **Gems:** tributes them at the nearest altar.
- **Otherwise:** farms the nearest camps.

## 2. Class-based draft
**Commit:** `c48a2ad` (Guancheng)

Every champion is tagged with a class in `constants.rs`:
- Tank: Ryth, Marcella
- DPS-AD: Vesper Vale
- DPS-AP: Stellar, Prism
- Assassin: Vendetta
- Assassin + DPS-AP: Cassi & Issac, Tavi
- Healer: Willow
- Bruiser: Ren, Varg

`draft.rs` builds a weight per class. The weights rise to counter the enemy's picks so far (e.g. enemy tanks boost DPS) and to round out our own team (e.g. a second assassin is heavily discouraged, and a second healer is impossible). If the enemy hasn't picked yet, Bruiser gets a ×20 weight. We pick a class by weighted random choice, then a random unpicked champion from that class.

## 3. Recall and shopping tweaks
**Commit:** `c48a2ad`

- **Healing after a recall:** after a turn where the champion couldn't act (recalling, dead, tributing or teleporting), if it is now on spawn it waits there for one turn. The rules say you must stay on spawn until the turn ends to heal fully after a recall.
- **Tavi shops anywhere:** Tavi buys shards wherever he is (his passive), not only on spawn. This now lives in `champions/tavi.rs` as `shops_anywhere()`.

## 4. Fuel fix for camp farming (timeouts)
**Commit:** `d945bd1` (also applied to `starter-rust`)

- **Problem:** Rust bots get 100k fuel per turn. In the Rust SDK each new `Bitboard` costs about 2k fuel, and `farm_camps` created about 60 per turn (`spawn_tiles()`, `.intersect()`, `state()` for each of about 20 camp groups). It ran out of fuel on about 10% of turns, which suspended the champion until its next turn.
- **Fix:**
  - Set goal tiles directly with `set` / `is_set` instead of creating bitboards for each group.
  - Load `camp_groups()` once and reuse it (`thread_local` `OnceCell`).
  - Draw altar squares directly onto the goal board.
- **Result:** farming went from 95k+ fuel to about 23k on average and 37k at worst. A 200-round mirror on collision had 0 timed-out turns, with at least 48k fuel left at the end of every turn.
- **Details:** `docs/RustFuelPitfalls.md`, which also has a fuel-check recipe to run on new per-turn logic.

## 5. Per-build random seed
**Uncommitted**

- **Problem:** the match engine provides no entropy. We tested this: the clock reads 0, and OS randomness and heap addresses were identical across runs. Separately, `cambc` refuses build scripts. So the bot's randomness can only vary through game state or a constant in the source.
- **Change:** `random_seed()` now starts from a `const SEED` instead of 0. `scripts/reseed.sh [bot]` writes a fresh random value into it, so each submission can get its own seed. A seed that tests well can be kept.

## 6. Damaging abilities and attack resets
**Uncommitted**

- **What it does** (`common::fight_enemies`, called each turn when an enemy champion is visible):
  1. Basic-attack the weakest enemy in range.
  2. If a damaging ability is ready and an enemy is within its range, cast it. Unit-targeted spells go on the enemy; ground spells go on the enemy's tile.
  3. Otherwise, if we attacked this turn and an attack reset is ready, cast the reset and attack again immediately.
- **Damaging abilities:** damage, area damage, or crowd control with engine-reported damage above 0. Dashes, heals, shields, attack resets and pure stuns (Cassi's slot 1) are never cast at enemies.
- **Design choices:**
  - **Spell over reset:** only one cast per turn is allowed, so a damaging spell takes priority over a reset.
  - **No movement after a fight:** the turn ends after any fight action, as with the old attack code.
  - **No chasing:** it doesn't move toward enemies that are out of ability range.
- **Bug found while testing:** if the basic attack killed an enemy, the spell step queried the dead enemy and hit "Unit is not in vision". It now skips enemies that are dead or out of view.
- **Results:**
  - **Mirror matches** on 3 maps: correct casts for Prism, Ryth, Marcella, Tavi, Ren, Stellar and Cassi, every reset followed by a successful second attack, 0 errors, and at least 41k fuel left at the end of every turn.
  - **vs previous `my-bot`:** 15/18 wins across 9 maps, both sides. This comparison also includes the seed change.
  - **vs `starter-rust`:** 16/18 wins.

## 7. Per-champion code structure
**Uncommitted**

- **Change:** split the single `bot.rs` into the turn skeleton (`bot.rs`), shared helpers (`common.rs`) and one file per champion under `champions/`. Each file implements `ChampionAi`, which has hooks for `shops_anywhere`, `fight`, `bank_gems` and `farm`, with shared defaults. Only Tavi overrides anything so far.
- **Verification:** this was a pure refactor. All 18 matches vs `starter-rust` (9 maps, both sides) gave the same winner on the same round before and after, with 0 errors. Matches are deterministic, so any change in behaviour would have shown up.

## 8. `expected_dmg(tile)`: damage estimate for a tile
**Uncommitted.** Not called by any champion yet.

- **What it does:** `common::expected_dmg(tile)` estimates the damage our champion would take standing on `tile` for one turn. It adds up:
  - **Camps** (`camp_dmg`): the live Strength of every visible awake camp whose attack range (3) covers the tile. The SDK has no "awake" query, so a group counts as awake once a visible member is damaged or missing (dead). Camps never move, so a live camp is always on its spawn tile.
  - **Enemy champions** (`enemy_dmg`): one basic attack if the tile is within the enemy's attack range, plus its best single cast, since only one is allowed per turn. That cast is either its strongest ready damaging ability in range, using the engine's `damage_amount_against(me)` prediction, or an attack reset giving a second basic attack.
- **Assumptions:** it's an upper bound. Every threat is assumed to target us, basic attacks use raw Strength (no mitigation), enemies are not assumed to move before attacking, and only visible units count.
- **Accuracy:** 9 maps, 800 rounds, vs `starter-rust`, about 19.5k turns. The prediction for the end-of-turn tile was compared with the health actually lost before the next turn:

  | Outcome | Turns |
  |---|---|
  | No damage predicted, none taken | 12,734 |
  | Damage taken and predicted | 4,076 (3,648 exact, 427 over, 1 under) |
  | Damage predicted, none taken | 2,697 (2,176 camp only, 499 enemy only, 22 both) |
  | Damage taken, none predicted | 30 (mostly 45–65 hp, i.e. single basic attacks) |

- **Cost:** about 7k fuel per call on average, 16k at most.
- **Open questions / follow-ups:**
  - Most false alarms come from camps. Likely causes, not yet verified: groups that went back to sleep after two quiet rounds while still damaged or missing members, and camps attacking a different champion in range.
  - Tracking each group's awake/asleep state across turns could fix this.
  - The 30 misses look like enemies stepping into range and attacking. Adding melee move-then-attack reach would catch them, but would add false alarms.
  - Enemy false alarms are expected against the starter, which never casts abilities.

## 9. Averaged enemy abilities, threat list, and safe recalls
**Uncommitted.** Ideas from Guancheng.

- **Averaged ability damage:** `enemy_dmg` no longer counts a ready ability as a full hit. Each enemy damaging ability in range now counts `damage / max_cooldown` per turn (1/8 of a hit), whatever its current cooldown. An attack reset counts 1/8 of an extra basic attack. Basic attacks in range still count in full.
- **Threat list:** the damage code is now `Threats::scan()`. It gathers each camp, enemy attack and enemy ability as a `Threat { origin, range, damage }` once per turn, at about 7k fuel. After that, `damage_at(tile)` is cheap for any number of tiles, and `safe_tiles()` gives a bitboard of tiles with 0 expected damage. `expected_dmg(tile)` is still there as a single-tile shortcut.
- **Safe recall** (`common::retreat_and_recall`, used by `bot.rs` below 35% health): if our tile has expected damage above 0, walk toward the nearest tile with 0 expected damage (`move_towards_closest(safe_tiles)`). Recall once on a safe tile, in the same turn if the step reaches one. If no safe tile can be reached, recall anyway. Damage interrupts recalls, so this avoids wasted recall attempts.
- **Results:** vs `starter-rust`, 9 maps, both sides, compared with the previous commit `02555d1`:

  | | Previous | New |
  |---|---|---|
  | Wins | 16/18 | **18/18** |
  | Recalls started | 138 | 452 |
  | Recalls interrupted | 43 (31%) | **27 (6%)** |

  - **Head-to-head vs `02555d1`:** 9 maps, both sides. **The new version won 18/18**, each by reaching 1000 gems (rounds 470–1004), with 0 bot errors. Both versions use the same `SEED`, and the old one never called `expected_dmg`, so the only behaviour difference is the safe-recall logic.
  - **Retreat behaviour:** 454 turns were spent walking to safety, then 414 recalls started on the same turn as a step reached a safe tile. Recalling after moving works (no errors).
  - **Caveat:** a champion that died mid-recall and respawned would be miscounted as a completed recall. It's also unclear why the new version starts about 3× more recalls. Possibly champions survive to recall instead of dying, but this hasn't been checked.
- **Open questions / follow-ups:**
  - Camp false alarms (entry 8) also make some tiles look unsafe, which can lengthen retreats.
  - It doesn't cancel a recall that's already in progress when a threat appears; damage interrupts it anyway.

## 10. Chasing enemies we expect to beat (1v1 simulation)
**Uncommitted.** Idea from Guancheng.

- **What it does:** `common::fight_enemies` first strikes anything already in reach, as before. If nothing is in reach, it simulates a chase on each visible enemy (`plan_duel`):
  1. **Walk:** find the shortest path to a tile within our attack range of the enemy. Add up `Threats::damage_at` on every tile where we'd end a walking turn. This includes camps and every other enemy, not just the target.
  2. **1v1:** from the final tile, compare `kill_turns = ceil(enemy hp / our damage per turn)` with `die_turns = ceil(hp left after the walk / damage per turn on that tile)`. Both sides' damage per turn is a basic attack plus abilities averaged over their cooldown (1/8), the same model as `Threats`.
  3. **Winnable** if we survive the walk and `kill_turns <= die_turns`. Ties go our way, because we arrive and hit first.

  Among winnable enemies it chases the one with the fewest walking + killing turns. It keeps chasing the same target (remembered by champion type) while that target stays winnable. Ranged champions use the same logic, so their walk is usually zero.
- **Implementation notes:**
  - **Cheap path lookups:** one distance field from our tile, capped at 16 steps (`MAX_CHASE_STEPS`), is exported into a Rust array (`WalkGrid`). All path lookups for every enemy are then plain Rust instead of SDK calls. That took the per-enemy cost from about 12k to about 1–2k fuel.
  - **Movement:** the chase steps straight onto the first tile of the planned path. After moving, `strike` skips "excludes walking" abilities; trying to cast one was causing engine error 126 in testing.
  - **Fuel guard:** `bot.rs` skips banking and farming for the turn if less than 40k fuel is left (`MIN_FUEL_FOR_PATHING`). In the head-to-head it triggered on 101 of about 45k turns.
- **Results:**
  - **vs `starter-rust`:** 18/18 wins, 0 errors, 0 timed-out turns in about 51k turns. Before the fuel fixes the first version had 69 timeouts and 45 errors.
  - **vs the pre-chase version** (entry 9): **13/18 wins**, 0 errors. It lost interlock, packs and rows as B, and expanse and lanes as A. Expanse and lanes were won by the same side in the same round in both games, so those two look decided by side rather than by the bot.
- **Simplifications / follow-ups:**
  - **Enemies don't move in the model:** no kiting or retreat. Basic attacks use raw Strength.
  - **Chasing takes priority over banking:** a winnable chase beats banking gems even when we're carrying many.
  - **Attack range only:** reach uses attack range, not ability range, which undersells casters like Stellar.
  - **Camp over-prediction:** the over-prediction from entry 8 makes some duels look less winnable than they are.

## 11. Economy: hold gems, funnel gold to DPS, roamers spread out (experiment, replaced)
**Never committed.** Replaced by entry 12: gem-holding and roamer-spreading were removed, and the tank funnel was narrowed to AD carries. Kept here as a record of what was tried.

- **Hold gems** (`should_bank`, a new `ChampionAi` hook so champions can override it): don't bank as soon as we carry gems.
  - **Early game** (`EARLY_GAME_ROUNDS` = 300): hold gems and keep farming gold, unless the bag reaches `BIG_BAG` = 100.
  - **After that:** bank only when no enemy champion is within `SAFE_RADIUS` = 8.
  - **Always bank** in the last 100 rounds, or if the bag would win the game.
- **Tank funnel:** tanks still fight camps, but leave a cleared group's capture tiles if a DPS ally (DpsAd, DpsAp, Assassin+DpsAp) is within 6 tiles. Rewards are split between every ally on the region, so the DPS gets all of it.
- **Roamers spread out:** assassins and bruisers (Assassin, Bruiser, Assassin+DpsAp) skip a camp group an ally is already fighting, unless we're already in reach of it ourselves. If the fighting allies are losing (the camps would kill them before they clear the camps), the roamer goes to help instead.
- **Classes:** `constants::class_of` maps each champion to Guancheng's class table.
- **Results:** each variant vs `old-bot` (the version from entry 10), 9 maps × both sides × 5 seeds, giving 90 games each. The seed is the same for both bots in each game.

  | Variant | Wins |
  |---|---|
  | Hold gems only | 45/90 (50%) |
  | Tank funnel only | 44/90 (49%) |
  | Roamers spread only | 43/90 (48%) |
  | **All three** | **33/90 (37%)**, clearly worse |

  - **Triggers** (all three on, 18 games): tank left a capture 1,228 times, roamer skipped a camp 2,629 times, roamer helped 559 times. 347 banks averaged 41 gems.
- **Takeaways:** each change alone is neutral at these settings, but together they hurt. The cause isn't known yet. One guess: roamers carry more gems further from allies, so they die with bigger bags.
- **Known issue:** farming now costs up to about 70–75k fuel on some turns (about 37k before). The 40k fuel guard is too small, giving about 11 timed-out turns per 9 games. Needs fixing whichever settings are kept.

## 12. Fuel spike fix, AD-carry funnel, and "willingness to go back to base"
**Uncommitted.** Ideas from Guancheng. Tested against `bots/old-bot` (a copy of commit `c777c3d`).

- **Fuel spike fix:** farming was costing up to 70–75k fuel.
  - **Causes:**
    - `attack_if_in_range` built a new bitboard (`me.attack_tiles()`, about 2k fuel) for every unit it checked. It now compares Chebyshev distance with the attack range, which gives the same answer.
    - The camp-attack scan now checks `can_attack` once and pre-filters camps by distance.
    - The group scan no longer reads every camp's health, which was only needed for the removed roaming logic.
    - Ally info is only read when a tank finds a capturable group.
  - **Result:** farming averages 15k fuel (53k max), with 0 timed-out turns in about 61k turns.
- **AD-carry funnel (`TANK_FUNNEL`):** a tank still fights camps, but won't stand on a cleared group's capture tiles if an AD carry ally (`ClassKind::DpsAd`, currently only Vesper Vale) has any of them within its attack range. Rewards are split between every ally on the region, so the carry gets all of it. It triggered 426 times in 18 games.
- **Willingness to go back to base** (`common::base_willingness`, checked in `bot.rs` after fighting):
  `W = BASE_W_GOLD × gold/150 + BASE_W_MISSING_HP × (1 − hp/max_hp) + BASE_W_ROUNDS_AWAY × rounds since last on spawn`
  - **Gold** is measured in emblems' worth (150 gold). Champions that can shop anywhere (Tavi) get no gold term.
  - **At W ≥ 1**, the champion walks to a safe tile and recalls (`retreat_and_recall`), then shops and heals at spawn.
  - **Unchanged:** the hard recall below 35% health. Gems are still banked whenever carried; the old "bank immediately" behaviour now sits behind the `should_bank` hook.
- **Tuning:** each setting vs `old-bot`, 9 maps × both sides × 5 seeds = 90 games, same seed for both bots in each game.

  | gold / hp / per-round | Wins |
  |---|---|
  | 0 / 0 / 0 (funnel + fuel fix only) | 48/90 |
  | 0.25 / 1.0 / 0.004 | 44/90 |
  | 0.5 / 1.0 / 0.004 | 55/90 |
  | 0.5 / 0.5 / 0.004 | 50/90 |
  | 0.5 / 1.0 / 0 | 51/90 |
  | 0.5 / 1.0 / 0.008 | 49/90 |
  | 1.0 / 1.0 / 0.004 | 58/90 |
  | 1.5 / 1.0 / 0.004 | 52/90 |
  | 2.0 / 1.0 / 0.004 | 50/90 |
  | 3.0 / 1.0 / 0.004 | 51/90 |
  | **1.5 / 0.5 / 0.004** | **58/90** |
  | 1.5 / 1.0 / 0 | 55/90 |

  - **Holdout check:** the top two settings were rechecked on 5 seeds that weren't used for tuning. 1.0/1.0/0.004 got 50/90; **1.5/0.5/0.004 got 55/90, so 113/180 (63%) overall**. That setting is now the default.
  - **Shape of the results:** gold is the term that matters. Too little gold pull is no better than none; between about 1 and 1.5 is best. Rounds-away and health have only small effects.
- **Final check** (your `SEED`, 18 games vs `old-bot`): 13/18 wins, 0 errors, 0 timed-out turns, with at least 13k fuel always left.
- **Follow-ups:**
  - Gold could be weighed against what it buys: a nearly complete emblem is worth more.
  - The funnel could also count AP carries if Vesper Vale alone is too narrow.
  - The worst-case farming cost (53k) is mostly `move_towards_closest` and could be trimmed further.

## 13. Fix: champions oscillating at the edge of vision of a respawning camp
**Uncommitted.** Spotted by Tristan around round 740 on corners (`my-bot` as A vs `old-bot`).

- **Bug:** farming skipped a camp group only while it was *visible* and respawning. A champion at the edge of vision saw the group respawning, so it targeted another camp and stepped away. It lost sight of the group, which made it the nearest target again, so it stepped back, and repeated.
  - **Debugging:** per-turn decision logs (matches are deterministic, so logging replays the same game) showed every champion doing this while farming. Varg, for example, bounced between (8,32) and (7,33) for 39 turns while the group at (10,41) had about 40 rounds of respawn left.
- **Fix:** when a champion sees a group respawning, it records the round it will be back (`RESPAWN_READY`, per champion) and skips that group until then, whether or not it's visible.
- **Results:**
  - **Oscillation:** across 18 games vs `old-bot`, runs of 6+ alternating turns fell from 104 (1,696 turns lost) for `old-bot` to 7 (55 turns) for `my-bot`, with 0 errors. Corners is now won from both sides.
  - **Win rate:** vs `old-bot` over the same 180 games used for tuning, **111/180 (62%)**, compared with 113/180 before the fix. That's no measurable change in strength. The oscillation seems to have been partly "useful", because it kept champions near a camp that was about to respawn.
- **Tried and rejected:** keeping a respawning group as a target if it would be back by the time we arrived. That got 30/90. Champions arrived early and stood on the spawn tiles, and an occupied spawn tile delays the whole group's respawn.
- **Follow-up idea:** wait *next to* (not on) a nearby camp that's about to respawn, instead of walking to a far one. This needs walking distances to each group, e.g. from `WalkGrid`.

## 14. Per-champion build orders (shopping)
**Uncommitted.** Build table by Guancheng.

- **What changed:** each champion file now has a `build()` hook: an ordered list of emblems (family + stat), each given by the shard that builds it. Shopping (`Bot::buy_items`) completes them one at a time:
  1. Find the first emblem on the list the champion doesn't own yet.
  2. Buy only the shards it still needs.
  3. Carry leftover gold into the next emblem.
  4. Repeat the last entry once the list is done.

  This focuses on one stat early (completing a family) and branches later. Champions without a build (none now) fall back to the old random single stat.
- **Builds** (AP = Focus, AD = Strength, HP = Health; one emblem per entry):

  | Champion | Build |
  |---|---|
  | Cassi | Treasure AP, Power AP, Power AP, Power HP, Power AP… |
  | Issac | Treasure AD, Monster AD, Power AD, Bruiser HP, Power AD… |
  | Ryth | Power HP, Bruiser HP, Durability HP, Power HP… |
  | Vesper Vale | Treasure AD, Power AD, Power HP, Power AD… |
  | Prism | Monster AP, Power AP, Bruiser HP, Power AP… |
  | Stellar | Power AP, Treasure AP, Power AP, Bruiser AP, Power AP… |
  | Vendetta | Power AD, Bruiser AD, Power AD… (Treasure-if-ahead not modelled) |
  | Marcella | Durability HP, Power HP, Bruiser HP, Power HP… |
  | Tavi | Power AP, Power AP, Bruiser AD, Power AD… (HP skipped: HP bought in the field doesn't heal) |
  | Willow | Power AP, Bruiser AP, Power AP… |
  | Ren | Bruiser HP, Durability HP, Power AP (retaliation scales with Focus), Power HP… |
  | Varg | Treasure AD, Monster AD, Bruiser AD, Durability HP, Power AD… |

- **Verification:** purchase logs show five shards combining into the planned emblem in order, with leftover gold rolling into the next one, and no errors. First emblems usually complete between rounds 50 and 370.
- **Results:** each version vs `old-bot`, 9 maps × both sides × 10 seeds = 180 games:

  | Version | Wins |
  |---|---|
  | No build orders (previous `my-bot`) | 111/180 (62%) |
  | **Build table** | **145/180 (81%)** |
  | Table with Treasure → Power | 141/180 (78%) |

  Also tried on seeds 1–5 (90 games), with no gain over the table (71/90): Jungle HP first for the tanks got 70/90, and Home (shorter recall) as everyone's second emblem except Tavi got 70/90.
- **Follow-ups:**
  - Vendetta's "Treasure if super ahead" isn't modelled.
  - Builds could react to the game, e.g. Durability when the enemy has a lot of burst.

## 15. Ranged kiting, engaging nearby enemies, and retreating toward base
**Uncommitted.** Ideas from Tristan.

- **Retreat toward base:** fleeing (`retreat_and_recall`) used to go to the nearest safe tile, with ties broken by tile order, so champions drifted bottom-right. Each step now also costs `RETREAT_HOME_PULL` (0.5) × Chebyshev distance to our spawn centre (`move_towards_closest_with_pull`). Because the pull is below 1, every step still gets closer to safety. Not measured separately: it is in both sides of the tests below.
- **Kiting** (`kite_melee`, `KITE_DISTANCE` = 2): a ranged champion with a melee enemy within 2 tiles (close enough to walk in and hit next turn) steps to the reachable tile furthest from it (capped at our attack range). Ties go to less expected damage, then to staying in attack range of an enemy. Ranged champions can either move or attack each turn, so a kite step costs a shot. Walk-compatible abilities are still cast after the step. Fired about 18 times per game.
- **Engaging nearby enemies** (`ENGAGE_RADIUS` = 3): an enemy within 3 tiles is chased even if the duel model says we'd lose, as long as we survive the walk. This pulls us off a camp to fight. Enemies further away are still only chased when the duel is winnable. Fired about 9 times per game.
- **Results:** each variant vs the same bot with both off, 9 maps × both sides, same seed for both bots. Seeds 1–10 were used for tuning; seeds 11–20 are the holdout.

  | Variant | Seeds 1–10 | Seeds 11–20 | Total |
  |---|---|---|---|
  | Kite at ≤2 | 98/180 | 95/180 | 193/360 (54%) |
  | Kite at ≤3 | 87/180 | | |
  | Engage within 3 | 98/180 | 92/180 | 190/360 (53%) |
  | Engage within 5 | 84/180 | | |
  | Engage within 8 | 79/180 | | |
  | **Kite ≤2 + engage 3** | **103/180** | **101/180** | **204/360 (57%)** |

  - Engaging from further away hurts: we abandon camps for fights we expect to lose.
  - Kiting from 3 tiles away costs too many shots.
- **Follow-ups:**
  - Kiting treats every melee enemy alike. It could skip kiting when we win the duel anyway, or when the enemy has a dash (Issac, Vendetta, Ren) and stepping back gains nothing.
  - Engage still uses the duel model's camp damage; a camp we're fighting makes nearby duels look worse than they are.

## 16. Remembering a lost chase target, and warding where it vanished
**Uncommitted.** Spotted by Tristan.

- **Bug:** a chase path that bends away from the enemy (around a wall) could drop it out of vision. With no enemy in sight, the champion went back to farming, stepped back, saw the enemy again and resumed the chase: back and forth.
- **Fix:**
  - **Memory** (`CHASE_MEMORY_ROUNDS` = 3): the chase target now stores its last seen position and round, refreshed every turn it's visible. If it drops out of sight and nothing else is worth chasing, `follow_lost_target` keeps walking to that position. It forgets the target after 3 rounds, or on arriving without finding it.
  - **Ward** (`WARD_LOST_TARGET`): if the last seen position is hidden (in a bush or behind a wall) and within ward range (2), the champion places its ward there, or on a free tile next to it, bushes first. Wards are free and recharge after a round.
- **Results:** vs the same bot with both off, 9 maps × both sides. "Runs" are 6+ turns alternating between two tiles.

  | Variant (seeds 1–10) | Wins | Runs | Turns in runs |
  |---|---|---|---|
  | Off | | 276 | 2,943 |
  | Memory 3, no ward | 94/180 | 218 | 2,685 |
  | Memory 2 + ward | 93/180 | 206 | 2,619 |
  | **Memory 3 + ward** | 90/180 | **191** | **2,317** |
  | Memory 6 + ward | 95/180 | 209 | 2,345 |

  - **Holdout** (seeds 11–20): memory 3 + ward got 89/180, with runs down from 296 to 209 (2,906 → 2,057 turns). Over 360 games that's 179/360: neutral on win rate.
  - **About 1.5 wards per game** were placed on a lost target, and about 55 turns per game were spent walking after one.
- **What's left:** a per-turn branch log (18 games) showed no remaining runs from a lost target. Most were mutual dances: our chaser against their kiting ranged champion (or the reverse), each stepping in and out of reach. Five were plain farming runs.
- **Follow-ups:** a chase could stop when the target only steps in and out of range, e.g. by holding position if the duel tile alternates.

## 17. Dashing, Willow's heal and Varg's shield
**Uncommitted.** Ideas from Tristan.

- **Travel dash** (`DASH_TRAVEL_MIN_STEPS` = 4, in `move_towards_closest_with_pull`): champions with a dash (Issac, Vendetta, Ren, Vesper Vale) dash along the path when the goal is 4+ steps away and no enemy is in sight. Fleeing (`retreat_and_recall`) always dashes. After a dash we still take the normal step. Blocked tiles are recomputed first, since the dash can reveal units (otherwise engine error 22, "known occupied destination").
- **Chase dash** (`DASH_CHASE`, `dash_at_chase_target`): when chasing, dash to the tile with the shortest walk to our attack range of the target, then walk (melee) and strike.
  - Against a ranged target, we only dash if dash + a walking step puts us in range this turn. Until then we walk, which stops us just outside its range of 3, ready to dash in next turn.
  - Against melee targets we dash whenever it gets us closer.
  - **Mechanics checked in logs:** after a dash a champion can still walk and basic-attack, and ranged Vesper Vale can dash and shoot in the same turn. A dash uses the turn's one spell cast.
- **Willow heal** (`HEAL_BELOW` = 0.5, `Willow::support`): heals the ally in range (4) with the lowest health fraction, once one is below 50%.
- **Varg shield** (`SHIELD_ENEMY_WITHIN` = 2, `Varg::support`): shields when Varg lost health since his last turn, or an enemy champion is within 2 tiles.
- **New hook:** `ChampionAi::support()` runs every turn before the low-health recall and fighting, so heals and shields also happen while retreating.
- **Results:** each variant vs the same bot with all four off, 9 maps × both sides × 10 seeds (180 games).

  | Variant | Seeds 1–10 | Seeds 11–20 |
  |---|---|---|
  | **All four on** | **127/180** | **127/180** (254/360, 71%) |
  | All on, always chase-dash (no ranged-target rule) | 126/180 | 131/180 (257/360) |
  | Travel dash only | 104/180 | |
  | Chase dash only | 96/180 | |
  | Chase dash only, always dash | 105/180 | |
  | Heal only (below 50%) | 89/180 | |
  | Heal only (below 75%) | 92/180 | |
  | Shield only (hit or enemy within 2) | 103/180 | |
  | Shield only (when hit) | 103/180 | |

  - Together they're worth far more than the parts suggest, probably because every game has some of these champions and their gains add up.
  - The ranged-target rule makes no measurable difference against always dashing. It's kept as the default.
  - Heal alone is neutral.
- **Errors:** dash/heal/shield add no new error types in 54 debug games vs `old-bot`. Separately, code from `5efa6e7` (`cast_abilities_on_camps`) errors every time it reaches an attack reset (`NoRange`: resets have no spell range, so the reset-on-camp branch never runs). Ren and Ryth also hit engine error 126 (walk-excluding spell cast after moving). Both happen at the end of the turn and appear on both sides of these tests.

## 18. Bug fixes, healers funnel captures, and less walking to altars
**Uncommitted.** Requested by Tristan.

- **Testing change:** to keep runs short, each test is now 80 games: 5 seeds × 8 maps × both sides. Each seed skips a different map (map number = seed mod 9), so all 9 maps get used. Results below are vs the same bot with every change in this entry off. One 80-game run has a standard error of about ±5.6 points (±4.5 wins), so single runs only catch large effects.
- **Fixes to `cast_abilities_on_camps` (from `5efa6e7`):**
  - Attack resets asked for a spell range, which they don't have (`NoRange` error every time). They now use our attack range, so the reset-and-attack-again on camps finally runs.
  - Walk-excluding spells (Ren's and Ryth's stuns) were cast at camps after moving (engine error 126). They're now skipped after a move.
  - Errors went from about 20,000 per 54 games to a handful of rare ones that the baseline has too. Win rate alone: 40/80.
- **Fix to dashing (entry 17):** a dash onto a tile hiding an unseen unit fails and reveals it. The walking step planned before the dash could then target that same tile, which is engine error 22, every turn (22 errors in one game). Blocked tiles are now recomputed after any dash attempt, and the chase step checks champion tiles first.
- **Healers funnel too** (`FUNNEL_HEALERS`): Willow, like tanks, leaves a cleared group's capture to an AD carry covering it. Alone: 41/80.
- **Banking.** About 85% of the turns spent walking to an altar came within 30 rounds of leaving spawn: champions recalled, then walked to an altar with a small bag.

  | Variant (vs all off) | Seeds 1–5 | Seeds 6–10 | Bank-walk turns/game (base → variant) |
  |---|---|---|---|
  | Bank before every go-home recall | 30/80 | | |
  | Bank before go-home recall if an altar is within 8 (`BANK_BEFORE_RECALL_TILES`) | 44/80 | 40/80 | 518 → 476 |
  | … within 4 | 46/80 | 37/80 | |
  | Hold gems until 20 (`BANK_MIN_GEMS`; any bag if an altar is within 6) | 39/80 | | |
  | Hold until 40 | 43/80 | 38/80 | 458 → 280 |
  | Hold until 60 / 80 | 36/80 / 36/80 | | |
  | Hold until 40, near radius 3 / 10 | 32/80 / 40/80 | | |
  | Bank-first within 8 + hold until 40 | 44/80 | 37/80 | ~460 → 265 |

  - Banking before *every* go-home recall clearly hurts, probably from long walks to altars before healing. Limiting it to nearby altars, or holding gems, is neutral on win rate.
- **All of entry 18 together** (fixes + healer funnel + bank-first within 8 + hold until 40): **92/160 (58%)**, with bank walking down from 457 to 275 turns per game (205 vs 383 within 30 rounds of leaving spawn). The same without gem holding got 93/160, so holding is kept for the walking it saves rather than for wins.
- **Remaining errors:** about 1 per 20 games, all kinds that also appear in the baseline: "unit is not in vision", "champion cannot act", and rarely "no visible entity".

## 19. New maps
**Uncommitted.** Maps added by Tristan, renamed from their UUIDs after looking at each layout.

| Map | Size | Layout |
|---|---|---|
| `moat` | 41×30 | Spawns left/right; central altar in a bush between two lines of holes |
| `citadel` | 100×100 | Holes fill two opposite corners; walled rooms in the middle |
| `trenches` | 50×65 | Long vertical channels of holes with gold clusters between them |
| `rift` | 30×30 | Small; holes cut off two corners along a diagonal |
| `hedgerows` | 31×31 | Open, crossed by two full-width bush lines; six altars |
| `spine` | 30×80 | Both spawns side by side in the middle; holes down the centre with wall rungs; altars at both ends |
| `orchard` | 60×50 | Columns of bushes in a band of holes; mostly gold clusters, gem camps far from spawn |
| `spiral` | 45×30 | Holes ring the map; walls spiral round the central altars |

- **Results:** current working tree (entries 15–18) vs the latest commit `5efa6e7`, 8 new maps × both sides × 5 seeds: **63/80 (79%)**.

  | Map | Wins | Rounds |
  |---|---|---|
  | rift | 9/10 | 697–1289 |
  | citadel | 9/10 | 688–817 |
  | orchard | 9/10 | all 2000 |
  | moat | 8/10 | 716–1254 |
  | trenches | 8/10 | 955–1232 |
  | hedgerows | 7/10 | 550–745 |
  | spiral | 7/10 | 610–1249 |
  | spine | 6/10 | all 2000 |

- **Problems found (both versions):**
  - **spine:** the spawns touch, so both teams stand on them attacking each other across the border. Spawn tiles heal to full every turn, so nobody dies or leaves; 1,404 of the first ~1,500 champion turns were fights. One game ended 44 gems to 0. A fix would be to ignore enemies standing on their own spawn.
  - **orchard:** farming goes to the nearest camps, which here are gold clusters (35 gold, 0 gems). Champions farm all game but carry no gems: one game ended 360 to 0, with about 12k gold earned each. A fix would be to prefer gem-giving camps (or skip gold clusters when gem camps are reachable).

## 20. Farming: gold first, then gems
**Uncommitted.** Requested by Tristan: prefer gold early (gems early give no real advantage), then move to gem clusters and monoliths.

- **What changed** (`FARM_PHASE_FILTER`, `FARM_GEM_PHASE_ROUND` = 150, in `farm_camps`): still walk to the nearest available camp, but only among the kinds preferred in the current phase.
  - **Before round 150:** skip gem clusters (0 gold) and monoliths.
  - **From round 150:** skip gold clusters (35 gold, 0 gems).
  - If none of the preferred kinds is available, fall back to all camps.
- **Testing:** vs the same bot with the filter off. 80 games on the 8 new maps (5 seeds × both sides) + 20 on the old maps (one seed on all 9 maps × both sides, plus one map).

  | Switch round | New maps | Old maps | Total |
  |---|---|---|---|
  | 0 (gems from the start) | 47/80 | 8/20 | 55/100 |
  | 75 | 45/80 | 11/20 | 56/100 |
  | **150** | **52/80** | **11/20** | **63/100** |
  | 225 | 49/80 | 9/20 | 58/100 |
  | 300 | 49/80 | 9/20 | 58/100 |
  | 150, holdout (seeds 6–10 / 6–7) | 49/80 | 10/20 | 59/100 |

  - Round 150 totals **122/200 (61%)**. Orchard (entry 19), where farming the nearest gold clusters left teams with no gems, went 20/20. Trenches 16/20.
- **Tried first and dropped: choosing one camp group by value.** Each champion scored every available group by (gold + gem weight × gems) ÷ (walk + kill + capture turns). The gem weight ramped from 0 to 10 gold per gem. Champions kept their target unless another scored 1.3× better.

  | Variant | New maps | Old maps |
  |---|---|---|
  | Ramp rounds 150→450 | 39/80 | 8/20 |
  | Ramp 50→200 | 42/80 | 7/20 |
  | Reward split between allies on the group | 35/80 | 8/20 |
  | Both | 36/80 | 4/20 |
  | Nearest with per-type distance nudges, committed to one group | 34/80 | 7/20 |

  - Orchard was fixed every time (9–10/10), but moat and spiral fell to 1–3/10. A solo kill-time estimate makes elk groups look slow, so champions walked off to cheap camps alone. Nearest-of-all farming keeps the team together.
  - Scoring was also expensive: 27–36k fuel per re-score after trimming (initially 50–75k, from copying a whole-map distance field into Rust and per-camp `is_set` host calls). Pure Rust work is metered too.
- **Fixes found along the way:**
  - The attack reset on camps (entry 18) could be cast while the attack was still available (engine error 26). It's now only cast after the attack is spent.
  - The single-goal shortcut in `move_towards_closest` used the SDK's `move_towards`, which can step onto occupied tiles. It's removed.
  - A step or dash onto a tile our occupancy view hasn't caught up with (seen right after dashes) is now treated as not moving (`step_to`, `dash_to_best`) instead of an error that ends the turn.
- **Still open:** spine stays 5/10 in every test. Its spawns touch, and both teams fight across the border at spawn forever (entry 19).

## 21. Hot spots (experiment, off), draft fixes, and custom test maps
**Uncommitted.** Idea from Tristan. `HOTSPOTS = false`: every version tested lost.

- **What it does** (`src/hotspots.rs`):
  - **Draft:** rank camp groups by walking distance from our spawn into a gold list (not monoliths) and a gem list, then write a plan into the 1024-byte draft memory.
  - **Carries** (not tanks or healers; ADCs and mages first) each get a different #1 spot.
  - **Tanks and healers** copy a carry's lists, so they follow it.
  - **Play:** from `HOTSPOT_START_ROUND`, `farm_camps` heads for the first available spot among the top `HOTSPOT_TOP_K`, using the gold list before round 150 and the gem list after. Otherwise it farms the nearest camp as before.
  - **Options:** own half only (`HOTSPOT_SAFETY`), a detour cap (`HOTSPOT_MAX_DETOUR`), shared spots (`HOTSPOT_SQUADS`), and dropping the spot while an enemy is in sight (`HOTSPOT_AVOID_ENEMIES`).
- **Draft memory gotchas:**
  - Submit *before* picking: the pick ends the draft turn, and submitting after it is engine error 18.
  - Only the first draft turn has ~1M fuel; later turns get ~100k. The ranking is computed once on the first turn and cached, and later turns only re-encode the plan.
- **Draft bugs found (both fixed, kept):** our draft bot sometimes made no pick, so the game picked for us at random. This affected every earlier test, on both sides.
  - Printing the class weights (`{:?}` on floats) cost ~30k fuel per turn, enough to run out on turns after the first. Removed.
  - Rolling a class we had no champions left in (a second assassin: Vendetta is the only one) indexed an empty list and panicked. Such classes now get weight 0.
  - **After the fixes:** every pick was made in 54 test drafts. Win rate vs the old draft code: 9/22, within noise (expected: both sides were affected).
- **Results:** hot spots on vs off, 180 games: 4 custom maps × both sides × 10 seeds, 8 new maps × both sides × 5 seeds, 20 on old maps.

  | Variant | Custom | New | Old | Total |
  |---|---|---|---|---|
  | Default (own half, detour 8, top 3) | 25/80 | 32/80 | 9/20 | 66/180 |
  | Two squads | 24/80 | 36/80 | 8/20 | 68/180 |
  | One squad (whole team, planned spots) | 21/80 | 26/80 | 5/20 | 52/180 |
  | Drop spot while enemies visible | 26/80 | 28/80 | 7/20 | 61/180 |
  | Start at round 400 | 36/80 | 37/80 | 9/20 | 82/180 |
  | Tie-breaker only (detour 3, top 8) | 34/80 | 33/80 | 5/20 | 72/180 |

  - Results before the draft fixes are discarded: then the plan only reached the first 1–3 picks.
  - **Why it loses:**
    - Spread-out champions lose fights against a team that farms nearest camps and so stays together (one spokes game: 1 kill, 15 deaths).
    - But the whole team moving together to planned spots is worst of all (52/180). So the bigger problem is the fixed "closest to base" ranking itself: it keeps pulling champions back to base-side camps, while "nearest to me" sweeps outward.
- **Custom test maps** (`maps/spokes`, `pocket`, `ladder`, `scatter`; layouts and generator in `scripts/mapgen/`):
  - **spokes** (41×30): equal-distance wolf and gold camps in several directions from spawn, to reward spreading.
  - **pocket** (45×30): most resources in one walled area next to base, plus a few far camps.
  - **ladder** (30×80): camps strung along a long map.
  - **scatter** (60×50): an open general case with every camp kind, including monoliths.
  - **Generator:** `scripts/mapgen/mapgen.mjs` drives the map editor's engine wasm from Node. A map built from scratch is rejected by the game (engine error 7), so it starts from an existing map of the same size, erases it and draws the ASCII layout. See `scripts/mapgen/README.md`.

## 22. Fights against more than one enemy
**Uncommitted.** Requested by Tristan: the 1v1 model (entry 10) ignored other champions joining.

- **What changed** (`plan_duel`, `Threats::damage_at_turn`, `ally_helpers`, `losing_fight`):
  - **Turn-by-turn fight:** the duel is simulated a turn at a time, for at most 40 turns, instead of with fixed damage rates. We hit first each turn.
  - **Enemies join** (`DUEL_ENEMIES_JOIN`): each visible enemy champion counts once it could have walked into range of our duel tile, i.e. its distance minus its speed × turns elapsed ≤ its range. Walls are ignored, which errs towards danger. The walk to the duel tile uses the same time-aware damage. Camps don't move.
  - **Allies join** (`DUEL_ALLIES_JOIN`): visible allies add their basic attack on the target once they could reach it.
  - **Back off** (`FLEE_LOSING_FIGHTS`): when the damage expected on our tile over the next 3 turns, enemies closing in included, would kill us and no visible enemy is winnable, keep our distance (`keep_distance`) instead of trading or farming nearby.
    - **What "keep our distance" means:** stay just outside every visible enemy's reach (attack range + one turn of walking). Already outside: hold, still shooting anything in our own range (ranged champions can poke melee enemies from there). Inside: step to the tile that gets outside while staying as close as possible, without dashing.
    - **Replaced:** at first (the results below) it walked to the nearest tile no threat reached, leaning towards base and dashing. Per Tristan, it now holds the line rather than running.
    - **Results** vs the run-to-safety version, 20 games: 9/20 wins, kills/deaths 181/205 vs 184/205, chases followed by death 8% vs 8%, i.e. neutral.
    - **Behaviour check** (4 maps): 39% of these turns end out of reach, all 1–2 tiles outside it, against 21% for the old version. Median distance to the nearest enemy is 4 in both.
  - **Engage rule:** close enemies (`ENGAGE_RADIUS`, entry 15) are only fought despite a losing simulation if we'd survive at least `ENGAGE_MIN_SURVIVE_TURNS` = 4 turns.
  - With joining and back-off off, the model matches the old 1v1 one (apart from the 40-turn cap).
- **Testing:** 20 games per variant (10 maps × both sides) vs the same bot with all of it off. Chase episodes are runs of chase turns; we count those followed by our death within 10 rounds.

  | Variant | Wins | Chases followed by our death (variant / base) |
  |---|---|---|
  | Enemies + allies join, no back-off, no engage guard | 11/20 | (not measured) |
  | + back-off, seed 1 | 10–11/20 | 12% / 11%; winnable chases 7% / 10%, engage-rule chases 35% / 21% |
  | + back-off, seed 2 | 7/20 | (not measured) |
  | **+ back-off + engage guard (kept)** | **10/20** | **8% / 11%**; winnable 7% / 9%, engage-rule 22% / 22% |
  | + back-off, engage rule removed | 8/20 | 8% / 13% |

  - The new model makes the chases it chooses safer: deaths after a "winnable" chase fell from about 10% to 7%.
  - The engage rule kept forcing exactly the fights the model now flags as lost to a group, until it was required to survive 4 turns.
  - Wins and total kills/deaths stay within noise at 20 games. Most deaths aren't after chases.
- **Follow-ups:**
  - Enemies out of sight (fog, bushes) still can't be counted. Remembering recently seen enemy positions, like the chase memory in entry 16, would help.
  - Allies count basic attacks only.

## 23. Getting out of losing fights: stuns, spawn, cornered, sightings (experiment, reverted)
**Never committed; code removed.** Requested by Tristan (ideas 2, 4, 6, 7 from a list of escape options). Reverted to the entry 22 behaviour because the kept set was neutral; kept here as a record.

- **What changed** (`escape`, called by `fight_enemies` when `losing_fight`; each part has its own switch):
  1. **Stun to escape** (`STUN_TO_ESCAPE`): cast a ready stun on the enemy closest to hitting us (smallest gap to its attack range + one step). Cassi's stun still allows walking that turn. The others (Prism, Ren, Ryth) end our movement, but the stunned enemy loses its turn too.
  2. **Go home** (`SPAWN_ESCAPE`): if our spawn is at most 10 steps away and the walk, at the damage we're taking now, costs under 90% of our health, head there (dashes allowed). On our spawn we heal to full each turn and enemies can't enter, so `losing_fight` there only fears a one-turn burst; otherwise we stand and fight. The spawn distance map is built once per champion, on a turn with fuel to spare (up to ~40k on big maps).
  3. **Fight when cornered** (`FIGHT_WHEN_CORNERED`): if no reachable tile gets outside every enemy's reach, stop stepping back. If we carry gems, an altar is in range and we expect to live until the tribute completes (damage doesn't interrupt it), start a tribute. Otherwise strike the weakest enemy in reach with what's ready.
  4. **Sightings** (`SIGHTING_THREAT_ROUNDS` = 5): each champion remembers where each enemy was last seen. For 5 rounds after an enemy goes out of sight it still counts as a threat at its last tile, with reach widened by how far it could have walked since. This feeds `losing_fight` and the duel model (entry 22).
  - Otherwise we keep our distance as before (entry 22).
- **Testing:** 20 games per variant (10 maps × both sides) vs the same bot with all of entry 23 off.

  | Variant | Wins | Kills/deaths (variant / base) | Triggers |
  |---|---|---|---|
  | All, incl. farm avoidance | 11/20 | 186/189 / 166/209 | cornered 127, stun 9, home 2 |
  | Cornered only | 11/20 | 158/161 / 143/175 | cornered 117 |
  | Sightings (threat + farm avoidance) | 7/20 | 138/194 / 173/158 | |
  | Sighting threat only | 11/20 | 199/194 / 175/227 | |
  | Farm avoidance only | 6/20 | 160/223 / 200/182 | |
  | **Kept** (no farm avoidance), seed 1 | 10/20 | 172/225 / 196/194 | cornered 220, stun 19, home 5 |
  | **Kept**, seed 2 | 9/20 | 180/197 / 172/211 | cornered 115, stun 16, home 4 |

  - **Farm avoidance** (skip camps within 6 tiles of an enemy seen in the last 15 rounds) was the only clearly bad part: 6/20 and 7/20, losing the kill trade both times. It's off (`FARM_AVOID_SIGHTINGS`). Staying away from places enemies were costs farming and concedes contested camps.
  - **The kept set** is neutral at 40 games: 19/40, kills/deaths 352/422 vs 368/405. Chases followed by our death are 8% vs 10%.
  - **The cornered fallback** fires most (115–220 times per 20 games). Stuns and going home are rare (5–19), because few losing fights happen with a stun ready or near spawn.
- **Not implemented, ideas for later:**
  - **A confidence heatmap** around last-seen enemy positions, which could double as a danger map. A grid isn't needed: the sightings list already holds the data, and "heat" at a tile can be computed when asked as Σ confidence(age) × spread(distance), with the spread radius growing by speed × age. That's cheap per query and needs no per-turn grid update.
  - **Caution:** the farm-avoidance result suggests that being wary of whole areas costs more than it saves. A heatmap would be better used to judge fights than to steer farming.

## 24. Play-test fixes: standing still, back and forth, recalls, wards, collecting, escorts
**Uncommitted.** Requested by Tristan after games against another team's bot (not in the repo, so not reproducible here).

- **Testing change:** all 21 maps (old, new and custom) × both sides × 6 seeds = 252 games, vs the working tree at the start of this entry (entries 15–22, called "baseline" below). One run's standard error is about ±7 wins. Single features were first screened on 2 seeds (84 games, ±4.6). Matches take 1–2 s each, so a 252-game run takes about 5 minutes.
- **Debug trace (`TRACE` in `common/trace.rs`):** when on, every turn prints one line with our tile, health, fuel and the branches taken (`trace("tag")`), and `checkpoint("tag")` records fuel at a point for profiling. A script then finds runs of 8+ turns on one tile ("standing still", not counting camp fights) and 6+ turns alternating between two tiles ("back and forth"). Caveats found the hard way:
  - Formatting a float (`{:.0}`) costs ~9k fuel in wasm, so the trace line prints integers.
  - A trace line that runs out of fuel suspends the champion into its next turn, so traced runs are for diagnosis only, never for win rates.

### What the trace showed (baseline mirror, 42 games)

| Problem | Cause | Example |
|---|---|---|
| Standing still | Fight logic used 60–80k of the 100k fuel when enemies were in sight but not chased; `bot.rs` then skipped farming (40k guard). 4.8% of all turns. | Orchard: 3 champions still for ~175 turns from round 71 |
| Standing still | Spawns touch on spine: both teams strike across the border all game, healing on spawn | 5 champions still for 1,950–1,997 turns |
| Back and forth | The phase filter (entry 20) dropped single camps: an elk-and-gold-cluster group whose elk was dead lost its last goal after round 150, so it could never be captured; from one tile the cluster was out of sight and the dead elk made the group a goal again | Elkring: 3 champions, 58–77 turns between two spawn tiles |
| Back and forth | `should_bank` checks altars within 6 tiles in a straight line, but the walk to the altar first stepped away, so banking turned off, farming stepped back | Interlock: up to 99 turns |

### Fuel

Measured with checkpoints; an SDK call costs about 50–100 fuel, a new `Bitboard` ~2k, listing visible champions ~4k (each `Champion` built reads its ability slots), and plain Rust is metered too (1,000 `Vec` pushes ≈ 35k).

- **Per-turn caches,** keyed by round and our tile and dropped after any attack, cast, dash or ward (any of which can change what we see): `sight()` (visible enemies, allies, camps and the visible board), `blocked_tiles()` (terrain combined once at startup), each enemy's threats (`enemy_threats_by_name`, shared by the threat scan and the sightings memory), and `farm_plan()`.
- **Cheaper loops:** camp threats start from the visible camps; `farm_plan` only looks at the camps of groups within vision range and builds the goal board from a precomputed one; the duel simulation adds threats as they arrive instead of re-summing all of them each turn; the chase walking grid is capped at 12 steps (was 16).
- **Standoff memo:** if last turn's fight decided not to act and nothing has changed (our tile and health, each enemy's tile and health, none within 4 tiles), skip re-deciding and go farm. This fixed the orchard case.
- **Extras last:** recording sightings and placing wards run after the turn's decisions, only with 30k+ fuel left.
- **Result:** turns skipped for fuel went from 4.8% to 3.1%.
- **Surprise:** the first version of the fuel fix alone *lost*, 76/168 with kills/deaths 1,212/1,572. With fuel to spare, champions that saw enemies they wouldn't chase walked on to camps next to them instead of freezing. Making them hold whenever an enemy was in sight recovered it (44/84), so the old standing still had been an accidental "don't farm near enemies". `FARM_AVOID_ENEMIES` (below) does that on purpose.

### Errors

- Some enemies on the occupancy board fail unit queries (health, name) with "Unit is not in vision", even mid-turn after passing them. Each such error ended the turn. `sight()` now drops enemies it can't query, and an error inside `fight()` is logged (`bot error (fight)`) and the turn carries on to banking and farming instead.
- Placing a ward replaces the old one, so units seen through it disappear mid-turn: the caches are dropped after placing one.

### Features (each has a switch)

Screening, 84 games each vs baseline. "Alone" means every other new feature off; "on top" means added to the five kept in the first block.

| Switch | What it does | Alone | On top |
|---|---|---|---|
| `WARD_HIDDEN` | When the ward is ready and ours is more than 3 tiles away, ward a hidden tile within 2: bushes first, then behind walls, furthest first | **54** | |
| `RECALL_SAFE_TURNS` = 5 | Recall only where no known threat can reach us during the 5-turn channel; otherwise walk (dashing, leaning home) towards such a tile and recall on arrival | **53** | |
| `SIGHTING_ROUNDS` = 5 | An enemy that left vision still counts as a threat for 5 rounds, from where it was seen, its reach growing a step per round. Used by recalls and retreats only | 53 with the above | |
| `CAPTURE_HOLD` | Standing on a cleared group's capture tile: don't chase, bank or go home until it pays out | 49 | |
| `BANK_COMMIT` | Once banking, keep at it until the bag is tributed | 45 | |
| *(those five together)* | | | **58** |
| `IGNORE_ENEMIES_ON_SPAWN` | Enemies on their own spawn are threats but not targets | 40 | 63 |
| `FARM_AVOID_ENEMIES` | Skip camp groups a visible enemy could reach within 2 turns | 41 | 64 |
| `ESCORT` + `ESCORT_CHASE` | Tanks and Willow only chase an enemy a damage dealer is near | 42 | 66 |
| `RETREAT_FROM_LOSING` | From a fight we'd lose, walk to tiles nothing reaches within 2 turns (dashing, leaning home) instead of holding just outside reach (`keep_distance`, now the fallback) | 38 | 58 |
| `ESCORT_FOLLOW` | Tanks and Willow walk back to the nearest damage dealer when more than 4 away | 34 | |
| `ESCORT_RECALL` | Tanks and Willow recall when every damage dealer in sight is recalling | 35 | |
| `CAMP_FIGHT` | Simulate fighting a camp group from here; nearby visible allies within 7 tiles contribute basic and cooldown-averaged ability damage, and incoming camp damage is shared. Champions retreat individually at 35% health | 25 | 49 |
| `GREEDY` | Nearby clusters and uncaptured rewards within 8 tiles can come before chasing and the farming phase; ordinary winnable camps are excluded (`GREEDY_CAMPS = false`) | 23 | |
| `GO_HOME_NEEDS_ENEMY` | The literal reading of "don't recall when no enemy is in vision": skip the go-home recall with no enemy in sight | (171/252 on the final set, deaths 1,608 vs 1,226) | |

Also, unswitched: **lowest health first.** Camps in range, camps for spells and chase targets are picked weakest first (chase ties go to the quicker kill).

- **Variants tried** for the losers (84 games each):
  - `CAMP_FIGHT`: skip only if we'd die, 37; camp damage shared with allies in range, 34; both, 40.
  - `GREEDY`: only clusters and rewards, 25; within 4 tiles, 29; steering farming without blocking chases, 28 and 36.
  - `ESCORT_FOLLOW`: not abandoning a camp in range, 39.
  - `FARM_PHASE_BY_GROUP` (the elkring fix) is neutral on wins (42 either way) but removes that back and forth. Kept.
- **Why the losers lose, as far as we can tell:**
  - **Greedy** sends champions past nearer wolves to 35-gold clusters, and to rewards an ally is already capturing, which only splits them. Rewards with an ally on them are now excluded, but it still loses.
  - **The camp-fight model** has to assume every camp hits us, because which champion a camp attacks isn't documented. That makes elk groups look unwinnable, so champions walk further alone.
  - **Escort following and recalling** cost farming time.
- **Confirmation over 252 games** (seeds 1–6):

  | Version | Wins | Kills/deaths |
  |---|---|---|
  | Five kept features | 180 | 2,472/1,331 |
  | + spawn / + avoid / + escort chase / + retreat (each alone) | 178 / 181 / 179 / 177 | |
  | **Default: all nine** | **173–174** | **2,370–2,403 / 1,226–1,370** |

  - The four extras are neutral, each and together (all within one standard error). They're on because they were asked for or fix an observed problem (spine), and they give the fewest deaths.

### Results (defaults)

- **vs baseline:** **173/252 (69%)**, kills/deaths 2,370/1,370.
  - Weakest maps: rows 4/12 (also 4/12 with only the five kept features, so not caused by this entry) and spine 4/12 (between 4 and 9/12 across variants at 12 games).
- **vs `old-bot`:** 84/84 (baseline: 82/84), with 129 deaths (baseline: 255).
- **Behaviour** (traced mirror, 42 games):

  | | Baseline | Now |
  |---|---|---|
  | Standing still, not counting camp fights | 15,148 turns | 1,495 |
  | Longest standing-still run | 1,997 turns (spine) | 36 |
  | Back and forth | 100 runs, 1,295 turns | 40 runs, 345 turns |
  | Longest back-and-forth run | 99 turns | 42 |
  | Turns skipped for fuel | 4.8% | 3.1% |

- **Errors:** about 1.2 per game (baseline about 0.6). Most are the logged, now non-fatal fight errors.
- **Follow-ups:**
  - Which champion a camp attacks is worth measuring: with that, `CAMP_FIGHT` could be calibrated instead of assuming the worst.
  - Allies can't see each other's sightings or wards: shared memory could carry enemy positions.
  - `GREEDY` might work as a narrow tie-breaker (a cluster no further than the nearest camp) rather than an override.
  - Rows loses from both sides in every version; worth a look on its own.

## 25. Farm target diagnostics, team camp estimates, and retreat attacks
**Commit:** Uncommitted

- **Why:** Replay traces showed target switching, repeated selection of camps judged unwinnable, and champions fleeing without using melee attacks that were still available. The verbose trace was also making it harder to isolate why a remembered target changed.
- **Farm diagnostics:** `TRACE` is off, and farm-specific per-turn tags/checkpoints were removed. A concise `farm-target-change` line is emitted only when a champion with a remembered target changes or loses it; the reason identifies the exclusion or priority that displaced it.
- **Farm target selection:** damaged, eligible camps prefer the nearest damaged target when it is within 3 tiles of the nearest regular goal. A reward group already being captured by an ally is excluded to avoid competing for its tile and split. Below 35% health, a champion may stay to capture a nearby available reward if it is not in imminent danger and has enough fuel to complete the checks and pathing; otherwise the normal recall remains.
- **Camp-fight estimate (`collect.rs`):**
  - Nearby visible allies contribute basic damage plus damaging abilities averaged over cooldown. Ability damage comes from the SDK's `damage_amount()`, which includes Focus scaling; it is not multiplied by Focus again.
  - The simulation tracks each champion's health and 35% retreat threshold separately. When a champion reaches that threshold, it stops contributing damage; the rest of the team can keep fighting while above their own thresholds.
-  - Groups whose visible camps are judged unwinnable are excluded from the current farm goals; there is no special wait or help-request behavior.
- **Enemy camp avoidance:** a visible enemy only excludes a nearby camp if the existing duel estimate does not expect us to beat that enemy. The duel check reuses its cached fight view, and only builds one with ample fuel remaining; when fuel is tight, it keeps the existing conservative avoidance behavior.
- **Greedy collection:** `GREEDY_CAMPS` is now false. Nearby greedy selection still includes eligible uncaptured rewards and cluster groups, but does not promote ordinary camps merely because the fight estimate considers them winnable. This does not disable the separate damaged-camp farming preference.
- **Retreating melee attacks (`safety.rs`):** before retreating to recall, a melee champion tries a legal in-range basic attack and uses the shared strike logic, which can apply a ready attack reset for a second hit. Retreat movement and recall still proceed afterward.
- **Results:** `cargo check` and `git diff --check` pass. No replay batch has yet been run to measure the enemy avoidance or retreat behavior.
- **Open questions / follow-ups:** the camp estimate remains a heuristic: it assumes nearby allies engage, splits camp damage across active allies, and averages abilities rather than simulating their exact casts and ranges.

---

## 26. Regions: camp groups we can't walk to (`regions.rs`)
**Commit:** Uncommitted

- **Why:** on ambush (and citadel, the same map file) a camp group sits in a room sealed off by walls and one-tile strips of holes. Champions picked it as their farm target, found no walking path (`DistanceField` unreachable), took no step, and kept the target (the farm-target memory keeps a target until it's within attack range). After their first recall every champion of both teams stood on spawn for the remaining ~1,740 rounds.
- **Dash rule, measured against the engine on ambush** (the rules only say "a legal dash path"): up to 2 steps in any of the 8 directions; the middle tile may be a hole but not a wall; the landing tile must be free ground. Straight, knight-move and vertical dashes across the holes worked both ways; a dash whose only middle tile is a wall, or across a wall column, failed with "Destination is unreachable within movement range".
- **Draft (first turn, ~1M fuel):** for each camp group: **Walk** (a camp tile is reachable on foot from our spawn), **Dash** (the group's pocket has a tile within 2 of our region with a legal dash path), **Crystal** (within 3: teleport-crystal range, terrain ignored) or **Unreachable**, with the crossing's launch and landing tiles (launch closest to our spawn on foot). Stored in the draft memory at offset 512, after the hot-spot plan: `b'R'`, version, count, then 7 bytes per group. Pockets reached only through another pocket count as unreachable.
  - Fuel: 20–90k on maps without pockets, ~400k on ambush (two 33-tile pockets). Rust-side loops are metered too: scanning a whole-map buffer for one pocket cost ~400k and checking every launch/landing pair ~260k, so the host floods the pocket (`reachable_tiles`), marks launch candidates by growing the pocket mask with shifts, and the search stops at the first legal crossing. Pockets left unsearched below 150k fuel count as unreachable, and classification only starts with 500k (the first draft turn), so the pick is never lost.
  - The draft instance keeps its state between draft turns (checked), so the table is computed once and re-submitted with the hot-spot plan before each pick (a submit replaces the whole memory).
- **Play:**
  - `farm_statics` drops groups this champion can't reach (non-dashers: anything but Walk; dashers: Crystal and Unreachable), so they're never goals, greedy targets or hot spots.
  - `move_towards_closest` crosses when no goal is in our region (farming, banking, anything): walk to the launch tile, wait for the dash, dash to a free tile across (the planned landing first). Any free tile, not just the planned one: two champions crossing opposite ways each stood on the other's landing tile for ~85 turns. Leaving works the same way. On maps without dash pockets this returns at once: building the region masks there cost a dasher its move and lost games (`eight`/`spokes` 6/28 vs 18/28 with regions off; 29/56 against regions-off once skipped).
  - Fallback: a farm target with no walking path is skipped for 20 rounds (`UNREACHABLE_TARGET_ROUNDS`). It didn't fire in traced games on eight, moat or spokes.
  - Teleport crystals are classified but not bought or used yet.
- **Results** (vs the committed bot `49388ec`, 29 maps × both sides × seeds 1–3, 174 games): **89/174**; ambush/citadel **10/12** with 9,004 gems submitted vs 720; other maps 79/162 (most exactly 3/6, i.e. unchanged play). Traced ambush vs committed, seeds 1–10 both sides: 20/20.
- **Open questions / follow-ups:**
  - On ambush side B gathers far fewer gems than side A (288–960 vs ~1,000). The traced games show 16.5k turns of back-and-forth there, from greedy collecting (`GREEDY`) flipping with the phase filter at the edge of `GREEDY_RADIUS` (a cluster at 57,51 vs a camp at 68,55, every turn). The same flip is in the original replay's log, so it predates this change.
  - One untraced game (ambush seed 3, side B) still went badly: five champions earned almost nothing after round ~150 while hovering near the side-B pocket crossing. The trace (even inside `debug_block`) changes the game, so it couldn't be reproduced; three dashers sharing a 1-tile-wide crossing is the likely suspect.
  - Crystals (buy one when a valuable Crystal group is nearby), and multi-hop pockets.

---

## 27. Back-and-forth and fuel: camp-fight verdicts, farm-plan caching
**Commit:** Uncommitted

- **Why:** traced games (24 games, 12 maps) had 3,016 turns of back-and-forth. Three causes, from the farm-target diagnostics: greedy collecting vs the phase filter at the edge of `GREEDY_RADIUS` (straight-line distance; the walk round a wall goes 8 → 9 away), camp-fight verdicts that changed with every step (~280 flips), and dead camp vs attack range (54).
- **Camp-fight verdict (`collect.rs`):** a camp counts as hitting us if its attack range is at least ours (to fight it we stand within our own range), not only if it covers our current tile: the group was "winnable" from just outside a camp's range and lost one step in. A group judged lost stays lost for 10 rounds (`CAMP_LOSS_MEMORY_ROUNDS`), also while out of sight (`recently_lost` in `farm_plan`), and is forgotten on our spawn. Back-and-forth 3,016 → 1,550 turns; **103/174 vs the previous build**.
- **Tried and dropped:** walking distances per camp group (one `DistanceField` per group, from its camp tiles, built in spare fuel, read at our tile) for the greedy radius and target ranking, with a 3-step greedy hysteresis, and a flip guard (going back to a target left within 4 rounds locks it for 10). Back-and-forth fell to ~100 turns, but **79/174** and, after cutting its allocations, **69/174** vs the camp-fight build; with walking off, 81/174. The lookups (1–3k a turn) cost more than the oscillation did while fuel was short. The flip guard didn't stop the dead-camp flip (the dead camp leaves the candidates, which releases the lock). Worth retrying now that fuel isn't short.
- **Fuel:** 27% of champion turns ran out of fuel (over half on lanes and scatter). Profiling showed 84% of turns had no enemy in fight range, yet the fight code built the whole farm plan (~32k) to check `greedy`; half of all plans were never used (we fought, banked, or attacked a camp already in range).
  - The fight code only asks for the plan when there is a lost chase target to follow, and checks `imminent_danger` (cheap) before it.
  - Camp-fight verdicts (~12k a plan: ~6k for damage estimates, ~4.7k per group judged): camp stats are read once per camp kind (camps respawn with base values), damage estimates (which read every ability) are kept per champion for 10 rounds, and a group's verdict is reused while its inputs (camp healths and whether each can hit us, our health/damage/floor, the allies in sight) are unchanged.
  - The farm plan is reused for up to 5 rounds while our tile and health, the phase, the visible camps' health and the visible enemies' and allies' positions are unchanged.
  - Out-of-fuel turns 27% → **1%**; mean fuel per completed turn 61.8k → 52.2k (traced).
- **Results:** vs the camp-fight build, 29 maps × both sides × seeds 1–3: **135/174 (78%)**, gems 146,498 vs 89,474, kills/deaths 1,589/1,093. Only rows is behind (2/6).
- **Open questions / follow-ups:**
  - Greedy vs phase flip on ambush side B is still there (~1,900 traced turns); walking distances would fix it, and may now pay off.
  - Engine error 98 ("No visible entity of this kind occupies the tile") right after a farming step (the attack-after-moving path): 22 in 24 traced games before, 95 now that fewer turns run out of fuel before reaching it. Each costs the rest of a turn.
  - Fights (~5% of turns) still cost ~25k for the fight view, ~10k of it the 12-step walking grid.

---

## 28. Rewards in sight are always taken; GREEDY off
**Commit:** Uncommitted

- **GREEDY off** (set by hand): greedy on vs off with entry 27's code, 174 games: 86/174 for greedy on (even), gems 121,564 vs 130,640. Traced, greedy off cut back-and-forth from 2,168 turns to 56 and halved farm-target changes (greedy vs the phase filter at the edge of `GREEDY_RADIUS` was nearly all of it).
- **Why:** with greedy off, a cleared gold cluster in the gem phase (round 150+) was no farm goal: the phase filter keeps only preferred groups, and only greedy collecting used to add rewards back. On ambush both middle gold clusters were cleared (rounds 226 and 700) and never captured, so they never respawned either (a group respawns only after its reward is claimed).
- **Change (`farm_plan`):** a cleared group in sight with its reward waiting is the farm goal ahead of the phase filter, damaged camps and greedy (holding a capture we stand on still comes first). It's skipped when an enemy that could reach it within 2 turns isn't one we'd beat 1v1 (the existing `plan_duel` check; below 70k fuel the check is skipped and so is the reward). A "judged lost" memory from while the group was alive no longer blocks its reward. Tanks still leave a reward a carry ally covers, and nobody heads for one an ally stands on.
- **Results:** ambush replay: every clear captured (the red-side middle cluster 14 times instead of once). vs entry 27 with GREEDY off, 174 games: **106/174 (61%)**, gems 131,432 vs 118,284, behind on no map.
- **Open questions / follow-ups:** the opening walk on ambush (blue heads for group 10 at 78,56, 39 tiles away in a straight line but ~50 on foot round a wall, and judges it lost on arrival) needs the camp-fight check when the target is chosen, and walking-distance ranking.

---

## 29. Gold on the way in the gem phase (`GEM_PHASE_GOLD_DETOUR`)
**Commit:** Uncommitted

- **What changed (`farm_plan`):** after round 150 the phase filter skipped gold-only groups (gold clusters) entirely. Now they're farm candidates while they're at most `GEM_PHASE_GOLD_DETOUR` (3) tiles further than the nearest gem goal: on the way, or the closest thing around. They join the candidates instead of replacing the goals (as greedy collecting did), so the farm-target memory keeps the current target while we walk and there's no flip at the threshold. 0 turns it off.
- **Why:** gold still buys items late on; walking past a gold cluster next to the path wasted it. A fixed schedule ("gold every N rounds") would send champions on long trips for 35 gold.
- **Results** vs entry 28 (gold skipped), 174 games: detour 3 **102/174 (59%)**, gems 129,384 vs 127,482; detour 6 100/174 (57%). Kept 3.

---

## 30. Elk rallies (`common/rally.rs`, `ELK_RALLY`)
**Commit:** Uncommitted

- **Why:** two elk have 4,800 health and hit for 120 a round, so the camp-fight check (rightly) judges a lone champion lost and nobody starts the fight. On ambush the six groups with elk were cleared 3 times in 1,916 rounds, the gold and gem clusters 63 times. Two elk are worth 320 gold and 72 gems.
- **What changed:** a champion that sees an elk group it would lose alone, but would beat with 1–3 more allies like itself (stand-ins at full health next to the group, `collect::winnable_with_standins`), writes it to the team's shared memory (byte 0: group + 1, byte 1: round called). Champions with 60%+ health make the rally group their farm goal (after waiting rewards). Camps only fight back once attacked, so they gather next to it; nobody attacks until the camp-fight check, counting the allies actually there, says we'd win (a rally group is judged afresh every turn, not from the loss memory). The rally ends when the group is cleared or respawning, or after 40 rounds. A group not worth a rally isn't rechecked for 10 rounds.
- **Per map:** rallies on every map lost overall, **76/174**: 12/12 on ambush/citadel, 6/6 elkring, 5/6 corners, but 0–1/6 on collision, spokes, hedgerows/packs, fragment/rift. Traced, most rallies there timed out without a kill (hedgerows and packs: 21 called, 0 completed): allies arrive late, hurt or not at all, and the stand-ins are optimistic. The winners are maps where elk groups are a large share of the camp groups; the losers have plenty of cheap camps close by.
- **The bot decides:** each champion counts, once, the camp groups it can reach and how many have elk; rallies are on when at least `ELK_RALLY_MIN_SHARE` (45%) do (ambush, citadel, elkring, corners, eight, moat, spiral, swirl). Chosen on seeds 1–3 (those maps: 33/48); validated on fresh seeds 4–6 vs entry 29: **103/174 (59%)**, gems 137,888 vs 121,216; rally maps **35/48 (73%)**, gems 41,316 vs 26,590; other maps 68/126 (unchanged play, noise).
- **Ranged champions keep their distance:** while waiting, a champion with attack range above 1 heads for the free tiles exactly at its range from the nearest live elk (the camp tiles if none are free), instead of next to the camps. Checked on ambush: ranged champions mostly wait at distance 3 (Stellar 80 turns, Prism 54), melee next to the camps. On the eight rally maps vs entry 30 before it: 27/48 (even), gems 38,418 vs 37,222.
- **Open questions / follow-ups:** rallies still time out often; tighter calling (at most one extra ally, ~15-round limit) or sharing rough positions in the remaining shared-memory bytes might make them work on more maps.

---

## 31. Shared team target: opening as a team, support pairs (`common/team.rs`, off)
**Commit:** Uncommitted

- **What changed:** a farm target shared through the team's shared memory (bytes 2-4: group + 1, round written, leader's champion type + 1). Until `OPENING_GROUP_ROUNDS` the first champion to plan each round leads and the rest follow its group (rewards in sight, a capture in progress and elk rallies first); after that only a damage dealer leads and only tanks and healers follow it (`SUPPORT_PAIRS`). A leader that stops writing loses the lead after a round.
- **Why:** champions choosing targets on their own walked to the same groups and turned back (895 trips in 24 traced games), and none could take wolves or elk alone early.
- **Results** vs entry 30 (with ranged spacing), 174 games each: opening to round 100 + pairs **77/174**; opening to round 200 + pairs **88/174**; pairs only **73/174**. None helps; pairs only looks harmful (like the escort-follow tests, entry 24). **`TEAM_TARGET` is off.**
- **Open questions / follow-ups:** in the opening, most champions still took the reward in sight or held a capture rather than following, so the team stays together anyway around the group it just cleared; the gain from forcing it is small.

---

## 32. Removed: features that were off and tested as not helping
**Commit:** Uncommitted

Each was off by default and lost (or did nothing) when tested; the code is gone rather than switched off:
- **Hot spots** (`hotspots.rs`, entry 21): every variant lost. The region table now starts at offset 0 of the draft memory.
- **Greedy collecting** (`GREEDY`, `GREEDY_CAMPS`, `GREEDY_BEFORE_CHASE`, `safe_group`, the fight code's `collect_instead`, entries 24 and 28): even on wins, and the main cause of back-and-forth. Its radius lives on as `DAMAGED_CAMP_RADIUS` (8) for finishing off damaged camps. With it went two things only greedy turned on: staying to capture a nearby reward at low health (`nearby_capture_reward`) and `imminent_danger`.
- **Escort recall and follow** (`ESCORT_RECALL`, `ESCORT_FOLLOW`, entry 24): 35/84 and 34/84. Escorts still only chase near a damage dealer.
- **Go home only with an enemy in sight** (`GO_HOME_NEEDS_ENEMY`, entry 24): more deaths.
- **Always chase-dash** (`DASH_CHASE_ALWAYS`, entry 17): no different from the default.
- **Per-camp phase filter** (`FARM_PHASE_BY_GROUP = false`): replaced by the per-group filter (entry 24).
- **Shared team target and support pairs** (`team.rs`, entry 31).

677 fewer lines. Check: the same 8 maps vs x-base before and after had the same winners (results differ slightly from fuel drift); cleaned vs before, all maps, both sides, seed 7: 30/58.

---

## 33. Monolith rallies, and realistic rally stand-ins
**Commit:** Uncommitted

- **Monolith rallies** (`MONOLITH_RALLY`, `rally.rs`): a monolith has 20,000 health and hits for 200, but drops 300 gold and 200 gems (a fifth of the target). From round 250 (`MONOLITH_RALLY_FROM_ROUND`), on any map with one, a monolith group gets a rally once we plus up to 4 allies (`MONOLITH_MAX_HELPERS`) would win; the check uses our current items, so it passes as the team grows. Elk rallies are unchanged (elk-heavy maps only). Maps with a reachable monolith: contest, monolith, rows, scatter. On **contest** the monoliths are the only gem source: before this every game ended 0–0 (decided for team B).
- **Bug fixed in rally checks** (`collect::winnable_with_standins`): the stand-in allies were copies of the caller added on top of the allies already there, so a tank caller planned with extra tanks and one call planned a five-champion team as eight. Now each champion remembers the teammates it has seen (attack range, damage, max health); a rally counts only teammates not in sight, strongest first, copies of us only for teammates never seen, and never more than the team has. Contest went from 54 monolith calls and no kills to 13 calls and a kill (200 gems, a win); elk rallies on elkring completed 40 of 46 (most timed out before).
- **Results** vs entry 32, 174 games: **85/174** (even). Monolith maps 15/24 on seeds 1–3, 22/48 on seeds 4–9 (37/72 together, even); elk-rally maps 24/48. On contest a monolith kill wins the game (5 of 12 games on seeds 4–9, all won); with no kill it's 0–0.
- **Open questions / follow-ups:** monolith fights still often fail or never start; the fight simulation ignores healing (Willow), and squishy carries hit their 35% retreat floor early against 200 a round.

---

## 34. bot-v7: what decides top ladder games (analysis, no code)
**Commit:** Uncommitted (`bots/bot-v7` is a copy of `bots/bot-v6`)

- **What:** 6,952 games from the summaries zip of 2026-10-21 (`summaries-2026-10-21-part19.zip`, ranked games with both teams in the current top 150, unranked scrims between top-100 teams), converted offline with `cambc summary`.
- **Results:**
  - The team ahead on **gold earned** wins 68% of games at round 150, 74% at 300 and 79% at 500. A lead of more than 50% at round 300 wins 89%.
  - Held against the other stats in a regression, gold counts about twice as much as gems and four times as much as kills up to round 300. Gems collected overtake gold from round 500.
  - Ahead on emblems built: 75% at round 300. First emblem: 63%. First kill: 62%. First camp clear: 51%. Wards, healing and fuel-exhausted turns barely matter.
  - More captures worth 20+ gems: 78–83%.
  - Champions (win rate when only one team has them): Heart of Ruin 56% (picked by 73% of top teams), Vendetta 54, Vesper 54, Marcella 53, Ryth 53 … Prism 47, Cassi 46, Varg 44, Stellar 43.
  - Classes: no AP 57% against one AP 43%. Exactly one AD 54% against two AD 47%.
- **Our 100 ladder games for comparison:** gold by round 300 equal to our opponents (1,074) but 18% under the top 20 (1,265). Gems by round 300 105 against 126. Fuel-exhausted turns 548 a game against about 40 for everyone else.

---

## 35. Gate siege on 01boss-battle (`collect::GATE_SIEGE`, off)
**Commit:** Uncommitted

- **The map:** a wall across the middle (rows 19–21) whose only gaps are the two monolith tiles (23,20) and (24,20) (one group: 20,000 health each, hit for 200 at range 5, 400 gems and 600 gold for the pair, 150-round respawn). The only altar (25,20) sits in the wall, and the only tile within tribute range we can stand on is the monolith at (24,20). Nobody tributes until a monolith dies, and the two halves only meet through dead monoliths.
- **Why the monoliths died late (rounds 1,046–1,977, or never):** the camp-fight check wants the whole pair killed in one go before anyone falls to their retreat floor. On top of that, `safety::ESCAPE_ANY_HP` sent a melee champion home once its health was under six hits of 400 (2,400). That's more than most champions have, and `FLEE_COMMIT_ROUNDS` then kept the group off its goals for 30 rounds.
- **Measured:** camps don't heal. A monolith hit to 19,280 stayed there for 90 rounds.
- **What changed:** a group that gates an altar (`base::is_gate_group`, computed from the map's altar access) counts a fight that does `GATE_SIEGE_SHARE` (30%) of the work as a win. Variants:
  - `GATE_SIEGE_ONE_HITTER`: the escape reserve counts one camp hitting us, not both.
  - `GATE_SIEGE_NO_RESERVE`: no escape reserve.
  - Gate rallies from round 150 (`GATE_RALLY_FROM_ROUND`), joined up to 30 steps further than the nearest camp.
- **Results** (01boss-battle against bot-v5, 8 seeds × both sides = 16 games each):
  - With the one-hitter reserve and shielded escape (entry 36), our first monolith kill came at a median round **1,008 against 1,959**, and we killed it first in 11 of 16 games.
  - But only 9/16 wins, against 8/16 for no change. The damage we left on the monolith let v5 finish it as well, and the team farmed less (gold −20%).
  - With the gate guard (entry 37): 12/16 with or without the siege.
  - **Off.** 8-game runs on this map swung from 4/8 to 7/8 for the same bot, so only 16-game runs are quoted.
- **Open questions / follow-ups:** a siege only pays if we also hold the gate the moment it falls. One idea is to siege only once the guard is ready, and only while the enemy isn't damaging the same monolith.

---

## 36. Shielded escape (`safety::SHIELDED_ESCAPE`, on)
**Commit:** Uncommitted

- **What changed:** camps hit the closest champion in their range (`threats::CAMPS_HIT_CLOSEST`, on since v3). So in the low-health retreat check (`low_hp_recall`), a camp with another champion (ally or enemy) strictly closer to it no longer counts against us. A camp that is hitting us only does so until we're further from it than the next closest champion. The escape cost is the steps to get behind them, not out of the camp's whole range.
- **Why:** with allies beside it, a melee champion next to a monolith still left at 2,400 health, and ranged champions behind a tank left elk fights early.
- **Results** (against bot-v5, one game a map, 4 seeds = 160 games): **111/160 against 105/160 off**. Gold 6.33 a round against 6.13, deaths 10.5 against 11.2, kills 14.4 against 14.1. On 01boss-battle (16 games) 8/16 either way, but we tribute 321 gems a game against 118.

---

## 37. Gate guard and gate queue on 01boss-battle (`base::GATE_GUARD`, `base::GATE_QUEUE`, both on)
**Commit:** Uncommitted

- **What changed:**
  - Where every altar is gated, once a gate group is down our tank (else first bruiser) stands on one of its tiles outside tribute range, (23,20) on boss-battle. An occupied tile holds the whole group's respawn off, so the altar stays open while the rest tribute through the gate tile, and the guard blocks the way through the wall.
  - Champions remember a gate they saw down as open until they see it alive again. Its respawn round passes while the guard holds it, so the respawn message alone would close it.
  - `GATE_QUEUE` (from v5, off there) lets a champion with gems wait on another tile of the gate group while an ally tributes.
- **Results** (01boss-battle against bot-v5, 16 games each, shielded escape on): guard **12/16 against 8/16** (tributes 587 a game against 321; v5's 205 against 309). Guard + queue 12/16 (tributes 645 against v5's 177). Queue alone 10/16.

---

## 38. Chasing alone (`chase::ALONE_CHASE_MARGIN` 3 → 2) and other kill tests
**Commit:** Uncommitted

- **What changed:** alone (no ally within 6 tiles), we chase an enemy the duel says we kill 2 turns before it would kill us (was 3).
- **Results** (against bot-v5, 160 games each, shielded escape on): **margin 2: 117/160 against 111/160** at 3. Kills 15.2 a game against 14.4, deaths 10.2 against 10.5, gold 6.42 a round against 6.33.
- **Tested and off:**
  - Following a lost chase target for 6 rounds instead of 3 (`CHASE_MEMORY_ROUNDS`): 113/160, kills unchanged.
  - Team-fight bounty (`teamfight::TEAMFIGHT_BOUNTY_GEMS`: an even trade is also a fight when the enemy bags we'd take beat ours by 200 gems; enemy bags are readable with `Champion::gem_count`): 24/40 against 31/40 off, kills unchanged.

---

## 39. Fewer fuel-exhausted turns (`farm::STALE_PLAN_ROUNDS`, `RALLY_DECISION_ROUNDS`, `STALE_SKIP_AT_GOAL`)
**Commit:** Uncommitted

- **Where turns go** (activity log, 10 games):
  - walking to camps 36.5%, hitting camps 12.6%;
  - idle 7.7%, mostly capture holding and stale plans;
  - fuel stalls 6.3%, capturing 6.3%.
- **Where cut-off turns run out:** about 74% are farm turns, and nearly every one rebuilds the farm plan. The group loop costs about 29k of the 100k turn, target choice about 24k, the rally join decision about 15k and the camp-fight context about 21k.
- **What changed** (160 games each against bot-v5, one game a map, 4 seeds; baseline 111/160 with fuel-exhausted turns at 171 per 1,000 rounds):
  - **`STALE_PLAN_ROUNDS` 3 → 5:** a turn with under 85k fuel left at the farm step keeps walking the last plan's goals for up to 5 rounds instead of 3. Against bot-v5: **112/160**, timeouts 148 (−13%), gold 6.35 a round against 6.33, deaths 10.2 against 10.5. But head to head against bot-v6 (same draft, seeds 1001–1004), bot-v7 won 76/160 with it and **97/160 without it** (gold 5.85 a round against 5.52): kept plans lose contested camps to a bot that replans. **Reverted to 3** (entry 40). Fresher plans against bot-v6 (same seeds, 3: 83/160): 2 rounds 88/160 and 1 round 87/160 (noise), with timeouts 176 and 196 against about 150.
  - **`RALLY_DECISION_ROUNDS` 5 → 15:** a rally join-or-skip decision (a second goals pass, about 15k) is kept for 15 rounds. **119/160**, timeouts 154 (−10%), gold 6.52, deaths 9.9, kills 15.3. **On.**
  - Both together on the same seeds: 114/160, timeouts 124 (−27%), but deaths 11.7. On fresh seeds against bot-v5: 113/160 against 108/160 for the rally change alone. Head to head against bot-v6 the stale-plan change lost (above).
  - **`STALE_SKIP_AT_GOAL`** (drop a kept plan whose goals include our own tile: after a capture paid out, the carry stood on the tile 2–3 turns for the kept plan): 107/160, no change in timeouts or gold. **Off.**
- **Open questions / follow-ups:** the plan's group loop and the camp-fight context are still most of a cut-off turn. Reusing the camp-fight context across rounds while nothing in sight changes is the next candidate.

---

## 40. bot-v7 against bot-v6, and gem-carry off
**Commit:** Uncommitted

- **Gem-carry off** (`gemcarry::GEM_CARRY`, entry for v5's gem-carry in its code comment):
  - On the six gem-carry maps against bot-v5 (4 seeds): off **17/24** against 12/24 on (gold against v5 1.24 against 0.96).
  - Against bot-v6 (which has it on) it's even: off 16/24 against 14/24 on (paired seeds), and 21/48 on fresh seeds.
  - The ladder data (entry 34) favours drafts without an AP champion. **Off**, but not settled.
- **Head to head against bot-v6** (bot-v6 = Tristan's allocator, partial sight, dense group lookup and meta draft; bot-v7 = entries 36–39 on top):
  - The first candidate (with `STALE_PLAN_ROUNDS` 5) was **158/320**: even.
  - A revert-one-at-a-time ablation against bot-v6 (160 games each, seeds 1001–1004; candidate 76/160):

    | Reverted | Wins |
    |---|---|
    | Rally decision back to 5 | 77 |
    | **Stale plans back to 3** | **97** |
    | Shielded escape off | 83 |
    | Chase margin back to 3 | 75 |
    | Gem-carry back on | 73 |

    Only the stale-plan change clearly mattered, and it was reverted.
  - **Corrected bot-v7 against bot-v6, fresh seeds 1021–1028: 172/320 (54%).**

    | | bot-v7 | bot-v6 |
    |---|---|---|
    | Gold a round | 5.64 | 5.39 |
    | Tributes a game | 28.9 | 27.0 |
    | Kills | 11.8 | 11.2 |
    | Deaths | 13.1 | 13.4 |
    | Fuel-exhausted turns per 1,000 rounds | 147 | 160 |

  - **Confirmed on a second fresh seed set (1031–1038): 172/320** again. Gold 5.66 a round against 5.42, tributes 29.2 against 26.9, deaths 12.7 against 13.3, kills 11.6 against 11.0, fuel-exhausted turns 147 against 161, 01boss-battle 7/8. **Together 344/640 (53.8%).**
  - Shielded escape off on the same seeds (1021–1024): 69/160 against 83/160. **Kept on.**
  - **01boss-battle against bot-v6** (16 games): **11/16**, tributes 532 a game against 218, gold +18%, our monolith kill first in 10 of 16.
- **Lesson:** against bot-v5, candidates scored 111–119/160 on the seeds the switches were tuned on (981–984), but bot-v7 scored 98/160 on fresh seeds, about bot-v6's 101/160. Tune on one seed set, confirm on another, and always test head to head against the previous bot: the stale-plan loss only showed up against bot-v6.
- **Open questions / follow-ups:**
  - Weak maps against bot-v6: 00monolith 1/8, 00eight 2/8 (gold 0.47), 00gem-fields 0/8 (gold 0.37) on seeds 1021–1028. 00gem-fields is very seed-dependent (3/4 on 1001–1004).
  - Our Heart of Ruin farms worst of our five (1,371 gold a game).

---

## 41. Heart of Ruin shields like Varg, and shields in the duel model
**Commit:** Uncommitted

- **What changed:**
  - **`champions::heart_of_ruin::HEART_SHIELD_SPAM`** (on): Heart of Ruin casts its self-shield whenever it lost health since last turn or an enemy champion is within 2 tiles, at any health, the way Varg does. Before, it saved the shield for half health or a lethal hit, to keep its passive's damage bonus (up to +50% at low health).
  - **`duel::DUEL_SHIELDS`** (on): in the 1v1 duel estimate, a ready self-shield counts as that much more health, for us and for the enemy. Time to kill against time to die used health alone.
- **Why:** the user's suspicion. Our Heart of Ruin farmed least of our five (1,371 gold a game) and died up to 13 times a game on fight maps, while it's the top ladder pick.
- **Results** (head to head on fresh seeds, 8 × 40 maps = 320 games each):
  - **Shield spam:** **182/320** against bot-v7 without it. Team gold 5.86 a round against 5.35, deaths 12.1 against 14.2, kills 12.7 against 10.5. Heart of Ruin's own gold 1,238 against 1,057, kills 1.8 against 1.2, deaths 3.1 against 3.5, shielding 18,035 a game against 7,086. Against bot-v6 on the entry-40 seeds: 167/320 (172 without), with gold against v6 1.08 against 1.05, tributes 30.1 against 28.9, deaths 12.3 against 13.1.
  - **Duel shields** (shield spam on both sides): **168/320**. Gold 5.77 a round against 5.45, kills 12.7 against 11.0, deaths 12.8 against 14.1.
- **Final bot-v7 (both on) against bot-v6, fresh seeds 1061–1068:** 163/320. Gold 5.72 a round against 5.51, tributes 28.5 against 27.0, deaths 13.4 against 13.7, kills 12.1 against 11.7; 01boss-battle 2/8 on this seed set.
  - All bot-v7 against bot-v6 runs so far: 172, 172, 167 and 163 of 320 (52–54%), always with more gold and tributes.
  - 01boss-battle against bot-v6 across all runs: 24/40.
  - The shield changes win clearly against bot-v7 without them, but add little against bot-v6.
- **Open questions / follow-ups:** the duel still ignores Heart of Ruin's passive and its landing dash. `dps::AVERAGED` models the passive but was off when tested in v5.

---

## 42. bot-v8: farm-plan fuel and rallies (diagnosis; every fix tested and off)
**Commit:** Uncommitted (`bots/bot-v8` is a copy of `bots/bot-v7`; each change has a switch, all off)

- **Profiling added** (debug builds only):
  - `trace::meter`/`metered`: fuel per code section on a turn, printed with the spill trace.
  - `rally::RALLY_LOG`: rally calls, joins (with the joiner's distance and health), first hits, ends, expiries, and the rally group's fight verdict near it.
  - Activity categories `walk-rally` and `wait-rally`.
- **Where a cut-off farm turn's fuel goes** (10 games, 7,060 cut-off turns; then 4 large maps):
  - The group loop costs 24–45k, about 1–1.6k for each of ~28 groups.
  - Within it: groups in sight about 5k each, including the camp-fight context (about 9.6k a build, mostly ally reads at ~250 fuel each). Groups out of sight about 0.6k each, plus ~0.4k of bookkeeping per group.
  - Target ranking 6.5–7.6k, of which the walking field from our tile is 5–8k when built.
  - The draft memory (1,024 bytes, already holding the region table and the opening plan) is too small to carry a farm plan.
- **Rallies** (10 games):
  - 366 rallies, of which 82% expired after 20 rounds without anyone hitting the group, and only 21 cleared it.
  - Waiting at or walking to rallies took **11% of all our champions' turns** (Heart of Ruin 16%).
  - Allies do join: a median 1.5 rounds after the call, from about 10 tiles, at full health.
  - But in 47 rallies (6 games) a joined ally stood within 6 tiles of the group, both at full health, and the fight was judged lost every turn. The need is worked out with the strongest absent teammate as the stand-in, while whoever joins may be weaker. The per-fighter escape reserve also makes every rally fight look harder than it is.
- **Fixes tested** (all off):

  | Switch | What it does | Result |
  |---|---|---|
  | `farm::FARM_PRUNE` | Visit groups nearest first; skip those 16+ tiles beyond the nearest available one | 148/320, timeouts up (192 against 150): pruned groups miss the far-group cache |
  | `collect::CTX_ALLY_CACHE` | Cache allies' range, damage, max health and toughness for 10 rounds | 153/320, timeouts −3% (neutral) |
  | `walk::GROUP_FIELDS` | Each group's walking-distance field over the terrain, built with spare end-of-turn fuel, nearest first (about 6k each, about 30k on 01big-world) | Promising alone, unconfirmed (see below) |
  | `farm::TARGET_PER_GROUP` | Rank one tile per group | Timeouts up (211 against 172) |
  | `collect::CHIP_SAFE` | The user's idea: on groups clearly on our side with no enemy near, a fight doing 35% of the work counts as worth it (hit, leave at the escape floor, heal, come back; camps don't heal) | Promising alone, unconfirmed (see below) |
  | `rally::RALLY_FAIL_FAST` | End an unanswered rally after 6 rounds, not 20 | Negative: gold −7%, more deaths |
  | `collect::RALLY_WEAKEST_STANDINS` | Work out a rally's need with the weakest absent teammates | Neutral to negative |

  - **Fuel switches** (pruning, ally cache) were tested over 320 games each.
  - **The other five** were tested over 50 games each (2 seeds × 25 maps), read game by game against a control: the same games with no change, which scored 21/50.
  - Chip and group fields each came out +7 net flips, one tile per group +9.
  - **Chip + group fields + one tile per group combined:** 28/50 against the control's 21 on those seeds, but 20/50 against a fresh control's 23 (net −3). So no switch is shown to help.
- **Lessons:**
  - 50-game tests can't separate effects this small. Even a change that doesn't alter decisions changes about half the games.
  - A control on the same seeds is needed: with only 2 seeds the tested copy's seed alone was worth −4 wins and −9% gold.
- **Team escape reserve** (`collect::TEAM_ESCAPE_RESERVE`, on in bot-v8): with allies in a camp fight, only the front fighter (shortest attack range, the one the camps hit) keeps the walk-out reserve.
  - Results (2 seed pairs × 50 games against same-game controls): 25/50 against 21/50 (+11/−7 flips) and 25/50 against 23/50 (+9/−7). Together +20/−14, a slight lean in favour.
  - Rallies barely changed: cleared 96 against 97, expired unhit 1,252 against 1,301.
  - Worked through the check, two early champions still honestly lose to two elk (about 30 rounds at 120 a round).
- **Open questions / follow-ups:**
  - What sinks rallies is their sizing. "One more ally" is worked out for the strongest absent teammate (often Vesper Vale), while the ally who comes is often weaker. Weakest stand-ins alone were neutral; try them with the team reserve.
  - 96 of 1,552 rallies had the fight judged winnable at some point but nobody attacked. Worth a trace.
- **Follow-up (same games, same controls, 2 seed pairs × 50 games each):**
  - **Team reserve + weakest stand-ins** (`RALLY_WEAKEST_STANDINS`, now on): **58/100 against the controls' 44/100**, +23/−9 game flips (about 2% likely by chance; a neutral change drifts about +2 here). Rallies cleared about +10%. **Both on in bot-v8.**
  - **The winnable-but-unattacked rallies, traced:** after the verdict turned winnable, the champion's next turns were most often `rally-skip-closer-camp`. The skip was either decided while far away and kept for `RALLY_DECISION_ROUNDS` (15), or the 10-step waiting charge let a camp 11 steps off beat a rally 2 steps off.
  - **`farm::RALLY_GO_WHEN_WINNABLE`** (within 8 tiles of a rally whose fight is winnable now: go, ignoring the wait charge, the kept decision, the cap and side camps): rallies cleared 302 against 219, but 51/100 (+29/−22). With weakest stand-ins as well: 50/100. **Off.**

## 43. What causes cut-off turns, and keeping plans that won't fit (`farm::FIT_STALE_ROUNDS`)
**Commit:** Uncommitted (`bots/bot-v8`)

- **Why:** against the top-50 ladder teams in low-interaction games, bot-v8 had about 227 fuel-exhausted (cut-off) turns per 1,000 rounds against their 26. A cut-off turn stops mid-code and finishes in the next round, so the champion loses a turn.
- **How it was found:** traced bot-v8 self-play logged every turn with its checkpoints and the number of enemies, allies and camps in sight. 40 games gave 236k turns, 4.1% of them cut off. The cut-off rate was then split by situation.
  - Profiling fixes first: fuel reads in the trace now happen inside `debug_block` (free locally). A turn counts as cut off only when fuel jumps up by more than 5k between checkpoints; refunded reads made back-to-back checkpoints look like small jumps.
- **The condition: building a farm plan with camps in sight.**
  - Turns that built a plan were 90% of all cut-offs. Turns that reused or kept a plan ran out 0–0.4% of the time.
  - Cut-off rate of plan builds by camps in sight: 0 camps 5%, 1–3 7%, 4–8 18%, 9+ 50%. On turns without a plan build, even 9+ camps only reached 2%.
  - Enemies in sight barely mattered (4.9% with 2+ against 4.0% with none). Allies in sight mattered somewhat (7.9% with 3+ against 2.6% with none), through the camp-fight context's ally reads.
  - Worst maps: 01yellow-brick-road (26% of all turns), 01boss-battle, 00split, 00lanes, 00quadrants.
  - Half of these cut-offs started below `FARM_PLAN_MIN_FUEL` (85k) with no kept plan young enough (`STALE_PLAN_ROUNDS` 3). In camp-dense spots the plan was rebuilt every third round and cut off most times. The other half started above 85k and still didn't fit.
- **Where a dense plan's fuel goes** (fine meters, 9+ camps in sight; ~65k a plan against ~25k with none):
  - Camp-fight verdicts 28k, including the camp-fight context at 11.7k a build: allies ~4k, camp list ~4k. A resumed turn builds the context again.
  - The live-camp scan of the near pass, 9k: each member of each group in sight against every camp in sight.
  - Target ranking 5k and far groups 5k.
  - The turn before the farm step costs another 25–30k (sight, support, safety checks), and turns spent hitting a camp end with only ~60k left. A dense plan has no spare turn to be built in.
- **Fixes that failed:**
  - **Camp reads kept per sight:** a camp host read is only ~100 fuel, and the thread-local lookup cost 400–500. Cut-off turns rose 23% over 100 games. Removed.
  - **Fight context kept across a step** (`collect::CTX_ROUND_REUSE`): no effect. The second context of a turn comes from resuming, not stepping. Off.
  - **Live-camp tile bitboard** (`farm::LIVE_TILES_LOOKUP`): saved 3k per dense plan but left cut-offs unchanged (176/185 against 178/177). Wins 50/100 against the controls' 55. Off.
- **The fix: `farm::FIT_STALE_ROUNDS`** (on, 6). Each champion measures what its last plan build cost. When the fuel left is below that cost plus a 15k move reserve, it keeps its last plan for up to 6 rounds instead of 3. It does so even above 85k.
  - **Self-play, 4 seed pairs × 50 games, each against a same-game control:** 105/200 against 101/200 (+44/−40). Cut-off turns 143 against 175 per 1,000 rounds (−18%), and −25 to −33% on the dense maps. Gold a little up.
  - **Milder variants:** 4 rounds gave 108/200 and −7%. 6 rounds only without enemies in sight gave 107/200 and −13%.
  - **Against bot-v7, 2 seed pairs:** 57/100 against 48/100 for bot-v8 without it (+28/−19), cut-off turns −17%. In entry 39, longer kept plans lost to a bot that replans. This version only keeps a plan when a fresh one would be cut off anyway, and it didn't lose.
  - **bot-v8 (without it) against bot-v7:** 48/100 against 43/100 for bot-v7 against itself on the same seeds (+26/−21).
  - The control runs reproduce exactly (0 flips over 100 games), so the same-game comparisons are sound.
- **Open questions / follow-ups:**
  - Dense plan builds still run out about half the time. The real fix is a cheaper camp-fight check with many camps in sight: fewer verdicts per plan, or a leaner context. Every fuel saving so far has been a few k out of ~100k.
  - Tools (scratchpad): `cut_conditions.py` (cut-off rate by situation), `plan_cost.py` (segment costs by camps in sight), `meters.py` (fine meters), `fuel_cmp.py` (cut-offs per map against a control).

## 44. Rally replies (`rally::RALLY_REPLIES`) and neutral fuel savers on
**Commit:** Uncommitted (`bots/bot-v8`)

- **What changed:** every champion answers a new rally on its first turn after the call, before its fight and farm code.
  - **No** if the rally already has the allies it needs.
  - **Yes** if it can arrive within 18 steps: on foot (needs 60%+ health), or by recalling first (5 turns, plus the walk from our spawn). So a rally near our base can be answered from anywhere.
  - **No** if it's dead, recalling, holding a capture, or the gate guard.
  - Yes replies use the join bits in shared byte 5; no replies use byte 4, the only free byte.
  - The caller cancels if too few say yes within 1 round, or as soon as everyone has answered, and gives that group up for 40 rounds.
  - Only the champions that said yes go. They only hit the rally group's camps on the way (or awake ones), and only reuse a kept plan if it's a rally plan.
  - Everyone else ignores the rally: no waiting charge, no cached skip.
- **Why:** 47% of rallies had nobody join and 82% expired before anyone hit the camp (about 11% of champion turns). The usual reason was `rally-skip-closer-camp`.
- **Results:** 4 seed pairs × 50 games against same-game controls (control 102/200; a rerun of one control reproduced it exactly).
  - Without the two on-the-way rules: 99/200 (+40/−43).
  - With them: 96/200 (+46/−52).
  - Fuel-exhausted turns 118 and 122 per 1,000 rounds against 138 (−12 to −15%). Gold ratio a little lower (0.97 against 0.99).
  - Traced (6 games): 53–62% of rallies were cancelled at once instead of running out 20 rounds. Helpers that said yes spent 48% of the rally on it (42% without the on-the-way rules), but answered rallies still mostly ran out unwon (17 of 251 cleared).
  - **On** (user rule: a fuel saver that tests neutral stays on).
- **Also on now, same rule:** `collect::CTX_ALLY_CACHE`, `farm::LIVE_TILES_LOOKUP`. `TARGET_PER_GROUP` stays off: it raised fuel-exhausted turns (211 vs 172).
- **Open questions / follow-ups:**
  - Answered rallies that still expire: the fight check doesn't turn winnable, or helpers arrive late.
  - Rewards left uncaptured (8% of clears never captured):
    - Melee champions hand rewards to Vesper Vale whenever she has a capture tile in range: 10% are then never captured, and the rest take a median 8 rounds.
    - Champions shoot other camps on the way to a capture.
    - They go home, bank or kite off the tile before the 5-round capture completes.

## 45. Rally commit, hand-off model 1, going home for real upgrades (overnight tests)
**Commit:** Uncommitted (`bots/bot-v8`)

- **What changed (all on together):**
  - **`rally::RALLY_COMMIT`:** once the caller and the allies it asked for (joined, within 3 tiles of the group) are all there, the fight is on without being judged again.
  - **`camps::HANDOFF_MODEL` 1:** a tank or melee champion leaves a capture to Vesper Vale only when she's within 2 tiles of a capture tile, instead of whenever one is in her attack range.
  - **`HOME_STAT_GAIN` 0.15 (bot.rs):** go home for gold only once it buys shards adding 15% to the stat the build buys next. Otherwise only missing health, or the low-health recall, sends a champion home. Before, 150 gold was enough.
  - Also: champions now recognise they've already answered a rally from their own bit in shared memory, so the rally timer can be restarted.
- **Why:**
  - Of 117 answered rallies, 28 had everyone there and the verdict stayed "lose" (the caller sized the rally with stand-ins). Another 37 had everyone there, judged winnable, and still ran out of time.
  - 10% of captures left to Vesper were never captured.
  - Champions went home a lot.
- **Results:** 4 seed pairs × 50 games each against same-game controls.
  - **Overnight** (seeds 2101–2108, control 97/200):

    | Config | Wins |
    |---|---|
    | `RALLY_COMMIT` | 111 (gold +4.5%, deaths −0.4) |
    | `RALLY_REFRESH` (restart the clock on answer and on hits) | 108 |
    | Refresh + commit | 108 |
    | Hand-off model 1 | 105 |
    | Hand-off model 2 (Vesper closer than us) | 99 |
    | `FUNNEL_MELEE` off | 109 |
    | `TANK_FUNNEL` off | 88 |
    | Going home at 15% / 25% / 40% | 109 / 108 / 107 (deaths rise with the threshold) |

  - **Confirmed on fresh seeds** (2201–2208, control 102/200):

    | Config | Wins |
    |---|---|
    | **Commit + model 1 + going home at 15%** | **137 (+71/−36), gold +6.9%, gems submitted +10%** |
    | Commit + `FUNNEL_MELEE` off + going home at 15% | 112 |
    | Same, plus refresh | 117 |
    | Commit alone | 107 |

  - **Against bot-v7** (2 seed pairs): 61/100 against 54/100 for bot-v8 without them (+24/−17), gold ratio 1.10–1.14 against 1.03–1.08.
- **Open questions / follow-ups:**
  - Kills didn't go up when ahead (11.7 against 11.0).
  - Going home less raises deaths slightly.
  - `RALLY_REFRESH` stays off.

## 46. Chase rules, rally availability (2026-10-24)
**Commit:** Uncommitted (`bots/bot-v8`)

- **Chase gate** (the user's rules), each tested over 100 games against the same control (50/100):

  | Rule | Wins | Note |
  |---|---|---|
  | All rules together | 42 | kills 8.3 against 10.9, deaths 12.9 against 13.2 |
  | Busy leash: 3 tiles while there's a reward, rally or capture to do | 45 | |
  | Vesper Vale only chases with dash + one-shot | 51 | |
  | Catch-up within 8 rounds | 42 | |
  | Catch-up within 15 rounds | 43 | |

  The chases end in kills and cost few deaths. **All off.**
- **`rally::RALLY_AVAIL`** (the user's design, on):
  - Shared byte 4 now marks who can't come to any rally right now: dead, or fought enemies this turn. Each champion updates its own bit every turn.
  - A rally is only called if enough allies are available for the need.
  - The caller sets priority 0 so it acts last in the next round (higher priority acts first), and sees every answer before it keeps or cancels.
  - Results: 119/200 against 101, then 96/200 against 95 on fresh seeds. Together 215/400 against 196 (+101/−82), gold +2–3%, gems submitted +7%, more kills, slightly fewer deaths.
- **Chase guards, off:**
  - `chase::ENEMY_BASE_NO_CHASE` (no chasing within 3 tiles of the enemy spawn): 107/200 against 101 alone.
  - `chase::CHASE_CAMP_GUARD` (no chase step into an awake camp's reach): 109/200 against 101 alone.
  - Both on top of `RALLY_AVAIL`, on fresh seeds: 86/200 against `RALLY_AVAIL`'s 96, with kills down.

## 47. Lost-world loops, Vesper idling, captures near fights, dense-area heuristic (2026-10-24)
**Commit:** Uncommitted (`bots/bot-v8`)

- **Lost-world loop (traced):**
  - Champions paced between (6–8, 11–12), above the long wall, for 10–13 rounds at a time.
  - From there the middle gem clusters show over the holes, about 20 steps away around the wall. A reward in sight came before everything, including a rally the champion had said yes to, so each step flipped the target: reward → nearest camp or rally → reward.
  - **`farm::REWARD_PRIORITY_STEPS` 8** (rewards only take priority within 8 steps on foot): loops 38 → 26 in 4 traced lost-world games, but 54/100 against 52 overall, with gold −3.7%. **Off.**
- **Vesper Vale idling** (25 traced games): 6% of her turns.

  | Share of her turns | Cause |
  |---|---|
  | 2.1% | No fuel left to move after the fight check |
  | 1.2% | All goal tiles blocked (57% of those on a kept plan) |
  | 1.2% | Capturing |
  | 0.7% | Rally (down from about 3.9% before `RALLY_AVAIL`) |

  **`farm::BLOCKED_REPLAN`** (drop the kept plan and rebuild when goals are blocked): 53/100 against 52. **Off.**
- **Captures given up for distant fights:** fighting runs before the capture check each turn, and enemies up to 7 tiles away pulled champions off capture tiles (leaving resets the capture).
  - **`fight::CAPTURE_FIGHT_TILES`:** while capturing, or with the plan heading for a reward or capture, only enemies within N tiles count.
  - 3 tiles: **61/100 against 52 (+27/−18), gold +7.9%, gems submitted +8.6%, kills up, deaths down. On.**
  - 5 tiles: 52/100. Off.
- **Dense-area heuristic, `collect::DENSE_ALIKE_VERDICTS`** (a group that looks like one already judged this turn takes its verdict): 100/200 against 116, gold −5.1%, fuel-exhausted turns up. **Off.**
- **No chasing within 3 tiles of the enemy spawn** (`chase::ENEMY_BASE_NO_CHASE`): **on**, at the user's call.

---

## Testing recipes

- **vs starter on every map, both sides:** the `cambc run-batch` loop (writes a manifest to `replays/vs-starter/`, runs with `--jobs 8`, prints the win count). Change `OPP=` to test against another bot.
- **vs an older version of our bot:** `git archive HEAD bots/my-bot | tar -x --strip-components=2 -C bots/my-bot-old`, then run against `bots/my-bot-old`.
- **Fuel / timeouts:** see `docs/RustFuelPitfalls.md`.
- **Matches are deterministic:** the same bots, map and sides always give the same result. Use more maps and both sides for a bigger sample. A refactor that leaves every result unchanged has not changed behaviour.

## Entry template

```md
## N. Title
**Commit:** `<hash>` or "Uncommitted"

- **What changed:** behaviour, in plain words, and where it lives (file / hook).
- **Why:** problem or hypothesis.
- **Results:** what was run (maps, sides, opponent) and the numbers. Note anything else that differed between versions.
- **Open questions / follow-ups:**
```
