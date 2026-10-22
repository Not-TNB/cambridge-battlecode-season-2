// Basic attacks and casting damaging abilities on enemies and camps.
use super::camps::{attack_camp_in_range, CampFocus};
use super::fight::fightable_enemies;
use super::me;
use super::world::{distance, forget_sight, sight};
use cambc::{Ability, Camp, Champion, Result, Unit, Vec2};

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
        super::trace::trace("cast-camp");
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
pub(super) fn cast_abilities_on_camps(moved: bool) -> Result<bool> {
    let me = me()?;
    let sight = sight()?;
    if !me.can_cast()? || !fightable_enemies()?.is_empty() {
        return Ok(false);
    }
    // A landing-damage dash into the camps we're fighting (IMPACT_DASH): walking-compatible.
    if IMPACT_DASH && super::movement::impact_radius().is_some() {
        let (targets, avoid) = super::camps::impact_camps(5, false)?;
        if impact_dash(&targets, &avoid)? {
            return Ok(false);
        }
    }
    // Same camps and order as basic attacks (camps::CampFocus): no spell into a sleeping group we
    // aren't clearing.
    let focus = CampFocus::new()?;
    let mut camps = Vec::new();
    for camp in &sight.camps {
        // Follow-only escorts leave sleeping camps alone (collect::may_hit_camp).
        if let Some(index) = super::world::group_at(camp.spawn_position())? {
            if !super::collect::may_hit_camp(camp, index)? {
                continue;
            }
            match focus.rank(index) {
                Some(rank) => camps.push(((rank, camp.health()?), camp)),
                None => continue,
            }
        } else {
            camps.push(((0, camp.health()?), camp));
        }
    }
    camps.sort_by(|a, b| a.0 .0.cmp(&b.0 .0).then(a.0 .1.total_cmp(&b.0 .1)));
    for ability in me.abilities() {
        if !ability_ready(ability, me)? || (!is_damaging(ability) && !ability.is_attack_reset()) {
            continue;
        }
        let blocks_walking = ability
            .info()?
            .is_some_and(|info| info.as_dyn().is_exclusive_with_movement());
        if moved && blocks_walking {
            continue;
        }
        for &(_, camp) in &camps {
            if ability.is_aoe_damage() && focus.splash_wakes(camp.spawn_position())? {
                continue;
            }
            super::camps::trace_wake(camp);
            if cast_at_camp(ability, camp)? {
                return Ok(blocks_walking);
            }
        }
    }
    Ok(false)
}

// Landing-damage dashes (Heart of Ruin: Focus x1.5 damage, 150 at base, and a one-round stun to
// every enemy and camp within radius 1 of the landing tile, every 8 turns, walking allowed) are
// aimed: in fights at the free tile in reach whose radius covers the most enemies, in camp fights at
// the one covering the most camps we may hit (never one next to a camp we'd leave alone). The shared
// code only cast damage, area-damage and stun abilities, so this dash was only ever used to travel,
// and the top three ladder teams all play Heart of Ruin (50 of 50 games) while our own games with
// it won 46%. Off: travel only.
pub const IMPACT_DASH: bool = true;

// Dash to the free tile in reach whose landing radius covers the most of `targets` (at least one),
// with none of `avoid` in it; ties go to the tile covering the weakest target. Returns whether we
// dashed.
pub fn impact_dash(targets: &[(Vec2, f32)], avoid: &[Vec2]) -> Result<bool> {
    if !IMPACT_DASH || targets.is_empty() {
        return Ok(false);
    }
    let Some(radius) = super::movement::impact_radius() else {
        return Ok(false);
    };
    let Some(dash) = super::movement::ready_dash()? else {
        return Ok(false);
    };
    let Some(info) = dash.info()? else {
        return Ok(false);
    };
    let me = me()?;
    let here = me.position()?;
    let range = info.as_dyn().range()? as i32;
    // Nothing within the dash's range plus its radius: nothing to hit.
    if !targets.iter().any(|&(p, _)| distance(p, here) <= range + radius) {
        return Ok(false);
    }
    let blocked = super::world::blocked_tiles()?;
    let mut best: Option<((usize, f32), Vec2)> = None;
    for tile in blocked.reachable_tiles(here, range as u16)?.positions()? {
        if tile == here || avoid.iter().any(|&a| distance(a, tile) <= radius) {
            continue;
        }
        let mut hits = 0;
        let mut weakest = f32::MAX;
        for &(p, health) in targets {
            if distance(p, tile) <= radius {
                hits += 1;
                weakest = weakest.min(health);
            }
        }
        if hits == 0 {
            continue;
        }
        let key = (hits, -weakest);
        if best.is_none_or(|(k, _)| key.0 > k.0 || (key.0 == k.0 && key.1 > k.1)) {
            best = Some((key, tile));
        }
    }
    let Some((_, tile)) = best else {
        return Ok(false);
    };
    let dashed = matches!(dash.cast_at(tile), Ok(true));
    if !dashed {
        super::world::remember_hidden_block(tile);
    }
    forget_sight();
    if dashed {
        super::trace::trace("impact-dash");
    }
    Ok(dashed)
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
    cambc::debug_block! {
        if super::activity::ACTIVITY || super::trace::TRACE {
            let camp = target
                .position()
                .and_then(|p| cambc::vision::camp_tiles()?.is_set(p))
                .unwrap_or(false);
            super::trace::trace(if camp { "hit-camp" } else { "hit-champion" });
        }
    }
    forget_sight();
    Ok(true)
}

