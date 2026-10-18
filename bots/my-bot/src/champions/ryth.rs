// Ryth (melee tank). Passive: gains Strength equal to 1% of bonus health.
// Slot 0: area stun, 1 x Focus damage and one-round stun in radius one, ground, range 4, excludes walking.
// Slot 1: attack reset.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Ryth;

impl ChampionAi for Ryth {
    // Build: HP max; Power / Bruiser / Durability.
    fn build(&self) -> &'static [I] {
        &[
            I::PowerShardOfHealth,
            I::BruiserShardOfHealth,
            I::DurabilityShardOfHealth,
            I::PowerShardOfHealth,
        ]
    }
}
