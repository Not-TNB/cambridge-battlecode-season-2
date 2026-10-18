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
thread_local! {
    static BLOCKED: std::cell::RefCell<Option<(u32, Vec2, Bitboard)>> = const { std::cell::RefCell::new(None) };
}

pub fn blocked_tiles() -> Result<Bitboard> {
    thread_local! {
        static TERRAIN: OnceCell<&'static Bitboard> = const { OnceCell::new() };
    }
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(blocked) = BLOCKED.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, b)| b.clone())
    }) {
        return Ok(blocked);
    }
    if TERRAIN.with(|c| c.get().is_none()) {
        let mut terrain = cambc::map::walls()?.clone();
        terrain.union_assign(cambc::map::holes()?)?;
        terrain.union_assign(cambc::map::altars()?)?;
        terrain.union_assign(cambc::map::spawn_region(Team::Enemy)?)?;
        let terrain: &'static Bitboard = Box::leak(Box::new(terrain));
        TERRAIN.with(|c| {
            let _ = c.set(terrain);
        });
    }
    let mut blocked = TERRAIN.with(|c| Bitboard::clone(c.get().expect("set above")));
    for mask in [
        cambc::vision::champion_tiles(None)?,
        cambc::vision::camp_tiles()?,
        cambc::vision::teleport_tiles()?,
    ] {
        blocked.union_assign(&mask)?;
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
    let dashed = dash.cast_at(best.0).unwrap_or(false);
    forget_sight();
    Ok(dashed)
}

// Take one ordinary step. Our occupancy view can lag a unit that just appeared (e.g. right after a
// dash), and stepping onto a known occupied tile is an error that changes nothing; treat it as not
// moving instead of ending the turn.
fn step_to(tile: Vec2) -> bool {
    me().and_then(|me| me.move_absolute(tile)).unwrap_or(false)
}

pub fn move_towards_closest(goals: Bitboard) -> Result<bool> {
    let may_dash = DASH_TRAVEL_MIN_STEPS > 0 && fightable_enemies()?.is_empty();
    move_towards_closest_with_pull(goals, |_| 0.0, may_dash)
}

