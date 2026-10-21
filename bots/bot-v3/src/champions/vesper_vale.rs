// Vesper Vale (ranged). Passive: +20% gold (+40% with a Treasure emblem).
// Slot 0: attack reset. Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct VesperVale;

impl ChampionAi for VesperVale {
    // Build: AD -> some hp; Treasure / Power.
    fn build(&self) -> &'static [I] {
        &[
            // Treasure first: her passive's +20% gold becomes +40% with a Treasure emblem.
            // 2026-10-21, 132 games: 72-60 against without, gold +7%.
            I::TreasureShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfHealth,
            I::PowerShardOfStrength,
        ]
    }
}
