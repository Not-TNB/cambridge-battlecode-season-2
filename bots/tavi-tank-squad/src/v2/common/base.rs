// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
// Going back: banking gems at altars, the willingness to recall to shop and heal, and our
// spawn.
use super::me;
use super::movement::move_towards_closest;
use super::trace::checkpoint;
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

// Tribute at an altar in range, otherwise walk toward the nearest unlocked altar.
pub fn bank_gems() -> Result<()> {
    let me = me()?;
    // Union all altar approach tiles: the field chooses the nearest reachable altar (on our side,
    // see BANK_OWN_SIDE; any altar if none is).
    let here = me.position()?;
    let range = cambc::constants().submit_gems_range;
    let mut goals = Bitboard::new()?;
    let mut fallback = Bitboard::new()?;
    for altar in cambc::map::altars()?.positions()? {
        // A lock check is a host call costing ~2.25k (40k a turn over 00lanes' 18 altars), and we
        // only see locks near us anyway: check altars within LOCK_CHECK_TILES.
        if distance(here, altar) <= LOCK_CHECK_TILES && cambc::vision::has_altar_lock(altar)? {
            continue;
        }
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
    if goals.is_empty()? {
        goals = fallback;
    }
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

pub fn should_bank() -> Result<bool> {
    let me = me()?;
    let gems = me.gem_count()?;
    if gems == 0 {
        BANKING.with(|b| b.set(false));
        return Ok(false);
    }
    if BANK_COMMIT && BANKING.with(|b| b.get()) {
        return Ok(true);
    }
    let here = me.position()?;
    let far = BANK_FAR_ALTAR_STEPS > 0
        && super::walk::altar_steps(here).is_some_and(|s| s > BANK_FAR_ALTAR_STEPS);
    let mut bank = gems >= if far { BANK_FAR_MIN_GEMS } else { BANK_MIN_GEMS };
    for altar in cambc::map::altars()?.positions()? {
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
    for altar in cambc::map::altars()?.positions()? {
        if distance(here, altar) <= tiles
            && altar_allowed(altar, here)?
            && !cambc::vision::has_altar_lock(altar)?
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
// old-bot over 180 games (writeup entry 12).
pub const BASE_W_GOLD: f32 = 1.5;
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
