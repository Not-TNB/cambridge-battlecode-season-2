// Regions: which camp groups we can walk to from our spawn, which only a dash reaches (across a
// strip of holes), which only a teleport crystal reaches, and which nobody can reach. On ambush a
// camp group sat in a room sealed off by walls and holes: champions picked it as their farm target,
// found no walking path and stood still on spawn for the rest of the game.
//
// The draft bot works the table out on its first turn (~1M fuel) and writes it into the draft
// memory, at REGION_OFFSET:
//   b'R', version, group count, then per group: its first camp's tile (x, y), access, then the
//   launch tile (x, y, in our spawn's region) and landing tile (x, y, in the group's pocket) of the
//   crossing; (255, 255) for both when there is none.
// Groups missing from the table count as walkable. Pockets reached only through another pocket
// (stepping stones) count as unreachable.
//
// Dashes, measured against the engine on ambush: up to 2 steps in any of the 8 directions, the
// middle tile may be a hole but not a wall, and the landing tile must be free ground. Crossings
// work both ways (landing back to launch). Teleport crystals (range 3, terrain ignored) are only
// classified so far: nothing buys or uses one yet.
use crate::common::{
    blocked_tiles, camp_groups, distance, forget_sight, me, move_towards_closest, ready_dash,
    tiles_around, trace,
};
use cambc::{Bitboard, DistanceField, Result, Team, Vec2};
use std::cell::OnceCell;

// Off: no group is filtered out of farming and nobody dashes into pockets.
pub const REGIONS: bool = true;
// Where the table starts in the draft memory.
const REGION_OFFSET: usize = 0;
const MAGIC: u8 = b'R';
const VERSION: u8 = 1;
const ENTRY_BYTES: usize = 7;
const NO_TILE: u8 = 255;
const DASH_STEPS: i32 = 2;
const CRYSTAL_RANGE: i32 = 3;
// Walking distances further than this count as unreachable (100x100 maps stay well under it).
const MAX_STEPS: u16 = 1000;
// Draft fuel kept for the pick: pockets not yet searched below this count as
// unreachable (missing the pick means the game picks for us at random).
const CLASSIFY_RESERVE_FUEL: u64 = 150_000;
// Fuel needed to start classifying at all (the first draft turn has ~1M).
const CLASSIFY_MIN_FUEL: u64 = 500_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Access {
    Walk = 0,
    Dash = 1,
    Crystal = 2,
    Unreachable = 3,
}

impl Access {
    fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0 => Access::Walk,
            1 => Access::Dash,
            2 => Access::Crystal,
            3 => Access::Unreachable,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy)]
pub struct Crossing {
    pub launch: Vec2,  // in our spawn's region
    pub landing: Vec2, // in the group's pocket
}

#[derive(Clone, Copy)]
struct Entry {
    key: Vec2, // the group's first camp tile
    access: Access,
    crossing: Option<Crossing>,
}

// Tiles we can't walk onto, whatever is standing there: walls, holes, altars and the enemy spawn.
pub(crate) fn walk_blocked() -> Result<Bitboard> {
    let mut blocked = cambc::map::walls()?.clone();
    for mask in [
        cambc::map::holes()?,
        cambc::map::altars()?,
        cambc::map::spawn_region(Team::Enemy)?,
    ] {
        blocked.union_assign(mask)?;
    }
    Ok(blocked)
}

// `mask` plus every tile within Chebyshev `by` of it: rows, then columns, so the cost doesn't
// depend on how big the mask is (drawing a square round each tile did).
fn grow(mask: &Bitboard, by: i32) -> Result<Bitboard> {
    let mut rows = mask.clone();
    let mut shifted = Bitboard::new()?;
    for k in 1..=by {
        for count in [k, -k] {
            mask.shift_right_into(count, &mut shifted)?;
            rows.union_assign(&shifted)?;
        }
    }
    let mut out = rows.clone();
    for k in 1..=by {
        for count in [k, -k] {
            rows.shift_up_into(count, &mut shifted)?;
            out.union_assign(&shifted)?;
        }
    }
    Ok(out)
}

