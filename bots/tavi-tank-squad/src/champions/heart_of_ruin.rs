// Heart of Ruin (melee bruiser, new in 3.0; takes Varg's place in the squad). Passive: deals more
// damage the more health it's missing, up to +50% at no health left.
// Slot 0: self-shield, 150 (cooldown 8): cast in support(), see below.
// Slot 1: dash, 175 damage where it lands: cast through the shared logic like any other ability.
use super::ChampionAi;
use crate::common::{burst_damage_here, me};
use cambc::{ItemType as I, Result};

// The shield is saved for when it pays most: with our health this low the passive is already worth
// a lot (at 50%: +25% damage), and the shield keeps us in that state for longer. Before that it's
// only cast if the next round could kill us outright (burst_damage_here). 0.0: only against kills.
pub const HEART_SHIELD_HEALTH: f32 = 0.5;

#[derive(Default)]
pub struct HeartOfRuin {
    last_health: Option<f32>, // health at the end of our previous turn
}

impl ChampionAi for HeartOfRuin {
    // Build: Strength (its damage is what the passive multiplies); a Health emblem late, since
    // more health only means more to lose before the passive peaks.
    fn build(&self) -> &'static [I] {
        &[
            I::MonsterShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfHealth,
            I::PowerShardOfStrength,
        ]
    }

    // Shield when we're being hit (lost health since last turn, or something can hit us this
    // round) and either our health is down to HEART_SHIELD_HEALTH or the next round could kill us.
    fn support(&mut self) -> Result<()> {
        let me = me()?;
        let health = me.health()?;
        let hit = self.last_health.is_some_and(|last| health < last);
        self.last_health = Some(health);
        let Some(shield) = me.abilities().iter().find(|a| a.is_shield()) else {
            return Ok(());
        };
        if !shield.is_ready()? {
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
