// Shared helpers every champion can use: movement, targeting, fighting and farming.
use cambc::{
    Ability, Bitboard, Camp, CampGroup, Champion, ChampionType, DistanceField, Result, Team, Unit,
    Vec2,
};
use std::cell::OnceCell;
use std::fmt::Write as _;

pub fn me() -> Result<&'static Champion> {
    Ok(cambc::self_champion()?.expect("play starts after the draft"))
}

// Verbose turn/fuel tracing is disabled; farm-target changes have their own concise diagnostic.
pub const TRACE: bool = false;
const FARM_TARGET_DIAGNOSTICS: bool = true;

thread_local! {
    static TRACE_TAGS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

pub fn trace(tag: &str) {
    if TRACE {
        TRACE_TAGS.with(|t| {
            let mut t = t.borrow_mut();
            t.push(' ');
            t.push_str(tag);
        });
    }
}

thread_local! {
    static CHECKPOINTS: std::cell::RefCell<Vec<(&'static str, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Record fuel left at a point in the turn (formatted only when the trace is printed, so the
// measurement itself costs little).
#[allow(dead_code)] // for profiling: sprinkle in, read the trace, remove
pub fn checkpoint(tag: &'static str) {
    if TRACE {
        CHECKPOINTS.with(|c| {
            c.borrow_mut()
                .push((tag, cambc::fuel_remaining().unwrap_or(0)))
        });
    }
}

pub fn take_trace() -> String {
    let mut out = TRACE_TAGS.with(|t| std::mem::take(&mut *t.borrow_mut()));
    let checkpoints = CHECKPOINTS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    if !checkpoints.is_empty() {
        out.push_str(" | fuel[k left]: ");
        let mut previous = None;
        for (tag, fuel) in checkpoints {
            if let Some(before) = previous {
                if fuel > before {
                    write!(out, " -> next turn: {tag} {}k", fuel / 1000)
                        .expect("writing to String cannot fail");
                } else {
                    let change = (fuel as i64 - before as i64) / 1000;
                    write!(out, " -> {tag} {}k ({change:+}k)", fuel / 1000)
                        .expect("writing to String cannot fail");
                }
            } else {
                write!(out, "{tag} {}k", fuel / 1000).expect("writing to String cannot fail");
            }
            previous = Some(fuel);
        }
    }
    out
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
    if let Some(groups) = GROUPS.with(|cell| cell.get().copied()) {
        return Ok(groups);
    }
    let groups: &'static [CampGroup] = Vec::leak(cambc::map::camp_groups()?);
    GROUPS.with(|cell| {
        let _ = cell.set(groups);
    });
    Ok(groups)
}

// Everything visible to this champion's own vision and ward. Listing visible champions costs ~4k
// fuel and camps ~2k, and a turn used to ask for them up to ten times, which left too little fuel
// to farm on turns with enemies in sight (the champion stood still). Cached per round and position,
// since moving changes what we see, and dropped after we attack or cast, which may kill what we saw.
pub struct Sight {
    pub enemies: Vec<Champion>,
    pub allies: Vec<Champion>, // without us
    pub camps: Vec<Camp>,
    pub visible: Bitboard,
}

thread_local! {
    static SIGHT: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<Sight>)>> = const { std::cell::RefCell::new(None) };
}

pub fn sight() -> Result<std::rc::Rc<Sight>> {
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(sight) = SIGHT.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, s)| s.clone())
    }) {
        return Ok(sight);
    }
    let name = me()?.champion_name()?;
    let mut allies = cambc::vision::champions(Some(Team::Ally))?;
    allies.retain(|ally| ally.champion_name().is_ok_and(|n| n != name));
    // Some enemies on the occupancy board fail unit queries (health, name) with "not in vision",
    // which ended the turn: keep only ones we can query.
    let mut enemies = cambc::vision::champions(Some(Team::Enemy))?;
    enemies.retain(|enemy| {
        enemy.position().is_ok() && enemy.health().is_ok() && enemy.champion_name().is_ok()
    });
    // Camps never move, so a live camp is the one spawning on its tile.
    let mut camps = Vec::new();
    let groups = camp_groups()?;
    for tile in cambc::vision::camp_tiles()?.positions()? {
        let Some(index) = group_at(tile)? else {
            continue;
        };
        if let Some(camp) = groups[index]
            .camps()
            .iter()
            .find(|c| c.spawn_position() == tile)
        {
            camps.push(camp.clone());
        }
    }
    let sight = std::rc::Rc::new(Sight {
        enemies,
        allies,
        camps,
        visible: cambc::vision::visible_tiles()?,
    });
    SIGHT.with(|c| *c.borrow_mut() = Some((key.0, key.1, sight.clone())));
    Ok(sight)
}

pub fn forget_sight() {
    SIGHT.with(|c| *c.borrow_mut() = None);
    forget_blocked();
}

// Index into camp_groups() of the group whose camp spawns on `tile`, if any (membership is fixed).
pub fn group_at(tile: Vec2) -> Result<Option<usize>> {
    thread_local! {
        static BY_TILE: OnceCell<Vec<((i32, i32), usize)>> = const { OnceCell::new() };
    }
    if BY_TILE.with(|c| c.get().is_none()) {
        let mut by_tile = Vec::new();
        for (index, group) in camp_groups()?.iter().enumerate() {
            for camp in group.camps() {
                by_tile.push(((camp.spawn_position().x, camp.spawn_position().y), index));
            }
        }
        by_tile.sort();
        BY_TILE.with(|c| {
            let _ = c.set(by_tile);
        });
    }
    Ok(BY_TILE.with(|c| {
        let by_tile = c.get().expect("set above");
        by_tile
            .binary_search_by_key(&(tile.x, tile.y), |&(t, _)| t)
            .ok()
            .map(|i| by_tile[i].1)
    }))
}

// Tiles we can't walk onto: terrain (walls, holes, altars, the enemy spawn; fixed, so combined
// once) plus visible occupants. Each new Bitboard costs ~2k fuel and this was rebuilt from seven
// several times a turn, so it's cached per round and position and dropped with forget_sight (and
// after a dash attempt, which can reveal a hidden unit).
// Tiles a step or dash onto failed because an unseen unit stands there (in a bush), each with the
// round to stop avoiding it. The engine reveals such a unit only for the rest of the round, so the
// next turn's path picked the same step and failed again, turn after turn for as long as the unit
// stayed (broken-curve: the Tavi squad's leader stood still from round 596 to the end of the game).
// 0: not remembered.
pub const HIDDEN_BLOCK_ROUNDS: u32 = 10;

thread_local! {
    static HIDDEN_BLOCKS: std::cell::RefCell<Vec<(Vec2, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Remember that an unseen unit blocks `tile` (see HIDDEN_BLOCK_ROUNDS).
pub fn remember_hidden_block(tile: Vec2) {
    if HIDDEN_BLOCK_ROUNDS == 0 {
        return;
    }
    let round = cambc::round_number().unwrap_or(0);
    HIDDEN_BLOCKS.with(|h| {
        let mut h = h.borrow_mut();
        h.retain(|&(t, until)| t != tile && round < until);
        h.push((tile, round + HIDDEN_BLOCK_ROUNDS));
    });
    forget_blocked();
}

thread_local! {
    static BLOCKED: std::cell::RefCell<Option<(u32, Vec2, Bitboard)>> = const { std::cell::RefCell::new(None) };
}

// The fixed part: walls, holes, altars and the enemy spawn, combined once per champion.
pub fn terrain_tiles() -> Result<&'static Bitboard> {
    thread_local! {
        static TERRAIN: OnceCell<&'static Bitboard> = const { OnceCell::new() };
    }
    if let Some(terrain) = TERRAIN.with(|c| c.get().copied()) {
        return Ok(terrain);
    }
    let mut terrain = cambc::map::walls()?.clone();
    terrain.union_assign(cambc::map::holes()?)?;
    terrain.union_assign(cambc::map::altars()?)?;
    terrain.union_assign(cambc::map::spawn_region(Team::Enemy)?)?;
    let terrain: &'static Bitboard = Box::leak(Box::new(terrain));
    TERRAIN.with(|c| {
        let _ = c.set(terrain);
    });
    Ok(terrain)
}

pub fn blocked_tiles() -> Result<Bitboard> {
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(blocked) = BLOCKED.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, b)| b.clone())
    }) {
        return Ok(blocked);
    }
    let mut blocked = terrain_tiles()?.clone();
    for mask in [
        cambc::vision::champion_tiles(None)?,
        cambc::vision::camp_tiles()?,
        cambc::vision::teleport_tiles()?,
    ] {
        blocked.union_assign(&mask)?;
    }
    for (tile, until) in HIDDEN_BLOCKS.with(|h| h.borrow().clone()) {
        if key.0 < until {
            blocked.set(tile, true)?;
        }
    }
    blocked.set(key.1, false)?;
    BLOCKED.with(|c| *c.borrow_mut() = Some((key.0, key.1, blocked.clone())));
    Ok(blocked)
}

fn forget_blocked() {
    BLOCKED.with(|c| *c.borrow_mut() = None);
}

// Champions with a dash use it to travel when the goal is at least this many steps away and no
// enemy is in sight (fleeing always dashes). 0 turns travel dashing off.
pub const DASH_TRAVEL_MIN_STEPS: u16 = 4;

// Our dash, if we have one and it's ready.
pub fn ready_dash() -> Result<Option<&'static Ability>> {
    for ability in me()?.abilities() {
        if ability.is_dash() && ability.is_ready()? {
            return Ok(Some(ability));
        }
    }
    Ok(None)
}

// Dash to the tile in dash range that `score` rates lowest, if it beats staying put. Dashes may
// cross holes, but we only consider tiles reachable on foot. Returns whether we dashed.
fn dash_to_best(
    dash: &Ability,
    blocked: &Bitboard,
    score: impl Fn(Vec2) -> Result<f32>,
) -> Result<bool> {
    let Some(info) = dash.info()? else {
        return Ok(false);
    };
    let origin = me()?.position()?;
    let mut best = (origin, score(origin)?);
    for tile in blocked
        .reachable_tiles(origin, info.as_dyn().range()? as u16)?
        .positions()?
    {
        let s = score(tile)?;
        if s < best.1 {
            best = (tile, s);
        }
    }
    // false: an unseen unit blocks the landing tile; just walk instead. A unit our occupancy view
    // hasn't caught up with makes it an error instead, which changes nothing: treat it the same.
    // Either way the attempt may have revealed a unit, so what's blocked must be looked up again.
    if best.0 == origin {
        return Ok(false);
    }
    let dashed = match dash.cast_at(best.0) {
        Ok(true) => true,
        Ok(false) => {
            remember_hidden_block(best.0);
            false
        }
        Err(_) => false,
    };
    forget_sight();
    Ok(dashed)
}

// Take one ordinary step. Our occupancy view can lag a unit that just appeared (e.g. right after a
// dash), and stepping onto a known occupied tile is an error that changes nothing; treat it as not
// moving instead of ending the turn.
// A step that fails because an unseen unit stands there is remembered, so we walk round it
// (HIDDEN_BLOCK_ROUNDS).
pub(crate) fn step_to(tile: Vec2) -> bool {
    match me().and_then(|me| me.move_absolute(tile)) {
        Ok(true) => true,
        Ok(false) => {
            remember_hidden_block(tile);
            false
        }
        Err(_) => false,
    }
}

pub fn move_towards_closest(goals: Bitboard) -> Result<bool> {
    let may_dash = DASH_TRAVEL_MIN_STEPS > 0 && fightable_enemies()?.is_empty();
    Ok(
        move_towards_closest_with_pull_status(
            goals,
            |_| 0.0,
            may_dash,
            DistanceField::UNREACHABLE,
        )?
        .0,
    )
}

// Like move_towards_closest, but each candidate step also pays `pull(tile)` (in steps), so among
// steps that get similarly close to the goals we prefer the one `pull` likes best. With `may_dash`,
// a ready dash first jumps along the way when the goals are DASH_TRAVEL_MIN_STEPS or more away.
pub fn move_towards_closest_with_pull(
    goals: Bitboard,
    pull: impl Fn(Vec2) -> f32,
    may_dash: bool,
) -> Result<bool> {
    Ok(move_towards_closest_with_pull_status(goals, pull, may_dash, DistanceField::UNREACHABLE)?.0)
}

