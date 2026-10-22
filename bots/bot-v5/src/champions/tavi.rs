// Tavi (ranged). Passive: buys shards anywhere on the map (consumables only at spawn or dead);
// can sell equipment at spawn for full value.
// Slot 0: attack reset.
// Slot 1: 1.4 x Focus damage (2.2 before the balance patch) and one-round stun, enemy or camp, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Tavi;

impl ChampionAi for Tavi {
    // Build: Bruiser Strength (the patch cuts the stun to 1.4 x Focus and raises base Strength to
    // 45, and basic attacks plus the reset are most of Tavi's damage; Bruiser adds one Health and
    // one Focus shard to the five), with a Durability emblem after the first (see
    // ChampionAi::build). 2026-10-22, 320 games of Bruiser builds against the Power ones: 193-127,
    // his gold +12%, his deaths 3.3 against 4.1.
    fn build(&self) -> &'static [I] {
        &[
            I::BruiserShardOfStrength,
            I::DurabilityShardOfHealth,
            I::BruiserShardOfStrength,
        ]
    }

    fn shops_anywhere(&self) -> bool {
        true
    }
}
