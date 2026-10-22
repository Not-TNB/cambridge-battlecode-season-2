// Marcella (melee tank). Passive: immune to stuns.
// Slot 0: enhanced attack reset, next attack stuns for one round.
// Slot 1: single-target damage, 2 x Focus, enemy or camp, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Marcella;

impl ChampionAi for Marcella {
    // Build: Jungle health, then Bruiser Strength with a Durability emblem after the first (see
    // ChampionAi::build). She has no health-to-damage passive (Ryth does), so the old all-health
    // build left her hitting for her base 45: 2026-10-21, 132 games, Strength and health
    // alternately went 69-63 against it. 2026-10-22, 320 games of Bruiser builds against that:
    // 193-127, her gold +9%, her deaths 2.5 against 3.0.
    fn build(&self) -> &'static [I] {
        &[
            I::JungleShardOfHealth, // less camp damage (see ChampionAi::build)
            I::BruiserShardOfStrength,
            I::DurabilityShardOfHealth,
            I::BruiserShardOfStrength,
        ]
    }
}
