// Shared helpers every champion can use: movement, targeting, fighting and farming.
use cambc::{
    Ability, Bitboard, CampGroup, Champion, ChampionType, DistanceField, Result, Team, Unit, Vec2, Camp
};
use std::cell::OnceCell;

pub fn me() -> Result<&'static Champion> {
    Ok(cambc::self_champion()?.expect("play starts after the draft"))
}

pub fn distance(a: Vec2, b: Vec2) -> i32 {
    a.chebyshev_distance(b) as i32
}

pub fn tiles_around(position: Vec2, radius: i32) -> Result<Bitboard> {
    let mut tiles = Bitboard::new()?;
    tiles.draw_chebyshev(position, radius as f32)?;
    Ok(tiles)
}

// Camp group membership never changes, so load it from the host once per champion.
pub fn camp_groups() -> Result<&'static [CampGroup]> {
    thread_local! {
        static GROUPS: OnceCell<&'static [CampGroup]> = const { OnceCell::new() };
    }
    if let Some(groups) = GROUPS.with(|cell| cell.get().copied()) { return Ok(groups); }
    let groups: &'static [CampGroup] = Vec::leak(cambc::map::camp_groups()?);
    GROUPS.with(|cell| { let _ = cell.set(groups); });
    Ok(groups)
}

pub fn blocked_tiles() -> Result<Bitboard> {
    // Combine terrain and visible occupants using native bitboard operations.
    let mut blocked = cambc::map::walls()?.clone();
    for mask in [cambc::map::holes()?.clone(), cambc::map::altars()?.clone(),
        cambc::map::spawn_region(Team::Enemy)?.clone(), cambc::vision::champion_tiles(None)?,
        cambc::vision::camp_tiles()?, cambc::vision::teleport_tiles()?] {
        blocked.union_assign(&mask)?;
    }
    blocked.set(me()?.position()?, false)?;
    Ok(blocked)
}

// Champions with a dash use it to travel when the goal is at least this many steps away and no
// enemy is in sight (fleeing always dashes). 0 turns travel dashing off.
pub const DASH_TRAVEL_MIN_STEPS: u16 = 4;

// Our dash, if we have one and it's ready.
pub fn ready_dash() -> Result<Option<&'static Ability>> {
    for ability in me()?.abilities() {
        if ability.is_dash() && ability.is_ready()? { return Ok(Some(ability)); }
    }
    Ok(None)
}

// Dash to the tile in dash range that `score` rates lowest, if it beats staying put. Dashes may
// cross holes, but we only consider tiles reachable on foot. Returns whether we dashed.
fn dash_to_best(dash: &Ability, blocked: &Bitboard, score: impl Fn(Vec2) -> Result<f32>) -> Result<bool> {
    let Some(info) = dash.info()? else { return Ok(false) };
    let origin = me()?.position()?;
    let mut best = (origin, score(origin)?);
    for tile in blocked.reachable_tiles(origin, info.as_dyn().range()? as u16)?.positions()? {
        let s = score(tile)?;
        if s < best.1 { best = (tile, s); }
    }
    // false: an unseen unit blocks the landing tile; just walk instead. A unit our occupancy view
    // hasn't caught up with makes it an error instead, which changes nothing: treat it the same.
    Ok(best.0 != origin && dash.cast_at(best.0).unwrap_or(false))
}

// Take one ordinary step. Our occupancy view can lag a unit that just appeared (e.g. right after a
// dash), and stepping onto a known occupied tile is an error that changes nothing; treat it as not
// moving instead of ending the turn.
fn step_to(tile: Vec2) -> bool {
    me().and_then(|me| me.move_absolute(tile)).unwrap_or(false)
}

pub fn move_towards_closest(goals: Bitboard) -> Result<bool> {
    let may_dash = DASH_TRAVEL_MIN_STEPS > 0 && cambc::vision::champions(Some(Team::Enemy))?.is_empty();
    move_towards_closest_with_pull(goals, |_| 0.0, may_dash)
}

// Like move_towards_closest, but each candidate step also pays `pull(tile)` (in steps), so among
// steps that get similarly close to the goals we prefer the one `pull` likes best. With `may_dash`,
// a ready dash first jumps along the way when the goals are DASH_TRAVEL_MIN_STEPS or more away.
pub fn move_towards_closest_with_pull(mut goals: Bitboard, pull: impl Fn(Vec2) -> f32, may_dash: bool) -> Result<bool> {
    let me = me()?;
    let origin = me.position()?;
    if !me.can_move()? || goals.is_set(origin)? { return Ok(false); }

    let mut blocked = blocked_tiles()?;
    goals.intersect_assign(&blocked.complement()?.union(&cambc::vision::camp_tiles()?)?)?;
    if goals.is_empty()? { return Ok(false); }

    let mut field = DistanceField::new()?;
    field.compute_to(&blocked.intersect(&goals.complement()?)?, &goals, &tiles_around(origin, 0)?, &mut Bitboard::new()?, DistanceField::UNREACHABLE)?;

    let score = |tile: Vec2| -> Result<f32> {
        let d = field.get(tile)?;
        Ok(if d == DistanceField::UNREACHABLE { f32::MAX } else { d as f32 + pull(tile) })
    };
    let mut dashed = false;
    if may_dash && DASH_TRAVEL_MIN_STEPS > 0 && field.get(origin)? >= DASH_TRAVEL_MIN_STEPS {
        if let Some(dash) = ready_dash()? {
            dashed = dash_to_best(dash, &blocked, score)?;
            // The dash (even a failed one, onto a hidden unit) may have revealed units: walking
            // onto a known occupied tile is an error, so recompute what's blocked.
            blocked = blocked_tiles()?;
        }
    }
    let origin = me.position()?;

    let mut step = origin;
    let mut best = field.get(origin)? as f32 + pull(origin);
    for tile in blocked.reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?.positions()? {
        let d = field.get(tile)?;
        if d == DistanceField::UNREACHABLE { continue; }
        let score = d as f32 + pull(tile);
        if score < best { step = tile; best = score; }
    }
    if step == origin { Ok(dashed) } else { Ok(step_to(step) || dashed) }
}

