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
    // The last entry repeats, so that build ended Power Health x10-29 in long games (pure health
    // after the first four emblems); Strength and Health now alternate to the end.
    fn build(&self) -> &'static [I] {
        &BUILD
    }
}

const BUILD: [I; 40] = {
    let mut build = [I::PowerShardOfHealth; 40];
    build[0] = I::JungleShardOfHealth; // less camp damage (see ChampionAi::build)
    build[1] = I::PowerShardOfStrength;
    build[2] = I::PowerShardOfHealth;
    build[3] = I::BruiserShardOfStrength;
    let mut i = 4;
    while i < build.len() {
        if i % 2 == 0 {
            build[i] = I::PowerShardOfStrength;
        }
        i += 1;
    }
    build
};
