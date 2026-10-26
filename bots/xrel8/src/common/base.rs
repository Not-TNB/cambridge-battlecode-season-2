// Going back: banking gems at altars, the willingness to recall to shop and heal, and our
// spawn.
use super::me;
use super::movement::move_towards_closest;
use super::trace::{checkpoint, trace};
use super::world::distance;
use cambc::{Bitboard, Result, Team, Vec2};

// How much a step closer to our spawn (on foot, walk::spawn_steps) is worth, in steps closer to
// safety, when fleeing or walking to a recall spot. Above 1, so heading home can win over the
// nearest safe tile a little: at 0.5 (per straight-line tile) the nearest safe tile always won,
// and with enemies about that could lie across the middle of the map (hedgerows: Prism fled
// through the centre and died there). Our spawn heals and enemies can't walk onto it.
pub const RETREAT_HOME_PULL: f32 = 1.25;

// The pull towards home for `tile` (see RETREAT_HOME_PULL).
pub fn home_pull(tile: Vec2) -> Result<f32> {
    let steps = match super::walk::spawn_steps(tile) {
        Some(steps) => steps,
        None => distance(tile, ally_spawn_centre()?),
    };
    Ok(RETREAT_HOME_PULL * steps as f32)
}

// Centre of the enemy's spawn region.
pub fn enemy_spawn_centre() -> Result<Vec2> {
    spawn_centre(Team::Enemy)
}

// Bank only at altars on our side of the map (no further from our spawn than from theirs), unless
// one is within BANK_NEAR_TILES: the nearest altar sent our champions to the one next to the
// enemy base (hedgerows: 18% of Prism's banking walks, and two of its deaths). Off: any altar.
pub const BANK_OWN_SIDE: bool = true;

fn altar_allowed(altar: Vec2, here: Vec2) -> Result<bool> {
    Ok(!BANK_OWN_SIDE
        || distance(here, altar) <= BANK_NEAR_TILES
        || distance(altar, ally_spawn_centre()?) <= distance(altar, enemy_spawn_centre()?))
}

// Centre of our spawn region.
pub fn ally_spawn_centre() -> Result<Vec2> {
    spawn_centre(Team::Ally)
}

// Spawn centres never change: worked out once per champion (listing the spawn tiles is a host
// call, and banking asked for both centres for every altar: 23k fuel a turn on 00lanes' 18 altars).
fn spawn_centre(team: Team) -> Result<Vec2> {
    thread_local! {
        static CENTRES: std::cell::Cell<[Option<Vec2>; 2]> = const { std::cell::Cell::new([None, None]) };
    }
    let slot = usize::from(team == Team::Enemy);
    if let Some(centre) = CENTRES.with(|c| c.get()[slot]) {
        return Ok(centre);
    }
    let tiles = cambc::map::spawn_region(team)?.positions()?;
    let n = tiles.len().max(1) as i32;
    let (x, y) = tiles.iter().fold((0, 0), |(x, y), t| (x + t.x, y + t.y));
    let centre = Vec2::new(x / n, y / n);
    CENTRES.with(|c| {
        let mut all = c.get();
        all[slot] = Some(centre);
        c.set(all);
    });
    Ok(centre)
}

const LOCK_CHECK_TILES: i32 = 8;
// An enemy lock on the altar we're at is broken (a tribute at a locked altar attacks the lock;
// five hits break it) unless an unlocked altar is within LOCK_BREAK_TILES; with none to go to, a
// locked one is walked to and broken. On 01boss-battle (one altar) an enemy lock cancelled an
// 89-turn tribute and the champion never tried again. Off: locked altars are skipped.
pub const BREAK_ENEMY_LOCKS: bool = true;
const LOCK_BREAK_TILES: i32 = 6;

