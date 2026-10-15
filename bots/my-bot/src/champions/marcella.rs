// Marcella (melee tank). Passive: immune to stuns.
// Slot 0: enhanced attack reset, next attack stuns for one round.
// Slot 1: single-target damage, 2 x Focus, enemy or camp, range 4, excludes walking.
use super::ChampionAi;

#[derive(Default)]
pub struct Marcella;

impl ChampionAi for Marcella {}
