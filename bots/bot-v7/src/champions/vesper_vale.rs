// Vesper Vale (ranged). Passive: +20% gold (+40% with a Treasure emblem).
// Slot 0: attack reset. Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct VesperVale;

impl ChampionAi for VesperVale {
    // Build: Treasure, then Bruiser Strength with a Durability emblem after the first (see
    // ChampionAi::build). 2026-10-22, 320 games of Bruiser builds against the Power ones: 193-127,
    // her gold +7%, her deaths 1.6 against 2.2.
    fn build(&self) -> &'static [I] {
        &[
            // Treasure first: her passive's +33% gold becomes +53% with a Treasure emblem.
            // 2026-10-21, 132 games: 72-60 against without, gold +7%.
            I::TreasureShardOfStrength,
            I::BruiserShardOfStrength,
            I::DurabilityShardOfHealth,
            I::BruiserShardOfStrength,
        ]
    }
}
