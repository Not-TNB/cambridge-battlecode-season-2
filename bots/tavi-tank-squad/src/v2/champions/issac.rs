// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
// Issac (melee, Cassi & Issac pick): linked with Cassi, if either dies both die.
// Slot 0: attack reset. Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Issac;

impl ChampionAi for Issac {
    // Build: AD -> some hp; Treasure / Monster -> Power / Bruiser.
    fn build(&self) -> &'static [I] {
        &[
            I::TreasureShardOfStrength,
            I::MonsterShardOfStrength,
            I::PowerShardOfStrength,
            I::BruiserShardOfHealth,
            I::PowerShardOfStrength,
        ]
    }
}
