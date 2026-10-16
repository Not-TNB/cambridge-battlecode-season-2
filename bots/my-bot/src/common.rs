// Shared helpers every champion can use: movement, targeting, fighting and farming.
use cambc::{
    Ability, Bitboard, CampGroup, Champion, ChampionType, DistanceField, Result, Team, Unit, Vec2,
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

pub fn move_towards_closest(mut goals: Bitboard) -> Result<bool> {
    let me = me()?;
    let origin = me.position()?;
    if !me.can_move()? || goals.is_set(origin)? { return Ok(false); }

    let blocked = blocked_tiles()?;
    goals.intersect_assign(&blocked.complement()?.union(&cambc::vision::camp_tiles()?)?)?;
    if goals.is_empty()? { return Ok(false); }
    if goals.count()? == 1 && !blocked.is_set(goals.positions()?[0])? {
        return me.move_towards(goals.positions()?[0]);
    }

    let mut field = DistanceField::new()?;
    field.compute_to(&blocked.intersect(&goals.complement()?)?, &goals, &tiles_around(origin, 0)?, &mut Bitboard::new()?, DistanceField::UNREACHABLE)?;

    let mut step = origin;
    for tile in blocked.reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?.positions()? {
        if field.get(tile)? < field.get(step)? { step = tile; }
    }
    if step == origin { Ok(false) } else { me.move_absolute(step) }
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
// beat in a 1v1 after walking to it. Returns whether we attacked, cast or chased this turn.
pub fn fight_enemies() -> Result<bool> {
    let mut enemies = cambc::vision::champions(Some(Team::Enemy))?;
    if enemies.is_empty() { set_chase_target(None); return Ok(false); }
    // Focus the weakest enemy first.
    enemies.sort_by(|a, b| a.health().unwrap_or(f32::MAX).total_cmp(&b.health().unwrap_or(f32::MAX)));

    if strike(&enemies, false)? { return Ok(true); }

    let Some(step) = choose_chase_step(&enemies)? else { return Ok(false) };
    // Step along the planned path; false means an unseen unit blocked it, so try again next turn.
    if !me()?.can_move()? || !me()?.move_absolute(step)? { return Ok(false); }
    // Melee champions can still attack after stepping into range.
    strike(&enemies, true)?;
    Ok(true)
}

// Enemies further than this many steps away are never chased; also bounds the pathfinding cost.
const MAX_CHASE_STEPS: u16 = 16;

// The enemy we are chasing, remembered across turns so we don't flip between targets.
// Each champion runs as its own program, so this is per champion; one of each type per team.
thread_local! {
    static CHASE_TARGET: std::cell::Cell<Option<ChampionType>> = const { std::cell::Cell::new(None) };
}

fn set_chase_target(target: Option<ChampionType>) {
    CHASE_TARGET.with(|cell| cell.set(target));
}

// Keep the current target while we still expect to win against it; otherwise pick the winnable
// enemy we can finish fastest (walking turns + turns to kill). Returns the next tile to step to,
// or None if no enemy is worth chasing (or we are already in range of the one that is).
fn choose_chase_step(enemies: &[Champion]) -> Result<Option<Vec2>> {
    let threats = Threats::scan()?;
    let blocked = blocked_tiles()?;
    // Walking distance from our tile to everywhere, computed once and shared by every enemy.
    let from_me = WalkGrid::from(me()?.position()?, &blocked, MAX_CHASE_STEPS)?;
    let current = CHASE_TARGET.with(|cell| cell.get());
    let mut best: Option<(ChampionType, Option<Vec2>, u32)> = None;
    for enemy in enemies {
        let Some(duel) = plan_duel(enemy, &threats, &from_me)? else { continue };
        if !duel.winnable { continue; }
        let name = enemy.champion_name()?;
        if Some(name) == current { return Ok(duel.next_step); }
        let total = duel.walk_turns + duel.kill_turns;
        if best.is_none_or(|(_, _, t)| total < t) { best = Some((name, duel.next_step, total)); }
    }
    set_chase_target(best.map(|(name, _, _)| name));
    Ok(best.and_then(|(_, step, _)| step))
}

// Simulated chase: walk the shortest path to a tile within our attack range of `enemy`, taking
// the expected threat damage on every tile we end a turn on, then trade blows from that tile.
#[allow(dead_code)] // hp_after_walk and die_turns are kept for debugging and tuning
pub struct Duel {
    pub next_step: Option<Vec2>, // first tile on the path; None if already in range
    pub walk_turns: u32,
    pub hp_after_walk: f32,
    pub kill_turns: u32, // our turns to kill the enemy
    pub die_turns: u32,  // turns for everything threatening the duel tile to kill us
    pub winnable: bool,
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

// `from_me` holds walking distances from our tile. None if no tile in range of the enemy is reachable.
pub fn plan_duel(enemy: &Champion, threats: &Threats, from_me: &WalkGrid) -> Result<Option<Duel>> {
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

    // Damage on the way: every stop except the duel tile, which the trade below accounts for.
    let walk_damage: f32 = stops.iter().rev().skip(1).map(|&tile| threats.damage_at(tile)).sum();
    let hp_after_walk = me.health()? - walk_damage;

    // Both sides deal a basic attack per turn plus abilities averaged over their cooldown.
    let outgoing = turn_damage_against(enemy)?;
    let incoming = threats.damage_at(duel_tile);
    let kill_turns = if outgoing > 0.0 { (enemy.health()? / outgoing).ceil() as u32 } else { u32::MAX };
    let die_turns = if incoming > 0.0 { (hp_after_walk / incoming).ceil().max(0.0) as u32 } else { u32::MAX };
    Ok(Some(Duel {
        next_step,
        walk_turns: stops.len() as u32,
        hp_after_walk,
        kill_turns,
        die_turns,
        // We arrive and hit first, so a tie in turns goes our way.
        winnable: hp_after_walk > 0.0 && kill_turns != u32::MAX && kill_turns <= die_turns,
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
pub struct Threat {
    pub origin: Vec2,
    pub range: i32,
    pub damage: f32,
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
        self.0.iter().filter(|t| distance(t.origin, tile) <= t.range).map(|t| t.damage).sum()
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
            threats.push(Threat { origin: camp.spawn_position(), range: camp.attack_range()?, damage: camp.strength()? });
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
        threats.push(Threat { origin, range: attack_range, damage: strength });
        for ability in enemy.abilities() {
            let Some(info) = ability.info()? else { continue };
            let per_turn = 1.0 / info.as_dyn().max_cooldown().max(1) as f32;
            if ability.is_attack_reset() {
                threats.push(Threat { origin, range: attack_range, damage: strength * per_turn });
            } else if is_damaging(ability) {
                // Engine prediction includes our mitigation and shields; fall back to raw damage.
                let hit = ability.damage_amount_against(me).or_else(|_| ability.damage_amount())?;
                threats.push(Threat { origin, range: info.as_dyn().range()? as i32, damage: hit * per_turn });
            }
        }
    }
    Ok(())
}

// Low on health: step toward the nearest tile with zero expected damage, and recall once on one,
// since damage interrupts a recall. Recalls anyway if no safe tile can be reached.
pub fn retreat_and_recall() -> Result<()> {
    let me = me()?;
    let threats = Threats::scan()?;
    if threats.damage_at(me.position()?) > 0.0 && move_towards_closest(threats.safe_tiles()?)?
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

// Bank whenever we carry gems (holding them back tested neutral to worse; writeup entry 11).
pub fn should_bank() -> Result<bool> {
    Ok(me()?.gem_count()? > 0)
}

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

// A tank leaves a cleared camp group's capture to an AD carry ally whose attack range covers it:
// capture rewards are split between every ally on the region, so the carry gets all of it.
const TANK_FUNNEL: bool = true;

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

// Farm the nearest camps that aren't respawning (including ones we saw respawning earlier). Tanks still fight camps but leave the capture
// of a cleared group to a covering AD carry (see TANK_FUNNEL).
pub fn farm_camps() -> Result<()> {
    if attack_camp_in_range()? { return Ok(()); }

    let is_tank = TANK_FUNNEL && crate::constants::class_of(me()?.champion_name()?) == crate::constants::ClassKind::Tank;
    // Set goal bits directly: each new Bitboard costs ~2k fuel in the Rust SDK.
    let mut goals = Bitboard::new()?;
    let visible = cambc::vision::visible_tiles()?;
    let live_camps = cambc::vision::camp_tiles()?;
    let round = cambc::round_number()?;
    let groups = camp_groups()?;
    let mut ready = RESPAWN_READY.with(|cell| cell.take());
    ready.resize(groups.len(), 0);
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
        for camp in group.camps() { goals.set(camp.spawn_position(), true)?; }
    }
    RESPAWN_READY.with(|cell| cell.replace(ready));

    if move_towards_closest(goals)? { attack_camp_in_range()?; }
    Ok(())
}