// Cast a damaging ability on a camp within range: unit spells on the camp, ground spells on its tile.
// An attack reset instead resets our attack and hits a camp in attack range again.
pub fn cast_at_camp(ability: &Ability, camp: &Camp) -> Result<bool> {
    let Some(info) = ability.info()? else { return Ok(false) };
    // Our attack this turn may have killed it.
    if !camp.is_visible()? || camp.health()? <= 0.0 { return Ok(false); }
    let target = camp.spawn_position();   // camps never move
    let me = me()?;
    // Attack resets have no spell range (asking for one errors); they reach as far as our attack.
    if ability.is_attack_reset() {
        // Only after this turn's attack is spent; resetting an available attack is an error.
        if me.can_attack()? || distance(me.position()?, target) > me.attack_range()? { return Ok(false); }
        return Ok(ability.cast_untargeted()? && attack_camp_in_range()?);
    }
    if distance(me.position()?, target) as f32 > info.as_dyn().range()? { return Ok(false); }
    if ability.can_target_camps()? { return ability.cast_on(camp); }
    if ability.can_target_ground()? { return ability.cast_at(target); }
    Ok(false)
}

// With no enemy champion in sight, spend ready damaging abilities on camps in range. After a move
// (`moved`), abilities that exclude walking are skipped: the engine rejects them (error 126).
// Returns true if we cast something that excludes walking (so the turn's move is used up).
fn cast_abilities_on_camps(moved: bool) -> Result<bool> {
    let me = me()?;
    if !me.can_cast()? || !cambc::vision::champions(Some(Team::Enemy))?.is_empty() {
        return Ok(false);
    }
    let camps = cambc::vision::camps()?;
    for ability in me.abilities() {
        if !ability.is_ready()? || (!is_damaging(ability) && !ability.is_attack_reset()) { continue; }
        let blocks_walking = ability.info()?
            .is_some_and(|info| info.as_dyn().is_exclusive_with_movement());
        if moved && blocks_walking { continue; }
        for camp in &camps {
            if cast_at_camp(ability, camp)? { return Ok(blocks_walking); }
        }
    }
    Ok(false)
}

// Make one basic attack if the target is already in attack range.
pub fn attack_if_in_range(target: &Unit) -> Result<bool> {
    let me = me()?;
    // Distance check instead of me.attack_tiles(), which builds a ~2k-fuel bitboard per call.
    if !me.can_attack()? || !target.is_visible()? || target.health()? <= 0.0
        || distance(me.position()?, target.position()?) > me.attack_range()? {
        return Ok(false);
    }
    me.attack(target)?;
    Ok(true)
}

// Damage, area damage and damaging crowd control count; dashes, heals, shields, resets and pure stuns do not.
pub fn is_damaging(ability: &Ability) -> bool {
    (ability.is_damage() || ability.is_aoe_damage() || ability.is_cc())
        && matches!(ability.damage_amount(), Ok(damage) if damage > 0.0)
}

// Cast a damaging ability on an enemy within its range: unit spells on the champion, ground spells on its tile.
pub fn cast_at_enemy(ability: &Ability, enemy: &Champion) -> Result<bool> {
    let Some(info) = ability.info()? else { return Ok(false) };
    // Our attack this turn may have killed the enemy, taking it out of vision.
    if !enemy.is_visible()? || enemy.health()? <= 0.0 { return Ok(false); }
    let target = enemy.position()?;
    if me()?.position()?.chebyshev_distance(target) as f32 > info.as_dyn().range()? { return Ok(false); }
    if ability.can_target_enemies()? { return ability.cast_on(enemy); }
    if ability.can_target_ground()? { return ability.cast_at(target); }
    Ok(false)
}

// Fight visible enemies: strike anything already in reach; otherwise chase an enemy we expect to
// beat in a 1v1 after walking to it, or keep after one we just lost sight of.
// Returns whether we attacked, cast or chased this turn.
pub fn fight_enemies() -> Result<bool> {
    let mut enemies = cambc::vision::champions(Some(Team::Enemy))?;
    refresh_chase_target(&enemies)?;
    if enemies.is_empty() { return follow_lost_target(); }
    // Focus the weakest enemy first.
    enemies.sort_by(|a, b| a.health().unwrap_or(f32::MAX).total_cmp(&b.health().unwrap_or(f32::MAX)));

    if kite_melee(&enemies)? { return Ok(true); }
    if FLEE_LOSING_FIGHTS && losing_fight(&enemies)? && flee()? {
        strike(&enemies, true)?; // melee may still hit something on the way out
        return Ok(true);
    }
    if strike(&enemies, false)? { return Ok(true); }

    let Some(step) = choose_chase_step(&enemies)? else { return follow_lost_target() };
    if dash_at_chase_target(&enemies)? {
        // Close the rest on foot (melee can still attack after walking); ranged just shoots if it can.
        if me()?.can_move()? && me()?.attack_range()? <= 1 {
            if let Some(target) = chase_target() { move_towards_closest_with_pull(tiles_around(target.position, 1)?, |_| 0.0, false)?; }
        }
        strike(&enemies, true)?;
        return Ok(true);
    }
    // Step along the planned path; false means an unseen unit blocked it, so try again next turn.
    // A failed dash may have revealed a unit on that tile, which would make the step an error.
    if !me()?.can_move()? || !step_to(step) { return Ok(false); }
    // Melee champions can still attack after stepping into range.
    strike(&enemies, true)?;
    Ok(true)
}

