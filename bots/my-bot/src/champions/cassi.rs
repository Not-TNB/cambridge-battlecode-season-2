// Cassi (ranged, Cassi & Issac pick): linked with Issac, if either dies both die.
// Slot 0: single-target damage, 2 x Focus, enemy or camp, range 4, excludes walking.
// Slot 1: one-round stun with no damage, enemy or camp, range 4 (not cast: non-damaging).
use super::ChampionAi;

#[derive(Default)]
pub struct Cassi;

impl ChampionAi for Cassi {}
