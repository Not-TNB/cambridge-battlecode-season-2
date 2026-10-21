// Hot spots: during the draft, rank camp groups by walking distance from our spawn (separately for
// gold and gems) and give each carry a different top spot, so carries spread out when farming.
// Tanks and healers copy the lists of a carry (squishy ADCs and mages first) and so follow it.
// The plan goes into the 1024-byte draft memory, which champions read (read-only) during play.
//
// Draft memory layout: b'H', version, entry count, then per entry:
//   draft pick (u8), role (0 carry, 1 follower), gold count, gold spots (x, y as u8),
//   gem count, gem spots (x, y as u8).
// A spot is the spawn tile of a group's first camp.
use crate::constants::{draft_class, draft_of, ClassKind};
use cambc::{Bitboard, ChampionDraft, ChampionType, DistanceField, Result, Team, Vec2};
use std::cell::OnceCell;

// Turn hot spots off (champions then farm the nearest camps).
pub const HOTSPOTS: bool = false;
// Before this round everyone farms the nearest camps together (nobody has resources yet).
pub const HOTSPOT_START_ROUND: u32 = 30;
// In play, only a champion's first this-many spots count; otherwise it farms the nearest camps.
pub const HOTSPOT_TOP_K: usize = 3;
// Only rank groups on our side: the enemy's walking distance to them must be more than
// HOTSPOT_SAFETY times ours (1.0 = our half). Lone carries far out get picked off.
pub const HOTSPOT_OWN_HALF: bool = true;
pub const HOTSPOT_SAFETY: f32 = 1.0;
// Distinct top spots handed out (carries share them round-robin); 0 = one per carry.
pub const HOTSPOT_SQUADS: usize = 0;
// In play, drop the hot spot (back to nearest-camp farming with the team) while an enemy
// champion is in sight.
pub const HOTSPOT_AVOID_ENEMIES: bool = false;
// In play, skip a spot more than this many tiles (Chebyshev) further than the nearest available camp.
pub const HOTSPOT_MAX_DETOUR: i32 = 8;
// Spots stored per list.
const LIST_LEN: usize = 8;
const MAGIC: u8 = b'H';
const VERSION: u8 = 1;

// Base gold and gems per monster, from the rules.
fn rewards(kind: cambc::CampType) -> (u32, u32) {
    use cambc::CampType as C;
    match kind {
        C::Wolf => (90, 22),
        C::Elk => (160, 36),
        C::GemCluster => (0, 24),
        C::GoldCluster => (35, 0),
        C::Monolith => (300, 200),
        C::Golem => (160, 36), // new in 3.0
    }
}

fn is_follower(pick: ChampionDraft) -> bool {
    matches!(draft_class(pick), ClassKind::Tank | ClassKind::Healer)
        || pick == ChampionDraft::HeartOfRuin
}

// Squishy carries first: they get the closest spots and the followers.
fn carry_order(pick: ChampionDraft) -> u8 {
    match draft_class(pick) {
        ClassKind::DpsAd | ClassKind::DpsAp => 0,
        ClassKind::AssassinAndDpsAp => 1,
        ClassKind::Assassin => 2,
        _ => 3,
    }
}

// Fuel to keep for the pick itself; below this the plan is skipped (missing a pick means the game
// picks for us at random).
const PLAN_MIN_FUEL: u64 = 20_000;

// Draft: submit the plan for our picks so far (rewritten before each of our picks). Only the first
// draft turn has ample fuel (~1M; later turns get ~100k), so the ranking is computed then and kept.
pub fn submit_plan(picks: &[ChampionDraft]) -> Result<()> {
    if !HOTSPOTS {
        return Ok(());
    }
    thread_local! {
        static RANKED: OnceCell<(Vec<Vec2>, Vec<Vec2>)> = const { OnceCell::new() };
    }
    if RANKED.with(|cell| cell.get().is_none()) {
        let ranked = rank_spots()?;
        RANKED.with(|cell| {
            let _ = cell.set(ranked);
        });
    }
    if cambc::fuel_remaining()? < PLAN_MIN_FUEL {
        return Ok(());
    }
    let bytes = RANKED.with(|cell| encode_plan(picks, cell.get().expect("set above")));
    cambc::draft_memory().submit(&bytes)
}

