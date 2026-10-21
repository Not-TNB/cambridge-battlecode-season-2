// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
// Chasing an enemy: picking the target, dashing at it, and following it after losing sight.
use super::duel::{fight_view, plan_duel};
use super::fight::FIGHT_ENEMY_MAX_DISTANCE;
use super::me;
use super::movement::{dash_to_best, move_towards_closest, ready_dash};
use super::trace::{checkpoint, trace};
use super::ward::ward_hidden_tile;
use super::world::{blocked_tiles, distance, forget_sight, tiles_around};
use cambc::{Bitboard, Champion, ChampionType, DistanceField, Result, Vec2};

// Dash while chasing. Against a ranged target only when dash (+ a walking step for melee) puts us
// in attack range this turn: until then we walk, which stops us just outside its range, ready to
// dash in. Against melee targets, dash whenever it gets us closer.
pub const DASH_CHASE: bool = true;

pub(super) fn dash_at_chase_target(enemies: &[Champion]) -> Result<bool> {
    if !DASH_CHASE {
        return Ok(false);
    }
    let Some(dash) = ready_dash()? else {
        return Ok(false);
    };
    let Some(target) = chase_target() else {
        return Ok(false);
    };
    let Some(enemy) = enemies
        .iter()
        .find(|e| e.champion_name().is_ok_and(|n| n == target.name))
    else {
        return Ok(false);
    };
    let me = me()?;
    let range = me.attack_range()?;
    let walk = if range <= 1 {
        me.base_stats()?.base_move_speed as i32
    } else {
        0
    };
    let blocked = blocked_tiles()?;
    // Walking distance from every tile to our attack range of the target.
    let mut goals = tiles_around(target.position, range)?;
    goals.intersect_assign(&blocked.complement()?)?;
    if goals.is_empty()? {
        return Ok(false);
    }
    let mut field = DistanceField::new()?;
    field.compute(&blocked, &goals, &mut Bitboard::new()?, MAX_CHASE_STEPS)?;
    let steps = |tile: Vec2| -> Result<f32> {
        let d = field.get(tile)?;
        Ok(if d == DistanceField::UNREACHABLE {
            f32::MAX
        } else {
            d as f32
        })
    };
    if enemy.attack_range()? > 1 {
        // Only commit if some landing tile leaves at most `walk` steps.
        let Some(info) = dash.info()? else {
            return Ok(false);
        };
        let mut reaches = false;
        for tile in blocked
            .reachable_tiles(me.position()?, info.as_dyn().range()? as u16)?
            .positions()?
        {
            if steps(tile)? <= walk as f32 {
                reaches = true;
                break;
            }
        }
        if !reaches {
            return Ok(false);
        }
    }
    dash_to_best(dash, &blocked, steps)
}

// Enemies within this many tiles are fought even when the duel model says we'd lose, as long as we
// survive the walk; this pulls us off a camp to fight. 0 turns it off.
pub const ENGAGE_RADIUS: i32 = 3;
pub const ENGAGE_MIN_SURVIVE_TURNS: u32 = 4;

// With more than one enemy around, a chase is only worth it if we'd still live this many turns
// after the kill, to get away from the rest: "we kill the target before dying" kept champions
// chasing into the enemy's arriving backup, and the close-enemy exception above doesn't apply.
// 0 turns it off. 3 seeds: 68/126 against 63/126 without it, and 8% fewer of our deaths.
pub const CHASE_ESCAPE_TURNS: u32 = 3;

// Enemies further than this many steps away are never chased; also bounds the pathfinding cost.
pub(super) const MAX_CHASE_STEPS: u16 = 12;

// After losing sight of the chase target, keep walking to where we last saw it for this many
// rounds. Without it, a path that bends away from the enemy drops it out of vision, we go back to
// farming, see it again and turn around: back and forth. 0 turns it off.
pub const CHASE_MEMORY_ROUNDS: u32 = 3;