// Back off from fights we'd lose: when the damage expected on our tile over the next
// FLEE_HORIZON_TURNS turns (enemies closing in included) would kill us and no enemy in sight is
// winnable (see plan_duel), walk towards safety instead of trading blows or farming nearby.
pub const FLEE_LOSING_FIGHTS: bool = true;
const FLEE_HORIZON_TURNS: u32 = 3;

fn losing_fight(enemies: &[Champion]) -> Result<bool> {
    let me = me()?;
    let here = me.position()?;
    let threats = Threats::scan()?;
    let danger: f32 = (0..FLEE_HORIZON_TURNS).map(|t| threats.damage_at_turn(here, t)).sum();
    if danger < me.health()? { return Ok(false); }
    let allies = ally_helpers()?;
    let from_me = WalkGrid::from(here, &blocked_tiles()?, MAX_CHASE_STEPS)?;
    for enemy in enemies {
        if plan_duel(enemy, &threats, &allies, &from_me)?.is_some_and(|duel| duel.winnable) { return Ok(false); }
    }
    Ok(true)
}

// Step towards the nearest tile no threat reaches, leaning towards our base (as when retreating).
fn flee() -> Result<bool> {
    let threats = Threats::scan()?;
    let home = ally_spawn_centre()?;
    move_towards_closest_with_pull(threats.safe_tiles()?, |tile| RETREAT_HOME_PULL * distance(tile, home) as f32, true)
}

// Ranged champions step away from a melee enemy this close: it could walk in and hit us next turn.
// 0 turns kiting off. Ranged champions either move or attack each turn, so a kite step costs a shot.
pub const KITE_DISTANCE: i32 = 2;

// Ranged vs melee: when a melee enemy is within KITE_DISTANCE, step to the reachable tile furthest
// from the nearest melee enemy (ties: less expected damage, then still in attack range of an enemy).
// Walk-compatible abilities can still be cast after the step. Returns whether we moved.
fn kite_melee(enemies: &[Champion]) -> Result<bool> {
    let me = me()?;
    let range = me.attack_range()?;
    if KITE_DISTANCE <= 0 || range <= 1 || !me.can_move()? { return Ok(false); }
    let mut melee = Vec::new();
    for enemy in enemies {
        if enemy.attack_range()? <= 1 { melee.push(enemy.position()?); }
    }
    let origin = me.position()?;
    let gap = |tile: Vec2| melee.iter().map(|&m| distance(m, tile)).min().unwrap_or(i32::MAX);
    if gap(origin) > KITE_DISTANCE { return Ok(false); }

    let mut targets = Vec::new();
    for enemy in enemies { targets.push(enemy.position()?); }
    let threats = Threats::scan()?;
    let score = |tile: Vec2| {
        let in_range = targets.iter().any(|&t| distance(t, tile) <= range);
        (gap(tile).min(range), -threats.damage_at(tile), in_range)
    };
    let mut step = origin;
    let mut best = score(origin);
    for tile in blocked_tiles()?.reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?.positions()? {
        let s = score(tile);
        if s.partial_cmp(&best) == Some(std::cmp::Ordering::Greater) { step = tile; best = s; }
    }
    if step == origin || !step_to(step) { return Ok(false); }
    strike(enemies, true)?;
    Ok(true)
}

// Dash while chasing. Against a ranged target only when dash (+ a walking step for melee) puts us
// in attack range this turn: until then we walk, which stops us just outside its range, ready to
// dash in. Against melee targets, or with DASH_CHASE_ALWAYS, dash whenever it gets us closer.
pub const DASH_CHASE: bool = true;
pub const DASH_CHASE_ALWAYS: bool = false;

fn dash_at_chase_target(enemies: &[Champion]) -> Result<bool> {
    if !DASH_CHASE { return Ok(false); }
    let Some(dash) = ready_dash()? else { return Ok(false) };
    let Some(target) = chase_target() else { return Ok(false) };
    let Some(enemy) = enemies.iter().find(|e| e.champion_name().is_ok_and(|n| n == target.name)) else { return Ok(false) };
    let me = me()?;
    let range = me.attack_range()?;
    let walk = if range <= 1 { me.base_stats()?.base_move_speed as i32 } else { 0 };
    let blocked = blocked_tiles()?;
    // Walking distance from every tile to our attack range of the target.
    let mut goals = tiles_around(target.position, range)?;
    goals.intersect_assign(&blocked.complement()?)?;
    if goals.is_empty()? { return Ok(false); }
    let mut field = DistanceField::new()?;
    field.compute(&blocked, &goals, &mut Bitboard::new()?, MAX_CHASE_STEPS)?;
    let steps = |tile: Vec2| -> Result<f32> {
        let d = field.get(tile)?;
        Ok(if d == DistanceField::UNREACHABLE { f32::MAX } else { d as f32 })
    };
    if enemy.attack_range()? > 1 && !DASH_CHASE_ALWAYS {
        // Only commit if some landing tile leaves at most `walk` steps.
        let Some(info) = dash.info()? else { return Ok(false) };
        let mut reaches = false;
        for tile in blocked.reachable_tiles(me.position()?, info.as_dyn().range()? as u16)?.positions()? {
            if steps(tile)? <= walk as f32 { reaches = true; break; }
        }
        if !reaches { return Ok(false); }
    }
    dash_to_best(dash, &blocked, steps)
}

// Enemies within this many tiles are fought even when the duel model says we'd lose, as long as we
// survive the walk; this pulls us off a camp to fight. 0 turns it off.
pub const ENGAGE_RADIUS: i32 = 3;
pub const ENGAGE_MIN_SURVIVE_TURNS: u32 = 4;

