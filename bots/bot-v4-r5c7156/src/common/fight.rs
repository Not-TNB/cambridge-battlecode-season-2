// Fighting visible enemies: picking who to fight, standoffs, backing off from losing fights and
// kiting melee enemies.
use super::attack::strike;
use super::chase::{
    chase_target, choose_chase_step, dash_at_chase_target, follow_lost_target, refresh_chase_target,
};
use super::duel::{fight_view, plan_duel};
use super::me;
use super::movement::{move_towards_closest_with_pull, step_to};
use super::trace::{checkpoint, trace};
use super::world::{distance, sight, tiles_around};
use cambc::{Champion, ChampionType, Result, Team, Vec2};

// Enemies standing on their own spawn heal to full every turn, so trading blows with them (or
// chasing them there) gets nowhere: on maps where the spawns touch, both teams stood at the border
// all game. They still count as threats, just not as targets.
pub const IGNORE_ENEMIES_ON_SPAWN: bool = true;

// Enemies revealed by wards are still tracked as threats, but only nearby ones enter combat logic.
// Strict comparison: an enemy must be fewer than 7 Chebyshev tiles away.
pub const FIGHT_ENEMY_MAX_DISTANCE: i32 = 7;

// Pack aggression (v4): with at least PACK_FIGHT_ALLIES allies within PACK_FIGHT_ALLY_TILES and
// more of us there than enemies in sight, enemies up to PACK_FIGHT_DISTANCE away are fought
// (chased, if the duel model says we'd win): the group goes from its camp to the enemy. Alone it
// stays FIGHT_ENEMY_MAX_DISTANCE (10 for everyone: 60/132, gold -10%: chases pulled us off camps).
// Equal to FIGHT_ENEMY_MAX_DISTANCE: off.
// 2026-10-21, 132 games against v4 without it: 49 (deaths 18.2 against 15.5): chasing out to 10
// ran into enemies not yet in sight. Off.
pub const PACK_FIGHT_DISTANCE: i32 = 7;
const PACK_FIGHT_ALLIES: usize = 2;
const PACK_FIGHT_ALLY_TILES: i32 = 5;

fn fight_distance() -> Result<i32> {
    if PACK_FIGHT_DISTANCE <= FIGHT_ENEMY_MAX_DISTANCE {
        return Ok(FIGHT_ENEMY_MAX_DISTANCE);
    }
    let sight = sight()?;
    let here = me()?.position()?;
    let mut near = 0;
    for ally in &sight.allies {
        if ally.position().is_ok_and(|p| distance(p, here) <= PACK_FIGHT_ALLY_TILES) {
            near += 1;
        }
    }
    Ok(if near >= PACK_FIGHT_ALLIES && near + 1 > sight.enemies.len() {
        PACK_FIGHT_DISTANCE
    } else {
        FIGHT_ENEMY_MAX_DISTANCE
    })
}

// Nearby visible enemies worth fighting, weakest first (health read once per enemy).
pub fn fightable_enemies() -> Result<Vec<Champion>> {
    let spawn = cambc::map::spawn_region(Team::Enemy)?;
    let here = me()?.position()?;
    let mut by_health = Vec::new();
    let sight = sight()?;
    let max_distance = if sight.enemies.is_empty() { FIGHT_ENEMY_MAX_DISTANCE } else { fight_distance()? };
    for enemy in &sight.enemies {
        let position = enemy.position()?;
        if distance(here, position) >= max_distance
            || (IGNORE_ENEMIES_ON_SPAWN && spawn.is_set(position)?)
        {
            continue;
        }
        by_health.push((enemy.health().unwrap_or(f32::MAX), enemy.clone()));
    }
    by_health.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(by_health.into_iter().map(|(_, e)| e).collect())
}

