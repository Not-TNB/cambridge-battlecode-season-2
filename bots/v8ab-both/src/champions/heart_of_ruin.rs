// Heart of Ruin (melee bruiser, new in 3.0). Passive: deals more damage the more health it's
// missing, up to +50% (HEART_OF_RUIN_MISSING_HEALTH_DAMAGE_MAX_BONUS) at no health left.
// Its kit has a self-shield; the rest is cast through the shared logic like any other champion.
use super::ChampionAi;
use crate::common::{burst_damage_here, me};
use cambc::{ItemType as I, Result};

// The shield is saved for when it pays most: with our health this low the passive is already worth
// a lot (at 50%: +25% damage), and the shield keeps us in that state for longer. Before that it's
// only cast if the next round could kill us outright (burst_damage_here). 0.0: only against kills.
pub const HEART_SHIELD_HEALTH: f32 = 0.5;
// v7 (the user, 2026-10-23): shield like Varg instead, whenever we lost health since last turn or an
// enemy champion is within HEART_SHIELD_ENEMY_WITHIN tiles, at any health. Saving it for low health
// to keep the passive's damage bonus meant taking every hit unshielded until half health, and our
// Heart of Ruin farmed least of our five (1,371 gold a game) and died most on fight maps (up to 13 a
// game on 00reverse-spiral). Off: HEART_SHIELD_HEALTH and lethal hits only.
// 2026-10-23, head to head against bot-v7 with it off, fresh seeds (8 x 40 maps, 320 games): 182/320
// (team gold 5.86 a round against 5.35, deaths 12.1 against 14.2, kills 12.7 against 10.5); Heart of
// Ruin itself gold 1,238 against 1,057, kills 1.8 against 1.2, deaths 3.1 against 3.5, shielding
// 18,035 a game against 7,086.
pub const HEART_SHIELD_SPAM: bool = true;
const HEART_SHIELD_ENEMY_WITHIN: i32 = 2;
// Print the kit (abilities and base stats) once, on our first turn. For checking what the engine
// gives this champion; off in games.
const PRINT_KIT: bool = false;

#[derive(Default)]
pub struct HeartOfRuin {
    last_health: Option<f32>, // health at the end of our previous turn
    printed: bool,
}

impl ChampionAi for HeartOfRuin {
    // Build: Jungle, then Bruiser Focus, with a Durability emblem after the first (see
    // ChampionAi::build). Focus makes the landing dash (1.5 x Focus around where it lands) and the
    // shield, at 12 a shard. The top three ladder teams build Heart Focus x10-14. 2026-10-22, 320
    // games of Bruiser builds against the old Strength-first ones: 193-127, gold +9%, deaths 13.5
    // against 16.2.
    fn build(&self) -> &'static [I] {
        &[
            I::JungleShardOfFocus, // less camp damage (see ChampionAi::build)
            I::BruiserShardOfFocus,
            I::DurabilityShardOfHealth,
            I::BruiserShardOfFocus,
        ]
    }

    // Shield when we're being hit (lost health since last turn, or something can hit us this
    // round) and either our health is down to HEART_SHIELD_HEALTH or the next round could kill us.
    fn support(&mut self) -> Result<()> {
        let me = me()?;
        if PRINT_KIT && !self.printed {
            self.printed = true;
            let stats = me.base_stats()?;
            println!(
                "HEART kit: hp {} str {} focus {} range {} speed {}",
                stats.base_health,
                stats.base_strength,
                stats.base_focus,
                stats.base_attack_range,
                stats.base_move_speed
            );
            for ability in me.abilities() {
                println!(
                    "HEART ability: shield {} damage {} dash {} reset {} stun {} heal {} cooldown {:?} shield_amount {:?} damage_amount {:?}",
                    ability.is_shield(),
                    ability.is_damage(),
                    ability.is_dash(),
                    ability.is_attack_reset(),
                    ability.is_cc(),
                    ability.is_heal(),
                    ability.info().ok().flatten().map(|i| i.as_dyn().max_cooldown()),
                    ability.shield_amount().ok(),
                    ability.damage_amount().ok(),
                );
            }
        }
        let health = me.health()?;
        let hit = self.last_health.is_some_and(|last| health < last);
        self.last_health = Some(health);
        let Some(shield) = me.abilities().iter().find(|a| a.is_shield()) else {
            return Ok(());
        };
        if !crate::common::ability_ready(shield, me)? {
            return Ok(());
        }
        if HEART_SHIELD_SPAM {
            let mut threatened = hit;
            if !threatened {
                for enemy in &crate::common::sight()?.enemies {
                    if crate::common::distance(enemy.position()?, me.position()?)
                        <= HEART_SHIELD_ENEMY_WITHIN
                    {
                        threatened = true;
                        break;
                    }
                }
            }
            if threatened {
                crate::common::trace("heart-shield-spam");
                shield.cast_untargeted()?;
            }
            return Ok(());
        }
        let burst = burst_damage_here()?;
        if !hit && burst <= 0.0 {
            return Ok(());
        }
        let low = health <= HEART_SHIELD_HEALTH * me.max_health()?;
        let lethal = burst >= health;
        if low || lethal {
            crate::common::trace(if lethal {
                "heart-shield-lethal"
            } else {
                "heart-shield-low"
            });
            shield.cast_untargeted()?;
        }
        Ok(())
    }
}