// Enemies further than this many steps away are never chased; also bounds the pathfinding cost.
const MAX_CHASE_STEPS: u16 = 16;

// After losing sight of the chase target, keep walking to where we last saw it for this many
// rounds. Without it, a path that bends away from the enemy drops it out of vision, we go back to
// farming, see it again and turn around: back and forth. 0 turns it off.
pub const CHASE_MEMORY_ROUNDS: u32 = 3;

// Place our ward where we lost the chase target when that tile is hidden (a bush or behind a wall).
pub const WARD_LOST_TARGET: bool = true;

// The enemy we are chasing and where and when we last saw it, remembered across turns so we don't
// flip between targets. Each champion runs as its own program, so this is per champion; one of
// each type per team.
#[derive(Clone, Copy)]
struct ChaseTarget {
    name: ChampionType,
    position: Vec2,
    round: u32,
}

thread_local! {
    static CHASE_TARGET: std::cell::Cell<Option<ChaseTarget>> = const { std::cell::Cell::new(None) };
}

fn chase_target() -> Option<ChaseTarget> {
    CHASE_TARGET.with(|cell| cell.get())
}

fn set_chase_target(target: Option<(ChampionType, Vec2)>) -> Result<()> {
    let round = cambc::round_number()?;
    CHASE_TARGET.with(|cell| cell.set(target.map(|(name, position)| ChaseTarget { name, position, round })));
    Ok(())
}

// Update where the chase target is while we can see it.
fn refresh_chase_target(enemies: &[Champion]) -> Result<()> {
    let Some(target) = chase_target() else { return Ok(()) };
    for enemy in enemies {
        if enemy.champion_name()? == target.name { return set_chase_target(Some((target.name, enemy.position()?))); }
    }
    Ok(())
}

// The chase target is out of sight: walk to where we last saw it, warding that tile if it's
// hidden. Forget it once the memory runs out or we get there without finding it.
fn follow_lost_target() -> Result<bool> {
    let Some(target) = chase_target() else { return Ok(false) };
    let round = cambc::round_number()?;
    if target.round == round { return Ok(false); } // still in sight: nothing lost
    let me = me()?;
    let here = me.position()?;
    if round.saturating_sub(target.round) > CHASE_MEMORY_ROUNDS || distance(here, target.position) <= 1 {
        set_chase_target(None)?;
        return Ok(false);
    }
    if WARD_LOST_TARGET { ward_hidden_tile(target.position)?; }
    Ok(move_towards_closest(tiles_around(target.position, 1)?)?)
}

// Ward `spot` (or a free tile next to it, bushes first) if we can't see it and can reach it.
fn ward_hidden_tile(spot: Vec2) -> Result<()> {
    let me = me()?;
    if me.ward_cooldown()? > 0 || cambc::vision::visible_tiles()?.is_set(spot)? { return Ok(()); }
    let radius = cambc::constants().ward_placement_radius as i32;
    let blocked = blocked_tiles()?;
    let bushes = cambc::map::bushes()?;
    let mut best: Option<(Vec2, (bool, i32))> = None;
    for dx in -1..=1 {
        for dy in -1..=1 {
            let tile = Vec2::new(spot.x + dx, spot.y + dy);
            if distance(me.position()?, tile) > radius || blocked.is_set(tile).unwrap_or(true) { continue; }
            let rank = (!bushes.is_set(tile)?, distance(tile, spot));
            if best.is_none_or(|(_, r)| rank < r) { best = Some((tile, rank)); }
        }
    }
    // An unseen enemy on the tile makes placement fail or error; either way we just walk on.
    if let Some((tile, _)) = best { let _ = me.place_ward(tile); }
    Ok(())
}

// Keep the current target while we still expect to win against it; otherwise pick the winnable
// enemy we can finish fastest (walking turns + turns to kill). Returns the next tile to step to,
// or None if no enemy is worth chasing (or we are already in range of the one that is).
fn choose_chase_step(enemies: &[Champion]) -> Result<Option<Vec2>> {
    let threats = Threats::scan()?;
    let allies = ally_helpers()?;
    let blocked = blocked_tiles()?;
    // Walking distance from our tile to everywhere, computed once and shared by every enemy.
    let from_me = WalkGrid::from(me()?.position()?, &blocked, MAX_CHASE_STEPS)?;
    let current = chase_target().map(|t| t.name);
    let mut current_visible = false;
    let mut best: Option<(ChampionType, Vec2, Option<Vec2>, u32)> = None;
    for enemy in enemies {
        let Some(duel) = plan_duel(enemy, &threats, &allies, &from_me)? else { continue };
        // Close enemies are fought even when the fight looks lost, unless we'd die within
        // ENGAGE_MIN_SURVIVE_TURNS (other enemies joining make those fights deadly).
        let close = distance(me()?.position()?, enemy.position()?) <= ENGAGE_RADIUS && duel.hp_after_walk > 0.0
            && duel.die_turns >= ENGAGE_MIN_SURVIVE_TURNS;
        if !duel.winnable && !close { continue; }
        let name = enemy.champion_name()?;
        if Some(name) == current { return Ok(duel.next_step); }
        let total = duel.walk_turns + duel.kill_turns;
        if best.is_none_or(|(_, _, _, t)| total < t) { best = Some((name, enemy.position()?, duel.next_step, total)); }
    }
    for enemy in enemies { current_visible |= Some(enemy.champion_name()?) == current; }
    // Nothing worth chasing in sight: keep the memory of a target that went out of view.
    if best.is_none() && !current_visible { return Ok(None); }
    set_chase_target(best.map(|(name, position, _, _)| (name, position)))?;
    Ok(best.and_then(|(_, _, step, _)| step))
}

