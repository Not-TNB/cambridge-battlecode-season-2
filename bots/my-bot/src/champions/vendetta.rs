// Vendetta (melee assassin). Passive: kills (including camps) reset all ability cooldowns.
// Slot 0: attack reset. Slot 1: dash up to 2 tiles (not cast: non-damaging).
use super::ChampionAi;

#[derive(Default)]
pub struct Vendetta;

impl ChampionAi for Vendetta {}
