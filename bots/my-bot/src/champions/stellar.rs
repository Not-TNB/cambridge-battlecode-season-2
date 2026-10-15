// Stellar (ranged spellcaster).
// Slot 0: single-target damage, 2 x Focus, enemy or camp, range 4, excludes walking.
// Slot 1: area damage, 1.4 x Focus in radius one, ground, range 4, excludes walking.
use super::ChampionAi;

#[derive(Default)]
pub struct Stellar;

impl ChampionAi for Stellar {}
