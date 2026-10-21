// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
// Duel estimates: whether we beat an enemy 1v1 after walking to it, with allies and enemies
// joining, and the per-turn fight view they share.
use super::attack::is_damaging;
use super::chase::MAX_CHASE_STEPS;
use super::me;
use super::movement::WalkGrid;
use super::threats::Threats;
use super::trace::checkpoint;
use super::world::{blocked_tiles, distance, sight};
use cambc::{Bitboard, Champion, DistanceField, Result, Vec2};

// Simulated chase. Use a cheap Chebyshev estimate of approach time and straight-line damage while
// walking; the walk grid still verifies reachability and supplies the actual first chase step.
#[allow(dead_code)] // die_turns is kept for debugging and tuning
pub struct Duel {
    pub next_step: Option<Vec2>, // first tile on the path; None if already in range
    pub walk_turns: u32,
    pub hp_after_walk: f32,
    pub kill_turns: u32, // turns on the duel tile to kill the enemy (MAX: we die first or it drags on)
    pub die_turns: u32,  // turns on the duel tile until we die (MAX: we don't within the fight)
    pub winnable: bool,  // we kill the target before dying
}

// Visible moving enemies and allies may contribute to the duel estimate.
pub const DUEL_ENEMIES_JOIN: bool = true;
pub const DUEL_ALLIES_JOIN: bool = true;

// An allied champion in sight that could join a fight: basic attacks only (its abilities aren't
// readable from here).
pub struct Helper {
    origin: Vec2,
    range: i32,
    speed: i32,
    damage: f32,
}

pub fn ally_helpers() -> Result<Vec<Helper>> {
    let mut helpers = Vec::new();
    if !DUEL_ALLIES_JOIN {
        return Ok(helpers);
    }
    let me = me()?.champion_name()?;
    for ally in &sight()?.allies {
        if ally.champion_name()? == me || !ally.is_alive()? {
            continue;
        }
        helpers.push(Helper {
            origin: ally.position()?,
            range: ally.attack_range()?,
            speed: ally.base_stats()?.base_move_speed as i32,
            damage: ally.strength()?,
        });
    }
    Ok(helpers)
}

// `from_me` holds walking distances from our tile. None if no tile in range of the enemy is reachable.
pub fn plan_duel(
    enemy: &Champion,
    threats: &Threats,
    allies: &[Helper],
    from_me: &WalkGrid,
) -> Result<Option<Duel>> {
    checkpoint("duel-start");
    let me = me()?;
    let origin = me.position()?;
    let target = enemy.position()?;
    let range = me.attack_range()?;
    checkpoint("duel-inputs");

    let mut next_step = None;
    let mut duel_tile = origin;
    let mut walk_turns = 0;
    if distance(origin, target) > range {
        // Nearest reachable tile within our attack range of the enemy.
        let mut end: Option<(Vec2, u16)> = None;
        for dx in -range..=range {
            for dy in -range..=range {
                let tile = Vec2::new(target.x + dx, target.y + dy);
                let d = from_me.get(tile);
                if d != DistanceField::UNREACHABLE && end.is_none_or(|(_, best)| d < best) {
                    end = Some((tile, d));
                }
            }
        }
        let Some((end, _)) = end else { return Ok(None) };
        let Some(path) = from_me.path_to(end) else {
            return Ok(None);
        };
        next_step = path.first().copied();
        duel_tile = end;
        let approach_distance = distance(origin, end) as u32;
        let speed = me.base_stats()?.base_move_speed.max(1) as u32;
        walk_turns = approach_distance.div_ceil(speed);
    }
    checkpoint("duel-approach");

    // Approximate the route as a straight line toward the selected duel tile. Obstacles can make
    // this underestimate both walk time and incoming damage, but actual chasing still follows
    // the reachable path from `from_me`.
    let approach_distance = distance(origin, duel_tile) as u32;
    let speed = me.base_stats()?.base_move_speed.max(1) as u32;
    let direction = Vec2::new(
        (duel_tile.x - origin.x).signum(),
        (duel_tile.y - origin.y).signum(),
    );
    let mut walk_damage = 0.0;
    for turn in 1..walk_turns {
        let steps = (turn * speed).min(approach_distance) as i32;
        let tile = Vec2::new(
            origin.x + direction.x * steps,
            origin.y + direction.y * steps,
        );
        walk_damage += threats.damage_at_turn(tile, turn);
    }
    let hp_after_walk = me.health()? - walk_damage;
    checkpoint("duel-walk-damage");

    // Compare estimated time-to-kill and time-to-die directly instead of simulating every turn.
    let mut outgoing = turn_damage_against(enemy)?;
    checkpoint("duel-own-dps");
    for ally in allies {
        if distance(ally.origin, target) <= ally.range || ally.speed > 0 {
            outgoing += ally.damage;
        }
    }
    let mut incoming = 0.0;
    for threat in &threats.0 {
        let shielded = threat.shield.is_some_and(|s| distance(threat.origin, duel_tile) > s);
        if (distance(threat.origin, duel_tile) <= threat.range || threat.speed > 0) && !shielded {
            incoming += threat.damage;
        }
    }
    // An awake camp with the enemy as the closest champion in its range hits it for us
    // (threats::CAMPS_HIT_CLOSEST): fighting an enemy next to an elk, from further away than it.
    if super::threats::CAMPS_HIT_CLOSEST {
        let sight = sight()?;
        for camp in &sight.camps {
            if !camp.is_awake().unwrap_or(false) {
                continue;
            }
            let p = camp.spawn_position();
            let gap = distance(p, target);
            if gap > camp.attack_range()? || distance(p, duel_tile) <= gap {
                continue;
            }
            let mut closer = false;
            for ally in allies {
                closer |= distance(p, ally.origin) <= gap;
            }
            for other in &sight.enemies {
                if let Ok(q) = other.position() {
                    closer |= q != target && distance(p, q) < gap;
                }
            }
            if !closer {
                outgoing += camp.strength()?;
            }
        }
    }
    checkpoint("duel-total-dps");
    let Ok(target_hp) = enemy.health() else {
        return Ok(None);
    };
    let kill_turns = rounds_to_defeat(target_hp, outgoing);
    let die_turns = rounds_to_defeat(hp_after_walk, incoming);
    checkpoint("duel-time-comparison");
    Ok(Some(Duel {
        next_step,
        walk_turns,
        hp_after_walk,
        kill_turns,
        die_turns,
        winnable: kill_turns <= die_turns,
    }))
}

