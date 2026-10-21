// Marcella (melee tank). Passive: immune to stuns.
// Slot 0: enhanced attack reset, next attack stuns for one round.
// Slot 1: single-target damage, 2 x Focus, enemy or camp, range 4, excludes walking.
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Marcella;

impl ChampionAi for Marcella {
    // Build: Jungle health, then Strength and health alternately. She has no health-to-damage
    // passive (Ryth does), so the old all-health build left her hitting for her base 45.
    // 2026-10-21, 132 games: 69-63 against all-health, 23-19 where both sides had her.
    fn build(&self) -> &'static [I] {
        &[
            I::JungleShardOfHealth, // less camp damage (see ChampionAi::build)
            I::PowerShardOfStrength,
            I::PowerShardOfHealth,
            I::BruiserShardOfStrength,
            I::PowerShardOfHealth,
        ]
    }
}
