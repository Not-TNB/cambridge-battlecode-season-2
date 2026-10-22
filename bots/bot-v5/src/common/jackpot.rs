// Jackpots: the camp groups worth the most gems a round. On the ladder (135 games, 2026-10-22) the
// team that took the most gems from a map's top group won: we went 62-4 when it was us and 16-50
// when it was the opponent (3-22 where that group gives 25%+ of the map's gems). We farmed the
// nearest camps whatever they were worth, joined elk rallies only when no camp was closer, and
// dropped any group an enemy could reach: opponents sat on 01elk-arena's middle double elk (1,440
// gems to our 144) and won though we won the fights 33-15.
//
// A group is a jackpot when its gems a round (gems / (respawn + JACKPOT_ROUND_OVERHEAD)) are at least
// JACKPOT_SHARE of the map's total (groups nobody can walk to left out). Jackpot play is on when the
// jackpots lie in at most JACKPOT_MAX_CLUSTERS places (groups within JACKPOT_VICINITY tiles of each
// other are one place): with more, chasing them spreads the team over the map. Worked out from the
// map alone, so every champion (and the draft) comes to the same answer. Off: no jackpots.
use super::world::{camp_groups, distance};
use cambc::{Result, Vec2};
use std::cell::OnceCell;

pub const JACKPOT: bool = true;
// Monolith groups (20,000 health, hitting for 200) can be jackpots too. Off: they're left out, rates
// and all, so a map whose only jackpots are monoliths has none.
pub const JACKPOT_MONOLITHS: bool = true;
// Jackpot play is off where a jackpot has a monolith (it takes most of the team, and waiting for it
// starved the rest of the farming). Off: monolith jackpots are played like any other.
pub const JACKPOT_NO_MONOLITH_MAPS: bool = true;
const JACKPOT_SHARE: f32 = 0.15;
const JACKPOT_ROUND_OVERHEAD: f32 = 20.0;
const JACKPOT_VICINITY: i32 = 10;
// 2026-10-22, ~2,900 games of jackpot variants against v4 (40 maps), by map: one jackpot place, no
// monolith 55% (off 53%: 01elk-arena 69%, 00gem-fields 70%, 01concentrated-wealth 66%); one place
// with a monolith 37% (off 51%); two places 30-35% (off 45-48%). So one place only.
const JACKPOT_MAX_CLUSTERS: usize = 1;

pub struct Jackpots {
    // Jackpot groups (indices into camp_groups()); empty when jackpot play is off on this map.
    pub groups: Vec<usize>,
    // Places (clusters) the jackpots lie in, and whether play is on.
    pub clusters: usize,
    pub on: bool,
}

fn centre(group: &cambc::CampGroup) -> Vec2 {
    let n = group.camps().len().max(1) as i32;
    let (x, y) = group.camps().iter().fold((0, 0), |(x, y), c| {
        (x + c.spawn_position().x, y + c.spawn_position().y)
    });
    Vec2::new(x / n, y / n)
}

// The map's jackpots, worked out once. `walk_check` leaves out groups the region table says nobody
// walks to (not available during the draft: every group counts there).
fn analyse(walk_check: bool) -> Result<Jackpots> {
    let groups = camp_groups()?;
    let mut rates = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let walkable = !walk_check || crate::regions::walkable(index)?;
        let monolith = group
            .camps()
            .iter()
            .any(|c| c.kind() == cambc::CampType::Monolith);
        let rate = if walkable && (JACKPOT_MONOLITHS || !monolith) {
            group.gem_value() as f32
                / (group.max_respawn_timer().max(0) as f32 + JACKPOT_ROUND_OVERHEAD)
        } else {
            0.0
        };
        rates.push(rate);
    }
    let total: f32 = rates.iter().sum();
    let mut picked = Vec::new();
    let mut centres = Vec::new(); // of the picked groups, for the clusters
    if total > 0.0 {
        for (index, &rate) in rates.iter().enumerate() {
            if rate > 0.0 && rate >= JACKPOT_SHARE * total {
                picked.push(index);
                centres.push(centre(&groups[index]));
            }
        }
    }
    // Clusters: single linkage within JACKPOT_VICINITY.
    let mut cluster_of: Vec<usize> = (0..picked.len()).collect();
    for i in 0..picked.len() {
        for j in 0..i {
            if distance(centres[i], centres[j]) <= JACKPOT_VICINITY {
                let (a, b) = (cluster_of[i], cluster_of[j]);
                for c in cluster_of.iter_mut() {
                    if *c == a {
                        *c = b;
                    }
                }
            }
        }
    }
    let mut distinct = cluster_of.clone();
    distinct.sort_unstable();
    distinct.dedup();
    let clusters = distinct.len();
    let monolith = picked.iter().any(|&i| {
        groups[i]
            .camps()
            .iter()
            .any(|c| c.kind() == cambc::CampType::Monolith)
    });
    let on = JACKPOT
        && clusters >= 1
        && clusters <= JACKPOT_MAX_CLUSTERS
        && !(JACKPOT_NO_MONOLITH_MAPS && monolith);
    if !on {
        picked.clear();
    }
    Ok(Jackpots {
        groups: picked,
        clusters,
        on,
    })
}

pub fn jackpots() -> Result<&'static Jackpots> {
    thread_local! {
        static JACKPOTS: OnceCell<&'static Jackpots> = const { OnceCell::new() };
    }
    if let Some(j) = JACKPOTS.with(|c| c.get().copied()) {
        return Ok(j);
    }
    // In the draft no champion exists and the region table is still being written: work it out
    // without the walking check, and don't keep it.
    let in_play = cambc::self_champion().ok().flatten().is_some();
    let j: &'static Jackpots = Box::leak(Box::new(analyse(in_play)?));
    if !in_play {
        return Ok(j);
    }
    cambc::debug_block! {
        println!("jackpots: {} places, on {}, groups {:?}", j.clusters, j.on, j.groups);
    }
    JACKPOTS.with(|c| {
        let _ = c.set(j);
    });
    Ok(j)
}

// Whether group `index` is a jackpot (and jackpot play is on).
pub fn is_jackpot(index: usize) -> bool {
    jackpots().is_ok_and(|j| j.on && j.groups.contains(&index))
}

// The jackpot squad: only the champions of our first JACKPOT_SQUAD draft picks play for jackpots
// (farm.rs: the pull, the return, contesting, the rally reach); the rest farm as before. With the
// whole team on them (2026-10-22, 84 traced games on the 21 jackpot maps against v4) our share of
// the jackpots' gems went from 44% to 58%, but every other group's gems fell as much (five
// champions crowding a group two or three clear, the reward split between them) and gold fell 14%.
// 5 (or more): everyone.
// 2026-10-22, the 8 maps jackpot play is on for (160 games each against v4): off 75, squad 3 86,
// squad 5 95 (jackpot gems 62% ours, gems collected +22%, gold -8%).
pub const JACKPOT_SQUAD: usize = 5;

pub fn in_squad() -> bool {
    thread_local! {
        static IN: OnceCell<bool> = const { OnceCell::new() };
    }
    IN.with(|c| {
        *c.get_or_init(|| {
            let mine = crate::common::me()
                .and_then(|m| m.champion_name())
                .map(crate::constants::draft_of);
            let picks = cambc::draft::current_draft(cambc::Team::Ally).unwrap_or_default();
            match mine {
                Ok(pick) => picks
                    .iter()
                    .position(|&p| p == pick)
                    .is_some_and(|i| i < JACKPOT_SQUAD),
                Err(_) => true,
            }
        })
    })
}