// Draft: the table for our spawn. Every lookup runs on buffers copied into Rust memory, so the
// host calls are a handful per pocket rather than one per tile.
fn classify() -> Result<Vec<Entry>> {
    const OPEN: u8 = 0;
    const HOLE: u8 = 1;
    const WALL: u8 = 2;
    const OTHER: u8 = 3; // altars and the enemy spawn: no landing, and assumed to stop a dash
    let blocked = walk_blocked()?;
    let mut field = DistanceField::new()?;
    field.compute(
        &blocked,
        cambc::map::spawn_region(Team::Ally)?,
        &mut Bitboard::new()?,
        MAX_STEPS,
    )?;
    let (w, h) = (field.width(), field.height());
    let main = field.export_linear_buffer()?; // walking distance from our spawn
    let at = |p: Vec2| (p.y * w + p.x) as usize;
    let inside = |p: Vec2| p.x >= 0 && p.y >= 0 && p.x < w && p.y < h;

    let mut kind = vec![OPEN; (w * h) as usize];
    for (mask, k) in [
        (cambc::map::holes()?, HOLE),
        (cambc::map::altars()?, OTHER),
        (cambc::map::spawn_region(Team::Enemy)?, OTHER),
        (cambc::map::walls()?, WALL),
    ] {
        for p in mask.positions()? {
            kind[at(p)] = k;
        }
    }
    let groups = cambc::map::camp_groups()?;
    let mut camp = vec![false; (w * h) as usize];
    for group in &groups {
        for c in group.camps() {
            camp[at(c.spawn_position())] = true;
        }
    }
    // Launch and landing tiles: free ground in our region / the pocket, and not a camp's tile
    // (a live camp stands there).
    let launch_ok = |p: Vec2| {
        inside(p)
            && main[at(p)] != DistanceField::UNREACHABLE
            && kind[at(p)] == OPEN
            && !camp[at(p)]
    };

    // Pocket per tile, filled in as groups need them: 0 = not computed yet.
    let mut pocket_of = vec![0u16; (w * h) as usize];
    let mut main_board: Option<Bitboard> = None; // our region as a mask, once a pocket needs it
    let mut pockets: Vec<(Access, Option<Crossing>)> = Vec::new();
    let mut entries = Vec::new();
    for group in &groups {
        let Some(first) = group.camps().first() else {
            continue;
        };
        let key = first.spawn_position();
        let walkable = group
            .camps()
            .iter()
            .any(|c| main[at(c.spawn_position())] != DistanceField::UNREACHABLE);
        if walkable {
            entries.push(Entry {
                key,
                access: Access::Walk,
                crossing: None,
            });
            continue;
        }
        if pocket_of[at(key)] == 0 && cambc::fuel_remaining()? < CLASSIFY_RESERVE_FUEL {
            entries.push(Entry {
                key,
                access: Access::Unreachable,
                crossing: None,
            });
            continue;
        }
        if pocket_of[at(key)] == 0 {
            let id = pockets.len() as u16 + 1;
            // The host floods the pocket: scanning a whole-map buffer in Rust cost ~400k fuel.
            let pocket = blocked.reachable_tiles(key, MAX_STEPS)?;
            let tiles = pocket.positions()?;
            for &t in &tiles {
                pocket_of[at(t)] = id;
            }
            pocket_of[at(key)] = id; // even if the camp tile itself is enclosed
            let landing_ok = |t: Vec2| {
                inside(t) && pocket_of[at(t)] == id && kind[at(t)] == OPEN && !camp[at(t)]
            };
            // A 2-step dash needs a middle tile next to both ends that isn't a wall.
            let dash_ok = |l: Vec2, t: Vec2| {
                (-1..=1).any(|my| {
                    (-1..=1).any(|mx| {
                        let m = Vec2::new(l.x + mx, l.y + my);
                        inside(m)
                            && (m.x - t.x).abs().max((m.y - t.y).abs()) <= 1
                            && matches!(kind[at(m)], OPEN | HOLE)
                    })
                })
            };
            if main_board.is_none() {
                let spawn = cambc::map::spawn_region(Team::Ally)?.positions()?;
                let Some(&spawn) = spawn.first() else { break };
                main_board = Some(blocked.reachable_tiles(spawn, MAX_STEPS)?);
            }
            // Dashes first, then crystals; within each, the launch tile closest to our spawn on
            // foot. The host marks the launch candidates (our region's tiles in range of the
            // pocket): checking every tile pair in Rust cost ~260k fuel for a 33-tile pocket.
            let mut best: Option<(Access, Crossing)> = None;
            for (access, range) in [(Access::Dash, DASH_STEPS), (Access::Crystal, CRYSTAL_RANGE)] {
                let mut near = grow(&pocket, range)?;
                near.intersect_assign(main_board.as_ref().expect("set above"))?;
                let mut launches: Vec<Vec2> = near
                    .positions()?
                    .into_iter()
                    .filter(|&l| launch_ok(l))
                    .collect();
                launches.sort_by_key(|&l| main[at(l)]);
                best = launches.iter().find_map(|&l| {
                    (-range..=range)
                        .flat_map(|dy| {
                            (-range..=range).map(move |dx| Vec2::new(l.x + dx, l.y + dy))
                        })
                        .find(|&t| landing_ok(t) && (access != Access::Dash || dash_ok(l, t)))
                        .map(|t| {
                            (
                                access,
                                Crossing {
                                    launch: l,
                                    landing: t,
                                },
                            )
                        })
                });
                if best.is_some() {
                    break;
                }
            }
            pockets.push(match best {
                Some((access, crossing)) => (access, Some(crossing)),
                None => (Access::Unreachable, None),
            });
        }
        let (access, crossing) = pockets[pocket_of[at(key)] as usize - 1];
        entries.push(Entry {
            key,
            access,
            crossing,
        });
    }
    Ok(entries)
}

