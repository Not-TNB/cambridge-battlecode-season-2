// Support champions (Willow) joining their allies' fights: shooting what the allies nearby are
// attacking, an enemy champion or an awake camp, from the tile in range furthest from enemy
// champions. Without it Willow followed and healed but hardly attacked: ~1.5k damage to champions
// and ~5.6k to camps a game, against 5-15k and 18-55k for everyone else.
use super::attack::{attack_if_in_range, strike};
use super::fight::fightable_enemies;
use super::me;
use super::movement::{move_towards_closest_with_pull, step_to};
use super::trace::trace;
use super::world::{blocked_tiles, distance, sight, tiles_around};
use cambc::{Camp, Champion, Result, Vec2};

// Off: Willow fights like before (fight_enemies, then following).
pub const ASSIST_ALLIES: bool = true;
// Allies further than this from us don't count: we join fights we can reach soon.
const ASSIST_RADIUS: i32 = 8;
// Below this share of our health we hang back (follow and heal) instead. 0.0: always assist.
pub const ASSIST_MIN_HEALTH: f32 = 0.6;
// Only shoot from tiles no enemy champion can reach next turn and no other awake camp hits; in range
// of the camp we shoot only with an ally closer to it (in front, tanking). With no such tile in
// range we don't assist this turn. Off: the safest tile in range.
// 2026-10-21, 132 games each against no assist: no gates 66 vs 66 (Willow 70% more camp damage but
// 30% more deaths); health gate only 69 vs 63; both gates 72 vs 60, and in the games where only
// one side drafted Willow, 17/33 against 11/33, with Willow dying less (2.9 against 3.1 a game).
pub const ASSIST_SAFE_TILES_ONLY: bool = true;

enum Target {
    Enemy(Champion),
    Camp(Camp),
}

// The fight to join: enemy champions an ally nearby can hit (or nearly: its attack range plus a
// step), weakest first; else an awake camp an ally nearby has in range, weakest first.
fn allies_target() -> Result<Option<(Target, Vec2)>> {
    let here = me()?.position()?;
    let sight = sight()?;
    let mut allies = Vec::new();
    for ally in &sight.allies {
        if ally.is_alive()? && distance(ally.position()?, here) <= ASSIST_RADIUS {
            allies.push((ally.position()?, ally.attack_range()?));
        }
    }
    if allies.is_empty() {
        return Ok(None);
    }
    let mut best: Option<(f32, Champion)> = None;
    for enemy in fightable_enemies()? {
        let p = enemy.position()?;
        if allies.iter().any(|&(a, r)| distance(a, p) <= r + 1) {
            let health = enemy.health()?;
            if best.as_ref().is_none_or(|(h, _)| health < *h) {
                best = Some((health, enemy));
            }
        }
    }
    if let Some((_, enemy)) = best {
        let p = enemy.position()?;
        return Ok(Some((Target::Enemy(enemy), p)));
    }
    let mut best: Option<(f32, Camp)> = None;
    for camp in &sight.camps {
        if !camp.is_awake().unwrap_or(false) {
            continue;
        }
        let p = camp.spawn_position();
        let reach = camp.attack_range()?;
        if allies.iter().any(|&(a, r)| distance(a, p) <= r.max(reach)) {
            let health = camp.health()?;
            if best.as_ref().is_none_or(|(h, _)| health < *h) {
                best = Some((health, camp.clone()));
            }
        }
    }
    Ok(best.map(|(_, camp)| {
        let p = camp.spawn_position();
        (Target::Camp(camp), p)
    }))
}

