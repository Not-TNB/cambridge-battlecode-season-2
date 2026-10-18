# Rust Bot Fuel Pitfalls

Team notes, not part of the official handbook. Read alongside BotAPIRust.md and the Fuel section of GameRules.md.

A Rust bot gets 100k fuel per turn. If it runs out, it is suspended mid-turn and resumes at the start of its next turn, so it effectively loses a turn. In a 200-round starter-rust mirror on `collision`, the unmodified starter timed out on about 10% of champion turns. All of them were in camp farming.

## Rule: avoid creating `Bitboard`s in loops

In the Rust SDK, **every new `Bitboard` costs about 2k fuel**. `Bitboard::new()` asks the host for the map layout again on every call, then validates it, allocates and zeroes the board (`sdk/rust/src/board.rs`). The C++ SDK keeps the layout from startup, which is the most likely reason the same starter logic does not time out in C++. That comes from reading the SDK code; the C++ bot has not been profiled.

These SDK calls create a new board each time they are called:

| Creates a new board (~2k fuel) | Use instead |
|---|---|
| `Bitboard::new()` | Make one before the loop and reuse it |
| `group.spawn_tiles()` | Loop over `group.camps()` and use `camp.spawn_position()` |
| `a.intersect(&b)`, `a.union(&b)`, `a.complement()`, `a.clone()` | `a.intersect_assign(&b)`, `a.union_assign(&b)` on an existing board |
| `vision::visible_tiles()`, `vision::camp_tiles()`, `vision::*_tiles()` | Call once per turn and reuse the result, or use the `*_into(&mut board)` variant |
| `unit.attack_tiles()` | `attack_tiles_into(&mut board)`, or compare `chebyshev_distance` with the attack range |
| `CampGroup::state()` | Creates a vision board internally; only call it when needed |
| `tiles_around(p, r)` then `union_assign` | `goals.draw_chebyshev(p, r as f32)` directly |

Calls that are cheap because they don't allocate: `is_set`, `set`, `is_empty`, `count`, `*_assign`, `draw_chebyshev`, `DistanceField::get`.

## The fix we made (starter-rust and my-bot, `farm_camps`)

The original loop created about three boards per camp group, about 60 boards per turn with 20 groups. That came to about 65k fuel before pathfinding even started:

```rust
// Before: ~3 new Bitboards per group
for group in cambc::map::camp_groups()? {
    if !group.spawn_tiles()?.intersect(&visible)?.is_empty()?
        && matches!(group.state()?, cambc::CampGroupState::Respawning(_)) { continue; }
    goals.union_assign(&group.spawn_tiles()?)?;
}
```

```rust
// After: no new Bitboards inside the loop
for group in camp_groups()? {
    let mut seen = false;
    for camp in group.camps() {
        if visible.is_set(camp.spawn_position())? { seen = true; break; }
    }
    if seen && matches!(group.state()?, cambc::CampGroupState::Respawning(_)) { continue; }
    for camp in group.camps() { goals.set(camp.spawn_position(), true)?; }
}
```

Two smaller wins:

- **Cache values that never change.** `cambc::map::camp_groups()` asks the host every call (~11k fuel), but camp membership is fixed for the game. Load it once into a `thread_local` `OnceCell` (see `camp_groups()` in `bot.rs`). `group.state()` is still live and must be queried each turn.
- **Draw directly onto the goal board.** In the altar loop, `goals.draw_chebyshev(altar, range)` replaces creating a new board per altar and merging it in.

Result in `farm_camps`: about 95k+ fuel became about 23k average and 37k worst case. In a 200-round mirror on `collision`, the bot had 0 timed-out turns and always had at least 50k fuel left at the end of a turn.

## Other costs we measured

From `checkpoint()` profiling in `my-bot` (writeup entry 24):

| Thing | Fuel |
|---|---|
| A simple SDK query (`position`, `is_set`, `health`, ...) | ~50–100 |
| `vision::champions(team)` | ~2–4k (it builds each `Champion`, which reads its ability slots) |
| `vision::camps()` | ~2k |
| Formatting one float (`{:.0}`) | ~9k: print integers in debug output |
| Plain Rust: 1,000 `Vec` pushes | ~35k: fuel is roughly wasm instructions, so loops over every camp of every group several times a turn add up |
| `farm_plan` (writeup entry 27) | ~32k a turn before caching; ~12k of it the camp-fight verdicts |
| A full-map `DistanceField::compute` | ~1.5k (30×30) to ~15k (100×100); exporting it to Rust costs about as much again |

- **Cache per turn:** results that only change when we move or act (visible units, blocked tiles, threats) can be cached per round and position. Drop the cache after an attack, cast, dash or ward placement, any of which can change what we see.
- **Don't build `Champion`s from `cambc::champions()` to reuse:** a champion built while hidden keeps "not in vision" errors in its ability slots.
- **Don't compute what the turn may not use:** the fight code built the farm plan to check one flag on 84% of turns, and half of those plans were never used. Check the cheap conditions first.
- **Reuse across turns, not just within one:** values that only change slowly (camp stats, damage estimates from abilities) or whose inputs haven't changed (a camp-fight verdict, a farm plan while standing still) can be kept between turns behind a small key.
- **Small per-turn costs lose games when fuel is tight:** with 27% of turns running out of fuel, adding 2–4k a turn (walking-distance lookups, entry 27) lost ~12 of 174 games on its own.
- **A debug print that runs out of fuel** suspends the champion into its next turn. Traced games are slower than real ones, so don't use them for win rates.

## Checking for timeouts

Before merging new per-turn logic, log fuel around it and run a mirror match:

```rust
// In main.rs's loop (remove before submitting)
let start_round = cambc::round_number().unwrap_or(0);
// ... play_turn ...
if !cambc::is_draft() {
    println!("FEND r={} started_r={} fuel={}", cambc::round_number().unwrap_or(0),
        start_round, cambc::fuel_remaining().unwrap_or(0));
}
```

```sh
cambc run bots/my-bot bots/my-bot --rounds 200 -o /tmp/fuelcheck
cd /tmp/fuelcheck && unzip -o -q *.repl26a -d out
grep -ao "FEND r=[0-9]* started_r=[0-9]*" out/team-a.stdout | awk '{split($2,a,"=");split($3,b,"="); if(a[2]!=b[2])n++} END{print n+0" timed-out turns"}'
```

A turn whose `r` differs from `started_r` ran out of fuel and was resumed. Bot stdout is stored inside the `.repl26a` replay, which is a zip file (`team-a.stdout`, `team-b.stdout`). Use `grep -a` because the logs contain non-text bytes.

To find which part of the code is expensive, print `cambc::fuel_remaining()` between steps. Keep a margin: the worst-case turn should stay well below 100k, and other maps may have more camp groups or longer paths. For optional work, stop and yield once `fuel_remaining()` drops low (see BotAPIRust.md).
