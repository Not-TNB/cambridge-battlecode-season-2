// Fighting one camp group and capturing its reward.
use super::attack::attack_if_in_range;
use super::me;
use super::world::{distance, group_at, sight};
use cambc::{Camp, CampGroup, Result};

// A tank or healer leaves a cleared camp group's capture to an AD carry ally whose attack range
// covers it: capture rewards are split between every ally on the region, so the carry gets all of it.
pub(super) const TANK_FUNNEL: bool = true;
pub const FUNNEL_HEALERS: bool = true;

// Attack the weakest camp already in range (camps never move, so their spawn tile is their
// position), skipping groups we'd lose to (collect::CAMP_FIGHT): leave those instead.
pub(super) fn attack_camp_in_range() -> Result<bool> {
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
        if !super::collect::may_hit_camp(camp, index)?
            || super::collect::camp_fight_winnable(index)? == Some(false)
        {
            continue;
        }
        let health = camp.health()?;
        if best.is_none_or(|(h, _)| health < h) {
            best = Some((health, camp));
        }
    }
    match best {
        Some((_, camp)) => {
            let hit = attack_if_in_range(camp)?;
            if hit {
                if let Some(index) = group_at(camp.spawn_position())? {
                    super::collect::note_camp_hit(index);
                    super::rally::call_for_help(index)?;
                }
            }
            Ok(hit)
        }
        None => Ok(false),
    }
}

// Whether an ally (not us) stands on one of `group`'s camp tiles.
pub(super) fn ally_on_group(group: &CampGroup) -> Result<bool> {
    for ally in &sight()?.allies {
        let p = ally.position()?;
        if group.camps().iter().any(|c| c.spawn_position() == p) {
            return Ok(true);
        }
    }
    Ok(false)
}

// Whether an AD carry ally (not us) has any of `group`'s capture tiles within its attack range.
pub(super) fn carry_covers(group: &CampGroup) -> Result<bool> {
    let my_position = me()?.position()?;
    for ally in &sight()?.allies {
        let position = ally.position()?;
        if position == my_position
            || !crate::constants::class_of(ally.champion_name()?).is_funnel_carry()
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