// Simulated chase: walk the shortest path to a tile within our attack range of `enemy`, taking
// the expected threat damage on every tile we end a turn on, then trade blows from that tile.
#[allow(dead_code)] // die_turns is kept for debugging and tuning
pub struct Duel {
    pub next_step: Option<Vec2>, // first tile on the path; None if already in range
    pub walk_turns: u32,
    pub hp_after_walk: f32,
    pub kill_turns: u32, // turns on the duel tile to kill the enemy (MAX: we die first or it drags on)
    pub die_turns: u32,  // turns on the duel tile until we die (MAX: we don't within the fight)
    pub winnable: bool,  // we kill the target before dying
}

// Walking distances from one tile, exported into Rust memory so lookups cost no host calls
// (each DistanceField::get is a host call). Blocked tiles, and tiles more than `max_steps`
// away, are UNREACHABLE.
pub struct WalkGrid {
    width: i32,
    height: i32,
    dist: Vec<u16>,
}

impl WalkGrid {
    pub fn from(origin: Vec2, blocked: &Bitboard, max_steps: u16) -> Result<Self> {
        let mut field = DistanceField::new()?;
        field.compute(blocked, &tiles_around(origin, 0)?, &mut Bitboard::new()?, max_steps)?;
        Ok(Self { width: field.width(), height: field.height(), dist: field.export_linear_buffer()? })
    }

    // Steps from the origin to `tile`, or UNREACHABLE (also for tiles off the map).
    pub fn get(&self, tile: Vec2) -> u16 {
        if tile.x < 0 || tile.y < 0 || tile.x >= self.width || tile.y >= self.height {
            return DistanceField::UNREACHABLE;
        }
        self.dist[(tile.y * self.width + tile.x) as usize]
    }

    // Shortest path from the origin to `end`, excluding the origin, traced back along the grid.
    pub fn path_to(&self, end: Vec2) -> Option<Vec<Vec2>> {
        let mut path = vec![end];
        let mut pos = end;
        while self.get(pos) > 1 {
            let d = self.get(pos);
            pos = (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| Vec2::new(pos.x + dx, pos.y + dy)))
                .find(|&n| self.get(n) == d - 1)?;
            path.push(pos);
        }
        path.reverse();
        Some(path)
    }
}

