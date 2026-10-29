// What the map and our vision hold: distances, camp groups, the per-turn sight cache and
// blocked tiles.
use super::me;
use cambc::{Bitboard, Camp, CampGroup, Champion, Result, Team, Vec2};
use std::cell::OnceCell;

pub fn distance(a: Vec2, b: Vec2) -> i32 {
    a.chebyshev_distance(b) as i32
}

pub fn tiles_around(position: Vec2, radius: i32) -> Result<Bitboard> {
    let mut tiles = Bitboard::new()?;
    tiles.draw_chebyshev(position, radius as f32)?;
    Ok(tiles)
}

// Camp group membership never changes, so load it from the host once per champion.
pub fn camp_groups() -> Result<&'static [CampGroup]> {
    thread_local! {
        static GROUPS: OnceCell<&'static [CampGroup]> = const { OnceCell::new() };
    }
    if let Some(groups) = GROUPS.with(|cell| cell.get().copied()) {
        return Ok(groups);
    }
    let groups: &'static [CampGroup] = Vec::leak(cambc::map::camp_groups()?);
    GROUPS.with(|cell| {
        let _ = cell.set(groups);
    });
    Ok(groups)
}

// Everything visible to this champion's own vision and ward. Listing visible champions costs ~4k
// fuel and camps ~2k, and a turn used to ask for them up to ten times, which left too little fuel
// to farm on turns with enemies in sight (the champion stood still). Cached per round and position,
// since moving changes what we see, and dropped after we attack or cast, which may kill what we saw.
pub struct Sight {
    pub enemies: Vec<Champion>,
    pub allies: Vec<Champion>, // without us
    pub camps: Vec<Camp>,
    pub visible: Bitboard,
}

thread_local! {
    static SIGHT: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<Sight>)>> = const { std::cell::RefCell::new(None) };
    // Parts of the cached sight an attack or cast may have changed (STALE_ENEMIES, STALE_CAMPS).
    static STALE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

// Partial refresh (v6): an attack or cast can only remove what it hit from sight, so it marks just
// the enemies or the camps stale (forget_hit) and the next sight() lists only those again, keeping
// the allies and the visible tiles; moving, dashing and wards still drop everything (forget_sight,
// and the position key). Units don't move during our turn, so the kept parts are what a full
// rebuild would list. On the turns that ran out of fuel, sight was rebuilt 2.65 times (~12k each:
// listing champions ~3.4k a team, camps ~2.7k, visible tiles), mostly after hitting camps
// (2026-10-23 local profile). Off: every attack or cast drops the whole sight.
// 2026-10-23, verified against a full rebuild on 104 games: 0.3% of refreshes differed (part of it
// the check itself: listing twice can differ at the edge of vision). Alone, 416 games head to head
// against off: 206/416, fuel-exhausted turns -1.5% (the extra rebuilds on those turns come from
// moving and from resuming in the next round). With DENSE_GROUP_AT: 248/416 against 230/416 for
// DENSE_GROUP_AT alone (same seeds). On.
pub const PARTIAL_SIGHT: bool = true;
// Profiling: also rebuild in full and print "SIGHT-MISMATCH" when a partial refresh differs.
const VERIFY_PARTIAL_SIGHT: bool = false;
pub const STALE_ENEMIES: u8 = 1;
pub const STALE_CAMPS: u8 = 2;

// After hitting something: the parts of sight it could have changed (`stale`, STALE_* bits) are
// listed again on the next sight(); with PARTIAL_SIGHT off, all of it.
// (Tried 2026-10-23: keeping each camp's health, max health, strength and state per sight instead of
// reading them again. A camp read is ~100 fuel and the lookup cost 400-500 (thread-local, closures,
// a scan by tile): the plan key's camp loop went 2.0k -> 7.3k with 9+ camps in sight, fuel-exhausted
// turns +23% over 100 games against same-game controls. Removed.)
pub fn forget_hit(stale: u8) {
    if !PARTIAL_SIGHT {
        forget_sight();
        return;
    }
    STALE.with(|s| s.set(s.get() | stale));
    forget_blocked();
}

