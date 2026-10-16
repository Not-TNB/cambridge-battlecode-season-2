# my-bot writeup log

A running log of every feature and change to `bots/my-bot`, written as we go so we can turn it into the final writeup. Add the newest entry at the bottom using the template at the end. Where we have them, include the commit, why we made the change, and how we measured it.

## Code layout

| File | What lives there |
|---|---|
| `src/main.rs` | Entry point: draft vs play dispatch, `random_number` / `random_seed` (with the per-build `SEED`) |
| `src/draft.rs`, `src/constants.rs` | Draft logic and the champion class table |
| `src/bot.rs` | The turn skeleton shared by every champion: stat choice, recall, shopping, then the champion's hooks |
| `src/common.rs` | Shared helpers: pathing, blocked tiles, attacks, ability casting, fighting, banking gems, farming |
| `src/champions/mod.rs` | The `ChampionAi` trait (decision hooks with shared defaults) and `for_champion` dispatch |
| `src/champions/<name>.rs` | One file per champion (12, with Cassi and Issac separate) with its kit summary and any overrides |

Each turn, `bot.rs` runs, in order: skip if unable to act, then shop (`shops_anywhere`), then recall when below 35% health, then `fight()`, then `bank_gems()` when carrying gems, otherwise `farm()`. To give a champion its own behaviour, override the hook in its file and leave the rest to `common.rs`.

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
