// Prism (ranged spellcaster). Passive: +0.2 Focus per carried gem.
// Slot 0: 1.5 x Focus damage and one-round stun, enemy or camp, range 4, excludes walking.
// Slot 1: area damage, 1 x Focus in radius one, ground, range 4, excludes walking.
use super::ChampionAi;

#[derive(Default)]
pub struct Prism;

impl ChampionAi for Prism {}
