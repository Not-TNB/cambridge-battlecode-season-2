// Ren (melee bruiser). Passive: retaliates for 0.2 x Focus when damaged.
// Slot 0: 1.25 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
// Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;
use cambc::ItemType as I;

#[derive(Default)]
pub struct Ren;

impl ChampionAi for Ren {
    // Build: Jungle health, then Focus (his stun-spell and his retaliation on every hit he takes
    // scale with it) and health alternately. 2026-10-21, 132 games: 72-60 against the old
    // health-first build (Bruiser, Durability, then Focus), 23-15 where only our side had him.
    fn build(&self) -> &'static [I] {
        &[
            I::JungleShardOfHealth, // less camp damage (see ChampionAi::build)
            I::PowerShardOfFocus,
            I::BruiserShardOfHealth,
            I::PowerShardOfFocus,
            I::PowerShardOfHealth,
        ]
    }
}
