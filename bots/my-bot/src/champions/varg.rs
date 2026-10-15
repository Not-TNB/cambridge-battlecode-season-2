// Varg (melee bruiser). Passive: basic attacks splash Strength damage in radius one.
// Slot 0: attack reset.
// Slot 1: self-shield, 2.5 x Focus for five turns (not cast: non-damaging).
use super::ChampionAi;

#[derive(Default)]
pub struct Varg;

impl ChampionAi for Varg {}
