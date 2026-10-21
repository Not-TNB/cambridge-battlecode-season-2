// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
// Stellar (ranged spellcaster).
// Slot 0: single-target damage, 2 x Focus, enemy or camp, range 4, excludes walking.
// Slot 1: area damage, 1.4 x Focus in radius one, ground, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Stellar;

impl ChampionAi for Stellar {
    // Build: AP max; Power / Treasure -> Power / Bruiser.
    fn build(&self) -> &'static [I] {
        &[
            I::PowerShardOfFocus,
            I::TreasureShardOfFocus,
            I::PowerShardOfFocus,
            I::BruiserShardOfFocus,
            I::PowerShardOfFocus,
        ]
    }
}