// Groups ranked closest to our spawn first, as (gold spots, gem spots).
fn rank_spots() -> Result<(Vec<Vec2>, Vec<Vec2>)> {
    // Walking distance from our spawn over the static map (camps are not obstacles yet).
    let mut walls = cambc::map::walls()?.clone();
    for mask in [
        cambc::map::holes()?,
        cambc::map::altars()?,
        cambc::map::spawn_region(Team::Enemy)?,
    ] {
        walls.union_assign(mask)?;
    }
    let mut field = DistanceField::new()?;
    field.compute(
        &walls,
        cambc::map::spawn_region(Team::Ally)?,
        &mut Bitboard::new()?,
        1000,
    )?;
    let mut enemy_walls = cambc::map::walls()?.clone();
    for mask in [
        cambc::map::holes()?,
        cambc::map::altars()?,
        cambc::map::spawn_region(Team::Ally)?,
    ] {
        enemy_walls.union_assign(mask)?;
    }
    let mut enemy_field = DistanceField::new()?;
    enemy_field.compute(
        &enemy_walls,
        cambc::map::spawn_region(Team::Enemy)?,
        &mut Bitboard::new()?,
        1000,
    )?;

    // (distance, value, spot) per group, for each list.
    let mut gold = Vec::new();
    let mut gems = Vec::new();
    for group in cambc::map::camp_groups()? {
        let camps = group.camps();
        let Some(first) = camps.first() else { continue };
        let dist = camps
            .iter()
            .map(|c| {
                field
                    .get(c.spawn_position())
                    .unwrap_or(DistanceField::UNREACHABLE)
            })
            .min()
            .unwrap_or(DistanceField::UNREACHABLE);
        if dist == DistanceField::UNREACHABLE {
            continue;
        }
        let enemy_dist = camps
            .iter()
            .map(|c| {
                enemy_field
                    .get(c.spawn_position())
                    .unwrap_or(DistanceField::UNREACHABLE)
            })
            .min()
            .unwrap_or(DistanceField::UNREACHABLE);
        if HOTSPOT_OWN_HALF && (enemy_dist as f32) <= dist as f32 * HOTSPOT_SAFETY {
            continue;
        }
        let (mut g, mut m, mut monolith) = (0, 0, false);
        for camp in camps {
            let (cg, cm) = rewards(camp.kind());
            g += cg;
            m += cm;
            monolith |= camp.kind() == cambc::CampType::Monolith;
        }
        let spot = first.spawn_position();
        if g > 0 && !monolith {
            gold.push((dist, g, spot));
        }
        if m > 0 {
            gems.push((dist, m, spot));
        }
    }
    // Closer first; ties go to the bigger reward.
    for list in [&mut gold, &mut gems] {
        list.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    }
    Ok((
        gold.iter().map(|&(_, _, s)| s).collect(),
        gems.iter().map(|&(_, _, s)| s).collect(),
    ))
}

fn encode_plan(picks: &[ChampionDraft], (gold, gems): &(Vec<Vec2>, Vec<Vec2>)) -> Vec<u8> {
    let mut carries: Vec<ChampionDraft> =
        picks.iter().copied().filter(|&p| !is_follower(p)).collect();
    carries.sort_by_key(|&p| carry_order(p));
    // Carry i gets the i-th closest spot on top (i mod squads), then the rest closest first.
    let rotate = |list: &[Vec2], i: usize| -> Vec<Vec2> {
        if list.is_empty() {
            return Vec::new();
        }
        let i = if HOTSPOT_SQUADS > 0 {
            i % HOTSPOT_SQUADS
        } else {
            i
        };
        let top = list[i % list.len()];
        std::iter::once(top)
            .chain(list.iter().copied().filter(|&s| s != top))
            .take(LIST_LEN)
            .collect()
    };
    let mut bytes = vec![MAGIC, VERSION, 0];
    let write = |bytes: &mut Vec<u8>, pick: ChampionDraft, role: u8, slot: usize| {
        bytes.push(pick as u8);
        bytes.push(role);
        for list in [rotate(gold, slot), rotate(gems, slot)] {
            bytes.push(list.len() as u8);
            for s in list {
                bytes.push(s.x as u8);
                bytes.push(s.y as u8);
            }
        }
        bytes[2] += 1;
    };
    for (slot, &pick) in carries.iter().enumerate() {
        write(&mut bytes, pick, 0, slot);
    }
    let mut follower = 0;
    for &pick in picks.iter().filter(|&&p| is_follower(p)) {
        // With no carries yet, a follower just takes its own slot.
        let slot = if carries.is_empty() {
            follower
        } else {
            follower % carries.len()
        };
        write(&mut bytes, pick, 1, slot);
        follower += 1;
    }
    bytes
}

// Play: this champion's (gold spots, gem spots) from the draft plan, if there is one.
pub fn my_spots() -> Option<&'static (Vec<Vec2>, Vec<Vec2>)> {
    thread_local! {
        static SPOTS: OnceCell<Option<&'static (Vec<Vec2>, Vec<Vec2>)>> = const { OnceCell::new() };
    }
    SPOTS.with(|cell| {
        *cell.get_or_init(|| {
            let me = cambc::self_champion()
                .ok()
                .flatten()?
                .champion_name()
                .ok()?;
            parse(cambc::draft_memory().bytes(), me).map(|spots| &*Box::leak(Box::new(spots)))
        })
    })
}

fn parse(bytes: &[u8], me: ChampionType) -> Option<(Vec<Vec2>, Vec<Vec2>)> {
    if bytes.len() < 3 || bytes[0] != MAGIC || bytes[1] != VERSION {
        return None;
    }
    let mine = draft_of(me) as u8;
    let mut at = 3;
    for _ in 0..bytes[2] {
        let pick = *bytes.get(at)?;
        at += 2; // pick, role
        let mut lists = [Vec::new(), Vec::new()];
        for list in &mut lists {
            let n = *bytes.get(at)? as usize;
            at += 1;
            for _ in 0..n {
                list.push(Vec2::new(
                    *bytes.get(at)? as i32,
                    *bytes.get(at + 1)? as i32,
                ));
                at += 2;
            }
        }
        if pick == mine {
            let [gold, gems] = lists;
            return Some((gold, gems));
        }
    }
    None
}
