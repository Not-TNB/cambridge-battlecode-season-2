// Vendetta (melee assassin). Passive: kills (including camps) reset all ability cooldowns.
// Slot 0: attack reset. Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Vendetta;

impl ChampionAi for Vendetta {
    // Build: Jungle, then Bruiser Strength with a Durability emblem after the first (see
    // ChampionAi::build). 2026-10-22, 320 games of Bruiser builds against the Power ones: 193-127,
    // her gold +22%.
    fn build(&self) -> &'static [I] {
        &[
            I::JungleShardOfStrength, // less camp damage (see ChampionAi::build)
            I::BruiserShardOfHealth,
            I::DurabilityShardOfHealth,
            I::BruiserShardOfStrength,
            I::BruiserShardOfHealth,
            I::BruiserShardOfStrength,
        ]
    }
}