// Like move_towards_closest, also returning whether a walking path within `max_steps` exists.
fn move_towards_closest_with_path_status(
    goals: Bitboard,
    max_steps: u16,
) -> Result<(bool, Option<bool>)> {
    let may_dash = DASH_TRAVEL_MIN_STEPS > 0 && fightable_enemies()?.is_empty();
    move_towards_closest_with_pull_status(goals, |_| 0.0, may_dash, max_steps)
}

fn move_towards_closest_with_pull_status(
    mut goals: Bitboard,
    pull: impl Fn(Vec2) -> f32,
    may_dash: bool,
    max_steps: u16,
) -> Result<(bool, Option<bool>)> {
    let me = me()?;
    let origin = me.position()?;
    if !me.can_move()? || goals.is_set(origin)? {
        return Ok((false, None));
    }

    let mut blocked = blocked_tiles()?;
    checkpoint("path-blocked");
    goals.intersect_assign(&blocked.complement()?.union(&cambc::vision::camp_tiles()?)?)?;
    if goals.is_empty()? {
        return Ok((false, None));
    }

    let mut field = DistanceField::new()?;
    field.compute_to(
        &blocked.intersect(&goals.complement()?)?,
        &goals,
        &tiles_around(origin, 0)?,
        &mut Bitboard::new()?,
        max_steps,
    )?;
    checkpoint("path-field");
    let origin_distance = field.get(origin)?;
    let pathable = origin_distance != DistanceField::UNREACHABLE;

    let score = |tile: Vec2| -> Result<f32> {
        let d = field.get(tile)?;
        Ok(if d == DistanceField::UNREACHABLE {
            f32::MAX
        } else {
            d as f32 + pull(tile) + centre_bias(tile)
        })
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
    checkpoint("path-dash");
    let origin = me.position()?;

    let mut step = origin;
    let mut best = origin_distance as f32 + pull(origin) + centre_bias(origin);
    for tile in blocked
        .reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        let d = field.get(tile)?;
        if d == DistanceField::UNREACHABLE {
            continue;
        }
        let score = d as f32 + pull(tile) + centre_bias(tile);
        if score < best {
            step = tile;
            best = score;
        }
    }
    checkpoint("path-step");
    if step == origin {
        Ok((dashed, Some(pathable)))
    } else {
        Ok((step_to(step) || dashed, Some(pathable)))
    }
}

// Cast a damaging ability on a camp within range: unit spells on the camp, ground spells on its tile.
// An attack reset instead resets our attack and hits a camp in attack range again.
pub fn cast_at_camp(ability: &Ability, camp: &Camp) -> Result<bool> {
    let Some(info) = ability.info()? else {
        return Ok(false);
    };
    // Our attack this turn may have killed it.
    if !camp.is_visible()? || camp.health()? <= 0.0 {
        return Ok(false);
    }
    let target = camp.spawn_position(); // camps never move
    let me = me()?;
    // Attack resets have no spell range (asking for one errors); they reach as far as our attack.
    if ability.is_attack_reset() {
        // Only after this turn's attack is spent; resetting an available attack is an error.
        if me.can_attack()? || distance(me.position()?, target) > me.attack_range()? {
            return Ok(false);
        }
        return Ok(ability.cast_untargeted()? && attack_camp_in_range()?);
    }
    if distance(me.position()?, target) as f32 > info.as_dyn().range()? {
        return Ok(false);
    }
    forget_sight();
    if ability.can_target_camps()? {
        return ability.cast_on(camp);
    }
    if ability.can_target_ground()? {
        return ability.cast_at(target);
    }
    Ok(false)
}

// Spend ready damaging abilities on camps in range. After a move (`moved`), abilities that exclude
// walking are skipped: the engine rejects them (error 126).
// Returns true if we cast something that excludes walking (so the turn's move is used up).
fn cast_abilities_on_camps(moved: bool, goals: Option<&Bitboard>) -> Result<bool> {
    let me = me()?;
    let sight = sight()?;
    if !me.can_cast()? {
        return Ok(false);
    }
    let mut camps = Vec::new();
    for camp in &sight.camps {
        if let Some(goals) = goals {
            if !goals.is_set(camp.spawn_position())? {
                continue;
            }
        }
        camps.push((camp.health()?, camp));
    }
    camps.sort_by(|a, b| a.0.total_cmp(&b.0));
    for ability in me.abilities() {
        if !ability.is_ready()?
            || (!is_damaging(ability) && !ability.is_attack_reset())
            || (goals.is_some() && ability.is_attack_reset())
        {
            continue;
        }
        let blocks_walking = ability
            .info()?
            .is_some_and(|info| info.as_dyn().is_exclusive_with_movement());
        if moved && blocks_walking {
            continue;
        }
        for &(_, camp) in &camps {
            if cast_at_camp(ability, camp)? {
                return Ok(blocks_walking);
            }
        }
    }
    Ok(false)
}

// Make one basic attack if the target is already in attack range.
pub fn attack_if_in_range(target: &Unit) -> Result<bool> {
    let me = me()?;
    // Distance check instead of me.attack_tiles(), which builds a ~2k-fuel bitboard per call.
    if !me.can_attack()?
        || !target.is_visible()?
        || target.health()? <= 0.0
        || distance(me.position()?, target.position()?) > me.attack_range()?
    {
        return Ok(false);
    }
    me.attack(target)?;
    forget_sight();
    Ok(true)
}

// Damage, area damage and damaging crowd control count; dashes, heals, shields, resets and pure stuns do not.
pub fn is_damaging(ability: &Ability) -> bool {
    (ability.is_damage() || ability.is_aoe_damage() || ability.is_cc())
        && matches!(ability.damage_amount(), Ok(damage) if damage > 0.0)
}

// Cast a damaging ability on an enemy within its range: unit spells on the champion, ground spells on its tile.
pub fn cast_at_enemy(ability: &Ability, enemy: &Champion) -> Result<bool> {
    let Some(info) = ability.info()? else {
        return Ok(false);
    };
    // Our attack this turn may have killed the enemy, taking it out of vision.
    if !enemy.is_visible()? || enemy.health()? <= 0.0 {
        return Ok(false);
    }
    let target = enemy.position()?;
    if me()?.position()?.chebyshev_distance(target) as f32 > info.as_dyn().range()? {
        return Ok(false);
    }
    forget_sight();
    if ability.can_target_enemies()? {
        return ability.cast_on(enemy);
    }
    if ability.can_target_ground()? {
        return ability.cast_at(target);
    }
    Ok(false)
}

// Enemies near their spawn are avoided as targets: they can retreat into the healing area, and
// chasing them there can keep the team pinned near the border.
pub const IGNORE_ENEMIES_ON_SPAWN: bool = true;
const ENEMY_BASE_BUFFER_TILES: i32 = 3;

fn enemy_base_buffer() -> Result<&'static Bitboard> {
    thread_local! {
        static BUFFER: OnceCell<&'static Bitboard> = const { OnceCell::new() };
    }
    if let Some(buffer) = BUFFER.with(|cell| cell.get().copied()) {
        return Ok(buffer);
    }
    let spawn_positions = cambc::map::spawn_region(Team::Enemy)?.positions()?;
    let mut buffer = Bitboard::new()?;
    if let Some(first) = spawn_positions.first() {
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (first.x, first.x, first.y, first.y);
        for position in &spawn_positions[1..] {
            min_x = min_x.min(position.x);
            max_x = max_x.max(position.x);
            min_y = min_y.min(position.y);
            max_y = max_y.max(position.y);
        }
        for x in (min_x - ENEMY_BASE_BUFFER_TILES)..=(max_x + ENEMY_BASE_BUFFER_TILES) {
            for y in (min_y - ENEMY_BASE_BUFFER_TILES)..=(max_y + ENEMY_BASE_BUFFER_TILES) {
                let tile = Vec2::new(x, y);
                if cambc::map::is_in_bounds(tile)? {
                    buffer.set(tile, true)?;
                }
            }
        }
    }
    let buffer: &'static Bitboard = Box::leak(Box::new(buffer));
    BUFFER.with(|cell| {
        let _ = cell.set(buffer);
    });
    Ok(buffer)
}

pub fn enemy_position_safe_to_target(position: Vec2) -> Result<bool> {
    if IGNORE_ENEMIES_ON_SPAWN {
        return Ok(!enemy_base_buffer()?.is_set(position)?);
    }
    Ok(true)
}

// Enemies revealed by wards are still tracked as threats, but only nearby ones enter combat logic.
// Strict comparison: an enemy must be fewer than 7 Chebyshev tiles away.
pub const FIGHT_ENEMY_MAX_DISTANCE: i32 = 7;

// Nearby visible enemies worth fighting, weakest first (health read once per enemy).
pub fn fightable_enemies() -> Result<Vec<Champion>> {
    let here = me()?.position()?;
    let mut by_health = Vec::new();
    for enemy in &sight()?.enemies {
        let position = enemy.position()?;
        if distance(here, position) >= FIGHT_ENEMY_MAX_DISTANCE
            || !enemy_position_safe_to_target(position)?
        {
            continue;
        }
        by_health.push((enemy.health().unwrap_or(f32::MAX), enemy.clone()));
    }
    by_health.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(by_health.into_iter().map(|(_, e)| e).collect())
}

pub fn fight_squad_enemy(rank: usize) -> Result<Option<Vec2>> {
    let enemies = fightable_enemies()?;
    if enemies.is_empty() {
        return Ok(None);
    }
    // Enemies in the keep-out zone round their spawn (enemy_keep_out) are only fought from where
    // we stand, never chased into it.
    let (here, range) = (me()?.position()?, me()?.attack_range()?);
    let mut ranked = Vec::with_capacity(enemies.len());
    for enemy in enemies {
        let position = enemy.position()?;
        if distance(here, position) > range && in_enemy_keep_out(position)? {
            continue;
        }
        ranked.push((enemy.health()?, position, enemy));
    }
    if ranked.is_empty() {
        return Ok(None);
    }
    ranked.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1.x, a.1.y).cmp(&(b.1.x, b.1.y)))
    });
    let target = ranked[rank % ranked.len()].2.clone();
    let position = target.position()?;
    fight_squad_enemy_at(&target)?;
    Ok(Some(position))
}

pub fn fight_squad_enemy_in_attack_range() -> Result<Option<Vec2>> {
    let me = me()?;
    let here = me.position()?;
    let range = me.attack_range()?;
    let mut ranked = Vec::new();
    for enemy in &sight()?.enemies {
        let position = enemy.position()?;
        if distance(here, position) <= range && enemy_position_safe_to_target(position)? {
            ranked.push((enemy.health()?, position, enemy.clone()));
        }
    }
    ranked.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1.x, a.1.y).cmp(&(b.1.x, b.1.y)))
    });
    let Some((_, position, target)) = ranked.into_iter().next() else {
        return Ok(None);
    };
    fight_squad_enemy_at(&target)?;
    Ok(Some(position))
}

// A melee enemy this close keeps hitting us in the back while we walk away (to recall, to an
// altar), so the leader stands still and hits it instead. 0 turns this off.
pub const STAND_AGAINST_MELEE_WITHIN: i32 = 1;

// Strike the weakest melee enemy within STAND_AGAINST_MELEE_WITHIN without moving. Enemies on
// their own spawn heal every turn, so trading with them just pins us at the border: they're left
// alone. Returns the enemy's tile if there was one, even if nothing was ready to hit it with.
pub fn fight_adjacent_melee() -> Result<Option<Vec2>> {
    if STAND_AGAINST_MELEE_WITHIN <= 0 {
        return Ok(None);
    }
    let here = me()?.position()?;
    let spawn = cambc::map::spawn_region(Team::Enemy)?;
    let mut target: Option<(f32, Champion)> = None;
    for enemy in &sight()?.enemies {
        let position = enemy.position()?;
        if distance(here, position) > STAND_AGAINST_MELEE_WITHIN
            || enemy.attack_range()? > 1
            || spawn.is_set(position)?
        {
            continue;
        }
        let health = enemy.health()?;
        if target.as_ref().is_none_or(|(weakest, _)| health < *weakest) {
            target = Some((health, enemy.clone()));
        }
    }
    let Some((_, enemy)) = target else {
        return Ok(None);
    };
    let position = enemy.position()?;
    strike(std::slice::from_ref(&enemy), false)?;
    Ok(Some(position))
}

