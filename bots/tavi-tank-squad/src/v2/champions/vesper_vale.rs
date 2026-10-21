// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
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
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::PowerShardOfHealth,
            I::PowerShardOfStrength,
        ]
    }
}