pub fn sight() -> Result<std::rc::Rc<Sight>> {
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(sight) = SIGHT.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, s)| s.clone())
    }) {
        let stale = STALE.with(|s| s.replace(0));
        if stale == 0 {
            return Ok(sight);
        }
        let refreshed = std::rc::Rc::new(Sight {
            enemies: if stale & STALE_ENEMIES != 0 {
                visible_champions(Team::Enemy, None)?
            } else {
                sight.enemies.clone()
            },
            allies: sight.allies.clone(),
            camps: if stale & STALE_CAMPS != 0 {
                visible_camps()?
            } else {
                sight.camps.clone()
            },
            visible: sight.visible.clone(),
        });
        if VERIFY_PARTIAL_SIGHT {
            verify_partial(&refreshed)?;
        }
        SIGHT.with(|c| *c.borrow_mut() = Some((key.0, key.1, refreshed.clone())));
        return Ok(refreshed);
    }
    STALE.with(|s| s.set(0));
    super::trace::checkpoint("sight-start");
    let name = me()?.champion_name()?;
    let allies = visible_champions(Team::Ally, Some(name))?;
    super::trace::checkpoint("sight-allies");
    let enemies = visible_champions(Team::Enemy, None)?;
    super::trace::checkpoint("sight-enemies");
    // Camps never move, so a live camp is the one spawning on its tile.
    let camps = visible_camps()?;
    super::trace::checkpoint("sight-camps");
    let sight = std::rc::Rc::new(Sight {
        enemies,
        allies,
        camps,
        visible: cambc::vision::visible_tiles()?,
    });
    super::trace::checkpoint("sight-visible");
    SIGHT.with(|c| *c.borrow_mut() = Some((key.0, key.1, sight.clone())));
    Ok(sight)
}

// Champions keep their handles, and building one (vision::champions) costs two ability metadata
// queries: listing allies was ~6k fuel of every turn. Champions seen before are kept with their
// type and last tile, and each champion on the occupancy board is matched to the kept one last seen
// nearest it, confirmed by its position; anything unmatched lists them afresh.
// Only champions on visible tiles are asked (asking about one out of vision builds an error, which
// costs more than listing). Only static ability data is kept; everything else is queried live.
// Off: list every time.
pub const KEEP_CHAMPIONS: bool = true;
// When the kept champion last seen nearest a tile isn't the one on it (allies walking in a line:
// each moved onto the tile the one ahead just left), try up to this many more of the kept ones, by
// how near they were last seen, before listing afresh. Re-listing (~6k fuel) happened on 54% of
// the turns that ran out of fuel. 0: list afresh on the first mismatch. 2026-10-22: re-listing
// went from 15-24% of turns to 5-7% (3 traced games each); 320 games against 0: 175-145.
pub const KEEP_CHAMPIONS_RETRIES: usize = 3;
// Only kept champions last seen within this many tiles of the tile are asked: asking about one
// out of vision builds an error, which costs more than listing.
const KEEP_CHAMPIONS_RETRY_TILES: i32 = 3;

thread_local! {
    static KNOWN: std::cell::RefCell<[Vec<(cambc::ChampionType, Champion, Vec2)>; 2]> =
        const { std::cell::RefCell::new([Vec::new(), Vec::new()]) };
}

