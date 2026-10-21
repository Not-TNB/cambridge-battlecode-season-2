// Tavi (ranged). Passive: buys shards anywhere on the map (consumables only at spawn or dead);
// can sell equipment at spawn for full value.
// Slot 0: attack reset.
// Slot 1: 1.4 x Focus damage (2.2 before the balance patch) and one-round stun, enemy or camp, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Tavi;

impl ChampionAi for Tavi {
    // Build: Strength first (the patch cuts the stun to 1.4 x Focus and raises base Strength to 45,
    // and basic attacks plus the reset are most of Tavi's damage), then Focus; no Health (bought
    // in the field it doesn't heal).
    fn build(&self) -> &'static [I] {
        &[
            I::PowerShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfStrength,
            I::PowerShardOfFocus,
            I::PowerShardOfStrength,
        ]
    }

    fn shops_anywhere(&self) -> bool {
        true
    }
}
