// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
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
}

pub fn sight() -> Result<std::rc::Rc<Sight>> {
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(sight) = SIGHT.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, s)| s.clone())
    }) {
        return Ok(sight);
    }
    let name = me()?.champion_name()?;
    let mut allies = cambc::vision::champions(Some(Team::Ally))?;
    allies.retain(|ally| ally.champion_name().is_ok_and(|n| n != name));
    // Some enemies on the occupancy board fail unit queries (health, name) with "not in vision",
    // which ended the turn: keep only ones we can query.
    let mut enemies = cambc::vision::champions(Some(Team::Enemy))?;
    enemies.retain(|enemy| {
        enemy.position().is_ok() && enemy.health().is_ok() && enemy.champion_name().is_ok()
    });
    // Camps never move, so a live camp is the one spawning on its tile.
    let mut camps = Vec::new();
    let groups = camp_groups()?;
    for tile in cambc::vision::camp_tiles()?.positions()? {
        let Some(index) = group_at(tile)? else {
            continue;
        };
        if let Some(camp) = groups[index]
            .camps()
            .iter()
            .find(|c| c.spawn_position() == tile)
        {
            camps.push(camp.clone());
        }
    }
    let sight = std::rc::Rc::new(Sight {
        enemies,
        allies,
        camps,
        visible: cambc::vision::visible_tiles()?,
    });
    SIGHT.with(|c| *c.borrow_mut() = Some((key.0, key.1, sight.clone())));
    Ok(sight)
}

pub fn forget_sight() {
    SIGHT.with(|c| *c.borrow_mut() = None);
    forget_blocked();
}

// Index into camp_groups() of the group whose camp spawns on `tile`, if any (membership is fixed).
pub fn group_at(tile: Vec2) -> Result<Option<usize>> {
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
