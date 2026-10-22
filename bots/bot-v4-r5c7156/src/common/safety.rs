// Staying alive: remembering where enemies were, recalling only when nothing can reach us during
// the channel, and retreating to safer ground from fights we'd lose.
use super::attack::strike;
use super::base::home_pull;
use super::fight::fightable_enemies;
use super::me;
use super::movement::move_towards_closest_with_pull;
use super::threats::{enemy_threats_by_name, Threat, Threats};
use super::trace::trace;
use super::world::distance;
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
    // The last round we saw an enemy champion (quiet_for).
    static LAST_ENEMY_SEEN: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

// Whether no enemy champion is in sight now nor was for the last `rounds` rounds.
pub fn quiet_for(rounds: u32) -> Result<bool> {
    let round = cambc::round_number()?;
    if !super::world::sight()?.enemies.is_empty() {
        LAST_ENEMY_SEEN.with(|l| l.set(Some(round)));
        return Ok(false);
    }
    Ok(LAST_ENEMY_SEEN.with(|l| l.get()).is_none_or(|seen| round >= seen + rounds))
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
    if !seen.is_empty() {
        LAST_ENEMY_SEEN.with(|l| l.set(Some(round)));
    }
    SIGHTINGS.with(|s| {
        let mut s = s.borrow_mut();
        s.retain(|old| {
            round.saturating_sub(old.round) <= SIGHTING_ROUNDS
                && !seen.iter().any(|new| new.name == old.name)
        });
        s.extend(seen);
    });
    // Awake camps close enough to matter, for CAMP_MEMORY_ROUNDS.
    if CAMP_MEMORY_ROUNDS > 0 {
        let here = me()?.position()?;
        let mut awake = Vec::new();
        for camp in &super::world::sight()?.camps {
            let at = camp.spawn_position();
            if distance(at, here) <= CAMP_NOTE_TILES && camp.is_awake().unwrap_or(false) {
                awake.push(Threat {
                    origin: at,
                    range: camp.attack_range()?,
                    damage: camp.strength()?,
                    speed: 0,
                    shield: None,
                });
            }
        }
        CAMPS_SEEN.with(|seen| {
            let mut seen = seen.borrow_mut();
            seen.retain(|(t, _)| !awake.iter().any(|a| a.origin == t.origin));
            seen.extend(awake.into_iter().map(|t| (t, round)));
        });
    }
    Ok(())
}

const CAMP_NOTE_TILES: i32 = 7;

// Everything we can see that could hurt us, plus enemies seen in the last SIGHTING_ROUNDS rounds
// that are now out of sight.
pub fn known_threats() -> Result<Threats> {
    let mut threats = Threats::scan()?;
    // Recalls and retreats don't count on another champion drawing a camp's hits: they may walk off
    // mid-channel (CAMPS_HIT_CLOSEST is for fights).
    for t in threats.0.iter_mut() {
        t.shield = None;
    }
    let round = cambc::round_number()?;
    remember_camps(&mut threats, round)?;
    SIGHTINGS.with(|s| {
        for old in s.borrow().iter().filter(|old| old.round < round) {
            let age = (round - old.round) as i32;
            for t in &old.threats {
                threats.0.push(Threat {
                    origin: t.origin,
                    range: t.range + t.speed * age,
                    damage: t.damage,
                    speed: t.speed,
                    shield: None,
                });
            }
        }
    });
    Ok(threats)
}

// Awake camps seen in the last CAMP_MEMORY_ROUNDS rounds stay threats while their tile is out of
// sight. Camps see (and hit) through bushes but we don't see into them: a low-health champion
// next to a bush with an elk in it saw no camp, stepped into the bush to recall (BUSH_RECALL_TILES),
// saw the elk from inside and stepped back out, and so on until the elk killed it. 77% of our
// deaths to camps were on the low-health retreat. 0: off.
// 2026-10-21, 140 games: 64 against off, and no fewer deaths to camps (3.5 a game against 3.4):
// the bush back-and-forth wasn't what killed them. Off.
pub const CAMP_MEMORY_ROUNDS: u32 = 0;