// Chase leash for champions without a dash in their kit: they can't close a gap on an enemy that
// walks away (it's as fast as they are), and followed one down the whole map. They still chase an
// enemy off (otherwise it stands at range and pokes us), but only while it's within this many
// tiles of where the chase began; past that they let it go and get back to farming. 0: no leash.
// Off: a 6-tile leash lost 58/126 games against 71/126 without it (3 seeds). Our deaths were the
// same, but we killed 25% fewer enemies (781 against 1,030): the long chases end in kills.
pub const NO_DASH_CHASE_LEASH: i32 = 0;
// The anchor outlives the chase by this many rounds, so a chase taken up again from the leash's
// edge doesn't start a new leash there (and so on down the map).
const LEASH_MEMORY_ROUNDS: u32 = 10;

thread_local! {
    // Where we stood when the current chase began, and the round it's forgotten (u32::MAX while
    // the chase is on).
    static CHASE_ANCHOR: std::cell::Cell<Option<(Vec2, u32)>> = const { std::cell::Cell::new(None) };
}

fn current_anchor(round: u32) -> Option<Vec2> {
    CHASE_ANCHOR
        .with(|a| a.get())
        .filter(|&(_, until)| round < until)
        .map(|(anchor, _)| anchor)
}

fn leashed() -> Result<bool> {
    Ok(NO_DASH_CHASE_LEASH > 0 && !me()?.abilities().iter().any(|a| a.is_dash()))
}

// Whether `position` is past our leash (always false for champions with a dash).
fn past_leash(position: Vec2) -> Result<bool> {
    if !leashed()? {
        return Ok(false);
    }
    let anchor = match current_anchor(cambc::round_number()?) {
        Some(anchor) => anchor,
        None => me()?.position()?, // a new chase would begin here
    };
    Ok(distance(anchor, position) > NO_DASH_CHASE_LEASH)
}

// Place our ward where we lost the chase target when that tile is hidden (a bush or behind a wall).
pub const WARD_LOST_TARGET: bool = true;

// The enemy we are chasing and where and when we last saw it, remembered across turns so we don't
// flip between targets. Each champion runs as its own program, so this is per champion; one of
// each type per team.
#[derive(Clone, Copy)]
pub(super) struct ChaseTarget {
    name: ChampionType,
    pub(super) position: Vec2,
    round: u32,
}

thread_local! {
    static CHASE_TARGET: std::cell::Cell<Option<ChaseTarget>> = const { std::cell::Cell::new(None) };
}

pub(super) fn chase_target() -> Option<ChaseTarget> {
    CHASE_TARGET.with(|cell| cell.get())
}

fn set_chase_target(target: Option<(ChampionType, Vec2)>) -> Result<()> {
    let round = cambc::round_number()?;
    // A chase anchors the leash where it begins (unless an anchor is still remembered); ending
    // it starts the anchor's countdown.
    let anchor = current_anchor(round);
    match target {
        Some(_) => {
            let at = match anchor {
                Some(at) => at,
                None => me()?.position()?,
            };
            CHASE_ANCHOR.with(|a| a.set(Some((at, u32::MAX))));
        }
        None => {
            if let Some(at) = anchor {
                CHASE_ANCHOR.with(|a| a.set(Some((at, round + LEASH_MEMORY_ROUNDS))));
            }
        }
    }
    CHASE_TARGET.with(|cell| {
        cell.set(target.map(|(name, position)| ChaseTarget {
            name,
            position,
            round,
        }))
    });
    Ok(())
}

// Update where the chase target is while we can see it.
pub(super) fn refresh_chase_target(enemies: &[Champion]) -> Result<()> {
    let Some(target) = chase_target() else {
        return Ok(());
    };
    let here = me()?.position()?;
    for enemy in enemies {
        if enemy.champion_name()? == target.name {
            let position = enemy.position()?;
            if distance(here, position) >= FIGHT_ENEMY_MAX_DISTANCE || past_leash(position)? {
                trace("chase-off");
                return set_chase_target(None);
            }
            return set_chase_target(Some((target.name, position)));
        }
    }
    Ok(())
}