fn rounds_to_defeat(hp: f32, damage_per_turn: f32) -> u32 {
    if hp <= 0.0 {
        0
    } else if damage_per_turn <= 0.0 {
        u32::MAX
    } else {
        (hp / damage_per_turn).ceil() as u32
    }
}

// Estimate damage per turn as a basic attack plus one eighth of each damaging ability/reset.
fn turn_damage_against(enemy: &Champion) -> Result<f32> {
    let me = me()?;
    let strength = me.strength()?;
    let mut damage = strength;
    for ability in me.abilities() {
        if ability.info()?.is_none() {
            continue;
        }
        let per_turn = 1.0 / 8.0;
        if ability.is_attack_reset() {
            damage += strength * per_turn;
        } else if is_damaging(ability) {
            damage += ability
                .damage_amount_against(enemy)
                .or_else(|_| ability.damage_amount())?
                * per_turn;
        }
    }
    Ok(damage)
}

// What the fight decisions share within a turn: threats, allies, blocked tiles and walking distances
// from our tile. Building it costs ~25k fuel, and the losing-fight check, kiting and the chase
// decision each used to build their own, which left too little fuel to farm on turns where an
// enemy was in sight but not worth chasing (the champion then stood still). Cached per round and
// position, so it's rebuilt if we move.
pub struct FightView {
    pub threats: Threats,
    pub allies: Vec<Helper>,
    pub blocked: Bitboard,
    pub from_me: WalkGrid,
}

thread_local! {
    static FIGHT_VIEW: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<FightView>)>> = const { std::cell::RefCell::new(None) };
}

pub fn fight_view() -> Result<std::rc::Rc<FightView>> {
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(view) = FIGHT_VIEW.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, v)| v.clone())
    }) {
        checkpoint("fight-view-cache");
        return Ok(view);
    }
    checkpoint("fight-view-blocked-start");
    let blocked = blocked_tiles()?;
    checkpoint("fight-view-blocked-done");
    let threats = Threats::scan()?;
    checkpoint("fight-view-threats");
    let allies = ally_helpers()?;
    checkpoint("fight-view-allies");
    let from_me = WalkGrid::from(key.1, &blocked, MAX_CHASE_STEPS)?;
    checkpoint("fight-view-walkgrid");
    let view = std::rc::Rc::new(FightView {
        threats,
        allies,
        from_me,
        blocked,
    });
    FIGHT_VIEW.with(|c| *c.borrow_mut() = Some((key.0, key.1, view.clone())));
    Ok(view)
}

pub(super) fn cached_fight_view() -> Result<Option<std::rc::Rc<FightView>>> {
    let key = (cambc::round_number()?, me()?.position()?);
    Ok(FIGHT_VIEW.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|(round, position, _)| (*round, *position) == key)
            .map(|(_, _, view)| view.clone())
    }))
}
