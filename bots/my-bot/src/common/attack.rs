// Basic attacks and casting damaging abilities on enemies and camps.
use super::camps::attack_camp_in_range;
use super::fight::fightable_enemies;
use super::me;
use super::world::{distance, forget_sight, sight};
use cambc::{Ability, Camp, Champion, Result, Unit};

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
pub(super) fn cast_abilities_on_camps(moved: bool) -> Result<bool> {
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