fn fight_squad_enemy_at(target: &Champion) -> Result<()> {
    let me = me()?;
    if FIGHT_OUTSIDE_CAMPS && step_out_of_camps(std::slice::from_ref(target))? {
        strike(std::slice::from_ref(target), true)?;
        return Ok(());
    }
    if strike(std::slice::from_ref(target), false)? {
        return Ok(());
    }
    let range = me.attack_range()?;
    let position = target.position()?;
    if distance(me.position()?, position) <= range || !me.can_move()? {
        return Ok(());
    }
    if DASH_INTO_FIGHTS && dash_into_range(position, range)? {
        strike(std::slice::from_ref(target), true)?;
        return Ok(());
    }
    let max_steps = if me.champion_name()? == ChampionType::Tavi {
        TAVI_COMBAT_MAX_PATH_STEPS
    } else {
        DistanceField::UNREACHABLE
    };
    let moved = move_towards_closest_with_pull_status(
        tiles_around(position, range)?,
        |_| 0.0,
        false,
        max_steps,
    )?
    .0;
    if moved {
        strike(std::slice::from_ref(target), true)?;
    }
    Ok(())
}

// A ready dash closes on the enemy we're fighting when it lands us in attack range of it; otherwise
// we walk. In the squad only Heart of Ruin has one (it lands for 175 damage), and dashes don't count
// as damaging abilities, so strike() never casts them. Off: dashes are only used to travel.
pub const DASH_INTO_FIGHTS: bool = true;

fn dash_into_range(target: Vec2, range: i32) -> Result<bool> {
    let Some(dash) = ready_dash()? else {
        return Ok(false);
    };
    let Some(info) = dash.info()? else {
        return Ok(false);
    };
    let origin = me()?.position()?;
    let mut best: Option<(i32, Vec2)> = None;
    for tile in blocked_tiles()?
        .reachable_tiles(origin, info.as_dyn().range()? as u16)?
        .positions()?
    {
        let gap = distance(tile, target);
        if gap <= range && best.is_none_or(|(b, _)| gap < b) {
            best = Some((gap, tile));
        }
    }
    let Some((_, tile)) = best else {
        return Ok(false);
    };
    // false: an unseen unit blocks the landing tile (see dash_to_best); we walk instead.
    let dashed = match dash.cast_at(tile) {
        Ok(true) => true,
        Ok(false) => {
            remember_hidden_block(tile);
            false
        }
        Err(_) => false,
    };
    forget_sight();
    if dashed {
        trace("dash-in");
    }
    Ok(dashed)
}

// Fighting enemies while an awake camp has us in range means tanking it as well (often we were
// clearing it when they arrived): step to a tile out of every awake camp's range, one with an enemy
// in our attack range if there is one; if no tile is out of range, to the one fewest camps reach
// (giving the camp up). Off: fight where we stand. Off since 2026-10-20: switching it off alone won
// 81/126 against 75/126 against the pre-port squad (more kills), and was even against bot-v2;
// stepping costs Tavi its walk-excluding stun.
pub const FIGHT_OUTSIDE_CAMPS: bool = false;

fn step_out_of_camps(enemies: &[Champion]) -> Result<bool> {
    let me = me()?;
    if !me.can_move()? || enemies.is_empty() {
        return Ok(false);
    }
    let here = me.position()?;
    let mut camps = Vec::new(); // (position, attack range) of awake camps
    for camp in &sight()?.camps {
        if camp.is_awake().unwrap_or(false) {
            camps.push((camp.spawn_position(), camp.attack_range()?));
        }
    }
    let hits = |tile: Vec2| {
        camps
            .iter()
            .filter(|&&(c, r)| distance(c, tile) <= r)
            .count()
    };
    let now = hits(here);
    if now == 0 {
        return Ok(false);
    }
    let range = me.attack_range()?;
    let mut targets = Vec::new();
    for enemy in enemies {
        targets.push(enemy.position()?);
    }
    let blocked = blocked_tiles()?;
    let mut best: Option<((usize, bool, i32), Vec2)> = None;
    for tile in blocked
        .reachable_tiles(here, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        if tile == here {
            continue;
        }
        let in_range = targets.iter().any(|&t| distance(t, tile) <= range);
        let nearest = targets
            .iter()
            .map(|&t| distance(t, tile))
            .min()
            .unwrap_or(0);
        let key = (hits(tile), !in_range, nearest);
        if best.is_none_or(|(k, _)| key < k) {
            best = Some((key, tile));
        }
    }
    let Some(((count, _, _), tile)) = best else {
        return Ok(false);
    };
    if count >= now || !step_to(tile) {
        return Ok(false);
    }
    trace(if count == 0 {
        "fight-step-out-of-camps"
    } else {
        "fight-leave-camp"
    });
    Ok(true)
}

pub fn fight_squad_enemy_at_position(target: Vec2) -> Result<bool> {
    if !enemy_position_safe_to_target(target)? {
        return Ok(false);
    }
    let sight = sight()?;
    for enemy in &sight.enemies {
        if enemy.position()? == target {
            fight_squad_enemy_at(enemy)?;
            return Ok(true);
        }
    }
    Ok(false)
}

