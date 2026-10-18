// Prism (ranged spellcaster). Passive: +0.2 Focus per carried gem.
// Slot 0: 1.5 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
// Slot 1: area damage, 1 x Focus in radius one, ground, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Prism;

impl ChampionAi for Prism {
    // Build: AP -> hp; Monster -> Power / Bruiser.
    fn build(&self) -> &'static [I] {
        &[I::MonsterShardOfFocus, I::PowerShardOfFocus, I::BruiserShardOfHealth, I::PowerShardOfFocus]
    }
}
