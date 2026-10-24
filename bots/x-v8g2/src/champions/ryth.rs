// Ryth (melee tank). Passive: gains Strength equal to 1% of bonus health.
// Slot 0: area stun, 1 x Focus damage and one-round stun in radius one, ground, range 4, excludes walking.
// Slot 1: attack reset.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Ryth;

impl ChampionAi for Ryth {
    // Build: Jungle, then Bruiser Health (his health also adds Strength) with a Durability emblem
    // after the first (see ChampionAi::build). 2026-10-22, 320 games of Bruiser builds against the
    // Power ones: 193-127, his deaths 3.1 against 3.8.
    fn build(&self) -> &'static [I] {
        &[
            I::JungleShardOfHealth, // less camp damage (see ChampionAi::build)
            I::BruiserShardOfHealth,
            I::DurabilityShardOfHealth,
            I::BruiserShardOfHealth,
        ]
    }
}