// Draft: the table's bytes, worked out once, on the first call: only the first draft turn has
// the fuel (~1M; later turns get ~100k, less than the classification can take). If that attempt
// fails or lacks the fuel, there is no table and every group counts as walkable. Groups that don't
// fit in the draft memory are left out too.
pub fn table_bytes() -> Result<Vec<u8>> {
    if !REGIONS {
        return Ok(Vec::new());
    }
    thread_local! {
        static TABLE: OnceCell<Vec<u8>> = const { OnceCell::new() };
    }
    if let Some(bytes) = TABLE.with(|t| t.get().cloned()) {
        return Ok(bytes);
    }
    let bytes = if cambc::fuel_remaining()? >= CLASSIFY_MIN_FUEL {
        classify().map(encode).unwrap_or_default()
    } else {
        Vec::new()
    };
    TABLE.with(|t| {
        let _ = t.set(bytes.clone());
    });
    Ok(bytes)
}

fn encode(entries: Vec<Entry>) -> Vec<u8> {
    // The opening plan (opening.rs) sits after the table.
    let end = (cambc::constants().draft_memory_bytes as usize).min(crate::opening::OPENING_OFFSET);
    let room = end.saturating_sub(REGION_OFFSET + 3);
    let count = entries.len().min(room / ENTRY_BYTES).min(u8::MAX as usize);
    let mut bytes = vec![MAGIC, VERSION, count as u8];
    let tile = |bytes: &mut Vec<u8>, p: Option<Vec2>| match p {
        Some(p) => bytes.extend([p.x as u8, p.y as u8]),
        None => bytes.extend([NO_TILE, NO_TILE]),
    };
    for e in &entries[..count] {
        bytes.extend([e.key.x as u8, e.key.y as u8, e.access as u8]);
        tile(&mut bytes, e.crossing.map(|c| c.launch));
        tile(&mut bytes, e.crossing.map(|c| c.landing));
    }
    bytes
}

// How many bytes the region table takes at the start of the draft memory `bytes` (0: none), so the
// camp graph (common::graph) knows where it starts.
pub fn table_len(bytes: &[u8]) -> usize {
    match bytes.get(REGION_OFFSET..REGION_OFFSET + 3) {
        Some(&[MAGIC, VERSION, count]) => REGION_OFFSET + 3 + count as usize * ENTRY_BYTES,
        _ => REGION_OFFSET,
    }
}

// A camp group's access and crossing; None: not in the table.
type TableEntry = Option<(Access, Option<Crossing>)>;