// The fight is simulated turn by turn rather than as a 1v1: other enemy champions in sight join
// once they could have walked into range of our duel tile (DUEL_ENEMIES_JOIN), and allies in sight
// add their basic attacks on the target once they could reach it (DUEL_ALLIES_JOIN).
pub const DUEL_ENEMIES_JOIN: bool = true;
pub const DUEL_ALLIES_JOIN: bool = true;
// Fights still undecided after this many turns count as lost.
const DUEL_MAX_TURNS: u32 = 40;

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
    if !DUEL_ALLIES_JOIN { return Ok(helpers); }
    let me = me()?.champion_name()?;
    for ally in cambc::vision::champions(Some(Team::Ally))? {
        if ally.champion_name()? == me || !ally.is_alive()? { continue; }
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
pub fn plan_duel(enemy: &Champion, threats: &Threats, allies: &[Helper], from_me: &WalkGrid) -> Result<Option<Duel>> {
    let me = me()?;
    let origin = me.position()?;
    let target = enemy.position()?;
    let range = me.attack_range()?;

    // Tiles we stand on at the end of each walking turn; the last one is where the duel happens.
    let mut stops = Vec::new();
    let mut next_step = None;
    if distance(origin, target) > range {
        // Nearest reachable tile within our attack range of the enemy.
        let mut end: Option<(Vec2, u16)> = None;
        for dx in -range..=range {
            for dy in -range..=range {
                let tile = Vec2::new(target.x + dx, target.y + dy);
                let d = from_me.get(tile);
                if d != DistanceField::UNREACHABLE && end.is_none_or(|(_, best)| d < best) { end = Some((tile, d)); }
            }
        }
        let Some((end, _)) = end else { return Ok(None) };
        let Some(path) = from_me.path_to(end) else { return Ok(None) };
        next_step = path.first().copied();
        let speed = me.base_stats()?.base_move_speed.max(1) as usize;
        for (i, &tile) in path.iter().enumerate() {
            if (i + 1) % speed == 0 || i + 1 == path.len() { stops.push(tile); }
        }
    }
    let duel_tile = stops.last().copied().unwrap_or(origin);

    // Damage on the way: every stop except the duel tile, which the fight below accounts for.
    // Stop k is reached k turns from now, by when enemies may have closed in.
    let walk_turns = stops.len() as u32;
    let walk_damage: f32 = stops.iter().take(stops.len().saturating_sub(1)).enumerate()
        .map(|(k, &tile)| threats.damage_at_turn(tile, k as u32 + 1)).sum();
    let hp_after_walk = me.health()? - walk_damage;

    // Turn by turn on the duel tile: we (and any allies that have arrived) hit the target first,
    // then everything in range of us hits back. Damage per turn is a basic attack plus abilities
    // averaged over their cooldown.
    let outgoing = turn_damage_against(enemy)?;
    let (mut hp, mut target_hp) = (hp_after_walk, enemy.health()?);
    let (mut kill_turns, mut die_turns) = (u32::MAX, u32::MAX);
    if hp > 0.0 {
        for turn in 1..=DUEL_MAX_TURNS {
            let elapsed = walk_turns + turn - 1;
            let help: f32 = allies.iter()
                .filter(|a| distance(a.origin, target) - a.speed * elapsed as i32 <= a.range)
                .map(|a| a.damage).sum();
            target_hp -= outgoing + help;
            if target_hp <= 0.0 { kill_turns = turn; break; }
            hp -= threats.damage_at_turn(duel_tile, elapsed);
            if hp <= 0.0 { die_turns = turn; break; }
        }
    }
    Ok(Some(Duel {
        next_step,
        walk_turns,
        hp_after_walk,
        kill_turns,
        die_turns,
        winnable: kill_turns != u32::MAX,
    }))
}

// Our expected damage per turn against `enemy`: a basic attack plus each ability averaged over
// its cooldown (an attack reset is worth a fraction of an extra basic attack).
fn turn_damage_against(enemy: &Champion) -> Result<f32> {
    let me = me()?;
    let strength = me.strength()?;
    let mut damage = strength;
    for ability in me.abilities() {
        let Some(info) = ability.info()? else { continue };
        let per_turn = 1.0 / info.as_dyn().max_cooldown().max(1) as f32;
        if ability.is_attack_reset() {
            damage += strength * per_turn;
        } else if is_damaging(ability) {
            damage += ability.damage_amount_against(enemy).or_else(|_| ability.damage_amount())? * per_turn;
        }
    }
    Ok(damage)
}

// Basic attack, then a ready damaging ability, else an attack reset to attack a second time.
// After a move (`moved`), abilities that exclude walking are skipped. Returns whether we acted.
fn strike(enemies: &[Champion], moved: bool) -> Result<bool> {
    let mut attacked = None;
    for enemy in enemies {
        if attack_if_in_range(enemy)? { attacked = Some(enemy); break; }
    }

    let me = me()?;
    if !me.can_cast()? { return Ok(attacked.is_some()); }
    for ability in me.abilities() {
        if !is_damaging(ability) || !ability.is_ready()? { continue; }
        if moved && ability.info()?.is_some_and(|info| info.as_dyn().is_exclusive_with_movement()) { continue; }
        for enemy in enemies {
            if cast_at_enemy(ability, enemy)? { return Ok(true); }
        }
    }

    // A reset only helps after an attack: it clears the attack cooldown so we can hit again now.
    if let Some(enemy) = attacked {
        for ability in me.abilities() {
            if ability.is_attack_reset() && ability.is_ready()? && ability.cast_untargeted()? {
                attack_if_in_range(enemy)?;
                break;
            }
        }
    }
    Ok(attacked.is_some())
}

// One source of damage: hits any tile within Chebyshev `range` of `origin` for `damage` per turn.
// Enemy champions can walk `speed` tiles a turn towards us; camps never move (speed 0).
pub struct Threat {
    pub origin: Vec2,
    pub range: i32,
    pub damage: f32,
    pub speed: i32,
}

// Everything visible that could damage us this turn. Scanning costs ~7k fuel, but checking a
// tile against the list afterwards is cheap, so scan once and reuse it for many tiles.
pub struct Threats(pub Vec<Threat>);

impl Threats {
    pub fn scan() -> Result<Self> {
        let mut threats = Vec::new();
        camp_threats(&mut threats)?;
        enemy_threats(&mut threats)?;
        Ok(Self(threats))
    }

    // Expected damage taken standing on `tile` for one turn.
    pub fn damage_at(&self, tile: Vec2) -> f32 {
        self.damage_at_turn(tile, 0)
    }

    // Expected damage on `tile` during the turn `turns` from now, if every enemy champion walks
    // straight at it meanwhile (ignoring walls, so this errs on the side of danger).
    pub fn damage_at_turn(&self, tile: Vec2, turns: u32) -> f32 {
        self.0.iter().filter(|t| distance(t.origin, tile) - t.speed * turns as i32 <= t.range).map(|t| t.damage).sum()
    }

    // Tiles no threat can reach (expected damage 0).
    pub fn safe_tiles(&self) -> Result<Bitboard> {
        let mut danger = Bitboard::new()?;
        for threat in self.0.iter().filter(|t| t.damage > 0.0) {
            danger.draw_chebyshev(threat.origin, threat.range as f32)?;
        }
        danger.complement()
    }
}

// Expected damage our champion would take standing on `tile` for one turn, from what we can see:
// awake camps within their attack range, plus enemy champions' attacks and averaged abilities.
// Every threat is assumed to target us, using raw Strength for attacks.
// Scans every call; to check several tiles, use `Threats::scan()` once and `damage_at` per tile.
#[allow(dead_code)] // kept as the simple single-tile entry point
pub fn expected_dmg(tile: Vec2) -> Result<f32> {
    Ok(Threats::scan()?.damage_at(tile))
}

// Awake camps attack once per round; camps never move, so a live camp sits on its spawn tile.
// There is no "awake" query: a group counts as awake once a visible member is damaged or missing
// (dead). Groups that went back to sleep after two quiet rounds are still counted, overestimating.
fn camp_threats(threats: &mut Vec<Threat>) -> Result<()> {
    let live_camps = cambc::vision::camp_tiles()?;
    let visible = cambc::vision::visible_tiles()?;
    for group in camp_groups()? {
        let mut live = Vec::new();
        let mut awake = false;
        for camp in group.camps() {
            let spawn = camp.spawn_position();
            if !visible.is_set(spawn)? { continue; }
            if !live_camps.is_set(spawn)? { awake = true; continue; }
            if camp.health()? < camp.max_health()? { awake = true; }
            live.push(camp);
        }
        if !awake { continue; }
        for camp in live {
            threats.push(Threat { origin: camp.spawn_position(), range: camp.attack_range()?, damage: camp.strength()?, speed: 0 });
        }
    }
    Ok(())
}

// Enemies basic-attack anything within attack range. Abilities are averaged over their cooldown
// (1/8 of a hit per turn) rather than assuming a ready ability is cast now; an attack reset is
// worth 1/8 of an extra basic attack.
fn enemy_threats(threats: &mut Vec<Threat>) -> Result<()> {
    let me = me()?;
    for enemy in cambc::vision::champions(Some(Team::Enemy))? {
        let origin = enemy.position()?;
        let attack_range = enemy.attack_range()?;
        let strength = enemy.strength()?;
        let speed = if DUEL_ENEMIES_JOIN { enemy.base_stats()?.base_move_speed as i32 } else { 0 };
        threats.push(Threat { origin, range: attack_range, damage: strength, speed });
        for ability in enemy.abilities() {
            let Some(info) = ability.info()? else { continue };
            let per_turn = 1.0 / info.as_dyn().max_cooldown().max(1) as f32;
            if ability.is_attack_reset() {
                threats.push(Threat { origin, range: attack_range, damage: strength * per_turn, speed });
            } else if is_damaging(ability) {
                // Engine prediction includes our mitigation and shields; fall back to raw damage.
                let hit = ability.damage_amount_against(me).or_else(|_| ability.damage_amount())?;
                threats.push(Threat { origin, range: info.as_dyn().range()? as i32, damage: hit * per_turn, speed });
            }
        }
    }
    Ok(())
}

// How much a step closer to our spawn is worth, in steps closer to safety, when fleeing. Below 1,
// so every step still makes progress toward safety; without it, ties between equally close safe
// tiles went to whichever came first in tile order, so we fled bottom-right.
const RETREAT_HOME_PULL: f32 = 0.5;

// Centre of our spawn region.
fn ally_spawn_centre() -> Result<Vec2> {
    let tiles = cambc::map::spawn_region(Team::Ally)?.positions()?;
    let n = tiles.len().max(1) as i32;
    let (x, y) = tiles.iter().fold((0, 0), |(x, y), t| (x + t.x, y + t.y));
    Ok(Vec2::new(x / n, y / n))
}

// Low on health: step toward the nearest tile with zero expected damage (leaning toward our base),
// and recall once on one, since damage interrupts a recall. Recalls anyway if no safe tile can be reached.
pub fn retreat_and_recall() -> Result<()> {
    let me = me()?;
    let threats = Threats::scan()?;
    let home = ally_spawn_centre()?;
    let toward_home = |tile: Vec2| RETREAT_HOME_PULL * distance(tile, home) as f32;
    if threats.damage_at(me.position()?) > 0.0
        && move_towards_closest_with_pull(threats.safe_tiles()?, toward_home, true)?
        && threats.damage_at(me.position()?) > 0.0 {
        return Ok(()); // still in danger after this step; keep walking next turn
    }
    me.recall()
}

// Tribute at an altar in range, otherwise walk toward the nearest unlocked altar.
pub fn bank_gems() -> Result<()> {
    let me = me()?;
    // Union all altar approach tiles: the field chooses the nearest reachable altar.
    let mut goals = Bitboard::new()?;
    for altar in cambc::map::altars()?.positions()? {
        if cambc::vision::has_altar_lock(altar)? { continue; }
        let range = cambc::constants().submit_gems_range;
        if distance(me.position()?, altar) <= range { me.submit_gems(altar)?; return Ok(()); }
        goals.draw_chebyshev(altar, range as f32)?;
    }
    move_towards_closest(goals)?;
    Ok(())
}

// Bank when we carry at least BANK_MIN_GEMS, or any gems with an altar within BANK_NEAR_TILES,
// so leaving spawn doesn't turn into a walk to the altar for a small bag. 1 banks whenever we
// carry gems. 40 tested neutral on win rate and halves bank walking (writeup entry 18).
pub const BANK_MIN_GEMS: u32 = 40;
pub const BANK_NEAR_TILES: i32 = 6;

pub fn should_bank() -> Result<bool> {
    let me = me()?;
    let gems = me.gem_count()?;
    if gems == 0 { return Ok(false); }
    if gems >= BANK_MIN_GEMS { return Ok(true); }
    let here = me.position()?;
    for altar in cambc::map::altars()?.positions()? {
        if distance(here, altar) <= BANK_NEAR_TILES { return Ok(true); }
    }
    Ok(false)
}

// Whether we know of an altar without an enemy lock on it within `tiles` of us.
pub fn unlocked_altar_within(tiles: i32) -> Result<bool> {
    let here = me()?.position()?;
    for altar in cambc::map::altars()?.positions()? {
        if distance(here, altar) <= tiles && !cambc::vision::has_altar_lock(altar)? { return Ok(true); }
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
    let gold = if shops_anywhere { 0.0 } else { me.gold_count()? as f32 / 150.0 };
    let missing_hp = 1.0 - me.health()? / me.max_health()?.max(1.0);
    Ok(BASE_W_GOLD * gold + BASE_W_MISSING_HP * missing_hp + BASE_W_ROUNDS_AWAY * rounds_away as f32)
}

// A tank or healer leaves a cleared camp group's capture to an AD carry ally whose attack range
// covers it: capture rewards are split between every ally on the region, so the carry gets all of it.
const TANK_FUNNEL: bool = true;
pub const FUNNEL_HEALERS: bool = true;

// Attack a camp already in range (camps never move, so their spawn tile is their position).
fn attack_camp_in_range() -> Result<bool> {
    let me = me()?;
    if !me.can_attack()? { return Ok(false); }
    let (position, range) = (me.position()?, me.attack_range()?);
    for camp in cambc::vision::camps()? {
        if distance(camp.spawn_position(), position) <= range && attack_if_in_range(&camp)? { return Ok(true); }
    }
    Ok(false)
}

// Whether an AD carry ally (not us) has any of `group`'s capture tiles within its attack range.
fn carry_covers(group: &CampGroup) -> Result<bool> {
    let my_position = me()?.position()?;
    for ally in cambc::vision::champions(Some(Team::Ally))? {
        let position = ally.position()?;
        if position == my_position || !crate::constants::class_of(ally.champion_name()?).is_ad_carry() { continue; }
        let range = ally.attack_range()?;
        if group.camps().iter().any(|c| distance(c.spawn_position(), position) <= range) { return Ok(true); }
    }
    Ok(false)
}

// Round each camp group (indexed as in camp_groups()) is expected back after we last saw it
// respawning. Without this, a respawning group only counts as respawning while in sight, so a
// champion at the edge of vision steps away, loses sight, steps back, and oscillates.
thread_local! {
    static RESPAWN_READY: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Phase filter on top of nearest-camp farming: before FARM_GEM_PHASE_ROUND we skip gem clusters
// and monoliths (gems early don't help; gold buys items), after it we skip gold clusters (35 gold,
// no gems). If no camp of the preferred kinds is available we fall back to all camps. Scoring
// camps by value per turn instead tested worse (writeup entry 20).
pub const FARM_PHASE_FILTER: bool = true;
pub const FARM_GEM_PHASE_ROUND: u32 = 150;

fn preferred_in_phase(kind: cambc::CampType, round: u32) -> bool {
    use cambc::CampType as C;
    if round < FARM_GEM_PHASE_ROUND { !matches!(kind, C::GemCluster | C::Monolith) } else { kind != C::GoldCluster }
}

// The first of our top HOTSPOT_TOP_K draft-planned spots (gold before the gem phase, gems after)
// whose group is available, as an index into camp_groups().
fn hotspot_group(available: &[bool], round: u32) -> Option<usize> {
    use crate::hotspots::{HOTSPOTS, HOTSPOT_MAX_DETOUR, HOTSPOT_START_ROUND, HOTSPOT_TOP_K};
    if !HOTSPOTS || round < HOTSPOT_START_ROUND { return None; }
    if crate::hotspots::HOTSPOT_AVOID_ENEMIES && !cambc::vision::champions(Some(Team::Enemy)).ok()?.is_empty() { return None; }
    let (gold, gems) = crate::hotspots::my_spots()?;
    let spots = if round < FARM_GEM_PHASE_ROUND { gold } else { gems };
    let groups = camp_groups().ok()?;
    let here = me().ok()?.position().ok()?;
    // Straight-line distance to the nearest available camp, to cap the detour to a spot.
    let nearest = groups.iter().enumerate().filter(|&(i, _)| available[i])
        .flat_map(|(_, g)| g.camps().iter().map(|c| distance(here, c.spawn_position()))).min()?;
    spots.iter().take(HOTSPOT_TOP_K).find_map(|&spot| {
        let index = groups.iter().position(|g| g.camps().first().is_some_and(|c| c.spawn_position() == spot))?;
        (available[index] && distance(here, spot) <= nearest + HOTSPOT_MAX_DETOUR).then_some(index)
    })
}

// Farm the nearest camps that aren't respawning (including ones we saw respawning earlier). Tanks and healers still fight camps but leave the capture
// of a cleared group to a covering AD carry (see TANK_FUNNEL).
pub fn farm_camps() -> Result<()> {
    let attacked = attack_camp_in_range()?;
    if attacked { 
        cast_abilities_on_camps(false)?;
        return Ok(()); 
    }

    let class = crate::constants::class_of(me()?.champion_name()?);
    let is_tank = TANK_FUNNEL && (class == crate::constants::ClassKind::Tank
        || (FUNNEL_HEALERS && class == crate::constants::ClassKind::Healer));
    // Set goal bits directly: each new Bitboard costs ~2k fuel in the Rust SDK.
    let mut goals = Bitboard::new()?;
    let visible = cambc::vision::visible_tiles()?;
    let live_camps = cambc::vision::camp_tiles()?;
    let round = cambc::round_number()?;
    let groups = camp_groups()?;
    let mut ready = RESPAWN_READY.with(|cell| cell.take());
    ready.resize(groups.len(), 0);
    let mut preferred = Bitboard::new()?;
    let mut available = vec![false; groups.len()];
    for (index, group) in groups.iter().enumerate() {
        let mut seen = false;
        let mut any_live = false;
        for camp in group.camps() {
            let spawn = camp.spawn_position();
            if !visible.is_set(spawn)? { continue; }
            seen = true;
            if live_camps.is_set(spawn)? { any_live = true; break; }
        }
        if any_live { ready[index] = 0; }
        if round < ready[index] { continue; } // seen respawning earlier, not back yet
        // Only a seen group with no live camps can be respawning or waiting for capture.
        if seen && !any_live {
            match group.state()? {
                cambc::CampGroupState::Respawning(timer) => { ready[index] = round + timer.max(0) as u32; continue }
                cambc::CampGroupState::RewardAvailable if is_tank && carry_covers(group)? => continue,
                _ => {}
            }
        }
        available[index] = true;
        for camp in group.camps() {
            goals.set(camp.spawn_position(), true)?;
            if preferred_in_phase(camp.kind(), round) { preferred.set(camp.spawn_position(), true)?; }
        }
    }
    RESPAWN_READY.with(|cell| cell.replace(ready));

    if FARM_PHASE_FILTER && !preferred.is_empty()? { goals = preferred; }
    // Past the early game, head for our highest-ranked available hot spot (see hotspots.rs).
    if let Some(index) = hotspot_group(&available, round) {
        goals.clear();
        for camp in groups[index].camps() { goals.set(camp.spawn_position(), true)?; }
    }

    if move_towards_closest(goals)? { 
        if attack_camp_in_range()? {
            cast_abilities_on_camps(true)?;
        }
    }
    Ok(())
}
