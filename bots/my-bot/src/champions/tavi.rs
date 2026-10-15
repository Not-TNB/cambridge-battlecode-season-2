// Tavi (ranged). Passive: buys shards anywhere on the map (consumables only at spawn or dead);
// can sell equipment at spawn for full value.
// Slot 0: attack reset.
// Slot 1: 2.2 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
use super::ChampionAi;

#[derive(Default)]
pub struct Tavi;

impl ChampionAi for Tavi {
    fn shops_anywhere(&self) -> bool { true }
}
