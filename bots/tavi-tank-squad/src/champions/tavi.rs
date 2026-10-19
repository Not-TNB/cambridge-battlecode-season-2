// Tavi (ranged). Passive: buys shards anywhere on the map (consumables only at spawn or dead);
// can sell equipment at spawn for full value.
// Slot 0: attack reset.
// Slot 1: 2.2 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Tavi;

impl ChampionAi for Tavi {
    // Build: AP max -> AD (HP only at base, skipped: HP bought in the field doesn't heal); Power -> Bruiser.
    fn build(&self) -> &'static [I] {
        &[
            I::TreasureShardOfStrength,
            I::MonsterShardOfStrength,
            I::DurabilityShardOfHealth,
            I::HomeShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfHealth,
            I::PowerShardOfHealth,
            I::PowerShardOfHealth,
            I::BruiserShardOfHealth,
            I::PowerShardOfHealth,
            I::PowerShardOfHealth,
        ]
    }

    fn shops_anywhere(&self) -> bool {
        true
    }
}