// Play: access and crossing per camp group, indexed as in camp_groups().
fn table() -> Result<&'static [TableEntry]> {
    thread_local! {
        static TABLE: OnceCell<&'static [TableEntry]> = const { OnceCell::new() };
    }
    if let Some(table) = TABLE.with(|t| t.get().copied()) {
        return Ok(table);
    }
    let groups = camp_groups()?;
    let mut table = vec![None; groups.len()];
    let bytes = cambc::draft_memory().bytes();
    if let Some(block) = bytes.get(REGION_OFFSET..) {
        if REGIONS && block.len() >= 3 && block[0] == MAGIC && block[1] == VERSION {
            let tile = |x: u8, y: u8| (x != NO_TILE).then(|| Vec2::new(x as i32, y as i32));
            for e in block[3..].chunks_exact(ENTRY_BYTES).take(block[2] as usize) {
                let key = Vec2::new(e[0] as i32, e[1] as i32);
                let Some(access) = Access::from_byte(e[2]) else {
                    continue;
                };
                let crossing = tile(e[3], e[4])
                    .zip(tile(e[5], e[6]))
                    .map(|(launch, landing)| Crossing { launch, landing });
                if let Some(index) = groups
                    .iter()
                    .position(|g| g.camps().first().is_some_and(|c| c.spawn_position() == key))
                {
                    table[index] = Some((access, crossing));
                }
            }
        }
    }
    let table: &'static [_] = Vec::leak(table);
    TABLE.with(|t| {
        let _ = t.set(table);
    });
    Ok(table)
}

// Whether the map has a pocket we could dash into. Most maps don't, and then nothing else here
// runs: building the region masks (a whole-map flood) on a busy turn cost a dasher its move.
fn any_dash_pocket() -> Result<bool> {
    thread_local! {
        static ANY: OnceCell<bool> = const { OnceCell::new() };
    }
    if let Some(any) = ANY.with(|a| a.get().copied()) {
        return Ok(any);
    }
    let any = table()?
        .iter()
        .flatten()
        .any(|&(access, crossing)| access == Access::Dash && crossing.is_some());
    ANY.with(|a| {
        let _ = a.set(any);
    });
    Ok(any)
}

fn has_dash() -> Result<bool> {
    Ok(me()?.abilities().iter().any(|a| a.is_dash()))
}

// Whether group `index` can be walked to from our spawn (not in the table: counts as walkable).
pub fn walkable(index: usize) -> Result<bool> {
    Ok(matches!(
        table()?.get(index).copied().flatten(),
        None | Some((Access::Walk, _))
    ))
}

// Whether this champion can get to group `index`: on foot, or across with its dash.
pub fn reachable_for_me(index: usize) -> Result<bool> {
    Ok(match table()?.get(index).copied().flatten() {
        None | Some((Access::Walk, _)) => true,
        Some((Access::Dash, Some(_))) => has_dash()?,
        Some(_) => false,
    })
}

// Our spawn's region and the pockets we can dash into, as walkable masks (computed once).
struct Pockets {
    main: Bitboard,
    dash: Vec<(Bitboard, Crossing)>,
}

fn pockets() -> Result<&'static Pockets> {
    thread_local! {
        static POCKETS: OnceCell<&'static Pockets> = const { OnceCell::new() };
    }
    if let Some(p) = POCKETS.with(|c| c.get().copied()) {
        return Ok(p);
    }
    let blocked = walk_blocked()?;
    let spawn = cambc::map::spawn_region(Team::Ally)?
        .positions()?
        .first()
        .copied()
        .unwrap_or(me()?.position()?);
    let main = blocked.reachable_tiles(spawn, MAX_STEPS)?;
    let mut dash: Vec<(Bitboard, Crossing)> = Vec::new();
    for &(access, crossing) in table()?.iter().flatten() {
        let (Access::Dash, Some(crossing)) = (access, crossing) else {
            continue;
        };
        if dash.iter().any(|(_, c)| c.landing == crossing.landing) {
            continue; // several groups share a pocket
        }
        dash.push((
            blocked.reachable_tiles(crossing.landing, MAX_STEPS)?,
            crossing,
        ));
    }
    let pockets: &'static Pockets = Box::leak(Box::new(Pockets { main, dash }));
    POCKETS.with(|c| {
        let _ = c.set(pockets);
    });
    Ok(pockets)
}

