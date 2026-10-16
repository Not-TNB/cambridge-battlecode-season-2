// Willow (ranged healer).
// Slot 0: ally heal, up to 3 x Focus, range 4 (not cast: non-damaging).
// Slot 1: attack reset.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Willow;

impl ChampionAi for Willow {
    // Build: AP max; Power -> Bruiser.
    fn build(&self) -> &'static [I] {
        &[I::PowerShardOfFocus, I::BruiserShardOfFocus, I::PowerShardOfFocus]
    }
}
