// Staying alive: remembering where enemies were, recalling only when nothing can reach us during
// the channel, and retreating to safer ground from fights we'd lose.
use crate::common::{
    ally_spawn_centre, distance, enemy_threats_by_name, fightable_enemies, me,
    move_towards_closest_with_pull, strike, trace, Threat, Threats, RETREAT_HOME_PULL,
};
use cambc::{Bitboard, ChampionType, Result, Vec2};

// An enemy that went out of sight still counts as a threat for this many rounds, from where we
// last saw it, with its reach growing by a turn of walking per round since.
pub const SIGHTING_ROUNDS: u32 = 5;

struct Sighting {
    name: ChampionType,
    round: u32,
    threats: Vec<Threat>, // as scanned when last seen
}

thread_local! {
    static SIGHTINGS: std::cell::RefCell<Vec<Sighting>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Record every enemy in sight. Call once per acting turn.
pub fn remember_enemies() -> Result<()> {
    let round = cambc::round_number()?;
    let seen: Vec<Sighting> = enemy_threats_by_name()?
        .iter()
        .map(|(name, threats)| Sighting {
            name: *name,
            round,
            threats: threats.clone(),
        })
        .collect();
    SIGHTINGS.with(|s| {
        let mut s = s.borrow_mut();
        s.retain(|old| {
            round.saturating_sub(old.round) <= SIGHTING_ROUNDS
                && !seen.iter().any(|new| new.name == old.name)
        });
        s.extend(seen);
    });
    Ok(())
}

// Everything we can see that could hurt us, plus enemies seen in the last SIGHTING_ROUNDS rounds
// that are now out of sight.
pub fn known_threats() -> Result<Threats> {
    let mut threats = Threats::scan()?;
    let round = cambc::round_number()?;
    SIGHTINGS.with(|s| {
        for old in s.borrow().iter().filter(|old| old.round < round) {
            let age = (round - old.round) as i32;
            for t in &old.threats {
                threats.0.push(Threat {
                    origin: t.origin,
                    range: t.range + t.speed * age,
                    damage: t.damage,
                    speed: t.speed,
                });
            }
        }
    });
    Ok(threats)
}

// No known threat can reach `tile` during the next `turns` turns.
fn safe_for(threats: &Threats, tile: Vec2, turns: u32) -> bool {
    (0..turns.max(1)).all(|t| threats.damage_at_turn(tile, t) <= 0.0)
}

// Tiles no known threat can reach within `turns` turns.
fn safe_tiles_for(threats: &Threats, turns: u32) -> Result<Bitboard> {
    let mut danger = Bitboard::new()?;
    for t in threats.0.iter().filter(|t| t.damage > 0.0) {
        danger.draw_chebyshev(
            t.origin,
            (t.range + t.speed * (turns.max(1) as i32 - 1)) as f32,
        )?;
    }
    danger.complement()
}

// A recall channels for five turns and any damage interrupts it, so only start one where no known
// threat (enemies just out of sight included) can reach us for that long. 1 = the old rule: only
// this turn's damage counts.
pub const RECALL_SAFE_TURNS: u32 = 5;

fn strike_while_retreating() -> Result<()> {
    let me = me()?;
    if me.attack_range()? <= 1 && me.can_attack()? {
        let enemies = fightable_enemies()?;
        strike(&enemies, true)?;
    }
    Ok(())
}

// Recall where it's safe for the whole channel; otherwise walk (dashing, leaning towards base)
// towards tiles that are, and recall on arriving. If we can't move at all, recall anyway.
pub fn retreat_and_recall() -> Result<()> {
    let me = me()?;
    strike_while_retreating()?;
    let threats = known_threats()?;
    if safe_for(&threats, me.position()?, RECALL_SAFE_TURNS) {
        trace("recall");
        return me.recall();
    }
    let home = ally_spawn_centre()?;
    let toward_home = |tile: Vec2| RETREAT_HOME_PULL * distance(tile, home) as f32;
    let moved = move_towards_closest_with_pull(
        safe_tiles_for(&threats, RECALL_SAFE_TURNS)?,
        toward_home,
        true,
    )?;
    if moved && !safe_for(&threats, me.position()?, RECALL_SAFE_TURNS) {
        trace("retreat");
        return Ok(()); // keep walking next turn
    }
    trace(if moved { "recall" } else { "recall-cornered" });
    me.recall()
}

// Taken literally, "don't recall when no enemy is in vision": skip the go-home recall (gold and
// health pull, bot.rs) while no enemy is in sight. The low-health recall is unaffected.
pub const GO_HOME_NEEDS_ENEMY: bool = false;

// From a fight we'd lose, walk (dashing, leaning towards base) to tiles no known threat reaches
// within RETREAT_SAFE_TURNS turns, instead of holding just outside the enemies' reach
// (common::keep_distance, the fallback when this can't move). Returns whether we moved.
pub const RETREAT_FROM_LOSING: bool = true;
const RETREAT_SAFE_TURNS: u32 = 2;

pub fn recall_safe() -> Result<bool> {
    Ok(safe_for(
        &known_threats()?,
        me()?.position()?,
        RECALL_SAFE_TURNS,
    ))
}

pub fn recall_safe_tiles() -> Result<Bitboard> {
    safe_tiles_for(&known_threats()?, RECALL_SAFE_TURNS)
}

pub fn retreat_from_fight() -> Result<bool> {
    if !RETREAT_FROM_LOSING {
        return Ok(false);
    }
    let threats = known_threats()?;
    if safe_for(&threats, me()?.position()?, RETREAT_SAFE_TURNS) {
        return Ok(false);
    }
    let home = ally_spawn_centre()?;
    let toward_home = |tile: Vec2| RETREAT_HOME_PULL * distance(tile, home) as f32;
    let moved = move_towards_closest_with_pull(
        safe_tiles_for(&threats, RETREAT_SAFE_TURNS)?,
        toward_home,
        true,
    )?;
    if moved {
        trace("retreat-fight");
    }
    Ok(moved)
}

// An enemy in sight (other than one standing on its own spawn) could hit us within this many turns.
pub const IMMINENT_TURNS: i32 = 2;

pub fn imminent_danger() -> Result<bool> {
    let here = me()?.position()?;
    for enemy in crate::common::fightable_enemies()? {
        let reach =
            enemy.attack_range()? + enemy.base_stats()?.base_move_speed as i32 * IMMINENT_TURNS;
        if distance(enemy.position()?, here) <= reach {
            return Ok(true);
        }
    }
    Ok(false)
}
