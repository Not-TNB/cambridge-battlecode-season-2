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

// Each altar we could ever tribute at, with the camp tiles (and groups) next to it that it depends
// on: none if a floor tile is next to it. Worked out once.
fn altar_access() -> Result<&'static [(Vec2, Vec<(Vec2, usize)>)]> {
    thread_local! {
        static ACCESS: std::cell::OnceCell<&'static [(Vec2, Vec<(Vec2, usize)>)]> =
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
    let access: &'static [(Vec2, Vec<(Vec2, usize)>)] = Vec::leak(access);
    ACCESS.with(|a| {
        let _ = a.set(access);
    });
    Ok(access)
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
            for champion in sight.allies.iter().chain(&sight.enemies) {
                taken |= champion.position().is_ok_and(|p| p == tile);
            }
            if !taken {
                return Ok(true);
            }
            continue;
        }
        if super::farm::respawn_ready(group).is_some_and(|ready| ready > round + walk + GATE_MARGIN_ROUNDS) {
            return Ok(true);
        }
    }
    Ok(false)
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
pub const BANK_NEAR_TILES: i32 = 6;
// Where the nearest altar is more than BANK_FAR_ALTAR_STEPS steps away on foot, bank only from
// BANK_FAR_MIN_GEMS: on spine (altars at the far ends of the map) banking every 40 gems made walks
// to the altar ~40% of all walking. 0: off.
pub const BANK_FAR_ALTAR_STEPS: i32 = 16;
pub const BANK_FAR_MIN_GEMS: u32 = 80;

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
    let left = cambc::constants().max_rounds.saturating_sub(cambc::round_number()?);
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
    let mut bank = gems >= if let Some(min) = super::gemcarry::prism_bank_min() {
        min
    } else if far {
        BANK_FAR_MIN_GEMS
    } else {
        BANK_MIN_GEMS
    };
    for altar in open_altars()? {
        if distance(here, altar) <= BANK_NEAR_TILES {
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