// Visible champions of `team`, without `skip` (us). Enemies must answer queries: some on the
// occupancy board fail them with "not in vision", which ended the turn.
fn visible_champions(team: Team, skip: Option<cambc::ChampionType>) -> Result<Vec<Champion>> {
    let slot = usize::from(team == Team::Enemy);
    let usable = |c: &Champion| team == Team::Ally || (c.position().is_ok() && c.health().is_ok());
    if KEEP_CHAMPIONS {
        let tiles = cambc::vision::champion_tiles(Some(team))?.positions()?;
        let matched = KNOWN.with(|k| -> Result<Option<Vec<Champion>>> {
            let mut k = k.borrow_mut();
            let mut found = Vec::with_capacity(tiles.len());
            let mut used = [false; 8];
            for &tile in &tiles {
                // Unused kept champions by how near they were last seen (nearest first). On the stack:
                // at most `used.len()` of them, and a Vec a tile was a heap allocation each time.
                let mut buffer = [(0i32, 0usize); 8];
                let mut count = 0;
                for (i, (_, _, last)) in k[slot].iter().enumerate() {
                    if i < used.len() && !used[i] {
                        buffer[count] = (distance(*last, tile), i);
                        count += 1;
                    }
                }
                let near = &mut buffer[..count];
                near.sort_unstable();
                let mut matched = None;
                for (attempt, &(d, i)) in near.iter().enumerate() {
                    if attempt > 0
                        && (attempt > KEEP_CHAMPIONS_RETRIES || d > KEEP_CHAMPIONS_RETRY_TILES)
                    {
                        break;
                    }
                    if k[slot][i].1.position().ok() == Some(tile) {
                        matched = Some(i);
                        break;
                    }
                }
                let Some(i) = matched else {
                    return Ok(None);
                };
                let (kind, c, last) = &mut k[slot][i];
                used[i] = true;
                *last = tile;
                if Some(*kind) != skip {
                    if !usable(c) {
                        return Ok(None);
                    }
                    found.push(c.clone());
                }
            }
            Ok(Some(found))
        })?;
        if let Some(found) = matched {
            return Ok(found);
        }
    }
    super::trace::trace("sight-relist");
    let mut fresh = cambc::vision::champions(Some(team))?;
    let mut listed = Vec::with_capacity(fresh.len());
    for c in &fresh {
        listed.push((c.champion_name().ok(), c.position().ok()));
    }
    KNOWN.with(|k| {
        let mut k = k.borrow_mut();
        for (c, (kind, tile)) in fresh.iter().zip(&listed) {
            let (Some(kind), Some(tile)) = (*kind, *tile) else {
                continue;
            };
            match k[slot].iter_mut().find(|(o, _, _)| *o == kind) {
                Some(entry) => entry.2 = tile,
                None => k[slot].push((kind, c.clone(), tile)),
            }
        }
    });
    let mut keep = listed.into_iter();
    fresh.retain(|c| {
        let kind = keep.next().and_then(|(k, _)| k);
        kind.is_some() && kind != skip && usable(c)
    });
    Ok(fresh)
}

// Live camps in sight. Finding each visible camp's group and then the camp in it cost ~5-7k fuel a
// turn, so camps are looked up by tile in a table built once, and last turn's list is reused while
// the same tiles are in sight.
fn visible_camps() -> Result<Vec<Camp>> {
    thread_local! {
        static BY_TILE: OnceCell<Vec<((i32, i32), Camp)>> = const { OnceCell::new() };
        static LAST: std::cell::RefCell<Option<(Vec<Vec2>, Vec<Camp>)>> = const { std::cell::RefCell::new(None) };
    }
    let tiles = cambc::vision::camp_tiles()?.positions()?;
    if let Some(camps) = LAST.with(|l| {
        l.borrow()
            .as_ref()
            .filter(|(t, _)| *t == tiles)
            .map(|(_, c)| c.clone())
    }) {
        return Ok(camps);
    }
    if BY_TILE.with(|b| b.get().is_none()) {
        let mut by_tile = Vec::new();
        for group in camp_groups()? {
            for camp in group.camps() {
                by_tile.push((
                    (camp.spawn_position().x, camp.spawn_position().y),
                    camp.clone(),
                ));
            }
        }
        by_tile.sort_by_key(|(t, _)| *t);
        BY_TILE.with(|b| {
            let _ = b.set(by_tile);
        });
    }
    let camps: Vec<Camp> = BY_TILE.with(|b| {
        let by_tile = b.get().expect("set above");
        tiles
            .iter()
            .filter_map(|t| {
                by_tile
                    .binary_search_by_key(&(t.x, t.y), |(k, _)| *k)
                    .ok()
                    .map(|i| by_tile[i].1.clone())
            })
            .collect()
    });
    LAST.with(|l| *l.borrow_mut() = Some((tiles, camps.clone())));
    Ok(camps)
}

pub fn forget_sight() {
    SIGHT.with(|c| *c.borrow_mut() = None);
    STALE.with(|s| s.set(0));
    forget_blocked();
}

