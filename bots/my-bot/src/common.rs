// Shared helpers every champion can use: movement, targeting, fighting and farming.
use cambc::{
    Ability, Bitboard, CampGroup, Champion, DistanceField, Result, Team, Unit, Vec2,
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
    if !me.can_attack()? || !target.is_visible()? || target.health()? <= 0.0
        || !me.attack_tiles()?.is_set(target.position()?)? {
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

// Basic attack, then a ready damaging ability, else an attack reset to attack a second time.
// Returns whether we attacked or cast at an enemy.
pub fn fight_enemies() -> Result<bool> {
    let mut enemies = cambc::vision::champions(Some(Team::Enemy))?;
    if enemies.is_empty() { return Ok(false); }
    // Focus the weakest enemy first.
    enemies.sort_by(|a, b| a.health().unwrap_or(f32::MAX).total_cmp(&b.health().unwrap_or(f32::MAX)));

    let mut attacked = None;
    for enemy in &enemies {
        if attack_if_in_range(enemy)? { attacked = Some(enemy); break; }
    }

    let me = me()?;
    if !me.can_cast()? { return Ok(attacked.is_some()); }
    for ability in me.abilities() {
        if !is_damaging(ability) || !ability.is_ready()? { continue; }
        for enemy in &enemies {
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

pub fn farm_camps() -> Result<()> {
    // Attack a camp already in range, otherwise move toward the nearest camp tiles.
    for camp in cambc::vision::camps()? {
        if attack_if_in_range(&camp)? { return Ok(()); }
    }

    // Set goal bits directly: each new Bitboard costs ~2k fuel in the Rust SDK.
    let mut goals = Bitboard::new()?;
    let visible = cambc::vision::visible_tiles()?;
    for group in camp_groups()? {
        let mut seen = false;
        for camp in group.camps() {
            if visible.is_set(camp.spawn_position())? { seen = true; break; }
        }
        if seen && matches!(group.state()?, cambc::CampGroupState::Respawning(_)) { continue; }
        for camp in group.camps() { goals.set(camp.spawn_position(), true)?; }
    }

    if move_towards_closest(goals)? {
        for camp in cambc::vision::camps()? {
            if attack_if_in_range(&camp)? { return Ok(()); }
        }
    }
    Ok(())
}