// Altars we can tribute at: some tile within tribute range is floor (not a wall, hole, altar or
// camp tile), or is a camp's tile while that camp is dead. On 01boss-battle the only altar's
// neighbours are walls, holes and a monolith's tile: champions with gems stood next to it for 100+
// turns instead of farming, but once the monoliths are killed (late in the game we do) the tile is
// free for their 150-round respawn, and standing on it holds the respawn off. Off: every altar.
pub const OPEN_ALTARS_ONLY: bool = true;
// A gate camp's group known to be respawning (farm::respawn_ready) opens its altar while there's
// at least the walk there plus this many rounds left of the respawn.
const GATE_MARGIN_ROUNDS: u32 = 5;
// Queue at a gate: an ally tributing on the gate tile doesn't close the altar for us; we wait on
// another tile of the gate camp's group within GATE_QUEUE_TILES of the altar and step on when the
// gate frees. Standing on a group's tile holds its respawn off, so the queue keeps the altar open
// for as long as we have gems. On 01boss-battle (one altar, gated by a monolith pair) we kept
// 1,200-1,500 gems a game untributed: one champion at a time (a turn per 10 gems), the rest went
// back to farming, and the monoliths respawned the moment the gate was left. Off: an ally on the
// gate closes it for us.
// 2026-10-22, 01boss-battle against v4, 3 seeds both sides: 6/6 against 5/6 off, but tributes
// 637 and 1,118 a game against 940 and 992, carried at the end about the same (~1,450): the
// monoliths die late (first kill round 1,046-1,977, none in 3 of 12 games), so the gate is shut
// most of the game whatever we do at it. Off.
// v7: with safety::SHIELDED_ESCAPE, 16 games: 10/16 against 8/16 off (tributes 489 against 321);
// with GATE_GUARD 12/16 either way, tributes 645 against 587. On.
pub const GATE_QUEUE: bool = true;
const GATE_QUEUE_TILES: i32 = 2;

// v7 gate guard: where every altar is gated (01boss-battle: the only altar sits in the wall behind
// a monolith pair, whose tiles are also the only way between the two halves), once a gate group is
// down our tank (else first bruiser) stands on one of its tiles outside tribute range. An occupied
// tile holds the whole group's respawn off, so the altar stays open while the rest tribute through
// the gate tile, and the guard blocks the way through. Champions remember a gate they've seen down
// as open until they see it alive again (its respawn round passes while the guard holds it).
// Off: no guard; a gate is open only while seen down or known to be respawning.
// 2026-10-23, 01boss-battle against bot-v5, 16 games (8 seeds, both sides), with SHIELDED_ESCAPE:
// 12/16 against 8/16 off (tributes 587 a game against 321; v5's 205 against 309); with GATE_QUEUE
// as well 12/16 (tributes 645 against 177).
pub const GATE_GUARD: bool = true;