// Fight visible enemies: strike anything already in reach; otherwise chase an enemy we expect to
// beat in a 1v1 after walking to it, or keep after one we just lost sight of. Fights we'd lose are
// retreated from, captures are held, and safe camps nearby come before chasing.
// Returns whether we attacked, cast or chased this turn.
pub fn fight_enemies() -> Result<bool> {
    let sight = sight()?;
    checkpoint("fight-sight");
    refresh_chase_target(&sight.enemies)?;
    checkpoint("fight-refresh-target");
    let enemies = fightable_enemies()?;
    checkpoint("fight-nearby-enemies");
    if enemies.is_empty() {
        if !sight.enemies.is_empty() {
            trace("no-nearby-target");
        }
        return follow_lost_target();
    }
    let standoff = standoff_key(&enemies)?;
    checkpoint("fight-standoff-key");
    if STANDOFF.with(|s| s.borrow().as_ref() == Some(&standoff)) {
        trace("standoff");
        checkpoint("fight-standoff-hit");
        return Ok(false);
    }
    let acted = fight_visible(&enemies)?;
    checkpoint("fight-visible-done");
    // Remember a decision not to act, for next turn (see STANDOFF).
    STANDOFF.with(|s| {
        *s.borrow_mut() = (!acted).then(|| (standoff.0 + 1, standoff.1, standoff.2, standoff.3))
    });
    Ok(acted)
}

// A standoff: enemies in sight that we neither chase nor back off from. Deciding that again costs
// ~30-60k fuel (threats, walking distances, a fight simulation per enemy), which left too little to
// farm, so the champion stood still (175 turns on orchard). If nothing has changed since last
// turn's decision not to act (our tile and health, each enemy's tile and health, none close enough
// to hit us or be hit, STANDOFF_CLEAR tiles), skip straight to farming.
const STANDOFF_CLEAR: i32 = 4;

type StandoffKey = (u32, Vec2, i32, Vec<(ChampionType, Vec2, i32)>);

thread_local! {
    static STANDOFF: std::cell::RefCell<Option<StandoffKey>> = const { std::cell::RefCell::new(None) };
}

fn standoff_key(enemies: &[Champion]) -> Result<StandoffKey> {
    let me = me()?;
    let here = me.position()?;
    let mut seen = Vec::new();
    for enemy in enemies {
        let p = enemy.position()?;
        // Too close to call it a standoff: make sure the key never matches.
        let health = if distance(p, here) <= STANDOFF_CLEAR {
            i32::MIN
        } else {
            enemy.health()? as i32
        };
        seen.push((enemy.champion_name()?, p, health));
    }
    Ok((cambc::round_number()?, here, me.health()? as i32, seen))
}

// The fight decisions proper, for enemies in sight. Returns whether we acted.
fn fight_visible(enemies: &[Champion]) -> Result<bool> {
    if kite_melee(enemies)? {
        trace("kite");
        return Ok(true);
    }
    checkpoint("fight-kite");
    if FLEE_LOSING_FIGHTS && losing_fight(enemies)? {
        checkpoint("fight-losing-yes");
        if super::safety::retreat_from_fight()? {
            strike(enemies, true)?;
            return Ok(true);
        }
        checkpoint("fight-retreat");
        if keep_distance(enemies)? {
            return Ok(true);
        }
    }
    checkpoint("fight-losing-done");
    if FIGHT_OUTSIDE_CAMPS && step_out_of_camps(enemies)? {
        strike(enemies, true)?;
        return Ok(true);
    }
    if strike(enemies, false)? {
        trace("strike");
        return Ok(true);
    }
    checkpoint("fight-strike");
    if super::collect::capturing()? {
        trace("hold-capture");
        return Ok(false);
    }
    checkpoint("fight-capture");

    let Some(step) = choose_chase_step(enemies)? else {
        return follow_lost_target();
    };
    checkpoint("fight-chase-plan");
    if dash_at_chase_target(enemies)? {
        trace("chase-dash");
        // Close the rest on foot (melee can still attack after walking); ranged just shoots if it can.
        if me()?.can_move()? && me()?.attack_range()? <= 1 {
            if let Some(target) = chase_target() {
                move_towards_closest_with_pull(tiles_around(target.position, 1)?, |_| 0.0, false)?;
            }
        }
        strike(enemies, true)?;
        return Ok(true);
    }
    // Step along the planned path; false means an unseen unit blocked it, so try again next turn.
    // A failed dash may have revealed a unit on that tile, which would make the step an error.
    if !me()?.can_move()? || !step_to(step) {
        trace("chase-blocked");
        return Ok(false);
    }
    trace("chase");
    // Melee champions can still attack after stepping into range.
    strike(enemies, true)?;
    Ok(true)
}

