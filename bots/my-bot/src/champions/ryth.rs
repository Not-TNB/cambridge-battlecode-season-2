// Ryth (melee tank). Passive: gains Strength equal to 1% of bonus health.
// Slot 0: area stun, 1 x Focus damage and one-round stun in radius one, ground, range 4, excludes walking.
// Slot 1: attack reset.
use super::ChampionAi;

#[derive(Default)]
pub struct Ryth;

impl ChampionAi for Ryth {}
