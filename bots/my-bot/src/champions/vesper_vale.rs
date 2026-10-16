// Vesper Vale (ranged). Passive: +20% gold (+40% with a Treasure emblem).
// Slot 0: attack reset. Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct VesperVale;

impl ChampionAi for VesperVale {
    // Build: AD -> some hp; Treasure / Power.
    fn build(&self) -> &'static [I] {
        &[I::TreasureShardOfStrength, I::PowerShardOfStrength, I::PowerShardOfHealth, I::PowerShardOfStrength]
    }
}
