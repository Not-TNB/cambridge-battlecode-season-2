// Ren (melee bruiser). Passive: retaliates for 0.2 x Focus when damaged.
// Slot 0: 1.25 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
// Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Ren;

impl ChampionAi for Ren {
    // Build: Jungle health, then Bruiser Focus (his stun-spell and his retaliation on every hit he
    // takes scale with Focus) with a Durability emblem after the first (see ChampionAi::build).
    // 2026-10-21, 132 games: Focus and health alternately went 72-60 against the old health-first
    // build. 2026-10-22, 320 games of Bruiser builds against that: 193-127, his gold +19%, his
    // deaths 2.4 against 4.1.
    fn build(&self) -> &'static [I] {
        &[
            I::JungleShardOfHealth, // less camp damage (see ChampionAi::build)
            I::BruiserShardOfFocus,
            I::DurabilityShardOfHealth,
            I::BruiserShardOfFocus,
        ]
    }
}
