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