// Join the allies' fight (ASSIST_ALLIES). Returns None if there's none to join (the caller then
// fights or farms as usual), else whether we acted.
pub fn assist_allies() -> Result<Option<bool>> {
    if !ASSIST_ALLIES {
        return Ok(None);
    }
    let me = me()?;
    if me.health()? < ASSIST_MIN_HEALTH * me.max_health()? {
        return Ok(None);
    }
    let Some((target, at)) = allies_target()? else {
        return Ok(None);
    };
    let here = me.position()?;
    let range = me.attack_range()?;
    let sight = sight()?;
    // How exposed a tile is: enemy champions that could hit it next turn (their attack range plus
    // a turn's walk), awake camps with it in range, then (to maximise) the nearest enemy's distance.
    let mut enemies = Vec::new();
    for enemy in &sight.enemies {
        if let (Ok(p), Ok(r), Ok(stats)) = (enemy.position(), enemy.attack_range(), enemy.base_stats()) {
            enemies.push((p, r + stats.base_move_speed as i32));
        }
    }
    // Awake camps as (position, range, nearest other champion in range): one only hits a tile
    // nobody else in its range is closer to (threats::CAMPS_HIT_CLOSEST).
    let bodies = super::threats::other_champions()?;
    let mut camps = Vec::new();
    for camp in &sight.camps {
        if camp.is_awake().unwrap_or(false) {
            let (p, r) = (camp.spawn_position(), camp.attack_range()?);
            camps.push((p, r, super::threats::camp_shield(p, r, &bodies)));
        }
    }
    let camp_hits = |tile: Vec2| {
        camps
            .iter()
            .filter(|&&(c, r, s)| distance(c, tile) <= r && s.is_none_or(|s| distance(c, tile) <= s))
            .count()
    };
    let exposure = |tile: Vec2| {
        let reached = enemies.iter().filter(|&&(p, r)| distance(p, tile) <= r).count();
        let hit = camp_hits(tile);
        let nearest = enemies.iter().map(|&(p, _)| distance(p, tile)).min().unwrap_or(99);
        (reached, hit, -nearest)
    };
    // Allies' distances to the target camp, for "someone's in front of us" (ASSIST_SAFE_TILES_ONLY).
    let target_camp = matches!(target, Target::Camp(_));
    let mut ally_gap = i32::MAX;
    for ally in &sight.allies {
        if ally.is_alive()? {
            ally_gap = ally_gap.min(distance(ally.position()?, at));
        }
    }
    let acceptable = |tile: Vec2| {
        if !ASSIST_SAFE_TILES_ONLY {
            return true;
        }
        if enemies.iter().any(|&(p, r)| distance(p, tile) <= r) {
            return false;
        }
        camp_hits(tile) == 0 || (target_camp && ally_gap < distance(at, tile) && camp_hits(tile) == 1)
    };
    let in_range = |tile: Vec2| distance(tile, at) <= range && acceptable(tile);
    // The safest tile in range we can step to this turn (staying put included).
    let mut best: Option<((usize, usize, i32), Vec2)> = in_range(here).then(|| (exposure(here), here));
    if me.can_move()? {
        for tile in blocked_tiles()?
            .reachable_tiles(here, me.base_stats()?.base_move_speed as u16)?
            .positions()?
        {
            if tile != here && in_range(tile) && best.is_none_or(|(k, _)| exposure(tile) < k) {
                best = Some((exposure(tile), tile));
            }
        }
    }
    // A ranged champion moves or attacks, so a shot is only given up for a tile fewer enemies or
    // camps can hit; being a little further away isn't worth a shot (moving every turn as the
    // enemies move, we'd never shoot).
    if in_range(here) {
        if let Some((k, _)) = best {
            let (reached, hit_by, _) = exposure(here);
            if (k.0, k.1) >= (reached, hit_by) {
                best = Some((exposure(here), here));
            }
        }
    }
    match best {
        // Already on the safest tile in range: shoot (a ranged champion moves or attacks).
        Some((_, tile)) if tile == here => {
            trace("assist-attack");
            Ok(Some(hit(&target)?))
        }
        // A safer tile in range: step there; we shoot from it next turn.
        Some((_, tile)) => {
            trace("assist-reposition");
            if step_to(tile) {
                if range <= 1 {
                    hit(&target)?; // melee can still attack after a step
                }
                Ok(Some(true))
            } else {
                Ok(Some(hit(&target)?))
            }
        }
        // Out of range from anywhere reachable: walk towards it, keeping away from enemies.
        None => {
            // Only towards tiles we'd shoot from (ASSIST_SAFE_TILES_ONLY); none: hang back.
            let mut goals = cambc::Bitboard::new()?;
            for tile in tiles_around(at, range)?.positions()? {
                if in_range(tile) {
                    goals.set(tile, true)?;
                }
            }
            if goals.is_empty()? {
                trace("assist-hang-back");
                return Ok(None);
            }
            trace("assist-approach");
            let away = |tile: Vec2| -> f32 {
                let nearest = enemies.iter().map(|&(p, _)| distance(p, tile)).min().unwrap_or(99);
                -0.5 * nearest.min(8) as f32
            };
            Ok(Some(move_towards_closest_with_pull(goals, away, false)?))
        }
    }
}

// Attack the target from where we stand, with the attack reset for a second hit.
fn hit(target: &Target) -> Result<bool> {
    match target {
        Target::Enemy(enemy) => strike(std::slice::from_ref(enemy), false),
        Target::Camp(camp) => {
            let attacked = attack_if_in_range(camp)?;
            if attacked && me()?.can_cast()? {
                for ability in me()?.abilities() {
                    if ability.is_attack_reset()
                        && super::attack::ability_ready(ability, me()?)?
                        && ability.cast_untargeted()?
                    {
                        attack_if_in_range(camp)?;
                        break;
                    }
                }
            }
            Ok(attacked)
        }
    }
}
