// Shared helpers every champion can use, by topic. The rest of the crate goes through the
// re-exports below (crate::common::...); files in here import from each other directly.
pub mod activity;
mod assist;
mod attack;
pub mod claims;
pub mod locks;
mod respawns;
mod base;
mod camps;
mod chase;
pub mod collect;
mod duel;
mod farm;
mod fight;
mod movement;
mod rally;
pub mod safety;
mod threats;
mod trace;
mod walk;
mod ward;
mod world;

pub use assist::assist_allies;
pub use attack::ability_ready;
pub use base::{
    bank_gems, base_willingness, end_game_bank, should_bank, unlocked_altar_within, BANK_BEFORE_RECALL,
    BANK_BEFORE_RECALL_TILES,
};
use cambc::{Champion, Result};
pub use farm::{
    current_target, farm_camps, forget_far_cache, forget_presumed, hear_respawns, join_camp_fight,
};
pub use fight::{fight_enemies, fightable_enemies};
pub use movement::{move_towards_closest, ready_dash};
pub use trace::{checkpoint, take_trace, trace, trace_wanted, TRACE};
pub use walk::prepare_walk_fields;
pub use threats::burst_damage_here;
pub use ward::ward_hidden_spots;
pub use world::{blocked_tiles, camp_groups, distance, forget_sight, sight, tiles_around};

pub fn me() -> Result<&'static Champion> {
    Ok(cambc::self_champion()?.expect("play starts after the draft"))
}