// VERIFY_PARTIAL_SIGHT: a full rebuild, compared with the partial one by unit tiles and visible tiles.
fn verify_partial(partial: &Sight) -> Result<()> {
    let name = me()?.champion_name()?;
    let tiles = |cs: &[Champion]| -> Vec<Option<Vec2>> {
        let mut t: Vec<_> = cs.iter().map(|c| c.position().ok()).collect();
        t.sort_by_key(|p| p.map(|p| (p.x, p.y)));
        t
    };
    let camp_tiles = |cs: &[Camp]| -> Vec<(i32, i32)> {
        let mut t: Vec<_> = cs
            .iter()
            .map(|c| (c.spawn_position().x, c.spawn_position().y))
            .collect();
        t.sort();
        t
    };
    let allies = visible_champions(Team::Ally, Some(name))?;
    let enemies = visible_champions(Team::Enemy, None)?;
    let camps = visible_camps()?;
    let visible = cambc::vision::visible_tiles()?;
    let diff = [
        ("allies", tiles(&allies) != tiles(&partial.allies)),
        ("enemies", tiles(&enemies) != tiles(&partial.enemies)),
        ("camps", camp_tiles(&camps) != camp_tiles(&partial.camps)),
        ("visible", visible != partial.visible),
    ];
    cambc::debug_block! {
        let parts: Vec<&str> = diff.iter().filter(|d| d.1).map(|d| d.0).collect();
        if parts.is_empty() {
            println!("SIGHT-VERIFY same");
        } else {
            println!("SIGHT-VERIFY MISMATCH {}", parts.join(","));
        }
    }
    Ok(())
}

// Dense group lookup (v6): group_at reads one entry of a per-tile table (group index + 1, 0 for none)
// instead of a binary search over every camp tile. It's called ~37 times a turn on the turns that
// ran out of fuel (farm-target ranking calls it per candidate tile), ~350 fuel each: 12.8k of those
// turns (2026-10-23 local profile). Same answers. Off: the binary search.
// 2026-10-23, 416 games head to head against off (52 maps, both sides, seeds 0-3): 230/416
// (p = 0.035; fuel-exhausted turns 6.1% against 7.3%, gold at 500 +3%, gems tributed +9%). With
// PARTIAL_SIGHT: 248/416 (p < 0.001) and, on seeds 4-7, 226/416 (gold at 500 +6%, deaths -7%).
pub const DENSE_GROUP_AT: bool = true;

thread_local! {
    // (map width, map height, group index + 1 per tile)
    static GROUP_BY_TILE: OnceCell<Option<(i32, i32, Vec<u16>)>> = const { OnceCell::new() };
}

fn dense_group_table() -> Option<(i32, i32, Vec<u16>)> {
    let (w, h) = (cambc::map::width().ok()?, cambc::map::height().ok()?);
    let mut table = vec![0u16; (w * h).max(0) as usize];
    for (index, group) in camp_groups().ok()?.iter().enumerate() {
        for camp in group.camps() {
            let p = camp.spawn_position();
            if (0..w).contains(&p.x) && (0..h).contains(&p.y) {
                let slot = &mut table[(p.y * w + p.x) as usize];
                if *slot == 0 {
                    *slot = index as u16 + 1;
                }
            }
        }
    }
    Some((w, h, table))
}

// Index into camp_groups() of the group whose camp spawns on `tile`, if any (membership is fixed).
pub fn group_at(tile: Vec2) -> Result<Option<usize>> {
    if DENSE_GROUP_AT {
        let found = GROUP_BY_TILE.with(|c| {
            c.get_or_init(dense_group_table)
                .as_ref()
                .map(|(w, h, table)| {
                    if (0..*w).contains(&tile.x) && (0..*h).contains(&tile.y) {
                        table[(tile.y * w + tile.x) as usize]
                            .checked_sub(1)
                            .map(usize::from)
                    } else {
                        None
                    }
                })
        });
        if let Some(found) = found {
            return Ok(found);
        }
    }
    thread_local! {
        static BY_TILE: OnceCell<Vec<((i32, i32), usize)>> = const { OnceCell::new() };
    }
    if BY_TILE.with(|c| c.get().is_none()) {
        let mut by_tile = Vec::new();
        for (index, group) in camp_groups()?.iter().enumerate() {
            for camp in group.camps() {
                by_tile.push(((camp.spawn_position().x, camp.spawn_position().y), index));
            }
        }
        by_tile.sort();
        BY_TILE.with(|c| {
            let _ = c.set(by_tile);
        });
    }
    Ok(BY_TILE.with(|c| {
        let by_tile = c.get().expect("set above");
        by_tile
            .binary_search_by_key(&(tile.x, tile.y), |&(t, _)| t)
            .ok()
            .map(|i| by_tile[i].1)
    }))
}