// With nothing about to reach us (safety::imminent_danger) and something safe to collect nearby
// (collect::GREEDY), collect it instead of chasing.
fn collect_instead() -> Result<bool> {
    Ok(crate::collect::GREEDY
        && crate::collect::GREEDY_BEFORE_CHASE
        && farm_plan()?.greedy
        && !crate::safety::imminent_danger()?)
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
        if collect_instead()? {
            return Ok(false);
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
        if crate::safety::retreat_from_fight()? {
            strike(enemies, true)?;
            return Ok(true);
        }
        checkpoint("fight-retreat");
        if keep_distance(enemies)? {
            return Ok(true);
        }
    }
    checkpoint("fight-losing-done");
    if strike(enemies, false)? {
        trace("strike");
        return Ok(true);
    }
    checkpoint("fight-strike");
    if crate::collect::capturing()? {
        trace("hold-capture");
        return Ok(false);
    }
    checkpoint("fight-capture");
    if collect_instead()? {
        trace("greedy");
        return Ok(false);
    }
    checkpoint("fight-collect-check");

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
fn kite_melee(enemies: &[Champion]) -> Result<bool> {
    let me = me()?;
    let range = me.attack_range()?;
    if KITE_DISTANCE <= 0 || range <= 1 || !me.can_move()? {
        return Ok(false);
    }
    let mut melee = Vec::new();
    for enemy in enemies {
        if enemy.attack_range()? <= 1 {
            melee.push(enemy.position()?);
        }
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

// Dash while chasing. Against a ranged target only when dash (+ a walking step for melee) puts us
// in attack range this turn: until then we walk, which stops us just outside its range, ready to
// dash in. Against melee targets, or with DASH_CHASE_ALWAYS, dash whenever it gets us closer.
pub const DASH_CHASE: bool = true;
pub const DASH_CHASE_ALWAYS: bool = false;

fn dash_at_chase_target(enemies: &[Champion]) -> Result<bool> {
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
    if enemy.attack_range()? > 1 && !DASH_CHASE_ALWAYS {
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

// Enemies further than this many steps away are never chased; also bounds the pathfinding cost.
const MAX_CHASE_STEPS: u16 = 12;

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
fn refresh_chase_target(enemies: &[Champion]) -> Result<()> {
    let Some(target) = chase_target() else {
        return Ok(());
    };
    let here = me()?.position()?;
    for enemy in enemies {
        if enemy.champion_name()? == target.name {
            let position = enemy.position()?;
            if distance(here, position) >= FIGHT_ENEMY_MAX_DISTANCE {
                return set_chase_target(None);
            }
            return set_chase_target(Some((target.name, position)));
        }
    }
    Ok(())
}

// The chase target is out of sight: walk to where we last saw it, warding that tile if it's
// hidden. Forget it once the memory runs out or we get there without finding it.
fn follow_lost_target() -> Result<bool> {
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

// Ward `spot` (or a free tile next to it) if it is hidden and within placement range. Prefer open
// tiles to bushes, then prefer the tile closest to the remembered spot.
fn ward_hidden_tile(spot: Vec2) -> Result<()> {
    let me = me()?;
    if me.ward_cooldown()? > 0 || cambc::vision::visible_tiles()?.is_set(spot)? {
        return Ok(());
    }
    let radius = cambc::constants().ward_placement_radius as i32;
    let blocked = blocked_tiles()?;
    let bushes = cambc::map::bushes()?;
    let mut best: Option<(Vec2, (bool, i32))> = None;
    for dx in -1..=1 {
        for dy in -1..=1 {
            let tile = Vec2::new(spot.x + dx, spot.y + dy);
            if distance(me.position()?, tile) > radius || blocked.is_set(tile).unwrap_or(true) {
                continue;
            }
            let rank = (!bushes.is_set(tile)?, distance(tile, spot));
            if best.is_none_or(|(_, r)| rank < r) {
                best = Some((tile, rank));
            }
        }
    }
    // An unseen enemy on the tile makes placement fail or error; either way we just walk on.
    if let Some((tile, _)) = best {
        let _ = me.place_ward(tile);
    }
    Ok(())
}

// Keep the current target while we still expect to win against it; otherwise pick the winnable
// enemy with the least health, then the one we can finish fastest (walking turns + turns to kill).
// Returns the next tile to step to, or None if no enemy is worth chasing (or we are already in
// range of the one that is).
fn choose_chase_step(enemies: &[Champion]) -> Result<Option<Vec2>> {
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
        let close = distance(me()?.position()?, enemy.position()?) <= ENGAGE_RADIUS
            && duel.hp_after_walk > 0.0
            && duel.die_turns >= ENGAGE_MIN_SURVIVE_TURNS;
        if !duel.winnable && !close {
            continue;
        }
        if !crate::collect::escort_allows_chase(enemy.position()?)? {
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
        field.compute(
            blocked,
            &tiles_around(origin, 0)?,
            &mut Bitboard::new()?,
            max_steps,
        )?;
        Ok(Self {
            width: field.width(),
            height: field.height(),
            dist: field.export_linear_buffer()?,
        })
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
            // Of the neighbours a step closer, the one nearest the map's centre (centre_bias): the
            // first one found went the same way every time, along walls and edges.
            pos = (-1..=1)
                .flat_map(|dx| (-1..=1).map(move |dy| Vec2::new(pos.x + dx, pos.y + dy)))
                .filter(|&n| self.get(n) == d - 1)
                .min_by(|&a, &b| centre_bias(a).total_cmp(&centre_bias(b)))?;
            path.push(pos);
        }
        path.reverse();
        Some(path)
    }
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
        if distance(threat.origin, duel_tile) <= threat.range || threat.speed > 0 {
            incoming += threat.damage;
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

// Basic attack, then a ready damaging ability, else an attack reset to attack a second time.
// After a move (`moved`), abilities that exclude walking are skipped. Returns whether we acted.
pub fn strike(enemies: &[Champion], moved: bool) -> Result<bool> {
    let mut attacked = None;
    for enemy in enemies {
        if attack_if_in_range(enemy)? {
            attacked = Some(enemy);
            break;
        }
    }

    let me = me()?;
    if !me.can_cast()? {
        return Ok(attacked.is_some());
    }
    for ability in me.abilities() {
        if !is_damaging(ability) || !ability.is_ready()? {
            continue;
        }
        if moved
            && ability
                .info()?
                .is_some_and(|info| info.as_dyn().is_exclusive_with_movement())
        {
            continue;
        }
        for enemy in enemies {
            if cast_at_enemy(ability, enemy)? {
                return Ok(true);
            }
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

pub fn follow_squad_target(target: Vec2) -> Result<()> {
    let here = me()?.position()?;
    let on_camp_tile = is_camp_tile(here)?;
    if distance(here, target) <= 2 && !on_camp_tile {
        return Ok(());
    }
    let mut goals = tiles_around(target, 2)?;
    remove_camp_tiles(&mut goals)?;
    move_towards_closest(goals).map(|_| ())
}

// Ties between equally short steps go towards the middle of the map. Moving in 8 directions there
// are usually several, and the first one listed won, the same way every time, so paths ran along
// walls and the map's edge. The bias is at most CENTRE_TIE_BREAK of a step (straight-line distance
// to the centre, over the map's diagonal), so it never makes a path longer. 0.0: off.
pub const CENTRE_TIE_BREAK: f32 = 0.2;

pub fn centre_bias(tile: Vec2) -> f32 {
    thread_local! {
        static MAP: std::cell::OnceCell<(f32, f32, f32)> = const { std::cell::OnceCell::new() };
    }
    if CENTRE_TIE_BREAK <= 0.0 {
        return 0.0;
    }
    let (cx, cy, diagonal) = MAP.with(|m| {
        *m.get_or_init(|| {
            let walls = cambc::map::walls().ok();
            let (w, h) = walls.map_or((1, 1), |b| (b.width(), b.height()));
            let (w, h) = (w as f32, h as f32);
            ((w - 1.0) / 2.0, (h - 1.0) / 2.0, (w * w + h * h).sqrt().max(1.0))
        })
    });
    let (dx, dy) = (tile.x as f32 - cx, tile.y as f32 - cy);
    CENTRE_TIE_BREAK * (dx * dx + dy * dy).sqrt() / diagonal
}

// Idle near `target`: a random step among the free tiles within `max_distance` of it that take us no
// further from the map's centre (any free one if none do).
pub fn wander_adjacent_near(target: Vec2, max_distance: i32) -> Result<bool> {
    let me = me()?;
    let origin = me.position()?;
    if !me.can_move()? || distance(origin, target) > max_distance {
        return Ok(false);
    }
    let blocked = blocked_tiles()?;
    let mut candidates = Vec::new();
    for dx in -1..=1 {
        for dy in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let tile = Vec2::new(origin.x + dx, origin.y + dy);
            if cambc::map::is_in_bounds(tile)?
                && distance(tile, target) <= max_distance
                && !blocked.is_set(tile)?
            {
                candidates.push(tile);
            }
        }
    }
    if candidates.is_empty() {
        return Ok(false);
    }
    let inward: Vec<Vec2> = candidates
        .iter()
        .copied()
        .filter(|&t| centre_bias(t) <= centre_bias(origin))
        .collect();
    if !inward.is_empty() {
        candidates = inward;
    }
    let start = crate::random_number(candidates.len() as u32) as usize;
    for offset in 0..candidates.len() {
        let tile = candidates[(start + offset) % candidates.len()];
        if me.move_absolute(tile)? {
            return Ok(true);
        }
    }
    Ok(false)
}

// Followers head for the posted target themselves (tiles within their attack range of it, off the
// camp tiles) instead of staying within 2 tiles of Tavi until Tavi has it in range. Packed round
// Tavi they blocked its way (ambush: all four boxed Tavi in from round 104 to 1504), but that's
// now handled by Tavi starting the winnable camp in front of it (ATTACK_ELIGIBLE_IN_RANGE) and
// giving up a target it's stuck on (STUCK_TURNS). Off (2026-10-20, 132 games each against all of
// these off): following Tavi won 70 to 62 with fewer deaths (1740 against 1896); going to the
// target won 67 to 65 but died a third more (2220 against 1671: they fought ahead of Tavi in small
// groups), and with at most 2 tiles' lead (FOLLOWER_MAX_LEAD) 65 to 67.
pub const FOLLOWERS_GO_TO_TARGET: bool = false;
// ... but no more than this many tiles closer to it than Tavi (when we know where Tavi is), so they
// don't run ahead and fight on their own. 0: no limit.
pub const FOLLOWER_MAX_LEAD: i32 = 2;

pub fn farm_squad_target(target: Vec2, tavi_position: Option<Vec2>) -> Result<()> {
    if FOLLOWERS_GO_TO_TARGET {
        if attack_squad_camp(target)? {
            return Ok(());
        }
        if let Some(tavi) = tavi_position.filter(|_| FOLLOWER_MAX_LEAD > 0) {
            if distance(me()?.position()?, target) + FOLLOWER_MAX_LEAD < distance(tavi, target) {
                return Ok(());
            }
        }
        let mut goals = tiles_around(target, me()?.attack_range()?)?;
        remove_camp_tiles(&mut goals)?;
        return move_towards_closest(goals).map(|_| ());
    }
    if is_camp_tile(me()?.position()?)? {
        return follow_squad_target(tavi_position.unwrap_or(target));
    }
    if attack_squad_camp(target)? {
        return Ok(());
    }
    if let Some(position) = tavi_position {
        for ally in &sight()?.allies {
            if ally.champion_name()? == ChampionType::Tavi
                && distance(position, target) <= ally.attack_range()?
            {
                let mut goals = tiles_around(target, me()?.attack_range()?)?;
                remove_camp_tiles(&mut goals)?;
                return move_towards_closest(goals).map(|_| ());
            }
        }
        follow_squad_target(position)
    } else {
        follow_squad_target(target)
    }
}

fn is_camp_tile(tile: Vec2) -> Result<bool> {
    Ok(group_at(tile)?.is_some())
}

fn remove_camp_tiles(goals: &mut Bitboard) -> Result<()> {
    goals.intersect_assign(&farm_statics()?.all.complement()?)
}

fn attack_squad_camp(target: Vec2) -> Result<bool> {
    let me = me()?;
    let Some(camp) = sight()?
        .camps
        .iter()
        .find(|camp| camp.spawn_position() == target)
        .cloned()
    else {
        return Ok(false);
    };
    let mut acted = attack_if_in_range(&camp)?;
    if me.can_cast()? {
        for ability in me.abilities() {
            if ability.is_attack_reset() || !is_damaging(ability) || !ability.is_ready()? {
                continue;
            }
            if cast_at_camp(ability, &camp)? {
                acted = true;
                break;
            }
        }
    }
    Ok(acted)
}

pub fn current_farm_target() -> Option<Vec2> {
    FARM_TARGET_MEMORY.with(|memory| memory.borrow().current)
}

// Whether the current farm target is one we found no path to within TAVI_FARM_MAX_PATH_STEPS.
fn current_target_too_far() -> bool {
    FARM_TARGET_MEMORY.with(|memory| {
        let memory = memory.borrow();
        memory
            .current
            .is_some_and(|target| memory.unreachable.iter().any(|&(tile, _)| tile == target))
    })
}

pub fn farm_target_active(target: Vec2) -> Result<Option<bool>> {
    let Some(index) = group_at(target)? else {
        return Ok(Some(false));
    };
    let group = &camp_groups()?[index];
    let sight = sight()?;
    let mut all_visible = true;
    for camp in group.camps() {
        let spawn = camp.spawn_position();
        if !sight.visible.is_set(spawn)? {
            all_visible = false;
        } else if sight
            .camps
            .iter()
            .any(|live| live.spawn_position() == spawn)
        {
            return Ok(Some(true));
        }
    }
    Ok(all_visible.then_some(false))
}

pub fn claim_camp_reward(target: Vec2) -> Result<bool> {
    let Some(index) = group_at(target)? else {
        return Ok(false);
    };
    let group = &camp_groups()?[index];
    let sight = sight()?;
    let mut capture_tiles = Vec::new();
    for camp in group.camps() {
        let spawn = camp.spawn_position();
        if !sight.visible.is_set(spawn)? {
            return Ok(false);
        }
        if sight
            .camps
            .iter()
            .any(|visible| visible.spawn_position() == spawn)
        {
            return Ok(false);
        }
        capture_tiles.push(spawn);
    }
    if !matches!(group.state()?, cambc::CampGroupState::RewardAvailable) {
        return Ok(false);
    }
    let here = me()?.position()?;
    let Some(destination) = capture_tiles
        .into_iter()
        .min_by_key(|&tile| distance(here, tile))
    else {
        return Ok(false);
    };
    if here != destination {
        let mut goals = Bitboard::new()?;
        goals.set(destination, true)?;
        move_towards_closest(goals)?;
    }
    Ok(true)
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

fn cached_fight_view() -> Result<Option<std::rc::Rc<FightView>>> {
    let key = (cambc::round_number()?, me()?.position()?);
    Ok(FIGHT_VIEW.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|(round, position, _)| (*round, *position) == key)
            .map(|(_, _, view)| view.clone())
    }))
}

// One source of damage: hits any tile within Chebyshev `range` of `origin` for `damage` per turn.
// Enemy champions can walk `speed` tiles a turn towards us; camps never move (speed 0).
#[derive(Clone)]
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
        self.0
            .iter()
            .filter(|t| distance(t.origin, tile) - t.speed * turns as i32 <= t.range)
            .map(|t| t.damage)
            .sum()
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

// Off: the old guess only (see below).
pub const CAMP_AWAKE_QUERY: bool = true;

// Awake camps attack once per round; camps never move, so a live camp sits on its spawn tile. A group
// is awake if any visible member says so (Camp::is_awake). If that query fails, fall back to the old
// guess: awake once a visible member is damaged or missing (dead), which kept counting groups that
// had gone back to sleep after two quiet rounds. Only groups with a camp in sight can hurt us, so
// start from the visible camps.
fn camp_threats(threats: &mut Vec<Threat>) -> Result<()> {
    let sight = sight()?;
    if sight.camps.is_empty() {
        return Ok(());
    }
    let groups = camp_groups()?;
    let mut done = Vec::new();
    for camp in &sight.camps {
        let Some(index) = group_at(camp.spawn_position())? else {
            continue;
        };
        if done.contains(&index) {
            continue;
        }
        done.push(index);
        let mut live = Vec::new();
        let mut awake = false;
        for member in groups[index].camps() {
            let spawn = member.spawn_position();
            match sight.camps.iter().find(|c| c.spawn_position() == spawn) {
                Some(c) => {
                    awake |= match c.is_awake() {
                        Ok(a) if CAMP_AWAKE_QUERY => a,
                        _ => c.health()? < c.max_health()?,
                    };
                    live.push(c);
                }
                // Seen dead: the live members say whether the group is awake (the old guess
                // counted a dead member as awake).
                None => awake |= !CAMP_AWAKE_QUERY && sight.visible.is_set(spawn)?,
            }
        }
        if !awake {
            continue;
        }
        for c in live {
            threats.push(Threat {
                origin: c.spawn_position(),
                range: c.attack_range()?,
                damage: c.strength()?,
                speed: 0,
            });
        }
    }
    Ok(())
}

// The most damage we could take next round where we stand, if everything that can reach us hits at
// once: each enemy champion within its attack range plus a turn of walking lands a basic attack, a
// second one if its attack reset is ready, and its ready damaging abilities in range; every awake
// camp with us in range hits once. For "could we die this round" checks (Heart of Ruin's shield).
pub fn burst_damage_here() -> Result<f32> {
    let me = me()?;
    let here = me.position()?;
    let sight = sight()?;
    let mut burst = 0.0;
    for camp in &sight.camps {
        if camp.is_awake().unwrap_or(false)
            && distance(camp.spawn_position(), here) <= camp.attack_range()?
        {
            burst += camp.strength()?;
        }
    }
    for enemy in &sight.enemies {
        let gap = distance(enemy.position()?, here);
        let walk = enemy.base_stats()?.base_move_speed as i32;
        let range = enemy.attack_range()?;
        if gap > range + walk {
            continue;
        }
        let strength = enemy.strength()?;
        burst += strength;
        for ability in enemy.abilities() {
            if !ability.is_ready().unwrap_or(true) {
                continue;
            }
            if ability.is_attack_reset() {
                burst += strength;
            } else if is_damaging(ability) {
                let reach = ability
                    .info()
                    .ok()
                    .flatten()
                    .and_then(|i| i.as_dyn().range().ok())
                    .map_or(range, |r| r as i32);
                if gap <= reach + walk {
                    burst += ability
                        .damage_amount_against(me)
                        .or_else(|_| ability.damage_amount())
                        .unwrap_or(0.0);
                }
            }
        }
    }
    Ok(burst)
}

// Enemies basic-attack anything within attack range. Abilities are averaged over their cooldown
// (1/8 of a hit per turn) rather than assuming a ready ability is cast now; an attack reset is
// worth 1/8 of an extra basic attack.
fn enemy_threats(threats: &mut Vec<Threat>) -> Result<()> {
    for (_, of_enemy) in enemy_threats_by_name()?.iter() {
        threats.extend(of_enemy.iter().cloned());
    }
    Ok(())
}

// Each visible enemy's threats, read once per round and position (each costs a dozen host calls)
// and shared by the threat scan and the sightings memory (safety::remember_enemies).
pub fn enemy_threats_by_name() -> Result<std::rc::Rc<Vec<(ChampionType, Vec<Threat>)>>> {
    thread_local! {
        static CACHE: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<Vec<(ChampionType, Vec<Threat>)>>)>> = const { std::cell::RefCell::new(None) };
    }
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(cached) = CACHE.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, t)| t.clone())
    }) {
        return Ok(cached);
    }
    let mut all = Vec::new();
    for enemy in &sight()?.enemies {
        let mut threats = Vec::new();
        let Ok(name) = enemy.champion_name() else {
            continue;
        };
        if enemy_threats_of(enemy, &mut threats).is_err() {
            continue;
        }
        all.push((name, threats));
    }
    let all = std::rc::Rc::new(all);
    CACHE.with(|c| *c.borrow_mut() = Some((key.0, key.1, all.clone())));
    Ok(all)
}

pub fn enemy_threats_of(enemy: &Champion, threats: &mut Vec<Threat>) -> Result<()> {
    let me = me()?;
    let origin = enemy.position()?;
    let attack_range = enemy.attack_range()?;
    let strength = enemy.strength()?;
    let speed = if DUEL_ENEMIES_JOIN {
        enemy.base_stats()?.base_move_speed as i32
    } else {
        0
    };
    threats.push(Threat {
        origin,
        range: attack_range,
        damage: strength,
        speed,
    });
    for ability in enemy.abilities() {
        let Some(info) = ability.info()? else {
            continue;
        };
        let per_turn = 1.0 / info.as_dyn().max_cooldown().max(1) as f32;
        if ability.is_attack_reset() {
            threats.push(Threat {
                origin,
                range: attack_range,
                damage: strength * per_turn,
                speed,
            });
        } else if is_damaging(ability) {
            // Engine prediction includes our mitigation and shields; fall back to raw damage.
            let hit = ability
                .damage_amount_against(me)
                .or_else(|_| ability.damage_amount())?;
            threats.push(Threat {
                origin,
                range: info.as_dyn().range()? as i32,
                damage: hit * per_turn,
                speed,
            });
        }
    }
    Ok(())
}

// How much a step closer to our spawn (on foot, spawn_steps) is worth, in steps closer to safety,
// when fleeing. Below 1, every step still makes progress toward the nearest safe tile; above 1,
// heading home can win over it (bot-v2 uses 1.25: on hedgerows its Prism fled through the centre
// and died there). In the squad 1.25 with WALK_HOME_STEPS 5 lost to 0.5 with 0 (2026-10-20: 81/126
// against 75/126 against the pre-port squad, 50/126 against 44/126 against bot-v2), so it stays 0.5.
pub const RETREAT_HOME_PULL: f32 = 0.5;

// The pull towards home for `tile` (see RETREAT_HOME_PULL).
pub fn home_pull(tile: Vec2) -> Result<f32> {
    let steps = match spawn_steps(tile) {
        Some(steps) => steps,
        None => distance(tile, ally_spawn_centre()?),
    };
    Ok(RETREAT_HOME_PULL * steps as f32)
}

// Centre of our spawn region.
pub fn ally_spawn_centre() -> Result<Vec2> {
    spawn_centre(Team::Ally)
}

fn enemy_spawn_centre() -> Result<Vec2> {
    spawn_centre(Team::Enemy)
}

// Spawn centres never change: worked out once per champion (listing the spawn tiles is a host
// call, and banking asked for both centres for every altar).
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

// Walking steps from any tile to our spawn region over fixed terrain, worked out once per champion
// (one uncapped field, ~10-30k fuel, so at the end of a turn with fuel to spare: bot.rs) and copied
// into Rust memory, so lookups cost no host calls.
thread_local! {
    static SPAWN_STEPS: OnceCell<Option<(Vec<u16>, i32)>> = const { OnceCell::new() };
}

pub fn prepare_spawn_steps() {
    SPAWN_STEPS.with(|s| {
        s.get_or_init(|| spawn_field().ok());
    });
}

// Steps from `tile` to our spawn region. None until prepare_spawn_steps has run, or if it failed.
pub fn spawn_steps(tile: Vec2) -> Option<i32> {
    SPAWN_STEPS.with(|s| {
        let (steps, width) = s.get()?.as_ref()?;
        if tile.x < 0 || tile.y < 0 || tile.x >= *width {
            return None;
        }
        let d = *steps.get((tile.y * width + tile.x) as usize)?;
        (d != DistanceField::UNREACHABLE).then_some(d as i32)
    })
}

fn spawn_field() -> Result<(Vec<u16>, i32)> {
    let mut field = DistanceField::new()?;
    field.compute(
        terrain_tiles()?,
        cambc::map::spawn_region(Team::Ally)?,
        &mut Bitboard::new()?,
        DistanceField::UNREACHABLE,
    )?;
    Ok((field.export_linear_buffer()?, field.width()))
}

// Bank only at altars on our side of the map (no further from our spawn than from theirs), unless
// one is within BANK_NEAR_TILES: in bot-v2 the nearest altar sent champions to the one next to the
// enemy base (hedgerows: 18% of Prism's banking walks, and two of its deaths), and Tavi carries
// 200 gems. Off: any altar.
pub const BANK_OWN_SIDE: bool = true;

fn altar_allowed(altar: Vec2, here: Vec2) -> Result<bool> {
    Ok(!BANK_OWN_SIDE
        || distance(here, altar) <= BANK_NEAR_TILES
        || distance(altar, ally_spawn_centre()?) <= distance(altar, enemy_spawn_centre()?))
}

// A lock check is a host call costing ~2.25k (40k a turn over 00lanes' 18 altars), and we only see
// locks near us anyway: check altars within this many tiles.
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
    let mut bank = gems >= BANK_MIN_GEMS;
    let here = me.position()?;
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

// A tank or healer leaves a cleared camp group's capture to an AD carry ally whose attack range
// covers it: capture rewards are split between every ally on the region, so the carry gets all of it.
const TANK_FUNNEL: bool = true;
pub const FUNNEL_HEALERS: bool = true;

// Attack the weakest camp already in range (camps never move, so their spawn tile is their
// position), skipping groups we'd lose to (collect::CAMP_FIGHT): leave those instead.
pub(crate) fn attack_camp_in_range() -> Result<bool> {
    attack_camp_in_range_for_goals(None)
}

fn attack_camp_in_range_for_goals(goals: Option<&Bitboard>) -> Result<bool> {
    let me = me()?;
    if !me.can_attack()? {
        return Ok(false);
    }
    let (position, range) = (me.position()?, me.attack_range()?);
    let sight = sight()?;
    let mut best: Option<(f32, &Camp)> = None;
    for camp in &sight.camps {
        if let Some(goals) = goals {
            if !goals.is_set(camp.spawn_position())? {
                continue;
            }
        }
        if distance(camp.spawn_position(), position) > range {
            continue;
        }
        let Some(index) = group_at(camp.spawn_position())? else {
            continue;
        };
        if crate::collect::camp_fight_winnable(index)? == Some(false) {
            continue;
        }
        let health = camp.health()?;
        if best.is_none_or(|(h, _)| health < h) {
            best = Some((health, camp));
        }
    }
    match best {
        Some((_, camp)) => attack_if_in_range(camp),
        None => Ok(false),
    }
}

// Whether an ally (not us) stands on one of `group`'s camp tiles.
fn ally_on_group(group: &CampGroup) -> Result<bool> {
    for ally in &sight()?.allies {
        let p = ally.position()?;
        if group.camps().iter().any(|c| c.spawn_position() == p) {
            return Ok(true);
        }
    }
    Ok(false)
}

// A cleared group's reward is close enough to capture rather than recall for low health.
pub fn nearby_capture_reward() -> Result<bool> {
    if !crate::collect::GREEDY {
        return Ok(false);
    }
    let me = me()?;
    let here = me.position()?;
    let class = crate::constants::class_of(me.champion_name()?);
    let is_tank = TANK_FUNNEL
        && (class == crate::constants::ClassKind::Tank
            || (FUNNEL_HEALERS && class == crate::constants::ClassKind::Healer));
    let sight = sight()?;
    for group in camp_groups()? {
        if !group
            .camps()
            .iter()
            .any(|camp| distance(camp.spawn_position(), here) <= crate::collect::GREEDY_RADIUS)
        {
            continue;
        }
        let mut seen = false;
        let mut any_live = false;
        for camp in group.camps() {
            let spawn = camp.spawn_position();
            if !sight.visible.is_set(spawn)? {
                continue;
            }
            seen = true;
            if sight
                .camps
                .iter()
                .any(|visible| visible.spawn_position() == spawn)
            {
                any_live = true;
                break;
            }
        }
        if !seen || any_live || ally_on_group(group)? {
            continue;
        }
        if is_tank && carry_covers(group)? {
            continue;
        }
        if matches!(group.state()?, cambc::CampGroupState::RewardAvailable) {
            return Ok(true);
        }
    }
    Ok(false)
}

// Whether an AD carry ally (not us) has any of `group`'s capture tiles within its attack range.
fn carry_covers(group: &CampGroup) -> Result<bool> {
    let my_position = me()?.position()?;
    for ally in &sight()?.allies {
        let position = ally.position()?;
        if position == my_position
            || !crate::constants::class_of(ally.champion_name()?).is_ad_carry()
        {
            continue;
        }
        let range = ally.attack_range()?;
        if group
            .camps()
            .iter()
            .any(|c| distance(c.spawn_position(), position) <= range)
        {
            return Ok(true);
        }
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
// camps by value per turn instead tested worse (writeup entry 20). The filter applies to whole
// groups: skipping single camps abandoned a group whose elk was dead but whose gold cluster wasn't,
// so it could never be captured (and the champion stepped back and forth at its edge of vision).
pub const FARM_PHASE_FILTER: bool = true;
pub const FARM_GEM_PHASE_ROUND: u32 = 700;
pub const FARM_PHASE_BY_GROUP: bool = true; // false: the old per-camp filter

pub fn aggressive_gem_phase(round: u32) -> Result<bool> {
    Ok(round >= FARM_GEM_PHASE_ROUND || cambc::global_submitted_gems(Team::Enemy)? > 600)
}

fn preferred_in_phase(kind: cambc::CampType, round: u32) -> bool {
    use cambc::CampType as C;
    if round < FARM_GEM_PHASE_ROUND {
        !matches!(kind, C::GemCluster | C::Monolith)
    } else {
        kind != C::GoldCluster
    }
}

// The first of our top HOTSPOT_TOP_K draft-planned spots (gold before the gem phase, gems after)
// whose group is available, as an index into camp_groups().
fn hotspot_group(available: &[bool], round: u32) -> Option<usize> {
    use crate::hotspots::{HOTSPOTS, HOTSPOT_MAX_DETOUR, HOTSPOT_START_ROUND, HOTSPOT_TOP_K};
    if !HOTSPOTS || round < HOTSPOT_START_ROUND {
        return None;
    }
    if crate::hotspots::HOTSPOT_AVOID_ENEMIES && !fightable_enemies().ok()?.is_empty() {
        return None;
    }
    let (gold, gems) = crate::hotspots::my_spots()?;
    let spots = if round < FARM_GEM_PHASE_ROUND {
        gold
    } else {
        gems
    };
    let groups = camp_groups().ok()?;
    let here = me().ok()?.position().ok()?;
    // Straight-line distance to the nearest available camp, to cap the detour to a spot.
    let nearest = groups
        .iter()
        .enumerate()
        .filter(|&(i, _)| available[i])
        .flat_map(|(_, g)| g.camps().iter().map(|c| distance(here, c.spawn_position())))
        .min()?;
    spots.iter().take(HOTSPOT_TOP_K).find_map(|&spot| {
        let index = groups.iter().position(|g| {
            g.camps()
                .first()
                .is_some_and(|c| c.spawn_position() == spot)
        })?;
        (available[index] && distance(here, spot) <= nearest + HOTSPOT_MAX_DETOUR).then_some(index)
    })
}

// Don't farm next to enemies we aren't expected to beat: skip groups a visible enemy could reach
// within FARM_ENEMY_TURNS turns (its attack range plus that much walking).
pub const FARM_AVOID_ENEMIES: bool = true;
const FARM_ENEMY_TURNS: i32 = 2;
const FARM_ENEMY_DUEL_FUEL: u64 = 70_000;

// Where farming heads this turn, shared by the fight decision (collect::GREEDY) and farm_camps.
pub struct FarmPlan {
    pub goals: Bitboard,
    pub greedy: bool, // the goals are nearby uncaptured rewards or, when enabled, safe camps
    // Every goal before narrowing to the one target: camps of groups that passed the plan's checks
    // (reachable, not respawning, no enemy about to reach them, a camp fight we'd win).
    pub eligible: Bitboard,
}

const FARM_TARGET_COOLDOWN_ROUNDS: u32 = 2;
const FARM_TARGET_UNREACHABLE_COOLDOWN_ROUNDS: u32 = 20;
const DAMAGED_CAMP_MAX_DETOUR: i32 = 3;
// Tavi farms camp groups within this many steps on foot: one further away is put off for
// FARM_TARGET_UNREACHABLE_COOLDOWN_ROUNDS in favour of nearer ones (long walks away from home with
// a big bag lost games on trenches). If every candidate is that far, Tavi walks to the best of them
// anyway rather than standing still (on ambush it stood still for 600 rounds).
const TAVI_FARM_MAX_PATH_STEPS: u16 = 48;
// Bound failed Tavi routes so unreachable combat goals cannot exhaust fuel.
const TAVI_COMBAT_MAX_PATH_STEPS: u16 = 32;

#[derive(Default)]
struct FarmTargetMemory {
    current: Option<Vec2>,
    abandoned: Vec<(Vec2, u32)>,
    unreachable: Vec<(Vec2, u32)>,
}

thread_local! {
    static FARM_TARGET_MEMORY: std::cell::RefCell<FarmTargetMemory> = std::cell::RefCell::new(FarmTargetMemory::default());
}

fn report_farm_target_change(
    previous: Option<Vec2>,
    target: Option<Vec2>,
    reason: &str,
    round: u32,
) {
    if !FARM_TARGET_DIAGNOSTICS || previous == target || previous.is_none() {
        return;
    }
    let point =
        |tile: Option<Vec2>| tile.map_or_else(|| "none".to_owned(), |p| format!("{},{}", p.x, p.y));
    println!(
        "farm-target-change r{} {} -> {} reason={}",
        round,
        point(previous),
        point(target),
        reason,
    );
}

// Steps taken off a target's travel for the progress it makes towards the enemy base (Tavi pushes
// further out as the game goes on; `enemy_base` is the base and the push per tile of progress).
// Progress only counts down to the edge of the keep-out zone round the enemy spawn
// (enemy_keep_out), and a target inside it costs KEEP_OUT_PENALTY steps per tile it's in: Tavi
// favours the camps on the enemy's side, not the ones next to their base. Unbounded, from round
// 500 Tavi spent 10-13% of its turns three quarters of the way to the enemy spawn or deeper and
// died there twice as often as elsewhere (16% of its deaths within 10 tiles of the enemy spawn).
fn target_push(here: Vec2, target: Vec2, enemy_base: Option<(Vec2, f32)>) -> i32 {
    let Some((base, push)) = enemy_base else {
        return 0;
    };
    let keep_out = enemy_keep_out().unwrap_or(0);
    let depth = |tile: Vec2| distance(tile, base).max(keep_out);
    let progress = depth(here) - depth(target);
    let inside = (keep_out - distance(target, base)).max(0);
    -(push * progress as f32).round() as i32 + KEEP_OUT_PENALTY * inside
}

// The keep-out zone: within this share of the spawn-to-spawn distance of the enemy spawn centre,
// and at least KEEP_OUT_MIN_TILES (never more than half the distance). 0.0 turns it off (the push
// then runs right up to their base). 2026-10-20, 126 games against bot-v2: 62 wins against 50
// without it, 21% more gems, fewer deaths; even (60/126) against the squad without it.
pub const KEEP_OUT_SHARE: f32 = 0.3;
const KEEP_OUT_MIN_TILES: i32 = 10;
const KEEP_OUT_PENALTY: i32 = 3;

// Radius of the keep-out zone round the enemy spawn centre, in tiles (0: none).
pub fn enemy_keep_out() -> Result<i32> {
    if KEEP_OUT_SHARE <= 0.0 {
        return Ok(0);
    }
    let span = distance(ally_spawn_centre()?, enemy_spawn_centre()?);
    Ok(((span as f32 * KEEP_OUT_SHARE).round() as i32)
        .max(KEEP_OUT_MIN_TILES)
        .min(span / 2))
}

// Whether `tile` is in the keep-out zone round the enemy spawn.
pub fn in_enemy_keep_out(tile: Vec2) -> Result<bool> {
    let keep_out = enemy_keep_out()?;
    Ok(keep_out > 0 && distance(tile, enemy_spawn_centre()?) < keep_out)
}

// Farm targets are ranked by walking steps over fixed terrain, not straight-line distance: where a
// line of holes or walls splits the map, a camp a few tiles away can be a long walk round (bot-v2
// on spine: an elk 3 tiles away across the holes was ~47 steps, and 42% of new farm targets were
// such camps, walked to and often given up on). Off: straight-line distance.
pub const WALK_TARGETS: bool = true;
// The field from us stops here; tiles further away rank after every tile within it, by
// straight-line distance.
const FIELD_STEPS: u16 = 60;
// The field only needs to reach the nearest candidate (in a straight line) plus this many steps of
// detour: it costs by the area it covers, ~14k at 60 steps on a 100x100 map against ~7k at 20.
const NEAREST_FIELD_SLACK: i32 = 20;

// Steps from `origin` to any tile, the field built on the first lookup. Any failure falls back to
// straight-line distance.
struct StepsFrom {
    origin: Vec2,
    field: OnceCell<Option<DistanceField>>,
    cap: std::cell::Cell<u16>, // how far the field searches (set before it's built)
}

impl StepsFrom {
    fn new(origin: Vec2) -> Self {
        Self {
            origin,
            field: OnceCell::new(),
            cap: std::cell::Cell::new(FIELD_STEPS),
        }
    }

    // The nearest of `tiles` on foot, with `extra` steps added per tile. Steps are never fewer
    // than the straight-line distance, so one pass works out those lower bounds and only tiles
    // whose bound beats the best found so far are looked up (a lookup is a host call).
    fn nearest_with(&self, tiles: &[Vec2], extra: impl Fn(Vec2) -> i32) -> Option<Vec2> {
        if tiles.len() <= 1 {
            return tiles.first().copied();
        }
        let mut bounds = Vec::with_capacity(tiles.len());
        let mut first = 0;
        for (i, &tile) in tiles.iter().enumerate() {
            let bound = distance(self.origin, tile) + extra(tile);
            if bound < bounds.get(first).copied().unwrap_or(i32::MAX) {
                first = i;
            }
            bounds.push(bound);
        }
        if self.field.get().is_none() {
            let reach = (distance(self.origin, tiles[first]) + NEAREST_FIELD_SLACK)
                .clamp(1, FIELD_STEPS as i32);
            self.cap.set(reach as u16);
        }
        let mut best = (self.steps(tiles[first]) + extra(tiles[first]), first);
        for (i, &bound) in bounds.iter().enumerate() {
            if i != first && bound < best.0 {
                let steps = self.steps(tiles[i]) + extra(tiles[i]);
                if steps < best.0 {
                    best = (steps, i);
                }
            }
        }
        Some(tiles[best.1])
    }

    fn steps(&self, tile: Vec2) -> i32 {
        let straight = distance(self.origin, tile);
        if !WALK_TARGETS {
            return straight;
        }
        let field = self
            .field
            .get_or_init(|| steps_field(self.origin, self.cap.get()).ok());
        match field.as_ref().map(|f| f.get(tile)) {
            Some(Ok(d)) if d != DistanceField::UNREACHABLE => d as i32,
            Some(Ok(_)) => self.cap.get() as i32 + straight,
            _ => straight,
        }
    }
}

fn steps_field(origin: Vec2, cap: u16) -> Result<DistanceField> {
    let mut field = DistanceField::new()?;
    field.compute(
        terrain_tiles()?,
        &tiles_around(origin, 0)?,
        &mut Bitboard::new()?,
        cap,
    )?;
    Ok(field)
}

fn avoid_abandoned_farm_targets(
    goals: &mut Bitboard,
    round: u32,
    here: Vec2,
    reach: i32,
    unavailable_reason: Option<&'static str>,
    enemy_base: Option<(Vec2, f32)>,
) -> Result<()> {
    let candidates = goals.positions()?;
    let Some(candidates_for_target) = (!candidates.is_empty()).then_some(candidates) else {
        let previous = FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().current.take());
        report_farm_target_change(
            previous,
            None,
            unavailable_reason.unwrap_or("no-eligible-farm-goals"),
            round,
        );
        return Ok(());
    };

    let previous = FARM_TARGET_MEMORY.with(|memory| memory.borrow().current);
    let steps = StepsFrom::new(here);
    let best =
        |tiles: &[Vec2]| steps.nearest_with(tiles, |tile| target_push(here, tile, enemy_base));
    let (target, changed_reason) = FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        memory.abandoned.retain(|&(_, until)| round < until);
        memory.unreachable.retain(|&(_, until)| round < until);
        let previous_is_abandoned =
            previous.is_some_and(|old| memory.abandoned.iter().any(|&(tile, _)| tile == old));
        let previous_is_unreachable =
            previous.is_some_and(|old| memory.unreachable.iter().any(|&(tile, _)| tile == old));
        let not_unreachable: Vec<Vec2> = candidates_for_target
            .iter()
            .copied()
            .filter(|&tile| !memory.unreachable.iter().any(|&(old, _)| old == tile))
            .collect();
        let available: Vec<Vec2> = not_unreachable
            .iter()
            .copied()
            .filter(|&tile| !memory.abandoned.iter().any(|&(old, _)| old == tile))
            .collect();
        let target = if not_unreachable.is_empty() {
            // Every candidate is too far (or blocked), and they're all walkable (farm_statics), so
            // keep heading for one rather than stand still: the current target if it's still a
            // candidate, else the best. farm_camps walks to it uncapped.
            memory
                .current
                .filter(|tile| candidates_for_target.contains(tile))
                .or_else(|| best(&candidates_for_target))
        } else if available.is_empty() {
            memory.abandoned.clear();
            best(&not_unreachable)
        } else {
            let current_is_reachable = memory
                .current
                .is_some_and(|tile| distance(here, tile) <= reach);
            if !current_is_reachable {
                if let Some(current) = memory.current.filter(|tile| available.contains(tile)) {
                    Some(current)
                } else {
                    best(&available)
                }
            } else {
                best(&available)
            }
        };
        let reason = previous.filter(|&old| Some(old) != target).map(|old| {
            if !candidates_for_target.contains(&old) {
                unavailable_reason.unwrap_or("previous-target-not-in-farm-goals")
            } else if previous_is_unreachable {
                "previous-target-unreachable"
            } else if previous_is_abandoned {
                "previous-target-cooldown"
            } else if distance(here, old) <= reach {
                "previous-target-within-attack-range"
            } else {
                "closer-available-target"
            }
        });
        (target, reason)
    });

    FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        if memory.current != target {
            if let Some(previous) = memory.current {
                if distance(here, previous) > reach {
                    memory
                        .abandoned
                        .push((previous, round + FARM_TARGET_COOLDOWN_ROUNDS));
                }
            }
            memory.current = target;
        }
    });

    for tile in candidates_for_target.iter().copied() {
        if Some(tile) == target {
            continue;
        }
        if goals.is_set(tile)? {
            goals.set(tile, false)?;
        }
    }
    if goals.is_empty()? {
        let all_unreachable = FARM_TARGET_MEMORY.with(|memory| {
            let memory = memory.borrow();
            candidates_for_target.iter().all(|tile| {
                memory
                    .unreachable
                    .iter()
                    .any(|&(unreachable, _)| unreachable == *tile)
            })
        });
        if !all_unreachable {
            for tile in candidates_for_target {
                goals.set(tile, true)?;
            }
            FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().abandoned.clear());
        }
    }
    if let Some(reason) = changed_reason {
        report_farm_target_change(previous, target, reason, round);
    }
    Ok(())
}

