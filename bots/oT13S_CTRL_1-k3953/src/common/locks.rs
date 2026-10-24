// Altar locks (10 gold, one carried at a time). Placing one on an altar cancels every enemy tribute
// channel there and blocks enemy tributes until they break it with five basic attacks; our own team
// can still tribute there. We buy one whenever we shop, and place it on an altar within LOCK_RANGE
// where we see an enemy channelling a tribute (a 50-gem bag channels for 5 turns). Off: never
// bought or used.
use super::me;
use super::trace::trace;
use super::world::{distance, sight};
use cambc::{ConsumableType, Result};

pub const ALTAR_LOCKS: bool = true;
// Lock every altar we pass that has no lock on it (ours or theirs): ladder teams buy 4-6 locks a
// game, and an opponent that doesn't remember locks out of sight (as we didn't: base.rs
// REMEMBER_ENEMY_LOCKS) walks between locked altars without tributing. Our own team still tributes
// at our locks. Off: only to cancel an enemy tribute.
// 2026-10-21, 35 maps: vs bot-v2 (forgets locks) 118/140 against 105/140 off, bot-v2's tributes
// 520 -> 337 (00collision 426 -> 20); vs off with REMEMBER_ENEMY_LOCKS on both sides 136/280.
pub const LOCK_EVERY_ALTAR: bool = true;
// How far from us a lock can be placed.
const LOCK_RANGE: i32 = 2;

// On our spawn (or wherever we can shop): carry a lock.
pub fn buy_lock() -> Result<()> {
    if !ALTAR_LOCKS {
        return Ok(());
    }
    let me = me()?;
    if me.consumables()[ConsumableType::AltarLock].count()? == 0
        && me.can_buy_consumable(ConsumableType::AltarLock)?
    {
        me.buy_consumable(ConsumableType::AltarLock)?;
    }
    Ok(())
}

// Lock an altar an enemy in sight is tributing at, if it's in our reach. Returns whether we did.
pub fn lock_enemy_tribute() -> Result<bool> {
    if !ALTAR_LOCKS {
        return Ok(false);
    }
    let sight = sight()?;
    let me = me()?;
    let here = me.position()?;
    if LOCK_EVERY_ALTAR {
        for altar in cambc::map::altars()?.positions()? {
            if distance(altar, here) <= LOCK_RANGE && !cambc::vision::has_altar_lock(altar)? {
                let lock = &me.consumables()[ConsumableType::AltarLock];
                if lock.count()? > 0 && lock.use_at(altar)? {
                    trace("altar-lock");
                    return Ok(true);
                }
            }
        }
    }
    if sight.enemies.is_empty() {
        return Ok(false);
    }
    let range = cambc::constants().submit_gems_range;
    for enemy in &sight.enemies {
        if !enemy.is_submitting_gems().unwrap_or(false) {
            continue;
        }
        let Ok(at) = enemy.position() else {
            continue;
        };
        for altar in cambc::map::altars()?.positions()? {
            if distance(altar, at) > range
                || distance(altar, here) > LOCK_RANGE
                || cambc::vision::has_altar_lock(altar)?
            {
                continue;
            }
            let lock = &me.consumables()[ConsumableType::AltarLock];
            if lock.count()? == 0 {
                return Ok(false);
            }
            if lock.use_at(altar)? {
                trace("altar-lock");
                return Ok(true);
            }
        }
    }
    Ok(false)
}
