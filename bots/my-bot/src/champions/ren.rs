// Ren (melee bruiser). Passive: retaliates for 0.2 x Focus when damaged.
// Slot 0: 1.25 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
// Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;

#[derive(Default)]
pub struct Ren;

impl ChampionAi for Ren {}
