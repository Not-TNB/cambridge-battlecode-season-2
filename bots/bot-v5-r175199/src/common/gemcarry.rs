// Gem carry: on maps with plenty of gems but little gold, Marcella and Prism are drafted for their
// passives and fed the team's captures. Marcella gains 2 gold per gem she tributes, so early on she
// takes the gem captures and turns them into gold (items) at the altar; later Prism (1 Focus per
// carried gem) takes them and grows with the bag she carries. The rest of the team leaves a cleared
// group's capture to the carry when the carry is close by (GEM_CARRY_HELPERS).
//
// A map qualifies when its camp groups' gold a round over their gems a round (each group's value
// over its respawn plus GEM_CARRY_OVERHEAD rounds, monolith groups at a quarter) is at most
// GEM_CARRY_GOLD_PER_GEM: 00gem-fields 1.7, 00quadrants 1.8, 00gemfight 2.4, 00lost-world 3.0,
// 00great-wall 3.2, 00road-of-riches 3.3; the next is 00reverse-spiral at 3.8, most maps 5-10.
// Worked out from the map alone, so the draft and every champion agree.
// 2026-10-22, on those six maps against bot-v5 (96 games each): helpers always leave captures
// 43-53; only strong helpers leave them 43-53; always, plus Prism banking from 150 gems 49-47
// (before Prism's Health build). On at the user's request (2026-10-22: the top ladder team plays
// it). Retested on the current bot (with Prism's Health build), one seed on the six maps against
// it off: 8/12 (gold 3.8 a round against 3.0, deaths 11.5 against 16.2); with GEM_CARRY_PRISM_BANK
// 150 as well, 3/12 (tributes 21.4 a game against 35.2).
use super::me;
use super::world::{camp_groups, distance, sight};
use cambc::{CampGroup, ChampionDraft, ChampionType, Result};
use std::cell::OnceCell;

pub const GEM_CARRY: bool = true;
pub const GEM_CARRY_GOLD_PER_GEM: f32 = 3.5;
const GEM_CARRY_OVERHEAD: f32 = 20.0;
// Marcella carries before this round, Prism from it (either alone carries all game).
pub const GEM_CARRY_SWITCH_ROUND: u32 = 600;
// The rest of the team: 0 leaves every capture to the carry when it's within GEM_CARRY_NEAR_TILES
// of the group; 1 only once it has GEM_CARRY_STRONG_EMBLEMS emblems (weaker, it takes them).
pub const GEM_CARRY_HELPERS: u8 = 0;
const GEM_CARRY_STRONG_EMBLEMS: usize = 2;
const GEM_CARRY_NEAR_TILES: i32 = 8;
// While Prism carries she banks only from this many gems (her Focus grows with the bag). 0: as
// everyone else (base::BANK_MIN_GEMS).
pub const GEM_CARRY_PRISM_BANK: u32 = 0;

// Whether this map is a gem-carry map (cached).
pub fn gem_map() -> bool {
    thread_local! {
        static ON: OnceCell<bool> = const { OnceCell::new() };
    }
    if !GEM_CARRY {
        return false;
    }
    ON.with(|c| *c.get_or_init(|| ratio().is_some_and(|r| r <= GEM_CARRY_GOLD_PER_GEM)))
}

fn ratio() -> Option<f32> {
    let (mut gold, mut gems) = (0.0, 0.0);
    for group in camp_groups().ok()? {
        let monolith = group.camps().iter().any(|c| c.kind() == cambc::CampType::Monolith);
        let weight = if monolith { 0.25 } else { 1.0 };
        let cycle = group.max_respawn_timer().max(0) as f32 + GEM_CARRY_OVERHEAD;
        gold += weight * group.gold_value() as f32 / cycle;
        gems += weight * group.gem_value() as f32 / cycle;
    }
    (gems > 0.0).then(|| gold / gems)
}

// The draft: Marcella first, then Prism, while either is still free and missing from our team.
pub fn draft_pick(free: &[ChampionDraft], ours: &[ChampionDraft]) -> Option<ChampionDraft> {
    if !gem_map() {
        return None;
    }
    [ChampionDraft::Marcella, ChampionDraft::Prism]
        .into_iter()
        .find(|p| free.contains(p) && !ours.contains(p))
}

fn team_has(champion: ChampionType) -> bool {
    let draft = crate::constants::draft_of(champion);
    cambc::draft::current_draft(cambc::Team::Ally).is_ok_and(|t| t.contains(&draft))
}

// This round's carry, if this is a gem-carry map and we have one.
pub fn carry() -> Option<ChampionType> {
    if !gem_map() {
        return None;
    }
    let (marcella, prism) = (team_has(ChampionType::Marcella), team_has(ChampionType::Prism));
    let late = cambc::round_number().ok()? >= GEM_CARRY_SWITCH_ROUND;
    match (marcella, prism) {
        (true, true) => Some(if late { ChampionType::Prism } else { ChampionType::Marcella }),
        (true, false) => Some(ChampionType::Marcella),
        (false, true) => Some(ChampionType::Prism),
        _ => None,
    }
}

pub fn am_carry() -> bool {
    carry().is_some_and(|c| me().and_then(|m| m.champion_name()).is_ok_and(|n| n == c))
}

// Whether we (not the carry) leave `group`'s capture to the carry: it's alive, on its feet and
// within GEM_CARRY_NEAR_TILES of one of the group's camps.
pub fn leaves_to_carry(group: &CampGroup) -> Result<bool> {
    let Some(carry) = carry() else {
        return Ok(false);
    };
    if am_carry() {
        return Ok(false);
    }
    if GEM_CARRY_HELPERS == 1 && emblems()? < GEM_CARRY_STRONG_EMBLEMS {
        return Ok(false);
    }
    for ally in &sight()?.allies {
        if ally.champion_name()? != carry || !ally.is_alive()? || ally.is_recalling()? {
            continue;
        }
        let p = ally.position()?;
        if group.camps().iter().any(|c| distance(c.spawn_position(), p) <= GEM_CARRY_NEAR_TILES) {
            return Ok(true);
        }
    }
    Ok(false)
}

// Completed emblems we own (items that don't combine into anything).
fn emblems() -> Result<usize> {
    let mut n = 0;
    for (item, count) in me()?.item_count()?.iter() {
        if item.result()?.is_none() {
            n += count as usize;
        }
    }
    Ok(n)
}

// The bank threshold while Prism carries (GEM_CARRY_PRISM_BANK), if it applies to us.
pub fn prism_bank_min() -> Option<u32> {
    (GEM_CARRY_PRISM_BANK > 0
        && am_carry()
        && me().and_then(|m| m.champion_name()).is_ok_and(|n| n == ChampionType::Prism))
    .then_some(GEM_CARRY_PRISM_BANK)
}
