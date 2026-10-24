// Prism (ranged spellcaster). Passive: +1 Focus per carried gem (lost on tributing or dying).
// Slot 0: 1.5 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
// Slot 1: area damage, 1 x Focus in radius one, ground, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

// Health with several passives (Durability, Jungle, then Bruiser Health and Focus): her Focus comes
// from the gems she carries (base::PRISM_BANK_MIN_GEMS), and at 700 health she dies with the bag.
// Off: Monster, then Bruiser Focus. 2026-10-22, Prism in both teams, 320 games against off: 176-144
// (team tributes 28.9 against 27.6 a game; with base::PRISM_BANK_MIN_GEMS 150 as well 150-170, at 80
// 158-162: her bag grew, the team's tributes fell).
const PRISM_HEALTH_BUILD: bool = true;

#[derive(Default)]
pub struct Prism;

impl ChampionAi for Prism {
    // Build: Monster, then Bruiser Focus (see ChampionAi::build). 2026-10-22, 320 games of these
    // builds for Prism, Stellar, Willow, Cassi and Issac against the Power ones: 165-155 (even);
    // Prism's gems 163 against 146, deaths 3.6 against 4.0 (93 games).
    fn build(&self) -> &'static [I] {
        if PRISM_HEALTH_BUILD {
            &[
                I::MonsterShardOfFocus,
                I::DurabilityShardOfHealth,
                I::JungleShardOfHealth,
                I::BruiserShardOfHealth,
                I::BruiserShardOfFocus,
            ]
        } else {
            &[I::MonsterShardOfFocus, I::BruiserShardOfFocus]
        }
    }
}