// The chase target is out of sight: walk to where we last saw it, warding that tile if it's
// hidden. Forget it once the memory runs out or we get there without finding it.
pub(super) fn follow_lost_target() -> Result<bool> {
    let Some(target) = chase_target() else {
        return Ok(false);
    };
    let round = cambc::round_number()?;
    if target.round == round {
        return Ok(false);
    } // still in sight: nothing lost
    let me = me()?;
    let here = me.position()?;
    if round.saturating_sub(target.round) > CHASE_MEMORY_ROUNDS
        || distance(here, target.position) <= 1
        || past_leash(target.position)?
    {
        set_chase_target(None)?;
        return Ok(false);
    }
    trace("follow-lost");
    if WARD_LOST_TARGET {
        ward_hidden_tile(target.position)?;
        forget_sight();
    }
    Ok(move_towards_closest(tiles_around(target.position, 1)?)?)
}

// Keep the current target while we still expect to win against it; otherwise pick the winnable
// enemy with the least health, then the one we can finish fastest (walking turns + turns to kill).
// Returns the next tile to step to, or None if no enemy is worth chasing (or we are already in
// range of the one that is).
pub(super) fn choose_chase_step(enemies: &[Champion]) -> Result<Option<Vec2>> {
    // Walking distances from our tile etc., shared by every enemy (and with losing_fight).
    let view = fight_view()?;
    checkpoint("chase-view");
    let (threats, allies, from_me) = (&view.threats, &view.allies, &view.from_me);
    let current = chase_target().map(|t| t.name);
    let mut current_visible = false;
    let mut best: Option<(ChampionType, Vec2, Option<Vec2>, u32, f32)> = None;
    for enemy in enemies {
        let Some(duel) = plan_duel(enemy, threats, allies, from_me)? else {
            continue;
        };
        checkpoint("chase-duel");
        // Close enemies are fought even when the fight looks lost, unless we'd die within
        // ENGAGE_MIN_SURVIVE_TURNS (other enemies joining make those fights deadly).
        let backup = CHASE_ESCAPE_TURNS > 0 && enemies.len() > 1;
        let close = !backup
            && distance(me()?.position()?, enemy.position()?) <= ENGAGE_RADIUS
            && duel.hp_after_walk > 0.0
            && duel.die_turns >= ENGAGE_MIN_SURVIVE_TURNS;
        let winnable = duel.winnable
            && (!backup || duel.die_turns >= duel.kill_turns.saturating_add(CHASE_ESCAPE_TURNS));
        if !winnable && !close {
            if backup && duel.winnable {
                trace("chase-backup");
            }
            continue;
        }
        // Past the leash: chased off far enough.
        if past_leash(enemy.position()?)? {
            continue;
        }
        if !super::collect::escort_allows_chase(enemy.position()?)? {
            continue;
        }
        let name = enemy.champion_name()?;
        if Some(name) == current {
            return Ok(duel.next_step);
        }
        // Weakest first (enemies come sorted by health), then quickest to finish.
        let total = duel.walk_turns + duel.kill_turns;
        let health = enemy.health().unwrap_or(f32::MAX);
        if best.is_none_or(|(_, _, _, t, h)| (health, total) < (h, t)) {
            best = Some((name, enemy.position()?, duel.next_step, total, health));
        }
    }
    checkpoint("chase-target-choice");
    for enemy in enemies {
        current_visible |= Some(enemy.champion_name()?) == current;
    }
    // Nothing worth chasing in sight: keep the memory of a target that went out of view.
    if best.is_none() && !current_visible {
        return Ok(None);
    }
    set_chase_target(best.map(|(name, position, _, _, _)| (name, position)))?;
    Ok(best.and_then(|(_, _, step, _, _)| step))
}