thread_local! {
    // Gate groups last seen down (GATE_GUARD).
    static GATES_DOWN: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn note_gate(group: usize, down: bool) {
    GATES_DOWN.with(|g| {
        let mut g = g.borrow_mut();
        g.retain(|&x| x != group);
        if down {
            g.push(group);
        }
    });
}

fn remembered_down(group: usize) -> bool {
    GATE_GUARD && GATES_DOWN.with(|g| g.borrow().contains(&group))
}

// Whether every altar is gated (OPEN_ALTARS_ONLY found no free tile next to any of them).
fn all_altars_gated() -> Result<bool> {
    let access = altar_access()?;
    Ok(!access.is_empty() && access.iter().all(|(_, gates)| !gates.is_empty()))
}

// The tile we hold as the gate guard, if we're the guard and a gate group is down (GATE_GUARD).
pub fn gate_guard_tile() -> Result<Option<Vec2>> {
    if !GATE_GUARD || !all_altars_gated()? {
        return Ok(None);
    }
    use crate::constants::{class_of, ClassKind};
    let team = crate::opening::team();
    let guard = team
        .iter()
        .find(|&&c| class_of(c) == ClassKind::Tank)
        .or_else(|| team.iter().find(|&&c| class_of(c) == ClassKind::Bruiser));
    if guard != Some(&me()?.champion_name()?) {
        return Ok(None);
    }
    let range = cambc::constants().submit_gems_range;
    let sight = super::world::sight()?;
    let groups = super::world::camp_groups()?;
    let here = me()?.position()?;
    let mut best: Option<(i32, Vec2)> = None;
    for (altar, gates) in altar_access()? {
        for &(_, group) in gates {
            let mut seen = false;
            let mut alive = false;
            for camp in groups[group].camps() {
                let p = camp.spawn_position();
                if sight.visible.is_set(p)? {
                    seen = true;
                    alive |= sight.camps.iter().any(|c| c.spawn_position() == p);
                }
            }
            if seen {
                note_gate(group, !alive);
            }
            let down = (seen && !alive)
                || (!seen
                    && (super::farm::respawn_ready(group).is_some() || remembered_down(group)));
            if !down {
                continue;
            }
            for camp in groups[group].camps() {
                let p = camp.spawn_position();
                if distance(p, *altar) > range && best.is_none_or(|(d, _)| distance(p, here) < d) {
                    best = Some((distance(p, here), p));
                }
            }
        }
    }
    Ok(best.map(|(_, p)| p))
}

// Hold the gate (GATE_GUARD): walk to the guard tile, or stay on it. Returns whether we did.
pub fn guard_gate() -> Result<bool> {
    let Some(tile) = gate_guard_tile()? else {
        return Ok(false);
    };
    trace("gate-guard");
    if me()?.position()? == tile {
        return Ok(true);
    }
    let mut goal = Bitboard::new()?;
    goal.set(tile, true)?;
    move_towards_closest(goal)
}

// An altar and the camp tiles (and their groups) next to it that it depends on.
type AltarAccess = (Vec2, Vec<(Vec2, usize)>);

// Each altar we could ever tribute at, with the camp tiles (and groups) next to it that it depends
// on: none if a floor tile is next to it. Worked out once.
fn altar_access() -> Result<&'static [AltarAccess]> {
    thread_local! {
        static ACCESS: std::cell::OnceCell<&'static [AltarAccess]> =
            const { std::cell::OnceCell::new() };
    }
    if let Some(access) = ACCESS.with(|a| a.get().copied()) {
        return Ok(access);
    }
    let altars = cambc::map::altars()?.positions()?;
    let mut access = Vec::new();
    if OPEN_ALTARS_ONLY {
        let range = cambc::constants().submit_gems_range;
        let mut closed = cambc::map::walls()?.clone();
        closed.union_assign(cambc::map::holes()?)?;
        closed.union_assign(cambc::map::altars()?)?;
        let mut camp_tiles = Vec::new();
        for (index, group) in super::world::camp_groups()?.iter().enumerate() {
            for camp in group.camps() {
                closed.set(camp.spawn_position(), true)?;
                camp_tiles.push((camp.spawn_position(), index));
            }
        }
        let floor = closed.complement()?;
        for altar in altars {
            let mut around = Bitboard::new()?;
            around.draw_chebyshev(altar, range as f32)?;
            if !around.intersect(&floor)?.is_empty()? {
                access.push((altar, Vec::new()));
                continue;
            }
            let gates: Vec<(Vec2, usize)> = camp_tiles
                .iter()
                .copied()
                .filter(|&(tile, _)| distance(tile, altar) <= range)
                .collect();
            if !gates.is_empty() {
                access.push((altar, gates));
            }
        }
    } else {
        access.extend(altars.into_iter().map(|a| (a, Vec::new())));
    }
    let access: &'static [AltarAccess] = Vec::leak(access);
    ACCESS.with(|a| {
        let _ = a.set(access);
    });
    Ok(access)
}

// Whether group `index` gates an altar: its camp tiles are the only tiles within tribute range of
// that altar we could stand on (01boss-battle: the monolith pair walled in beside the only altar).
pub fn is_gate_group(index: usize) -> Result<bool> {
    Ok(altar_access()?
        .iter()
        .any(|(_, gates)| gates.iter().any(|&(_, group)| group == index)))
}

// Altars we can tribute at now (OPEN_ALTARS_ONLY): open ones, and gated ones with a gate camp we
// see is dead or know is respawning for long enough.
fn open_altars() -> Result<Vec<Vec2>> {
    let access = altar_access()?;
    let mut open = Vec::with_capacity(access.len());
    for (altar, gates) in access {
        if gates.is_empty() || gate_open(*altar, gates)? {
            open.push(*altar);
        }
    }
    Ok(open)
}

fn gate_open(altar: Vec2, gates: &[(Vec2, usize)]) -> Result<bool> {
    let sight = super::world::sight()?;
    let round = cambc::round_number()?;
    let walk = distance(me()?.position()?, altar).max(0) as u32;
    for &(tile, group) in gates {
        if sight.visible.is_set(tile)? {
            // A gate is one tile: another champion on it (an ally's tribute takes a turn per 10
            // gems) means queueing, so keep farming until it's free.
            let mut taken = sight.camps.iter().any(|c| c.spawn_position() == tile);
            if GATE_GUARD {
                note_gate(group, !taken);
            }
            for champion in sight.enemies.iter() {
                taken |= champion.position().is_ok_and(|p| p == tile);
            }
            let mut ally_on = false;
            for champion in sight.allies.iter() {
                ally_on |= champion.position().is_ok_and(|p| p == tile);
            }
            if !taken && (!ally_on || (GATE_QUEUE && !queue_tiles(altar, gates)?.is_empty()?)) {
                return Ok(true);
            }
            continue;
        }
        if super::farm::respawn_ready(group)
            .is_some_and(|ready| ready > round + walk + GATE_MARGIN_ROUNDS)
        {
            return Ok(true);
        }
        // Seen down and not since seen alive: the guard holds it (GATE_GUARD).
        if remembered_down(group) && super::farm::respawn_ready(group).is_none() {
            return Ok(true);
        }
    }
    Ok(false)
}