fn overlaps(a: &Bitboard, b: &Bitboard) -> Result<bool> {
    Ok(!a.intersect(b)?.is_empty()?)
}

// Whether a dash from `from` to `to` (up to 2 steps) has a middle tile that isn't a wall, altar or
// the enemy spawn (holes are fine).
pub(crate) fn dash_path_ok(from: Vec2, to: Vec2) -> Result<bool> {
    let stops = [
        cambc::map::walls()?,
        cambc::map::altars()?,
        cambc::map::spawn_region(Team::Enemy)?,
    ];
    for my in -1..=1 {
        for mx in -1..=1 {
            let m = Vec2::new(from.x + mx, from.y + my);
            if distance(m, to) > 1 || !cambc::map::is_in_bounds(m)? {
                continue;
            }
            let mut stopped = false;
            for mask in stops {
                stopped |= mask.is_set(m)?;
            }
            if !stopped {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

// Cross into `region` (the pocket, or our spawn's region on the way out) by the crossing from
// `launch` to `landing`. Walk to the launch tile; at or next to it, with the dash ready, dash to
// a free tile of `region` in reach, the planned landing tile first. Any free tile, not just the
// planned one: two champions crossing opposite ways each stood on the other's landing tile and
// waited there for ~85 turns.
fn cross(launch: Vec2, landing: Vec2, region: &Bitboard) -> Result<bool> {
    let here = me()?.position()?;
    if distance(here, launch) <= 1 {
        let Some(dash) = ready_dash()? else {
            trace("cross-wait");
            return Ok(false);
        };
        let mut options = region.intersect(&tiles_around(here, DASH_STEPS)?)?;
        options.intersect_assign(&blocked_tiles()?.complement()?)?;
        let mut options = options.positions()?;
        options.sort_by_key(|&t| distance(t, landing));
        for t in options {
            if !dash_path_ok(here, t)? {
                continue;
            }
            // false: an unseen unit stands there (now revealed); try the next tile.
            let dashed = dash.cast_at(t).unwrap_or(false);
            forget_sight();
            if dashed {
                trace("cross-dash");
                return Ok(true);
            }
        }
        trace("cross-blocked");
        if here == launch {
            return Ok(false);
        }
    }
    let mut goal = Bitboard::new()?;
    goal.set(launch, true)?;
    trace("cross-walk");
    move_towards_closest(goal)
}

// Walking towards `goals` (move_towards_closest) when none of them is in the region we stand in:
// head for the crossing into (or out of) a dash pocket. None when walking applies (a goal is in
// our region, or no known crossing leads to one); otherwise whether we moved.
pub fn cross_towards(goals: &Bitboard) -> Result<Option<bool>> {
    if !REGIONS || !any_dash_pocket()? || !has_dash()? || goals.is_empty()? {
        return Ok(None);
    }
    let pockets = pockets()?;
    if pockets.dash.is_empty() {
        return Ok(None);
    }
    let here = me()?.position()?;
    if pockets.main.is_set(here)? {
        if overlaps(&pockets.main, goals)? {
            return Ok(None);
        }
        let mut best: Option<(Crossing, &Bitboard)> = None;
        for (pocket, crossing) in &pockets.dash {
            if overlaps(pocket, goals)?
                && best
                    .is_none_or(|(b, _)| distance(crossing.launch, here) < distance(b.launch, here))
            {
                best = Some((*crossing, pocket));
            }
        }
        return match best {
            Some((c, pocket)) => cross(c.launch, c.landing, pocket).map(Some),
            None => Ok(None),
        };
    }
    // In a pocket: leave it once nothing we want is inside.
    for (pocket, crossing) in &pockets.dash {
        if pocket.is_set(here)? {
            if overlaps(pocket, goals)? {
                return Ok(None);
            }
            return cross(crossing.landing, crossing.launch, &pockets.main).map(Some);
        }
    }
    Ok(None)
}
