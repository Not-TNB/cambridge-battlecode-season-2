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
