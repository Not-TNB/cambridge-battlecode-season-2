// Varg (melee bruiser). Passive: basic attacks splash Strength damage in radius one.
// Slot 0: attack reset.
// Slot 1: self-shield, 2.5 x Focus for five turns: cast when we're being hit (support()).
use super::ChampionAi;
use crate::common::{distance, me};
use cambc::{ItemType as I, Result};

// Also shield before being hit when an enemy champion is within this many tiles. 0: only on damage.
pub const SHIELD_ENEMY_WITHIN: i32 = 2;

#[derive(Default)]
pub struct Varg {
    last_health: Option<f32>, // health at the end of our previous turn
}

impl ChampionAi for Varg {
    // Build: AD max, one Durability late; Treasure / Monster -> Bruiser / Durability.
    fn build(&self) -> &'static [I] {
        &[I::TreasureShardOfStrength, I::MonsterShardOfStrength, I::BruiserShardOfStrength, I::BruiserShardOfHealth, I::PowerShardOfStrength]
    }

    // Shield when we lost health since last turn (camps or enemies are hitting us), or an enemy
    // champion is close enough to hit us.
    fn support(&mut self) -> Result<()> {
        let me = me()?;
        let health = me.health()?;
        let hit = self.last_health.is_some_and(|last| health < last);
        self.last_health = Some(health);
        let Some(shield) = me.abilities().iter().find(|a| a.is_shield()) else { return Ok(()) };
        if !shield.is_ready()? { return Ok(()); }
        let mut threatened = hit;
        if !threatened && SHIELD_ENEMY_WITHIN > 0 {
            for enemy in &crate::common::sight()?.enemies {
                if distance(enemy.position()?, me.position()?) <= SHIELD_ENEMY_WITHIN { threatened = true; break; }
            }
        }
        if threatened { shield.cast_untargeted()?; }
        Ok(())
    }
}
