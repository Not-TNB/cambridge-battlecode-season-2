// Staying alive: remembering where enemies were, recalling only when nothing can reach us during
// the channel, and retreating to safer ground from fights we'd lose.
use crate::common::{
    distance, enemy_threats_by_name, fightable_enemies, home_pull, me,
    move_towards_closest_with_pull, spawn_steps, strike, trace, Threat, Threats,
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
// Within this many steps of our spawn, walk home instead of recalling: walking is no slower than
// the 5-turn recall, and nothing can interrupt it (in bot-v2 32% of recalls started that close).
// 0: always recall. Off in the squad (see common::RETREAT_HOME_PULL for the test); bot-v2 uses 5.
pub const WALK_HOME_STEPS: i32 = 0;

// Whether we're close enough to our spawn to walk home (WALK_HOME_STEPS); never on it.
pub fn walk_home_instead() -> Result<bool> {
    let here = me()?.position()?;
    Ok(WALK_HOME_STEPS > 0 && spawn_steps(here).is_some_and(|s| s > 0 && s <= WALK_HOME_STEPS))
}

// Step towards our spawn region. Returns whether we moved.
pub fn walk_home() -> Result<bool> {
    let spawn = cambc::map::spawn_region(cambc::Team::Ally)?.clone();
    let moved = move_towards_closest_with_pull(spawn, |_| 0.0, true)?;
    if moved {
        trace("walk-home");
    }
    Ok(moved)
}

// The low-health recall. With an enemy champion in sight: below LOW_HP_RECALL of max health. Without:
// once the camps that can hit us (awake, alive, in their range of us) would kill us within
// LOW_HP_CAMP_HITS more rounds of hits, or below LOW_HP_IDLE_RECALL with none of them around.
// Going home at 35% mid-fight threw away fights a wolf (20 a hit) or an elk (60) couldn't win
// (bot-v2: 69/126 against 67/126 for 35% everywhere, and more gold early).
pub const LOW_HP_RECALL: f32 = 0.35;
pub const LOW_HP_CAMP_HITS: f32 = 3.0;
pub const LOW_HP_IDLE_RECALL: f32 = 0.35;

pub fn low_hp_recall() -> Result<bool> {
    let me = me()?;
    let (hp, max) = (me.health()?, me.max_health()?);
    if hp > max * LOW_HP_RECALL {
        return Ok(false);
    }
    let sight = crate::common::sight()?;
    if !sight.enemies.is_empty() {
        return Ok(true);
    }
    let here = me.position()?;
    let mut per_round = 0.0;
    for camp in &sight.camps {
        if camp.is_awake().unwrap_or(true)
            && distance(camp.spawn_position(), here) <= camp.attack_range()?
        {
            per_round += camp.strength()?;
        }
    }
    Ok(if per_round > 0.0 {
        hp <= LOW_HP_CAMP_HITS * per_round
    } else {
        hp <= max * LOW_HP_IDLE_RECALL
    })
}

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
    if walk_home_instead()? && walk_home()? {
        return Ok(());
    }
    let threats = known_threats()?;
    if safe_for(&threats, me.position()?, RECALL_SAFE_TURNS) {
        trace("recall");
        return me.recall();
    }
    // Walk to the nearest tile safe for the whole channel, leaning towards home (RETREAT_HOME_PULL);
    // our spawn counts as safe (enemies can't walk onto it, and it heals).
    let moved =
        move_towards_closest_with_pull(recall_safe_tiles_from(&threats)?, toward_home, true)?;
    if moved && !safe_for(&threats, me.position()?, RECALL_SAFE_TURNS) {
        trace("retreat");
        return Ok(()); // keep walking next turn
    }
    if !moved && dodge_or_fight(&threats)? {
        return Ok(());
    }
    trace(if moved { "recall" } else { "recall-cornered" });
    me.recall()
}

// Cornered (no way to a tile safe for the whole channel) with something able to hit us here: any
// damage cancels a recall, so starting one in an awake elk's range only got us killed there
// (bot-v2: 10 of 11 camp deaths that followed a recall, elk maps). Step to the tile that takes the
// least damage instead, or hit back if none takes less; the recall waits until it's quiet.
// Returns whether we did (false: nothing can hit us here). Off: recall where we're cornered.
pub const DODGE_INSTEAD_OF_RECALL: bool = true;

pub fn dodge_or_fight(threats: &Threats) -> Result<bool> {
    let me = me()?;
    let here = me.position()?;
    let danger = threats.damage_at(here);
    if !DODGE_INSTEAD_OF_RECALL || danger <= 0.0 {
        return Ok(false);
    }
    if me.can_move()? {
        let key = |tile: Vec2| (threats.damage_at(tile), toward_home(tile));
        let mut best = (key(here), here);
        for tile in crate::common::blocked_tiles()?
            .reachable_tiles(here, me.base_stats()?.base_move_speed as u16)?
            .positions()?
        {
            let k = key(tile);
            if k.partial_cmp(&best.0) == Some(std::cmp::Ordering::Less) {
                best = (k, tile);
            }
        }
        if best.1 != here && best.0 .0 < danger && crate::common::step_to(best.1) {
            trace("recall-dodge");
            return Ok(true);
        }
    }
    trace("recall-cornered-fight");
    strike(&fightable_enemies()?, false)?;
    crate::common::attack_camp_in_range()?;
    Ok(true)
}

// The same for Tavi waiting at its recall spot: dodge or fight if something can hit us here.
pub fn dodge_if_in_danger() -> Result<bool> {
    dodge_or_fight(&known_threats()?)
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

// Tiles to walk to for a recall: safe from every known threat for the whole channel, plus our spawn.
fn recall_safe_tiles_from(threats: &Threats) -> Result<Bitboard> {
    let mut safe = safe_tiles_for(threats, RECALL_SAFE_TURNS)?;
    safe.union_assign(cambc::map::spawn_region(cambc::Team::Ally)?)?;
    Ok(safe)
}

// None if our tile is safe to recall on; otherwise the tiles to walk to for one.
pub fn recall_spot() -> Result<Option<Bitboard>> {
    let threats = known_threats()?;
    if safe_for(&threats, me()?.position()?, RECALL_SAFE_TURNS) {
        return Ok(None);
    }
    Ok(Some(recall_safe_tiles_from(&threats)?))
}

// The pull towards home for a step to `tile` (common::home_pull).
pub fn toward_home(tile: Vec2) -> f32 {
    home_pull(tile).unwrap_or(0.0)
}

pub fn retreat_from_fight() -> Result<bool> {
    if !RETREAT_FROM_LOSING {
        return Ok(false);
    }
    let threats = known_threats()?;
    if safe_for(&threats, me()?.position()?, RETREAT_SAFE_TURNS) {
        return Ok(false);
    }
    let mut safe = safe_tiles_for(&threats, RETREAT_SAFE_TURNS)?;
    safe.union_assign(cambc::map::spawn_region(cambc::Team::Ally)?)?;
    let moved = move_towards_closest_with_pull(safe, toward_home, true)?;
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