// Ability::is_ready without its cost: the SDK's version builds a new Champion for the owner on
// every call (two ability metadata queries, ~1.5k fuel), and it ran for each of our abilities on
// every farming turn and for each enemy ability in the threat model. Same checks, on the owner we
// already hold: has metadata, off cooldown, owner able to cast.
pub fn ability_ready(ability: &Ability, owner: &Champion) -> Result<bool> {
    Ok(ability.info()?.is_some() && !ability.is_on_cooldown()? && owner.can_cast()?)
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
        super::trace::trace("cast-enemy");
        return ability.cast_on(enemy);
    }
    if ability.can_target_ground()? {
        return ability.cast_at(target);
    }
    Ok(false)
}

// Basic attack, then a ready damaging ability, else an attack reset to attack a second time.
// After a move (`moved`), abilities that exclude walking are skipped. Returns whether we acted.
// Targets: an enemy we can kill outright (with a basic attack, or with the spell) first; otherwise
// a stun goes on the enemy dealing the most damage a turn (dps.rs), other spells and basic attacks
// keep the order they're given. Off: the order given, for everything.
// 2026-10-22, one game a map against off: 20/40, gold 5.91 a round against 5.08, tributes +6%,
// deaths 12.8 against 15.8, kills 13.7 against 10.3.
pub const STUN_HIGHEST_DAMAGE: bool = true;

pub fn strike(enemies: &[Champion], moved: bool) -> Result<bool> {
    let me = me()?;
    // A basic attack that kills goes first.
    let mut order: Vec<&Champion> = enemies.iter().collect();
    if STUN_HIGHEST_DAMAGE {
        let strength = me.strength()?;
        order.sort_by_key(|e| {
            let hp = e.health().unwrap_or(f32::MAX);
            hp > strength * super::mitigation::incoming(e)
        });
    }
    let mut attacked = None;
    for enemy in order.iter().copied() {
        if attack_if_in_range(enemy)? {
            attacked = Some(enemy);
            break;
        }
    }

    if !me.can_cast()? {
        return Ok(attacked.is_some());
    }
    for ability in me.abilities() {
        if !is_damaging(ability) || !ability_ready(ability, me)? {
            continue;
        }
        if moved
            && ability
                .info()?
                .is_some_and(|info| info.as_dyn().is_exclusive_with_movement())
        {
            continue;
        }
        let mut order: Vec<&Champion> = enemies.iter().collect();
        if STUN_HIGHEST_DAMAGE {
            // (not killable by this spell, -damage a turn if it's a stun): kills first, then the
            // hardest hitter for a stun.
            let mut keyed = Vec::with_capacity(order.len());
            for e in order {
                let hp = e.health().unwrap_or(f32::MAX);
                let hit = ability.damage_amount_against(e).or_else(|_| ability.damage_amount()).unwrap_or(0.0);
                let threat = if ability.is_cc() {
                    super::dps::turn_damage(e, None).unwrap_or(0.0)
                } else {
                    0.0
                };
                keyed.push((hit < hp, -threat, e));
            }
            keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
            order = keyed.into_iter().map(|(_, _, e)| e).collect();
        }
        for enemy in order {
            if cast_at_enemy(ability, enemy)? {
                return Ok(true);
            }
        }
    }

    // A reset only helps after an attack: it clears the attack cooldown so we can hit again now.
    if let Some(enemy) = attacked {
        for ability in me.abilities() {
            if ability.is_attack_reset() && ability_ready(ability, me)? && ability.cast_untargeted()? {
                attack_if_in_range(enemy)?;
                break;
            }
        }
    }
    Ok(attacked.is_some())
}
