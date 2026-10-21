// Vendetta (melee assassin). Passive: kills (including camps) reset all ability cooldowns.
// Slot 0: attack reset. Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Vendetta;

impl ChampionAi for Vendetta {
    // Build: AD max; Power / Bruiser (Treasure only if far ahead, not modelled).
    fn build(&self) -> &'static [I] {
        &[
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfStrength,
        ]
    }
}