// Tiles we can't walk onto: terrain (walls, holes, altars, the enemy spawn; fixed, so combined
// once) plus visible occupants. Each new Bitboard costs ~2k fuel and this was rebuilt from seven
// several times a turn, so it's cached per round and position and dropped with forget_sight (and
// after a dash attempt, which can reveal a hidden unit).
// Tiles a step or dash onto failed because an unseen unit stands there (in a bush), each with the
// round to stop avoiding it. The engine reveals such a unit only for the rest of the round, so the
// next turn's path picked the same step and failed again, turn after turn for as long as the unit
// stayed (broken-curve: the Tavi squad's leader stood still from round 596 to the end of the game).
// 0: not remembered.
pub const HIDDEN_BLOCK_ROUNDS: u32 = 10;

thread_local! {
    static HIDDEN_BLOCKS: std::cell::RefCell<Vec<(Vec2, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Remember that an unseen unit blocks `tile` (see HIDDEN_BLOCK_ROUNDS).
pub fn remember_hidden_block(tile: Vec2) {
    if HIDDEN_BLOCK_ROUNDS == 0 {
        return;
    }
    let round = cambc::round_number().unwrap_or(0);
    HIDDEN_BLOCKS.with(|h| {
        let mut h = h.borrow_mut();
        h.retain(|&(t, until)| t != tile && round < until);
        h.push((tile, round + HIDDEN_BLOCK_ROUNDS));
    });
    forget_blocked();
}

thread_local! {
    static BLOCKED: std::cell::RefCell<Option<(u32, Vec2, Bitboard)>> = const { std::cell::RefCell::new(None) };
}

// Tiles nobody can walk onto: walls, holes, altars and the enemy spawn (combined once).
pub fn terrain_tiles() -> Result<&'static Bitboard> {
    thread_local! {
        static TERRAIN: OnceCell<&'static Bitboard> = const { OnceCell::new() };
    }
    if let Some(terrain) = TERRAIN.with(|c| c.get().copied()) {
        return Ok(terrain);
    }
    let mut terrain = cambc::map::walls()?.clone();
    terrain.union_assign(cambc::map::holes()?)?;
    terrain.union_assign(cambc::map::altars()?)?;
    terrain.union_assign(cambc::map::spawn_region(Team::Enemy)?)?;
    let terrain: &'static Bitboard = Box::leak(Box::new(terrain));
    TERRAIN.with(|c| {
        let _ = c.set(terrain);
    });
    Ok(terrain)
}

pub fn blocked_tiles() -> Result<Bitboard> {
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(blocked) = BLOCKED.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, b)| b.clone())
    }) {
        return Ok(blocked);
    }
    let mut blocked = terrain_tiles()?.clone();
    for mask in [
        cambc::vision::champion_tiles(None)?,
        cambc::vision::camp_tiles()?,
        cambc::vision::teleport_tiles()?,
    ] {
        blocked.union_assign(&mask)?;
    }
    for (tile, until) in HIDDEN_BLOCKS.with(|h| h.borrow().clone()) {
        if key.0 < until {
            blocked.set(tile, true)?;
        }
    }
    blocked.set(key.1, false)?;
    BLOCKED.with(|c| *c.borrow_mut() = Some((key.0, key.1, blocked.clone())));
    Ok(blocked)
}

fn forget_blocked() {
    BLOCKED.with(|c| *c.borrow_mut() = None);
}