// Fighting enemies while an awake camp has us in range means tanking it as well (often we were
// clearing it when they arrived): step to a tile out of every awake camp's range, one with an enemy
// in our attack range if there is one; if no tile is out of range, to the one fewest camps reach
// (giving the camp up). Off: fight where we stand.
pub const FIGHT_OUTSIDE_CAMPS: bool = true;

fn step_out_of_camps(enemies: &[Champion]) -> Result<bool> {
    let me = me()?;
    if !me.can_move()? || enemies.is_empty() {
        return Ok(false);
    }
    let here = me.position()?;
    // (position, attack range, nearest other champion in range) of awake camps: one only hits a
    // tile nobody else in its range is closer to (threats::CAMPS_HIT_CLOSEST).
    let bodies = super::threats::other_champions()?;
    let mut camps = Vec::new();
    for camp in &sight()?.camps {
        if camp.is_awake().unwrap_or(false) {
            let (p, r) = (camp.spawn_position(), camp.attack_range()?);
            camps.push((p, r, super::threats::camp_shield(p, r, &bodies)));
        }
    }
    let hits = |tile: Vec2| {
        camps
            .iter()
            .filter(|&&(c, r, s)| distance(c, tile) <= r && s.is_none_or(|s| distance(c, tile) <= s))
            .count()
    };
    let now = hits(here);
    if now == 0 {
        cambc::debug_block! {
            if camps.iter().any(|&(c, r, _)| distance(c, here) <= r) {
                trace("camp-shielded");
            }
        }
        return Ok(false);
    }
    let range = me.attack_range()?;
    let mut targets = Vec::new();
    for enemy in enemies {
        targets.push(enemy.position()?);
    }
    let blocked = super::world::blocked_tiles()?;
    let mut best: Option<((usize, bool, i32), Vec2)> = None;
    for tile in blocked
        .reachable_tiles(here, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        if tile == here {
            continue;
        }
        let in_range = targets.iter().any(|&t| distance(t, tile) <= range);
        let nearest = targets.iter().map(|&t| distance(t, tile)).min().unwrap_or(0);
        let key = (hits(tile), !in_range, nearest);
        if best.is_none_or(|(k, _)| key < k) {
            best = Some((key, tile));
        }
    }
    let Some(((count, _, _), tile)) = best else {
        return Ok(false);
    };
    if count >= now || !super::movement::step_to(tile) {
        trace("no-camp-exit");
        return Ok(false);
    }
    trace(if count == 0 { "fight-step-out-of-camps" } else { "fight-leave-camp" });
    Ok(true)
}

// Back off from fights we'd lose: when the damage expected on our tile over the next
// FLEE_HORIZON_TURNS turns (enemies closing in included) would kill us and no enemy in sight is
// winnable (see plan_duel), retreat to safer ground (safety::retreat_from_fight), or if that can't
// move us, stay just out of the enemies' reach (keep_distance), instead of trading blows or
// farming nearby.
pub const FLEE_LOSING_FIGHTS: bool = true;
const FLEE_HORIZON_TURNS: u32 = 3;

fn losing_fight(enemies: &[Champion]) -> Result<bool> {
    let me = me()?;
    let here = me.position()?;
    let view = fight_view()?;
    checkpoint("losing-fight-view");
    let danger: f32 = (0..FLEE_HORIZON_TURNS)
        .map(|t| view.threats.damage_at_turn(here, t))
        .sum();
    if danger < me.health()? {
        return Ok(false);
    }
    for enemy in enemies {
        let winnable = plan_duel(enemy, &view.threats, &view.allies, &view.from_me)?
            .is_some_and(|duel| duel.winnable);
        checkpoint("losing-fight-duel");
        if winnable {
            return Ok(false);
        }
    }
    checkpoint("losing-fight-done");
    Ok(true)
}

