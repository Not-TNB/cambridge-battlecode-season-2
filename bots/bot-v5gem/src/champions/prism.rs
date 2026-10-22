// Prism (ranged spellcaster). Passive: +0.2 Focus per carried gem.
// Slot 0: 1.5 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
// Slot 1: area damage, 1 x Focus in radius one, ground, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Prism;

impl ChampionAi for Prism {
    // Build: Monster, then Bruiser Focus (see ChampionAi::build). 2026-10-22, 320 games of these
    // builds for Prism, Stellar, Willow, Cassi and Issac against the Power ones: 165-155 (even);
    // Prism's gems 163 against 146, deaths 3.6 against 4.0 (93 games).
    fn build(&self) -> &'static [I] {
        &[I::MonsterShardOfFocus, I::BruiserShardOfFocus]
    }
}
