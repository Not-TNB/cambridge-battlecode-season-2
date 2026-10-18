# my-bot writeup log

A running log of every feature and change to `bots/my-bot`, written as we go so we can turn it into the final writeup. Add the newest entry at the bottom using the template at the end. Where we have them, include the commit, why we made the change, and how we measured it.

## Code layout

| File | What lives there |
|---|---|
| `src/main.rs` | Entry point: draft vs play dispatch, `random_number` / `random_seed` (with the per-build `SEED`) |
| `src/draft.rs`, `src/constants.rs` | Draft logic and the champion class table |
| `src/bot.rs` | The turn skeleton shared by every champion: stat choice, recall, shopping, then the champion's hooks |
| `src/common.rs` | Shared helpers: per-turn caches (`sight`, `blocked_tiles`), pathing, attacks, ability casting, fighting, banking gems, farming (`farm_plan`), wards, the `TRACE` debug log |
| `src/safety.rs` | Enemy sightings memory, recalls that wait for a safe channel, retreating from losing fights |
| `src/collect.rs` | Capture holding, escorting (tanks and Willow), camp-fight estimates, and greedy collection |
| `src/champions/mod.rs` | The `ChampionAi` trait (decision hooks with shared defaults) and `for_champion` dispatch |
| `src/champions/<name>.rs` | One file per champion (12, with Cassi and Issac separate) with its kit summary and any overrides |

Each turn, `bot.rs` runs, in order: skip if unable to act, then shop (`shops_anywhere`), then `support()`, then recall when below 35% health, then `fight()`, then the go-home recall, then `bank_gems()` when `should_bank()`, otherwise `farm()`; with fuel to spare it then records enemy sightings and places a ward. To give a champion its own behaviour, override the hook in its file and leave the rest to `common.rs`.

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
- **Debug trace (`TRACE` in `common.rs`):** when on, every turn prints one line with our tile, health, fuel and the branches taken (`trace("tag")`), and `checkpoint("tag")` records fuel at a point for profiling. A script then finds runs of 8+ turns on one tile ("standing still", not counting camp fights) and 6+ turns alternating between two tiles ("back and forth"). Caveats found the hard way:
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