thread_local! {
    static FARM_PLAN: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<FarmPlan>)>> = const { std::cell::RefCell::new(None) };
}

// What farm_plan needs about each group that never changes, computed once. Pure Rust work is
// metered too (looping over every camp of every group several times a turn cost ~30k fuel), so a
// turn only looks at the camps of groups close enough to be in sight.
struct GroupInfo {
    centre: Vec2,
    radius: i32,          // furthest camp from the centre
    preferred: [bool; 2], // has a camp preferred before / from FARM_GEM_PHASE_ROUND
    gold_only: bool,
    reachable: bool, // a camp tile we can walk to from our spawn
}

struct FarmStatics {
    groups: Vec<GroupInfo>,
    all: Bitboard,            // every camp's tile
    preferred: [Bitboard; 2], // camps of preferred groups, per phase
}

// Tiles we can walk to from our spawn, whatever stands on them: walls, holes, altars and the enemy
// spawn block. None if we have no spawn tile (then every group counts as reachable).
fn walkable_from_spawn() -> Result<Option<Bitboard>> {
    let Some(&spawn) = cambc::map::spawn_region(Team::Ally)?.positions()?.first() else {
        return Ok(None);
    };
    Ok(Some(terrain_tiles()?.reachable_tiles(spawn, 1000)?))
}