// Stay just outside every visible enemy's reach: its attack range plus one turn of walking, so it
// can't step in and hit us next turn. Already outside: hold position (shooting anything in our own
// range). Inside: step to the reachable tile that gets outside while staying as close as possible
// (or, if none does, gets furthest away). No dashing: we keep our distance, we don't run.
// Returns whether we acted (moved, attacked or held).
fn keep_distance(enemies: &[Champion]) -> Result<bool> {
    let me = me()?;
    let origin = me.position()?;
    let mut reach = Vec::new();
    for enemy in enemies {
        reach.push((
            enemy.position()?,
            enemy.attack_range()? + enemy.base_stats()?.base_move_speed as i32,
        ));
    }
    // > 0: no enemy can hit this tile next turn; larger means further out than needed.
    let gap = |tile: Vec2| {
        reach
            .iter()
            .map(|&(p, r)| distance(p, tile) - r)
            .min()
            .unwrap_or(i32::MAX)
    };
    if gap(origin) > 0 {
        trace("keep-hold");
        strike(enemies, false)?;
        return Ok(true);
    }
    if !me.can_move()? {
        return Ok(false);
    }
    let view = fight_view()?;
    // Prefer tiles outside reach, then the smallest such gap (stay close), then less damage.
    let score = |tile: Vec2| {
        let g = gap(tile);
        (
            g > 0,
            if g > 0 { -g } else { g },
            -view.threats.damage_at(tile),
        )
    };
    let mut step = origin;
    let mut best = score(origin);
    for tile in view
        .blocked
        .reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        let s = score(tile);
        if s.partial_cmp(&best) == Some(std::cmp::Ordering::Greater) {
            step = tile;
            best = s;
        }
    }
    if step != origin && !step_to(step) {
        return Ok(false);
    }
    trace(if step == origin {
        "keep-stuck"
    } else {
        "keep-step"
    });
    strike(enemies, true)?; // melee may still hit something in reach after the step
    Ok(true)
}

// Ranged champions step away from a melee enemy this close: it could walk in and hit us next turn.
// 0 turns kiting off. Ranged champions either move or attack each turn, so a kite step costs a shot.
pub const KITE_DISTANCE: i32 = 2;

// Ranged vs melee: when a melee enemy is within KITE_DISTANCE, step to the reachable tile furthest
// from the nearest melee enemy (ties: less expected damage, then still in attack range of an enemy).
// Walk-compatible abilities can still be cast after the step. Returns whether we moved.
// Fix 7: melee enemies with a dash close any gap we open, so kiting them just trades our health
// for nothing (hedgerows: Prism kited Ren and Vendetta, then died on the walk home). Against them
// the usual fight logic decides: fight back if the duel says we win, else retreat towards base.
pub const KITE_DASHERS: bool = false;
const MELEE_DASHERS: [ChampionType; 4] = [
    ChampionType::Issac,
    ChampionType::Ren,
    ChampionType::Vendetta,
    ChampionType::HeartOfRuin,
];

fn kite_melee(enemies: &[Champion]) -> Result<bool> {
    let me = me()?;
    let range = me.attack_range()?;
    if KITE_DISTANCE <= 0 || range <= 1 || !me.can_move()? {
        return Ok(false);
    }
    let mut melee = Vec::new();
    for enemy in enemies {
        if enemy.attack_range()? <= 1
            && (KITE_DASHERS || !MELEE_DASHERS.contains(&enemy.champion_name()?))
        {
            melee.push(enemy.position()?);
        }
    }
    if melee.is_empty() {
        return Ok(false);
    }
    let origin = me.position()?;
    let gap = |tile: Vec2| {
        melee
            .iter()
            .map(|&m| distance(m, tile))
            .min()
            .unwrap_or(i32::MAX)
    };
    if gap(origin) > KITE_DISTANCE {
        return Ok(false);
    }

    let mut targets = Vec::new();
    for enemy in enemies {
        targets.push(enemy.position()?);
    }
    let view = fight_view()?;
    let score = |tile: Vec2| {
        let in_range = targets.iter().any(|&t| distance(t, tile) <= range);
        (
            gap(tile).min(range),
            -view.threats.damage_at(tile),
            in_range,
        )
    };
    let mut step = origin;
    let mut best = score(origin);
    for tile in view
        .blocked
        .reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        let s = score(tile);
        if s.partial_cmp(&best) == Some(std::cmp::Ordering::Greater) {
            step = tile;
            best = s;
        }
    }
    if step == origin || !step_to(step) {
        return Ok(false);
    }
    strike(enemies, true)?;
    Ok(true)
}