// Like move_towards_closest, but each candidate step also pays `pull(tile)` (in steps), so among
// steps that get similarly close to the goals we prefer the one `pull` likes best. With `may_dash`,
// a ready dash first jumps along the way when the goals are DASH_TRAVEL_MIN_STEPS or more away.
pub fn move_towards_closest_with_pull(
    mut goals: Bitboard,
    pull: impl Fn(Vec2) -> f32,
    may_dash: bool,
) -> Result<bool> {
    let me = me()?;
    let origin = me.position()?;
    if !me.can_move()? || goals.is_set(origin)? {
        return Ok(false);
    }

    let mut blocked = blocked_tiles()?;
    checkpoint("path-blocked");
    goals.intersect_assign(&blocked.complement()?.union(&cambc::vision::camp_tiles()?)?)?;
    if goals.is_empty()? {
        return Ok(false);
    }

    let mut field = DistanceField::new()?;
    field.compute_to(
        &blocked.intersect(&goals.complement()?)?,
        &goals,
        &tiles_around(origin, 0)?,
        &mut Bitboard::new()?,
        DistanceField::UNREACHABLE,
    )?;
    checkpoint("path-field");

    let score = |tile: Vec2| -> Result<f32> {
        let d = field.get(tile)?;
        Ok(if d == DistanceField::UNREACHABLE {
            f32::MAX
        } else {
            d as f32 + pull(tile)
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
    let mut best = field.get(origin)? as f32 + pull(origin);
    for tile in blocked
        .reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        let d = field.get(tile)?;
        if d == DistanceField::UNREACHABLE {
            continue;
        }
        let score = d as f32 + pull(tile);
        if score < best {
            step = tile;
            best = score;
        }
    }
    checkpoint("path-step");
    if step == origin {
        Ok(dashed)
    } else {
        Ok(step_to(step) || dashed)
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

// With no nearby enemy champion, spend ready damaging abilities on camps in range. After a move
// (`moved`), abilities that exclude walking are skipped: the engine rejects them (error 126).
// Returns true if we cast something that excludes walking (so the turn's move is used up).
fn cast_abilities_on_camps(moved: bool) -> Result<bool> {
    let me = me()?;
    let sight = sight()?;
    if !me.can_cast()? || !fightable_enemies()?.is_empty() {
        return Ok(false);
    }
    let mut camps = Vec::new();
    for camp in &sight.camps {
        camps.push((camp.health()?, camp));
    }
    camps.sort_by(|a, b| a.0.total_cmp(&b.0));
    for ability in me.abilities() {
        if !ability.is_ready()? || (!is_damaging(ability) && !ability.is_attack_reset()) {
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

// Enemies standing on their own spawn heal to full every turn, so trading blows with them (or
// chasing them there) gets nowhere: on maps where the spawns touch, both teams stood at the border
// all game. They still count as threats, just not as targets.
pub const IGNORE_ENEMIES_ON_SPAWN: bool = true;

// Enemies revealed by wards are still tracked as threats, but only nearby ones enter combat logic.
// Strict comparison: an enemy must be fewer than 7 Chebyshev tiles away.
pub const FIGHT_ENEMY_MAX_DISTANCE: i32 = 7;

// Nearby visible enemies worth fighting, weakest first (health read once per enemy).
pub fn fightable_enemies() -> Result<Vec<Champion>> {
    let spawn = cambc::map::spawn_region(Team::Enemy)?;
    let here = me()?.position()?;
    let mut by_health = Vec::new();
    for enemy in &sight()?.enemies {
        let position = enemy.position()?;
        if distance(here, position) >= FIGHT_ENEMY_MAX_DISTANCE
            || (IGNORE_ENEMIES_ON_SPAWN && spawn.is_set(position)?)
        {
            continue;
        }
        by_health.push((enemy.health().unwrap_or(f32::MAX), enemy.clone()));
    }
    by_health.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(by_health.into_iter().map(|(_, e)| e).collect())
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
            pos = (-1..=1)
                .flat_map(|dx| (-1..=1).map(move |dy| Vec2::new(pos.x + dx, pos.y + dy)))
                .find(|&n| self.get(n) == d - 1)?;
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

// Awake camps attack once per round; camps never move, so a live camp sits on its spawn tile.
// There is no "awake" query: a group counts as awake once a visible member is damaged or missing
// (dead). Groups that went back to sleep after two quiet rounds are still counted, overestimating.
// Only groups with a camp in sight can hurt us, so start from the visible camps.
fn camp_threats(threats: &mut Vec<Threat>) -> Result<()> {
    let sight = sight()?;
    if sight.camps.is_empty() {
        return Ok(());
    }
    let visible = &sight.visible;
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
                    if c.health()? < c.max_health()? {
                        awake = true;
                    }
                    live.push(c);
                }
                None => {
                    if visible.is_set(spawn)? {
                        awake = true;
                    }
                } // seen dead
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

// How much a step closer to our spawn is worth, in steps closer to safety, when fleeing. Below 1,
// so every step still makes progress toward safety; without it, ties between equally close safe
// tiles went to whichever came first in tile order, so we fled bottom-right.
pub const RETREAT_HOME_PULL: f32 = 0.5;

// Centre of our spawn region.
pub fn ally_spawn_centre() -> Result<Vec2> {
    let tiles = cambc::map::spawn_region(Team::Ally)?.positions()?;
    let n = tiles.len().max(1) as i32;
    let (x, y) = tiles.iter().fold((0, 0), |(x, y), t| (x + t.x, y + t.y));
    Ok(Vec2::new(x / n, y / n))
}

// Tribute at an altar in range, otherwise walk toward the nearest unlocked altar.
pub fn bank_gems() -> Result<()> {
    let me = me()?;
    // Union all altar approach tiles: the field chooses the nearest reachable altar.
    let mut goals = Bitboard::new()?;
    for altar in cambc::map::altars()?.positions()? {
        if cambc::vision::has_altar_lock(altar)? {
            continue;
        }
        let range = cambc::constants().submit_gems_range;
        if distance(me.position()?, altar) <= range {
            me.submit_gems(altar)?;
            return Ok(());
        }
        goals.draw_chebyshev(altar, range as f32)?;
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
        if distance(here, altar) <= tiles && !cambc::vision::has_altar_lock(altar)? {
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
fn attack_camp_in_range() -> Result<bool> {
    let me = me()?;
    if !me.can_attack()? {
        return Ok(false);
    }
    let (position, range) = (me.position()?, me.attack_range()?);
    let sight = sight()?;
    let mut best: Option<(f32, &Camp)> = None;
    for camp in &sight.camps {
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
pub const FARM_GEM_PHASE_ROUND: u32 = 150;
pub const FARM_PHASE_BY_GROUP: bool = true; // false: the old per-camp filter

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
}

const FARM_TARGET_COOLDOWN_ROUNDS: u32 = 2;
const DAMAGED_CAMP_MAX_DETOUR: i32 = 3;

#[derive(Default)]
struct FarmTargetMemory {
    current: Option<Vec2>,
    abandoned: Vec<(Vec2, u32)>,
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

fn avoid_abandoned_farm_targets(
    goals: &mut Bitboard,
    round: u32,
    here: Vec2,
    reach: i32,
    unavailable_reason: Option<&'static str>,
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
    let (target, changed_reason) = FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        memory.abandoned.retain(|&(_, until)| round < until);
        let previous_is_abandoned =
            previous.is_some_and(|old| memory.abandoned.iter().any(|&(tile, _)| tile == old));
        let available: Vec<Vec2> = candidates_for_target
            .iter()
            .copied()
            .filter(|&tile| !memory.abandoned.iter().any(|&(old, _)| old == tile))
            .collect();
        let target = if available.is_empty() {
            memory.abandoned.clear();
            candidates_for_target
                .iter()
                .copied()
                .min_by_key(|&tile| distance(here, tile))
        } else {
            let current_is_reachable = memory
                .current
                .is_some_and(|tile| distance(here, tile) <= reach);
            if !current_is_reachable {
                if let Some(current) = memory.current.filter(|tile| available.contains(tile)) {
                    Some(current)
                } else {
                    available
                        .iter()
                        .copied()
                        .min_by_key(|&tile| distance(here, tile))
                }
            } else {
                available
                    .iter()
                    .copied()
                    .min_by_key(|&tile| distance(here, tile))
            }
        };
        let reason = previous.filter(|&old| Some(old) != target).map(|old| {
            if !candidates_for_target.contains(&old) {
                unavailable_reason.unwrap_or("previous-target-not-in-farm-goals")
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
        for tile in candidates_for_target {
            goals.set(tile, true)?;
        }
        FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().abandoned.clear());
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
}

struct FarmStatics {
    groups: Vec<GroupInfo>,
    all: Bitboard,            // every camp's tile
    preferred: [Bitboard; 2], // camps of preferred groups, per phase
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
    for group in camp_groups()? {
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
        for (phase, round) in [0, FARM_GEM_PHASE_ROUND].into_iter().enumerate() {
            preferred[phase] = group
                .camps()
                .iter()
                .any(|c| preferred_in_phase(c.kind(), round));
        }
        for camp in group.camps() {
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
            false
        };
        if !enemy_is_winnable {
            enemy_reach.push((position, reach));
        }
    }
    let max_reach = enemy_reach.iter().map(|&(_, r)| r).max().unwrap_or(0);
    let vision = me()?.base_stats()?.vision_radius as i32;
    let phase = usize::from(round >= FARM_GEM_PHASE_ROUND);
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
        // While part of the group is alive, only its live (or unseen) camps are goals. A dead camp's
        // tile only becomes a goal once the whole group is cleared (standing there captures it):
        // otherwise a melee champion standing on one counts as having arrived and never walks over
        // to the live camp out of reach (seen stuck for 65 turns).
        let is_goal = |c: &Camp| !(any_live && seen_dead.contains(&c.spawn_position()));
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
            if any_live && seen_dead.contains(&tile) {
                previous_target_reason = Some("camp-dead-while-group-still-active");
            }
        }
        if reward && group.camps().iter().any(|c| c.spawn_position() == here) {
            holding = crate::collect::CAPTURE_HOLD;
        }
        if any_live {
            not_goals.extend(seen_dead.iter().copied());
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
        for tile in greedy {
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
        for tile in not_goals {
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
    if let Some(old) = previous_target {
        if !goals.is_set(old)? && previous_target_reason.is_none() {
            previous_target_reason = Some(selection_reason);
        }
    }
    avoid_abandoned_farm_targets(
        &mut goals,
        round,
        here,
        me()?.attack_range()?,
        previous_target_reason,
    )?;
    let plan = std::rc::Rc::new(FarmPlan {
        goals,
        greedy: is_greedy,
    });
    FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
    Ok(plan)
}

// Clear camps and capture their rewards, following farm_plan.
pub fn farm_camps() -> Result<()> {
    let attacked = attack_camp_in_range()?;
    if attacked {
        cast_abilities_on_camps(false)?;
        return Ok(());
    }
    let plan = farm_plan()?;
    let moved = move_towards_closest(plan.goals.clone())?;
    if moved {
        if attack_camp_in_range()? {
            cast_abilities_on_camps(true)?;
        }
    }
    Ok(())
}

// Ward a hidden tile next to us (a bush, or behind a wall) so we see into it: bushes first, then
// the furthest from us (the ward sees 6 tiles round it). Wards are free and recharge after a
// round; we keep our current one while it's within WARD_KEEP_TILES of us.
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
