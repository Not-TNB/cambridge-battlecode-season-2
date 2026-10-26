// Stellar (ranged spellcaster).
// Slot 0: single-target damage, 2 x Focus, enemy or camp, range 4, excludes walking.
// Slot 1: area damage, 1.4 x Focus in radius one, ground, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Stellar;

impl ChampionAi for Stellar {
    // Build: Treasure, then Bruiser Focus (see ChampionAi::build). 2026-10-22, 320 games of these
    // builds for Prism, Stellar, Willow, Cassi and Issac against the Power ones: 165-155 (even);
    // Stellar's deaths 3.0 against 3.9, kills 3.4 against 2.6 (36 games).
    fn build(&self) -> &'static [I] {
        &[I::TreasureShardOfFocus, I::BruiserShardOfFocus]
    }
}