// Free tiles of `altar`'s gate groups within GATE_QUEUE_TILES of it, other than the gate tiles in
// tribute range: where we wait for an ally on the gate (GATE_QUEUE). Only tiles we see.
fn queue_tiles(altar: Vec2, gates: &[(Vec2, usize)]) -> Result<Bitboard> {
    let sight = super::world::sight()?;
    let range = cambc::constants().submit_gems_range;
    let groups = super::world::camp_groups()?;
    let here = me()?.position()?;
    let mut tiles = Bitboard::new()?;
    for &(_, group) in gates {
        for camp in groups[group].camps() {
            let p = camp.spawn_position();
            let d = distance(p, altar);
            if d <= range || d > GATE_QUEUE_TILES || !sight.visible.is_set(p)? {
                continue;
            }
            if sight.camps.iter().any(|c| c.spawn_position() == p) {
                continue;
            }
            let mut taken = false;
            for champion in sight.allies.iter().chain(&sight.enemies) {
                taken |= champion.position().is_ok_and(|q| q == p);
            }
            if !taken || p == here {
                tiles.set(p, true)?;
            }
        }
    }
    Ok(tiles)
}

// Tribute at an altar in range, otherwise walk toward the nearest unlocked altar.
// Enemy locks we've seen, kept while their altar is out of sight (a lock stays until it's broken):
// we only see locks in sight, so with every altar locked (ladder teams bought 4-6 locks a game,
// and 00rushbait_v2 and 00collision have two altars each) a champion turned away from the locked
// altar in front of it for the other one, which looked free from afar, found it locked, turned
// back, and never tributed (804 gems carried, none tributed, lost). A known-locked altar is gone
// to and broken (a tribute at a locked altar attacks the lock; five hits) unless an altar not known
// to be locked is less than LOCK_DETOUR_TILES further. Off: locks out of sight are forgotten.
pub const REMEMBER_ENEMY_LOCKS: bool = true;
const LOCK_DETOUR_TILES: i32 = 10;

