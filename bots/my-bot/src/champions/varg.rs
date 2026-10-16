// Varg (melee bruiser). Passive: basic attacks splash Strength damage in radius one.
// Slot 0: attack reset.
// Slot 1: self-shield, 2.5 x Focus for five turns (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Varg;

impl ChampionAi for Varg {
    // Build: AD max, one Durability late; Treasure / Monster -> Bruiser / Durability.
    fn build(&self) -> &'static [I] {
        &[I::TreasureShardOfStrength, I::MonsterShardOfStrength, I::BruiserShardOfStrength, I::DurabilityShardOfHealth, I::PowerShardOfStrength]
    }
}