thread_local! {
    static CAMPS_SEEN: std::cell::RefCell<Vec<(Threat, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn remember_camps(threats: &mut Threats, round: u32) -> Result<()> {
    if CAMP_MEMORY_ROUNDS == 0 {
        return Ok(());
    }
    let visible = super::world::sight()?.visible.clone();
    CAMPS_SEEN.with(|seen| -> Result<()> {
        let mut seen = seen.borrow_mut();
        // A camp in sight now is scanned afresh (dead or asleep: no threat).
        let mut kept = Vec::new();
        for (t, r) in seen.drain(..) {
            if round <= r + CAMP_MEMORY_ROUNDS && !visible.is_set(t.origin)? {
                kept.push((t, r));
            }
        }
        *seen = kept;
        for (t, _) in seen.iter() {
            threats.0.push(t.clone());
        }
        // Camps are the threats that don't walk.
        for t in threats.0.iter().filter(|t| t.speed == 0) {
            if !seen.iter().any(|(s, _)| s.origin == t.origin) {
                seen.push((t.clone(), round));
            }
        }
        Ok(())
    })
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
// Fix 1: within this many steps of our spawn, walk home instead of recalling. 0: always recall.
pub const WALK_HOME_STEPS: i32 = 5;

fn strike_while_retreating() -> Result<()> {
    let me = me()?;
    if me.attack_range()? <= 1 && me.can_attack()? {
        let enemies = fightable_enemies()?;
        strike(&enemies, true)?;
    }
    Ok(())
}

// The low-health recall. With an enemy champion in sight: below LOW_HP_RECALL of max health. Without:
// once the camps that can hit us (awake, alive, in their range of us) would kill us within
// LOW_HP_CAMP_HITS more rounds of hits, or below LOW_HP_IDLE_RECALL with none of them around.
// Going home at 35% mid-fight threw away fights a wolf (20 a hit) or an elk (60) couldn't win.
// Staying out at 20% after the fight (3 seeds, 126 games): 65 wins, 13% more of our deaths and 20%
// more gems lost; recalling at 35% once no camp hits us: 69 wins, gold +56 at round 300 (the old
// 35% everywhere: 67, -32).
pub const LOW_HP_RECALL: f32 = 0.35;
pub const LOW_HP_CAMP_HITS: f32 = 3.0;
pub const LOW_HP_IDLE_RECALL: f32 = 0.35;
// Leaving takes hits too: the camps keep hitting while we walk out of their range (an elk reaches 4
// tiles, so a melee champion next to it takes 4 more), and 12.5% of our deaths were to camps on
// the way home, carrying the most gems. Recall once we're down to the hits it takes to step out of
// the furthest-reaching camp's range plus ESCAPE_EXTRA_HITS, when that's more than LOW_HP_CAMP_HITS.
// Off: LOW_HP_CAMP_HITS only.
pub const ESCAPE_AWARE_RECALL: bool = true;
const ESCAPE_EXTRA_HITS: f32 = 1.0;

// The escape rule above at any health, not only below LOW_HP_RECALL: against two elk (120 a round)
// or a monolith (200) walking out of range can cost more than 35% of our health, and we only began
// the retreat at 22% on average, so 706 of 878 deaths to camps (210 traced games) came on the
// retreat, the camps' hits alone doing it. Only with no enemy champion in sight (fights keep the
// old rule). Off: below LOW_HP_RECALL only.
// 2026-10-21, 140 games: 71 against off; deaths to camps 2.6 a game against 3.2 (-19%), gems
// collected -2% (fights left earlier).
pub const ESCAPE_ANY_HP: bool = true;
// No camp reaches further than this (monolith 5); camps further away aren't queried.
const MAX_CAMP_RANGE: i32 = 5;

pub fn low_hp_recall() -> Result<bool> {
    let me = me()?;
    let (hp, max) = (me.health()?, me.max_health()?);
    let low = hp < max * LOW_HP_RECALL;
    if !low && !ESCAPE_ANY_HP {
        return Ok(false);
    }
    let sight = super::world::sight()?;
    if !sight.enemies.is_empty() {
        return Ok(low);
    }
    let here = me.position()?;
    let mut per_round = 0.0;
    let mut escape_steps = 0;
    for camp in &sight.camps {
        let gap = distance(camp.spawn_position(), here);
        if gap > MAX_CAMP_RANGE {
            continue;
        }
        let range = camp.attack_range()?;
        if camp.is_awake().unwrap_or(true) && gap <= range {
            per_round += camp.strength()?;
            escape_steps = escape_steps.max(range + 1 - gap);
        }
    }
    let hits = if ESCAPE_AWARE_RECALL {
        LOW_HP_CAMP_HITS.max(escape_steps as f32 + ESCAPE_EXTRA_HITS)
    } else {
        LOW_HP_CAMP_HITS
    };
    if !low {
        return Ok(per_round > 0.0 && hp <= (escape_steps as f32 + ESCAPE_EXTRA_HITS) * per_round);
    }
    Ok(if per_round > 0.0 {
        hp <= hits * per_round
    } else {
        hp < max * LOW_HP_IDLE_RECALL
    })
}

// Leaving camps' range comes first on a retreat: the walk to tiles safe for the whole recall
// (leaning home) often ran along a camp's range instead of out of it (ranges are Chebyshev, so a
// sideways step stays as deep): of the steps on retreats that ended in a death to a camp, 34% got
// further out of range, 42% stayed as deep and 20% went deeper. While awake camps hit us and no
// enemy champion is in sight, dash (if ready) and step to the tile with the least exposure (each
// covering camp's strength times the steps still in its range), leaning home on ties. Off: walk to
// the safe tiles directly.
// 2026-10-21, 140 games: 67 against off, deaths to camps -5% only (3.05 against 3.21). Off.
pub const CAMP_ESCAPE_FIRST: bool = false;

fn escape_camps() -> Result<bool> {
    let me = me()?;
    let sight = super::world::sight()?;
    if !CAMP_ESCAPE_FIRST || !me.can_move()? || !sight.enemies.is_empty() {
        return Ok(false);
    }
    let here = me.position()?;
    let mut camps = Vec::new();
    for camp in &sight.camps {
        // Within a range and two steps of us: any we could step into.
        if distance(camp.spawn_position(), here) <= MAX_CAMP_RANGE + 2 && camp.is_awake().unwrap_or(false) {
            camps.push((camp.spawn_position(), camp.attack_range()?, camp.strength()?));
        }
    }
    let exposure = |tile: Vec2| -> f32 {
        camps
            .iter()
            .map(|&(c, r, s)| s * (r + 1 - distance(c, tile)).max(0) as f32)
            .sum()
    };
    let now = exposure(here);
    if now <= 0.0 {
        return Ok(false);
    }
    let pull = |tile: Vec2| home_pull(tile).unwrap_or(0.0) * 0.01;
    let blocked = super::world::blocked_tiles()?;
    let mut acted = false;
    if let Some(dash) = super::movement::ready_dash()? {
        if super::movement::dash_to_best(dash, &blocked, |t| Ok(exposure(t) + pull(t)))? {
            trace("retreat-camp-dash");
            acted = true;
        }
    }
    if !me.can_move()? {
        return Ok(acted);
    }
    let from = me.position()?;
    let blocked = super::world::blocked_tiles()?;
    let mut best = (exposure(from) + pull(from), from);
    for tile in blocked
        .reachable_tiles(from, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        let k = exposure(tile) + pull(tile);
        if k < best.0 {
            best = (k, tile);
        }
    }
    if best.1 != from && exposure(best.1) < exposure(from) && super::movement::step_to(best.1) {
        trace("retreat-camp-step");
        acted = true;
    }
    Ok(acted)
}

// Recall where it's safe for the whole channel; otherwise walk (dashing, leaning towards base)
// towards tiles that are, and recall on arriving. If we can't move at all, recall anyway.
// Recalls broken by something we couldn't see: from outside a bush you can't see into it, so an
// enemy waiting in one shoots a recalling champion it sees and we don't. The tile is remembered
// for INTERRUPT_MEMORY_ROUNDS rounds, and tiles within INTERRUPT_RADIUS of it don't count as safe
// to recall on. Off: forgotten.
pub const RECALL_INTERRUPT_MEMORY: bool = true;
const INTERRUPT_MEMORY_ROUNDS: u32 = 20;
const INTERRUPT_RADIUS: i32 = 4;
// Recall in a bush: enemies outside one can't see into it and attacks need a visible target, so
// only an enemy in the same bush (which we'd see) or a camp can break the channel. With a bush
// tile safe for the whole channel within BUSH_RECALL_TILES (and us not in a bush), we step into
// it first. 0: off.
pub const BUSH_RECALL_TILES: i32 = 2;

thread_local! {
    static INTERRUPTED: std::cell::RefCell<Vec<(Vec2, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

// bot.rs: a recall we started ended without taking us home and without us cancelling it.
pub fn recall_interrupted(at: Vec2) -> Result<()> {
    if !RECALL_INTERRUPT_MEMORY {
        return Ok(());
    }
    let round = cambc::round_number()?;
    INTERRUPTED.with(|i| {
        let mut i = i.borrow_mut();
        i.retain(|&(_, r)| round < r + INTERRUPT_MEMORY_ROUNDS);
        i.push((at, round));
    });
    Ok(())
}

// Tiles near a recent interrupted recall (RECALL_INTERRUPT_MEMORY), if any.
fn interrupt_zone() -> Result<Option<Bitboard>> {
    let round = cambc::round_number()?;
    let recent: Vec<Vec2> = INTERRUPTED.with(|i| {
        i.borrow()
            .iter()
            .filter(|&&(_, r)| round < r + INTERRUPT_MEMORY_ROUNDS)
            .map(|&(p, _)| p)
            .collect()
    });
    if recent.is_empty() {
        return Ok(None);
    }
    let mut zone = Bitboard::new()?;
    for p in recent {
        zone.draw_chebyshev(p, INTERRUPT_RADIUS as f32)?;
    }
    Ok(Some(zone))
}

pub fn retreat_and_recall() -> Result<()> {
    let me = me()?;
    strike_while_retreating()?;
    super::trace::checkpoint("recall-strike");
    if escape_camps()? {
        return Ok(());
    }
    // Fix 1: close to base, walking there is no slower than the 5-turn recall, and nothing can
    // interrupt it (32% of recalls started within 5 steps of base).
    if WALK_HOME_STEPS > 0
        && super::walk::spawn_steps(me.position()?).is_some_and(|s| s <= WALK_HOME_STEPS)
    {
        let spawn = cambc::map::spawn_region(cambc::Team::Ally)?.clone();
        if move_towards_closest_with_pull(spawn, |_| 0.0, true)? {
            trace("walk-home");
            return Ok(());
        }
    }
    let threats = known_threats()?;
    super::trace::checkpoint("recall-threats");
    let here = me.position()?;
    let zone = interrupt_zone()?;
    let in_zone = |tile: Vec2| zone.as_ref().is_some_and(|z| z.is_set(tile).unwrap_or(false));
    // A bush close by, safe for the channel and away from an interrupted recall: recall there.
    if BUSH_RECALL_TILES > 0 && !cambc::map::bushes()?.is_set(here)? && me.can_move()? {
        let mut bush = cambc::map::bushes()?.clone();
        bush.intersect_assign(&super::world::tiles_around(here, BUSH_RECALL_TILES)?)?;
        if !bush.is_empty()? {
            bush.intersect_assign(&safe_tiles_for(&threats, RECALL_SAFE_TURNS)?)?;
            if let Some(z) = zone.as_ref() {
                bush.intersect_assign(&z.complement()?)?;
            }
            if !bush.is_empty()? && move_towards_closest_with_pull(bush, |_| 0.0, true)? {
                trace("recall-to-bush");
                return Ok(());
            }
        }
    }
    if safe_for(&threats, here, RECALL_SAFE_TURNS) && !in_zone(here) {
        trace("recall");
        return me.recall();
    }
    // Walk to the nearest tile safe for the whole channel, leaning towards home (RETREAT_HOME_PULL);
    // our spawn counts as safe (enemies can't walk onto it, and it heals). Not near an
    // interrupted recall (RECALL_INTERRUPT_MEMORY).
    let toward_home = |tile: Vec2| home_pull(tile).unwrap_or(0.0);
    let mut safe = safe_tiles_for(&threats, RECALL_SAFE_TURNS)?;
    if let Some(z) = zone.as_ref() {
        safe.intersect_assign(&z.complement()?)?;
    }
    safe.union_assign(cambc::map::spawn_region(cambc::Team::Ally)?)?;
    super::trace::checkpoint("recall-safe-tiles");
    let moved = move_towards_closest_with_pull(safe.clone(), toward_home, true)?;
    // The pull towards home is worth more than a step towards safety (RETREAT_HOME_PULL), so where
    // the safe tiles lie away from home and the way home runs past the danger, no step scored better
    // than standing still, and we stood in an elk's range until it killed us (19% of our deaths,
    // carrying the most gems). Then take the shortest way to safety. Off: stand still.
    let moved = moved
        || (RETREAT_UNSTICK && super::me()?.can_move()? && move_towards_closest_with_pull(safe, |_| 0.0, true)?);
    if moved && (!safe_for(&threats, me.position()?, RECALL_SAFE_TURNS) || in_zone(me.position()?)) {
        trace("retreat");
        return Ok(()); // keep walking next turn
    }
    // Cornered (no way to a tile safe for the whole channel) with something able to hit us here:
    // any damage cancels a recall, so starting one in an awake elk's range only got us killed
    // there (10 of 11 camp deaths that followed a recall, elk maps). Step to the tile that takes
    // the least damage instead, or hit back if none takes less, and recall once it's quiet.
    if !moved && DODGE_INSTEAD_OF_RECALL && threats.damage_at(me.position()?) > 0.0 {
        if least_damage_step(&threats)? {
            trace("recall-dodge");
        } else {
            trace("recall-cornered-fight");
            strike(&fightable_enemies()?, false)?;
            super::camps::attack_camp_in_range()?;
        }
        return Ok(());
    }
    trace(if moved { "recall" } else { "recall-cornered" });
    me.recall()
}

pub const RETREAT_UNSTICK: bool = true;

// Off: recall where we're cornered whatever can hit us (the old rule).
pub const DODGE_INSTEAD_OF_RECALL: bool = true;

// Step to the reachable tile where `threats` deal the least damage this turn (ties: closer to home),
// if that's less than where we stand. Returns whether we moved.
fn least_damage_step(threats: &Threats) -> Result<bool> {
    let me = me()?;
    if !me.can_move()? {
        return Ok(false);
    }
    let here = me.position()?;
    let key = |tile: Vec2| (threats.damage_at(tile), home_pull(tile).unwrap_or(0.0));
    let mut best = (key(here), here);
    for tile in super::world::blocked_tiles()?
        .reachable_tiles(here, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        let k = key(tile);
        if k.partial_cmp(&best.0) == Some(std::cmp::Ordering::Less) {
            best = (k, tile);
        }
    }
    if best.1 == here || best.0 .0 >= threats.damage_at(here) {
        return Ok(false);
    }
    Ok(super::movement::step_to(best.1))
}

// From a fight we'd lose, walk (dashing, leaning towards base) to tiles no known threat reaches
// within RETREAT_SAFE_TURNS turns, instead of holding just outside the enemies' reach
// (fight::keep_distance, the fallback when this can't move). Returns whether we moved.
pub const RETREAT_FROM_LOSING: bool = true;
const RETREAT_SAFE_TURNS: u32 = 2;

pub fn retreat_from_fight() -> Result<bool> {
    if !RETREAT_FROM_LOSING {
        return Ok(false);
    }
    let threats = known_threats()?;
    if safe_for(&threats, me()?.position()?, RETREAT_SAFE_TURNS) {
        return Ok(false);
    }
    let toward_home = |tile: Vec2| home_pull(tile).unwrap_or(0.0);
    let mut safe = safe_tiles_for(&threats, RETREAT_SAFE_TURNS)?;
    safe.union_assign(cambc::map::spawn_region(cambc::Team::Ally)?)?;
    let moved = move_towards_closest_with_pull(safe.clone(), toward_home, true)?;
    // The pull towards home is worth more than a step towards safety (RETREAT_HOME_PULL), so where
    // the safe tiles lie away from home and the way home runs past the danger, no step scored better
    // than standing still, and we stood in an elk's range until it killed us (19% of our deaths,
    // carrying the most gems). Then take the shortest way to safety. Off: stand still.
    let moved = moved
        || (RETREAT_UNSTICK && super::me()?.can_move()? && move_towards_closest_with_pull(safe, |_| 0.0, true)?);
    if moved {
        trace("retreat-fight");
    }
    Ok(moved)
}