thread_local! {
    static KNOWN_LOCKS: std::cell::RefCell<Vec<Vec2>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Update the known locks from the altars in sight; returns whether `altar` is known locked.
fn note_locks(enemy_locks: &Bitboard) -> Result<()> {
    if !REMEMBER_ENEMY_LOCKS {
        return Ok(());
    }
    let visible = super::world::sight()?.visible.clone();
    let altars = cambc::map::altars()?.positions()?;
    KNOWN_LOCKS.with(|k| -> Result<()> {
        let mut k = k.borrow_mut();
        for altar in altars {
            if !visible.is_set(altar)? {
                continue;
            }
            let locked = enemy_locks.is_set(altar)?;
            let known = k.contains(&altar);
            if locked && !known {
                k.push(altar);
            } else if !locked && known {
                k.retain(|&a| a != altar);
            }
        }
        Ok(())
    })
}

fn known_locked(altar: Vec2) -> bool {
    REMEMBER_ENEMY_LOCKS && KNOWN_LOCKS.with(|k| k.borrow().contains(&altar))
}

pub fn bank_gems() -> Result<()> {
    let me = me()?;
    // Union all altar approach tiles: the field chooses the nearest reachable altar (on our side,
    // see BANK_OWN_SIDE; any altar if none is).
    let here = me.position()?;
    let range = cambc::constants().submit_gems_range;
    let mut goals = Bitboard::new()?;
    let mut fallback = Bitboard::new()?;
    let mut locked_goals = Bitboard::new()?;
    let mut locked_in_range = None;
    let mut nearest_unlocked = i32::MAX;
    let mut nearest_locked = i32::MAX;
    let enemy_locks = cambc::vision::altar_lock_tiles(Some(Team::Enemy))?;
    note_locks(&enemy_locks)?;
    for altar in open_altars()? {
        // Only enemy locks block us (our own, common::locks, don't). The enemy lock board is one
        // query; has_altar_lock cost ~2.25k an altar (two lookups, one ending in an error).
        if (distance(here, altar) <= LOCK_CHECK_TILES && enemy_locks.is_set(altar)?)
            || known_locked(altar)
        {
            if BREAK_ENEMY_LOCKS {
                locked_goals.draw_chebyshev(altar, range as f32)?;
                nearest_locked = nearest_locked.min(distance(here, altar));
                if distance(here, altar) <= range {
                    locked_in_range = Some(altar);
                }
            }
            continue;
        }
        nearest_unlocked = nearest_unlocked.min(distance(here, altar));
        let allowed = altar_allowed(altar, here)?;
        if allowed && distance(here, altar) <= range {
            me.submit_gems(altar)?;
            return Ok(());
        }
        if allowed {
            goals.draw_chebyshev(altar, range as f32)?;
            // An ally on the gate: wait on a queue tile beside it (GATE_QUEUE).
            if GATE_QUEUE {
                if let Some((_, gates)) = altar_access()?
                    .iter()
                    .find(|(a, g)| *a == altar && !g.is_empty())
                {
                    goals.union_assign(&queue_tiles(altar, gates)?)?;
                }
            }
        } else {
            fallback.draw_chebyshev(altar, range as f32)?;
        }
    }
    // At a locked altar with no unlocked one close by: break the lock (BREAK_ENEMY_LOCKS).
    if let Some(altar) = locked_in_range.filter(|_| nearest_unlocked > LOCK_BREAK_TILES) {
        trace("break-lock");
        me.submit_gems(altar)?;
        return Ok(());
    }
    // A known-locked altar much closer than any other: go and break it (REMEMBER_ENEMY_LOCKS).
    let break_closer = REMEMBER_ENEMY_LOCKS
        && nearest_locked != i32::MAX
        && nearest_locked.saturating_add(LOCK_DETOUR_TILES) < nearest_unlocked;
    if break_closer {
        trace("bank-to-locked");
        goals = locked_goals;
    } else {
        if goals.is_empty()? {
            goals = fallback;
        }
        if goals.is_empty()? {
            goals = locked_goals;
        }
    }
    // We tribute from a free tile, never a camp's: move_towards_closest keeps camp tiles as goals
    // (farming walks up to camps), so on 01shortcut, where the altar shares a room with the
    // monolith's gem clusters, the cluster next to us was the "nearest goal" and we stood beside it
    // for the rest of the game, two tiles from the altar (1,800 turns, 28 gems never tributed).
    goals.intersect_assign(&cambc::vision::camp_tiles()?.complement()?)?;
    checkpoint("bank-goals");
    move_towards_closest(goals)?;
    checkpoint("bank-move");
    Ok(())
}

// Bank when we carry at least BANK_MIN_GEMS, or any gems with an altar within BANK_NEAR_TILES,
// so leaving spawn doesn't turn into a walk to the altar for a small bag. 1 banks whenever we
// carry gems. 40 tested neutral on win rate and halves bank walking (writeup entry 18).
pub const BANK_MIN_GEMS: u32 = 40;
// 2026-10-22: 0 (no small bags near an altar) 16/40 against 6, tributes 567 against 787; with
// BANK_MIN_GEMS 60 as well 18/40 (trips 12 a game against 26, but gems lost on death +48%).
pub const BANK_NEAR_TILES: i32 = 6;
// Where the nearest altar is more than BANK_FAR_ALTAR_STEPS steps away on foot, bank only from
// BANK_FAR_MIN_GEMS: on spine (altars at the far ends of the map) banking every 40 gems made walks
// to the altar ~40% of all walking. 0: off.
pub const BANK_FAR_ALTAR_STEPS: i32 = 16;
pub const BANK_FAR_MIN_GEMS: u32 = 80;
// Prism gains 1 Focus per carried gem (5x the old 0.2): she banks only from PRISM_BANK_MIN_GEMS
// (or at the end of the game). 0: as everyone else. 2026-10-22, Prism in both teams, 320 games
// against 0: 150 136-184 (her kills and damage up, but team tributes 24.7 against 28.5 a game, gems
// lost on death +24%: gems only count once tributed). Off.
pub const PRISM_BANK_MIN_GEMS: u32 = 0;

// Once we decide to bank, keep at it until the bag is tributed (BANK_COMMIT). Without it, a walk to
// an altar that first steps away from a nearby one (around a wall) left BANK_NEAR_TILES, so we went
// back to farming, stepped back and banked again: back and forth for up to 99 turns.
pub const BANK_COMMIT: bool = true;

thread_local! {
    static BANKING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// The game's last rounds: bank whatever we carry once the rounds left only just cover the walk to
// the nearest altar and the tribute channel, plus END_BANK_MARGIN. Gems still carried at the end
// count for nothing (~40 a game a team, not counting 01boss-battle). 0: off.
pub const END_BANK_MARGIN: u32 = 12;

pub fn end_game_bank() -> Result<bool> {
    if END_BANK_MARGIN == 0 {
        return Ok(false);
    }
    let me = me()?;
    let gems = me.gem_count()?;
    if gems == 0 || open_altars()?.is_empty() {
        return Ok(false);
    }
    let here = me.position()?;
    let steps = match super::walk::altar_steps(here) {
        Some(steps) => steps.max(0) as u32,
        None => open_altars()?
            .iter()
            .map(|&a| distance(here, a).max(0) as u32 * 3 / 2)
            .min()
            .unwrap_or(0),
    };
    let left = cambc::constants()
        .max_rounds
        .saturating_sub(cambc::round_number()?);
    Ok(left <= steps + gems.div_ceil(10) + END_BANK_MARGIN)
}

pub fn should_bank() -> Result<bool> {
    let me = me()?;
    let gems = me.gem_count()?;
    if gems == 0 {
        BANKING.with(|b| b.set(false));
        return Ok(false);
    }
    // Nowhere to tribute: keep farming (OPEN_ALTARS_ONLY).
    if open_altars()?.is_empty() {
        return Ok(false);
    }
    if BANK_COMMIT && BANKING.with(|b| b.get()) {
        return Ok(true);
    }
    if end_game_bank()? {
        super::trace::trace("end-bank");
        BANKING.with(|b| b.set(true));
        return Ok(true);
    }
    let here = me.position()?;
    let far = BANK_FAR_ALTAR_STEPS > 0
        && super::walk::altar_steps(here).is_some_and(|s| s > BANK_FAR_ALTAR_STEPS);
    let carry_min = super::gemcarry::prism_bank_min();
    let prism = carry_min.is_some()
        || (PRISM_BANK_MIN_GEMS > 0 && me.champion_name()? == cambc::ChampionType::Prism);
    let mut bank = gems
        >= if let Some(min) = carry_min {
            min
        } else if prism {
            PRISM_BANK_MIN_GEMS
        } else if far {
            BANK_FAR_MIN_GEMS
        } else {
            BANK_MIN_GEMS
        };
    for altar in open_altars()? {
        if distance(here, altar) <= BANK_NEAR_TILES && !prism {
            bank = true;
        }
    }
    BANKING.with(|b| b.set(bank));
    Ok(bank)
}

// Whether we know of an altar without an enemy lock on it within `tiles` of us.
pub fn unlocked_altar_within(tiles: i32) -> Result<bool> {
    let here = me()?.position()?;
    let enemy_locks = cambc::vision::altar_lock_tiles(Some(Team::Enemy))?;
    for altar in open_altars()? {
        if distance(here, altar) <= tiles
            && altar_allowed(altar, here)?
            && !enemy_locks.is_set(altar)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

// When we decide to go home to shop and heal (not the low-health recall), first bank the gems we
// carry, so we don't recall and then walk all the way to an altar from spawn.
// Only when an altar is within BANK_BEFORE_RECALL_TILES: a long walk before healing costs more.
pub const BANK_BEFORE_RECALL: bool = true;
pub const BANK_BEFORE_RECALL_TILES: i32 = 8;

// "Willingness to go back to base": recall to shop and heal once this reaches 1. It grows with
// unspent gold (in emblems' worth, 150 gold), missing health, and rounds since we were last on
// our spawn. Champions that can shop anywhere (Tavi) get no pull from gold. Weights tuned against
// old-bot over 180 games (writeup entry 12). Gold 1.5 -> 1.0 (home at a full emblem's 150 gold,
// not 100, at full health; 2026-10-21, 132 games each): 73-59 twice, gold +6%; 0.75 (200) 65-67.
pub const BASE_W_GOLD: f32 = 1.0;
pub const BASE_W_MISSING_HP: f32 = 0.5;
pub const BASE_W_ROUNDS_AWAY: f32 = 0.004; // per round away from spawn

pub fn base_willingness(shops_anywhere: bool, rounds_away: u32) -> Result<f32> {
    let me = me()?;
    let gold = if shops_anywhere {
        0.0
    } else {
        me.gold_count()? as f32 / 150.0
    };
    let missing_hp = 1.0 - me.health()? / me.max_health()?.max(1.0);
    Ok(BASE_W_GOLD * gold
        + BASE_W_MISSING_HP * missing_hp
        + BASE_W_ROUNDS_AWAY * rounds_away as f32)
}