fn farm_statics() -> Result<&'static FarmStatics> {
    thread_local! {
        static STATICS: OnceCell<&'static FarmStatics> = const { OnceCell::new() };
    }
    if let Some(statics) = STATICS.with(|c| c.get().copied()) {
        return Ok(statics);
    }
    let mut statics = FarmStatics {
        groups: Vec::new(),
        all: Bitboard::new()?,
        preferred: [Bitboard::new()?, Bitboard::new()?],
    };
    // Only Heart of Ruin can dash, and the squad goes where Tavi walks, so a group cut off by holes
    // or walls can never be captured, and it's never a goal. On ambush Tavi kept picking the two
    // sealed gem rooms, found no path and stood still for a turn per camp tile, every time the
    // unreachable memory ran out.
    let walkable = walkable_from_spawn()?;
    for group in camp_groups()? {
        let mut reachable = walkable.is_none();
        if let Some(walkable) = &walkable {
            for camp in group.camps() {
                reachable |= walkable.is_set(camp.spawn_position())?;
            }
        }
        let n = group.camps().len().max(1) as i32;
        let (x, y) = group.camps().iter().fold((0, 0), |(x, y), c| {
            (x + c.spawn_position().x, y + c.spawn_position().y)
        });
        let centre = Vec2::new(x / n, y / n);
        let radius = group
            .camps()
            .iter()
            .map(|c| distance(c.spawn_position(), centre))
            .max()
            .unwrap_or(0);
        let mut preferred = [false; 2];
        let gold_only = group
            .camps()
            .iter()
            .all(|camp| camp.kind() == cambc::CampType::GoldCluster);
        for (phase, round) in [0, FARM_GEM_PHASE_ROUND].into_iter().enumerate() {
            preferred[phase] = group
                .camps()
                .iter()
                .any(|c| preferred_in_phase(c.kind(), round));
        }
        for camp in group.camps().iter().filter(|_| reachable) {
            statics.all.set(camp.spawn_position(), true)?;
            for (phase, round) in [0, FARM_GEM_PHASE_ROUND].into_iter().enumerate() {
                let keep = if FARM_PHASE_BY_GROUP {
                    preferred[phase]
                } else {
                    preferred_in_phase(camp.kind(), round)
                };
                if keep {
                    statics.preferred[phase].set(camp.spawn_position(), true)?;
                }
            }
        }
        statics.groups.push(GroupInfo {
            centre,
            radius,
            preferred,
            gold_only,
            reachable,
        });
    }
    let statics: &'static FarmStatics = Box::leak(Box::new(statics));
    STATICS.with(|c| {
        let _ = c.set(statics);
    });
    Ok(statics)
}

