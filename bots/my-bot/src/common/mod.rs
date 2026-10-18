// Shared helpers every champion can use, by topic. The rest of the crate goes through the
// re-exports below (crate::common::...); files in here import from each other directly.
mod attack;
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
mod ward;
mod world;

pub use base::{
    bank_gems, base_willingness, should_bank, unlocked_altar_within, BANK_BEFORE_RECALL,
    BANK_BEFORE_RECALL_TILES,
};
use cambc::{Champion, Result};
pub use farm::farm_camps;
pub use fight::fight_enemies;
pub use movement::{move_towards_closest, ready_dash};
pub use trace::{checkpoint, take_trace, trace, TRACE};
pub use ward::ward_hidden_spots;
pub use world::{blocked_tiles, camp_groups, distance, forget_sight, sight, tiles_around};

pub fn me() -> Result<&'static Champion> {
    Ok(cambc::self_champion()?.expect("play starts after the draft"))
}