// Plan farming: the nearest camps that aren't respawning (including ones we saw respawning
// earlier), skipping groups we'd lose to (collect::CAMP_FIGHT) or that enemies are next to. Safe
// groups nearby come first (collect::GREEDY), and a capture we're standing on is held. Tanks and
// healers still fight camps but leave the capture of a cleared group to a covering AD carry (see
// TANK_FUNNEL). Cached per round and position.
pub fn farm_plan() -> Result<std::rc::Rc<FarmPlan>> {
    let here = me()?.position()?;
    let round = cambc::round_number()?;
    if let Some(plan) = FARM_PLAN.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == (round, here))
            .map(|(_, _, p)| p.clone())
    }) {
        return Ok(plan);
    }
    let statics = farm_statics()?;
    let class = crate::constants::class_of(me()?.champion_name()?);
    let is_tank = TANK_FUNNEL
        && (class == crate::constants::ClassKind::Tank
            || (FUNNEL_HEALERS && class == crate::constants::ClassKind::Healer));
    let sight = sight()?;
    let groups = camp_groups()?;
    let stats = me()?.base_stats()?;
    let is_tavi = me()?.champion_name()? == ChampionType::Tavi;
    let skip_gold_only =
        is_tavi && stats.base_strength > 0.0 && me()?.strength()? >= stats.base_strength * 2.0;
    // Enemy reach: (position, attack range + FARM_ENEMY_TURNS turns of walking).
    let mut potential_enemy_reach = Vec::new();
    if FARM_AVOID_ENEMIES {
        for enemy in &sight.enemies {
            potential_enemy_reach.push((
                enemy.clone(),
                enemy.position()?,
                enemy.attack_range()?
                    + enemy.base_stats()?.base_move_speed as i32 * FARM_ENEMY_TURNS,
            ));
        }
    }
    let potentially_threatens_farm = potential_enemy_reach.iter().any(|(_, position, reach)| {
        groups.iter().any(|group| {
            group
                .camps()
                .iter()
                .any(|camp| distance(*position, camp.spawn_position()) <= *reach)
        })
    });
    let duel_view = if potentially_threatens_farm {
        match cached_fight_view()? {
            Some(view) => Some(view),
            None if cambc::fuel_remaining()? >= FARM_ENEMY_DUEL_FUEL => Some(fight_view()?),
            None => None,
        }
    } else {
        None
    };
    let mut enemy_reach = Vec::new();
    for (enemy, position, reach) in potential_enemy_reach {
        let enemy_is_winnable = if let Some(view) = &duel_view {
            plan_duel(&enemy, &view.threats, &view.allies, &view.from_me)?
                .is_some_and(|duel| duel.winnable)
        } else {
            true
        };
        if !enemy_is_winnable {
            enemy_reach.push((position, reach));
        }
    }
    let max_reach = enemy_reach.iter().map(|&(_, r)| r).max().unwrap_or(0);
    let vision = me()?.base_stats()?.vision_radius as i32;
    let phase = usize::from(aggressive_gem_phase(round)?);
    let previous_target = FARM_TARGET_MEMORY.with(|memory| memory.borrow().current);
    let previous_group = match previous_target {
        Some(tile) => group_at(tile)?,
        None => None,
    };
    let mut previous_target_reason = None;
    let mut damaged_camps = Vec::new();
    for camp in &sight.camps {
        if camp.health()? < camp.max_health()? {
            if let Some(index) = group_at(camp.spawn_position())? {
                damaged_camps.push((index, camp.spawn_position()));
            }
        }
    }
    let mut ready = RESPAWN_READY.with(|cell| cell.take());
    ready.resize(groups.len(), 0);
    let mut available = vec![false; groups.len()];
    let mut dropped = Vec::new(); // groups that aren't goals this turn
    let mut not_goals = Vec::new(); // dead camps of a group that's still partly alive
    let mut damaged = Vec::new();
    let mut greedy = Vec::new();
    let mut preferred_left = 0;
    let mut holding = false;
    for (index, group) in groups.iter().enumerate() {
        let info = &statics.groups[index];
        if !info.reachable {
            continue;
        }
        if skip_gold_only && info.gold_only {
            if previous_group == Some(index) {
                previous_target_reason = Some("tavi-strong-gold-only-group");
            }
            dropped.push(index);
            continue;
        }
        // Nearest any of its camps can be.
        let gap = distance(info.centre, here) - info.radius;
        let mut seen = false;
        let mut any_live = false;
        let mut seen_dead = Vec::new(); // camps we can see are gone
        if gap <= vision {
            for camp in group.camps() {
                let spawn = camp.spawn_position();
                if !sight.visible.is_set(spawn)? {
                    continue;
                }
                seen = true;
                if sight.camps.iter().any(|c| c.spawn_position() == spawn) {
                    any_live = true;
                } else {
                    seen_dead.push(spawn);
                }
            }
        }
        if any_live {
            ready[index] = 0;
        }
        if round < ready[index] {
            if previous_group == Some(index) {
                previous_target_reason = Some("group-respawning");
            }
            dropped.push(index);
            continue;
        }
        // Only a seen group with no live camps can be respawning or waiting for capture.
        let mut reward = false;
        if seen && !any_live {
            match group.state()? {
                cambc::CampGroupState::Respawning(timer) => {
                    ready[index] = round + timer.max(0) as u32;
                    if previous_group == Some(index) {
                        previous_target_reason = Some("group-respawning");
                    }
                    dropped.push(index);
                    continue;
                }
                cambc::CampGroupState::RewardAvailable if is_tank && carry_covers(group)? => {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("ally-carry-captures-reward");
                    }
                    dropped.push(index);
                    continue;
                }
                // An ally already on the capture region is collecting this reward; don't path to
                // the occupied capture tile or compete for the split.
                cambc::CampGroupState::RewardAvailable if ally_on_group(group)? => {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("ally-captures-reward");
                    }
                    dropped.push(index);
                    continue;
                }
                cambc::CampGroupState::RewardAvailable => reward = true,
                _ => {}
            }
        }
        let unwinnable = any_live && crate::collect::camp_fight_winnable(index)? == Some(false);
        if unwinnable && previous_group == Some(index) {
            previous_target_reason = Some("camp-fight-unwinnable");
        }
        // Some members dead but the group not cleared: the rest are alive out of our sight (a bush).
        let hidden_alive = crate::collect::PARTIAL_GROUPS && seen && !any_live && !reward;
        // While part of the group is alive, only its live (or unseen) camps are goals. A dead camp's
        // tile only becomes a goal once the whole group is cleared (standing there captures it):
        // otherwise a melee champion standing on one counts as having arrived and never walks over
        // to the live camp out of reach (seen stuck for 65 turns). With its live members out of
        // sight, its goals are the tiles we can't see, to find them.
        let is_goal =
            |c: &Camp| !((any_live || hidden_alive) && seen_dead.contains(&c.spawn_position()));
        if gap <= max_reach + vision
            && !enemy_reach.is_empty()
            && group.camps().iter().filter(|c| is_goal(c)).any(|c| {
                enemy_reach
                    .iter()
                    .any(|&(p, r)| distance(p, c.spawn_position()) <= r)
            })
        {
            if previous_group == Some(index) {
                previous_target_reason = Some("enemy-can-reach-group");
            }
            dropped.push(index);
            continue;
        }
        if unwinnable {
            dropped.push(index);
            continue;
        }
        if let Some(tile) = previous_target.filter(|tile| {
            group
                .camps()
                .iter()
                .any(|camp| camp.spawn_position() == *tile)
        }) {
            if (any_live || hidden_alive) && seen_dead.contains(&tile) {
                previous_target_reason = Some("camp-dead-while-group-still-active");
            }
        }
        if reward && group.camps().iter().any(|c| c.spawn_position() == here) {
            holding = crate::collect::CAPTURE_HOLD;
        }
        if any_live || hidden_alive {
            not_goals.extend(seen_dead.iter().copied());
        }
        if hidden_alive {
            trace("group-hidden-alive");
        }
        available[index] = true;
        if info.preferred[phase] {
            preferred_left += 1;
        }
        if gap <= crate::collect::GREEDY_RADIUS {
            damaged.extend(
                damaged_camps
                    .iter()
                    .filter(|&&(group_index, tile)| {
                        group_index == index
                            && distance(tile, here) <= crate::collect::GREEDY_RADIUS
                    })
                    .map(|&(_, tile)| tile),
            );
        }
        if crate::collect::GREEDY
            && gap <= crate::collect::GREEDY_RADIUS
            && (reward || crate::collect::safe_group(index)?)
        {
            for camp in group.camps().iter().filter(|c| is_goal(c)) {
                if distance(camp.spawn_position(), here) <= crate::collect::GREEDY_RADIUS {
                    greedy.push(camp.spawn_position());
                }
            }
        }
    }
    RESPAWN_READY.with(|cell| cell.replace(ready));

    let mut is_greedy = false;
    let mut goals;
    let selection_reason;
    if holding {
        selection_reason = "capture-hold";
        goals = Bitboard::new()?;
        goals.set(here, true)?;
    } else if !greedy.is_empty() {
        selection_reason = "nearby-greedy-goal";
        is_greedy = true;
        goals = Bitboard::new()?;
        for &tile in &greedy {
            goals.set(tile, true)?;
        }
    } else if let Some(index) = hotspot_group(&available, round) {
        selection_reason = "hotspot-priority";
        // Past the early game, head for our highest-ranked available hot spot (see hotspots.rs).
        goals = Bitboard::new()?;
        for camp in groups[index].camps() {
            goals.set(camp.spawn_position(), true)?;
        }
    } else {
        selection_reason = if FARM_PHASE_FILTER && preferred_left > 0 {
            "preferred-phase-goals"
        } else {
            "ordinary-farm-goals"
        };
        // Every camp (of the phase's preferred groups, if any is available), minus what's dropped.
        goals = if FARM_PHASE_FILTER && preferred_left > 0 {
            statics.preferred[phase].clone()
        } else {
            statics.all.clone()
        };
        for index in dropped {
            for camp in groups[index].camps() {
                goals.set(camp.spawn_position(), false)?;
            }
        }
        for tile in not_goals.iter().copied() {
            goals.set(tile, false)?;
        }
    }
    let previous_was_regular_goal = match previous_target {
        Some(tile) => goals.is_set(tile)?,
        None => false,
    };
    if !holding && !damaged.is_empty() {
        let nearest_regular = goals
            .positions()?
            .iter()
            .map(|&tile| distance(here, tile))
            .min();
        let nearest_damaged = damaged
            .iter()
            .copied()
            .min_by_key(|&tile| distance(here, tile));
        let damaged_distance = nearest_damaged.map_or(i32::MAX, |tile| distance(here, tile));
        if nearest_regular
            .is_none_or(|regular| damaged_distance <= regular + DAMAGED_CAMP_MAX_DETOUR)
        {
            is_greedy = true;
            goals = Bitboard::new()?;
            if let Some(tile) = nearest_damaged {
                goals.set(tile, true)?;
            }
            if previous_was_regular_goal {
                if let Some(old) = previous_target.filter(|old| Some(*old) != nearest_damaged) {
                    previous_target_reason = Some(if damaged.contains(&old) {
                        "closer-damaged-camp"
                    } else {
                        "damaged-camp-priority"
                    });
                }
            }
        } else if previous_was_regular_goal
            && previous_target.is_some_and(|old| damaged.contains(&old))
        {
            previous_target_reason = Some("damaged-camp-too-far-from-regular-target");
        }
    }
    if !holding && damaged.is_empty() {
        if let Some(old) = previous_target {
            if !not_goals.contains(&old)
                && group_at(old)?.is_some_and(|index| available[index])
                && !goals.is_set(old)?
            {
                goals.set(old, true)?;
            }
        }
    }
    if let Some(old) = previous_target {
        if !goals.is_set(old)? && previous_target_reason.is_none() {
            previous_target_reason = Some(selection_reason);
        }
    }
    let eligible = goals.clone();
    avoid_abandoned_farm_targets(
        &mut goals,
        round,
        here,
        me()?.attack_range()?,
        previous_target_reason,
        if me()?.champion_name()? == ChampionType::Tavi {
            let progress = (round as f32 / 500.0).min(1.0);
            Some((enemy_spawn_centre()?, 0.75 * progress))
        } else {
            None
        },
    )?;
    if goals.is_empty()? && !holding {
        for (index, group) in groups.iter().enumerate() {
            if !available[index] {
                continue;
            }
            for camp in group.camps() {
                let tile = camp.spawn_position();
                if !not_goals.contains(&tile) {
                    goals.set(tile, true)?;
                }
            }
        }
        if !goals.is_empty()? {
            avoid_abandoned_farm_targets(
                &mut goals,
                round,
                here,
                me()?.attack_range()?,
                Some("unreachable-farm-goal"),
                if is_tavi {
                    let progress = (round as f32 / 500.0).min(1.0);
                    Some((enemy_spawn_centre()?, 0.75 * progress))
                } else {
                    None
                },
            )?;
        }
    }
    if !holding && damaged.is_empty() {
        is_greedy = goals.positions()?.iter().any(|tile| greedy.contains(tile));
    }
    let plan = std::rc::Rc::new(FarmPlan {
        goals,
        greedy: is_greedy,
        eligible,
    });
    FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
    Ok(plan)
}

// Goals we found no path to (too far, or units standing in a corridor), with the rest of their camp
// groups: whatever rules out one camp rules out its neighbours too, and marking a tile at a time
// cost a turn standing still per camp tile (trenches: 4 turns per group, every 20 rounds).
fn remember_unreachable_farm_goals(goals: &Bitboard, round: u32) -> Result<()> {
    let until = round.saturating_add(FARM_TARGET_UNREACHABLE_COOLDOWN_ROUNDS);
    let groups = camp_groups()?;
    let mut targets = Vec::new();
    for tile in goals.positions()? {
        match group_at(tile)? {
            Some(index) => targets.extend(groups[index].camps().iter().map(|c| c.spawn_position())),
            None => targets.push(tile),
        }
    }
    FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        memory.unreachable.retain(|&(_, expires)| round < expires);
        for target in targets {
            if !memory
                .unreachable
                .iter()
                .any(|&(unreachable, _)| unreachable == target)
            {
                memory.unreachable.push((target, until));
            }
        }
    });
    Ok(())
}

// With less than this much fuel left, building a plan left too little for the move itself, where
// turns most often ran out of fuel in bot-v2 (the champion then stands still). Keep walking to the
// last plan's goals instead, if it's from the last STALE_PLAN_ROUNDS rounds. It only ever reuses
// the last full plan's own goals, so a low-fuel turn can't flip the target back and forth. 0: off.
pub const FARM_PLAN_MIN_FUEL: u64 = 60_000;
const STALE_PLAN_ROUNDS: u32 = 10;

pub fn farm_plan_or_stale() -> Result<std::rc::Rc<FarmPlan>> {
    if FARM_PLAN_MIN_FUEL > 0 && cambc::fuel_remaining()? < FARM_PLAN_MIN_FUEL {
        let round = cambc::round_number()?;
        if let Some(plan) = FARM_PLAN.with(|c| {
            c.borrow()
                .as_ref()
                .filter(|(built, _, _)| round < built + STALE_PLAN_ROUNDS)
                .map(|(_, _, plan)| plan.clone())
        }) {
            trace("plan-low-fuel");
            return Ok(plan);
        }
    }
    farm_plan()
}

pub const ATTACK_ELIGIBLE_IN_RANGE: bool = true;
// 0: never give a target up for being stuck.
const STUCK_TURNS: u32 = 5;

thread_local! {
    // Where we've stood without a step towards the farm goals, and for how many turns.
    static STUCK: std::cell::Cell<Option<(Vec2, u32)>> = const { std::cell::Cell::new(None) };
}

// Clear camps and capture their rewards, following farm_plan.
pub fn farm_camps() -> Result<()> {
    let plan = farm_plan_or_stale()?;
    if plan.goals.is_empty()? {
        return Ok(());
    }
    let mut attacked = attack_camp_in_range_for_goals(Some(&plan.goals))?;
    // A camp in range we'd win but that isn't the current target: start it rather than walk past
    // (ambush: Tavi stood 3 tiles from a winnable elk group for 1,400 rounds, its target further on).
    // The next plan then takes the damaged camp as its target. Off: only the target's camps.
    if !attacked && ATTACK_ELIGIBLE_IN_RANGE {
        attacked = attack_camp_in_range_for_goals(Some(&plan.eligible))?;
        if attacked {
            trace("farm-attack-eligible");
        }
    }
    if attacked {
        STUCK.with(|s| s.set(None));
        cast_abilities_on_camps(false, Some(&plan.goals))?;
        return Ok(());
    }
    // Tavi keeps to nearby groups (TAVI_FARM_MAX_PATH_STEPS), unless the target is one already found
    // too far: then every candidate was (avoid_abandoned_farm_targets), so the walk is uncapped.
    let capped = me()?.champion_name()? == ChampionType::Tavi && !current_target_too_far();
    let max_steps = if capped {
        TAVI_FARM_MAX_PATH_STEPS
    } else {
        DistanceField::UNREACHABLE
    };
    // Camps don't move: the ones in sight before the step are the ones we might reach after it.
    let camps_before: Vec<Vec2> = sight()?.camps.iter().map(|c| c.spawn_position()).collect();
    let (moved, pathable) = move_towards_closest_with_path_status(plan.goals.clone(), max_steps)?;
    if !moved && pathable == Some(false) && capped {
        remember_unreachable_farm_goals(&plan.goals, cambc::round_number()?)?;
    }
    // A path exists but no step gets us closer (units in the way): after STUCK_TURNS such turns in
    // a row on one tile, give the target up as unreachable for now, so the next plan picks another.
    if moved || pathable != Some(true) {
        STUCK.with(|s| s.set(None));
    } else {
        let here = me()?.position()?;
        let turns = STUCK.with(|s| match s.get() {
            Some((tile, n)) if tile == here => n + 1,
            _ => 1,
        });
        STUCK.with(|s| s.set(Some((here, turns))));
        if STUCK_TURNS > 0 && turns >= STUCK_TURNS {
            trace("farm-stuck");
            remember_unreachable_farm_goals(&plan.goals, cambc::round_number()?)?;
            STUCK.with(|s| s.set(None));
        }
    }
    // Attack after the step only with a camp in range of where we now stand: the check rebuilds
    // sight and the camp-fight context for the new tile (~16k fuel in bot-v2).
    if moved {
        let me = me()?;
        let (here, range) = (me.position()?, me.attack_range()?);
        if camps_before.iter().any(|&c| distance(c, here) <= range)
            && attack_camp_in_range_for_goals(Some(&plan.goals))?
        {
            cast_abilities_on_camps(true, Some(&plan.goals))?;
        }
    }
    Ok(())
}

// Ward a hidden tile next to us (a bush, or behind a wall) so we see into it: bushes first, then
// the furthest from us (the ward sees 6 tiles round it). Wards are free and recharge in 10 rounds
// (since the balance patch); we keep our current one while it's within WARD_KEEP_TILES of us.
// Tried 2026-10-20: warding only where the ward would show 8+ hidden tiles (wall shadows, bushes
// we aren't in; engine ward-vision query), with the same fuel use: 79 fewer wards a game cost
// bot-v2 55 wins against 71 for this rule (126 games), the squad 61 against 65. The extra vision
// from the cheap wards is worth more than saving the charge.
pub const WARD_HIDDEN: bool = true;
const WARD_KEEP_TILES: i32 = 5;

pub fn ward_hidden_spots() -> Result<()> {
    let me = me()?;
    if !WARD_HIDDEN || me.ward_cooldown()? > 0 {
        return Ok(());
    }
    let here = me.position()?;
    if me
        .ward_positions()?
        .iter()
        .any(|&w| distance(w, here) <= WARD_KEEP_TILES)
    {
        return Ok(());
    }
    checkpoint("ward-search-start");
    let radius = cambc::constants().ward_placement_radius as i32;
    let sight = sight()?;
    let visible = &sight.visible;
    let (bushes, walls, holes) = (
        cambc::map::bushes()?,
        cambc::map::walls()?,
        cambc::map::holes()?,
    );
    let mut best: Option<(Vec2, (bool, i32))> = None;
    for dx in -radius..=radius {
        for dy in -radius..=radius {
            let tile = Vec2::new(here.x + dx, here.y + dy);
            if !cambc::map::is_in_bounds(tile)?
                || visible.is_set(tile)?
                || walls.is_set(tile)?
                || holes.is_set(tile)?
            {
                continue;
            }
            let rank = (bushes.is_set(tile)?, distance(tile, here));
            if best.is_none_or(|(_, r)| rank > r) {
                best = Some((tile, rank));
            }
        }
    }
    checkpoint("ward-search-done");
    // An unseen unit on the tile makes placement fail or error; either way we just carry on. The
    // new ward replaces the old one, so what we see changes.
    if let Some((tile, _)) = best {
        if me.place_ward(tile).unwrap_or(false) {
            trace("ward");
        }
        forget_sight();
    }
    checkpoint("ward-place-done");
    Ok(())
}
