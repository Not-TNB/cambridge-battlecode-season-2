// The camp graph (v9, 2026-10-25, the user's idea): walking steps between every pair of camp
// groups, and between each group and our spawn, over the static terrain (walls, holes, altars and
// the enemy spawn block; units are ignored). Most farm decisions are made next to a group we just
// cleared (or at our spawn), so the distance to every other group is then a lookup instead of a
// distance field.
//
// Built in the draft (draft::submit_memory calls draft_step every draft turn): one capped distance
// field per node, as many rows a turn as the fuel allows (the first draft turn has ~1M), then the
// pairs the cap left unknown are filled through other nodes and the table goes into the draft
// memory after the regions table, one byte a pair. In play every champion reads it once. If it
// isn't there (no room, or not finished), build_step builds it from spare fuel at the end of turns.
use super::world::{camp_groups, distance};
use cambc::{Bitboard, DistanceField, Result, Vec2};
use std::cell::RefCell;

// Off: no graph.
pub const CAMP_GRAPH: bool = false; // replaced by common::flood
                                    // StepsFrom (walk.rs) answers from the graph when it can (see steps_from).
pub const GRAPH_STEPS: bool = true;
// Fuel to leave in hand after a row (the turn, or the draft pick, still has to happen).
const GRAPH_RESERVE: u64 = 100_000;
const PLAY_RESERVE: u64 = 25_000;
// Most graph work one draft turn does: a long draft turn ran into the engine's real-time limit on a
// loaded machine (00lost-world, ~780k of rows: the draft was cut off and every pick went random).
const DRAFT_BUDGET: u64 = 400_000;
// Field cap per row: the cost of a row grows with the area searched.
const GRAPH_CAP: u16 = 50;
const ROW_COST_GUESS: u64 = 25_000;
// A champion this close to a group (or our spawn) counts as standing at it (steps_from).
const ANCHOR_TILES: i32 = 2;
pub const UNKNOWN: u16 = u16::MAX;
const MAGIC: u8 = b'G';
const VERSION: u8 = 2;

struct Graph {
    n: usize,              // groups + 1 (the last node is our spawn)
    dist: Vec<u16>,        // n x n, UNKNOWN until filled
    next_row: usize,       // rows built so far (in node order)
    tiles: Vec<Vec<Vec2>>, // each node's tiles
    row_cost: u64,         // the most a row has cost so far
    closed: bool,          // symmetric and filled through other nodes
}

thread_local! {
    static GRAPH: RefCell<Option<Graph>> = const { RefCell::new(None) };
    // Play: whether the draft memory has been looked at.
    static LOADED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // Draft: the packed table and the rows it holds (draft_bytes).
    static PACKED: RefCell<Option<(usize, Vec<u8>)>> = const { RefCell::new(None) };
    // steps_from's anchor for the last origin: (origin, node, steps to it).
    static ANCHOR: std::cell::Cell<Option<(Vec2, Option<(usize, i32)>)>> = const { std::cell::Cell::new(None) };
}

fn node_tiles() -> Result<Vec<Vec<Vec2>>> {
    let mut tiles: Vec<Vec<Vec2>> = camp_groups()?
        .iter()
        .map(|g| g.camps().iter().map(|c| c.spawn_position()).collect())
        .collect();
    tiles.push(cambc::map::spawn_region(cambc::Team::Ally)?.positions()?);
    Ok(tiles)
}

fn new_graph() -> Result<Graph> {
    let tiles = node_tiles()?;
    let n = tiles.len();
    Ok(Graph {
        n,
        dist: vec![UNKNOWN; n * n],
        next_row: 0,
        tiles,
        row_cost: ROW_COST_GUESS,
        closed: false,
    })
}

fn terrain() -> Result<Bitboard> {
    cambc::map::walls()?
        .union(cambc::map::holes()?)?
        .union(cambc::map::altars()?)?
        .union(cambc::map::spawn_region(cambc::Team::Enemy)?)
}

// Build rows while fuel above `reserve` lasts, spending at most `budget`. Returns the rows built.
fn build_rows(reserve: u64, budget: u64) -> Result<usize> {
    let start_fuel = cambc::fuel_remaining()?;
    if GRAPH.with(|g| g.borrow().is_none()) {
        let graph = new_graph()?;
        GRAPH.with(|g| *g.borrow_mut() = Some(graph));
    }
    let (n, mut row, row_cost) = GRAPH.with(|g| {
        let g = g.borrow();
        let g = g.as_ref().expect("set above");
        (g.n, g.next_row, g.row_cost)
    });
    if row >= n {
        return Ok(0);
    }
    let blocked = terrain()?;
    let mut built = 0;
    while row < n
        && cambc::fuel_remaining()? >= row_cost + reserve
        && start_fuel.saturating_sub(cambc::fuel_remaining()?) + row_cost <= budget
    {
        let before = cambc::fuel_remaining()?;
        let (sources, targets) = GRAPH.with(|g| {
            let g = g.borrow();
            let g = g.as_ref().expect("set");
            (g.tiles[row].clone(), g.tiles.clone())
        });
        let mut roots = Bitboard::new()?;
        for &t in &sources {
            roots.set(t, true)?;
        }
        let mut field = DistanceField::new()?;
        field.compute(&blocked, &roots, &mut Bitboard::new()?, GRAPH_CAP)?;
        let mut out = vec![UNKNOWN; n];
        for (j, ts) in targets.iter().enumerate() {
            if j == row {
                out[j] = 0;
                continue;
            }
            for &t in ts {
                let d = field.get(t)?;
                if d != DistanceField::UNREACHABLE && d < out[j] {
                    out[j] = d;
                }
            }
        }
        let spent = before.saturating_sub(cambc::fuel_remaining()?);
        GRAPH.with(|g| {
            let mut g = g.borrow_mut();
            let g = g.as_mut().expect("set");
            g.dist[row * n..(row + 1) * n].copy_from_slice(&out);
            g.next_row = row + 1;
            g.row_cost = g.row_cost.max(spent);
        });
        row += 1;
        built += 1;
    }
    if row >= n {
        close();
    }
    Ok(built)
}

// Every row in: symmetric (the shorter of the two ways measured) and the pairs the cap left unknown
// filled through other nodes (Floyd-Warshall on ~15-40 nodes, once).
fn close() {
    GRAPH.with(|g| {
        let mut g = g.borrow_mut();
        let Some(g) = g.as_mut() else {
            return;
        };
        if g.closed {
            return;
        }
        let n = g.n;
        for i in 0..n {
            for j in (i + 1)..n {
                let d = g.dist[i * n + j].min(g.dist[j * n + i]);
                g.dist[i * n + j] = d;
                g.dist[j * n + i] = d;
            }
        }
        for k in 0..n {
            for i in 0..n {
                let ik = g.dist[i * n + k];
                if ik == UNKNOWN {
                    continue;
                }
                for j in 0..n {
                    let kj = g.dist[k * n + j];
                    if kj != UNKNOWN && ik.saturating_add(kj) < g.dist[i * n + j] {
                        g.dist[i * n + j] = ik + kj;
                    }
                }
            }
        }
        g.closed = true;
    });
}

// Draft: build what the fuel allows this turn (from draft::submit_memory).
pub fn draft_step() -> Result<()> {
    if CAMP_GRAPH {
        if super::trace::TRACE {
            println!("GRAPH draft start fuel {}", cambc::fuel_remaining()?);
        }
        let built = build_rows(GRAPH_RESERVE, DRAFT_BUDGET)?;
        if super::trace::TRACE {
            let (done, n, cost) = GRAPH.with(|g| {
                g.borrow()
                    .as_ref()
                    .map_or((0, 0, 0), |g| (g.next_row, g.n, g.row_cost))
            });
            println!(
                "GRAPH draft rows {done}/{n} (+{built}) row cost {cost} fuel left {}",
                cambc::fuel_remaining()?
            );
        }
    }
    Ok(())
}

// Draft: the table so far, if it fits in `room` bytes: header (magic, version, nodes, rows built),
// then one byte a pair i < j, from whichever of the two rows was built (255: unknown or 255+). The
// rows the draft didn't get to are built in play (build_step).
pub fn draft_bytes(room: usize) -> Vec<u8> {
    // Packing ~600 pairs costs ~50k fuel, more than later draft turns (~100k, the pick included)
    // can spare: pack again only when rows were added.
    let rows = GRAPH.with(|g| g.borrow().as_ref().map_or(0, |g| g.next_row));
    if let Some(bytes) = PACKED.with(|p| {
        p.borrow()
            .as_ref()
            .filter(|(r, b)| *r == rows && b.len() <= room)
            .map(|(_, b)| b.clone())
    }) {
        return bytes;
    }
    let bytes = pack(room);
    PACKED.with(|p| *p.borrow_mut() = Some((rows, bytes.clone())));
    bytes
}

fn pack(room: usize) -> Vec<u8> {
    GRAPH.with(|g| {
        let g = g.borrow();
        let Some(g) = g.as_ref().filter(|g| g.next_row > 0) else {
            return Vec::new();
        };
        let n = g.n;
        if n > 255 || 4 + n * (n - 1) / 2 > room {
            return Vec::new();
        }
        let mut bytes = vec![MAGIC, VERSION, n as u8, g.next_row as u8];
        for i in 0..n {
            for j in (i + 1)..n {
                bytes.push(g.dist[i * n + j].min(g.dist[j * n + i]).min(255) as u8);
            }
        }
        bytes
    })
}

// Play: read the table from the draft memory (once), right after the regions table.
fn load() -> Result<()> {
    if LOADED.with(|l| l.replace(true)) {
        return Ok(());
    }
    let bytes = cambc::draft_memory().bytes();
    // After the region table, or after the empty header written when there was none.
    let start = crate::regions::table_len(&bytes);
    if super::trace::TRACE {
        println!(
            "GRAPH load: memory {} bytes, region table ends at {}, bytes there {:?}",
            bytes.len(),
            start,
            bytes.get(start..start + 6)
        );
    }
    let Some(block) = [start, start + 3]
        .into_iter()
        .filter_map(|s| bytes.get(s..))
        .find(|b| b.len() >= 4 && b[0] == MAGIC && b[1] == VERSION)
    else {
        return Ok(());
    };
    let (n, rows) = (block[2] as usize, block[3] as usize);
    let tiles = node_tiles()?;
    if tiles.len() != n || rows > n || block.len() < 4 + n * (n - 1) / 2 {
        if super::trace::TRACE {
            println!(
                "GRAPH load refused: nodes {} here {}, rows {}, block {} bytes",
                n,
                tiles.len(),
                rows,
                block.len()
            );
        }
        return Ok(());
    }
    let mut dist = vec![UNKNOWN; n * n];
    let mut k = 4;
    for i in 0..n {
        dist[i * n + i] = 0;
        for j in (i + 1)..n {
            let d = if block[k] == 255 {
                UNKNOWN
            } else {
                u16::from(block[k])
            };
            dist[i * n + j] = d;
            dist[j * n + i] = d;
            k += 1;
        }
    }
    GRAPH.with(|g| {
        *g.borrow_mut() = Some(Graph {
            n,
            dist,
            next_row: rows,
            tiles,
            row_cost: ROW_COST_GUESS,
            closed: false,
        })
    });
    if rows >= n {
        close();
    }
    super::trace::trace("graph-from-draft");
    Ok(())
}

// Play: build what's missing from spare fuel (end of turn), if the draft didn't leave a table.
pub fn build_step() -> Result<usize> {
    if !CAMP_GRAPH {
        return Ok(0);
    }
    load()?;
    build_rows(PLAY_RESERVE, u64::MAX)
}

// Walking steps between nodes `a` and `b` (the spawn is the last node). Before every row is in,
// only pairs measured directly (either node's row built, within GRAPH_CAP) are known.
pub fn steps(a: usize, b: usize) -> Option<u16> {
    GRAPH.with(|g| {
        let g = g.borrow();
        let g = g.as_ref()?;
        let (ab, ba) = (*g.dist.get(a * g.n + b)?, *g.dist.get(b * g.n + a)?);
        let d = ab.min(ba);
        (d != UNKNOWN).then_some(d)
    })
}

// Steps from `origin` to the camp tile `tile` from the graph: when we stand within ANCHOR_TILES of
// a group (or our spawn), it's the walk to that group plus the graph's steps on to `tile`'s group.
// None when the graph can't say (not built, not near a node, not a camp tile).
pub fn steps_from(origin: Vec2, tile: Vec2) -> Result<Option<i32>> {
    if !CAMP_GRAPH || !GRAPH_STEPS {
        return Ok(None);
    }
    load()?;
    let Some(to) = super::world::group_at(tile)? else {
        return Ok(None);
    };
    let anchor = match ANCHOR.with(|a| a.get()).filter(|(o, _)| *o == origin) {
        Some((_, anchor)) => anchor,
        None => {
            let anchor = GRAPH.with(|g| {
                let g = g.borrow();
                let g = g.as_ref()?;
                let mut best: Option<(usize, i32)> = None;
                for (node, ts) in g.tiles.iter().enumerate() {
                    let d = ts
                        .iter()
                        .map(|&t| distance(t, origin))
                        .min()
                        .unwrap_or(i32::MAX);
                    if d <= ANCHOR_TILES && best.is_none_or(|(_, b)| d < b) {
                        best = Some((node, d));
                    }
                }
                best
            });
            ANCHOR.with(|a| a.set(Some((origin, anchor))));
            anchor
        }
    };
    let Some((from, near)) = anchor else {
        return Ok(None);
    };
    super::trace::trace("graph-steps");
    if from == to {
        return Ok(Some(distance(origin, tile)));
    }
    Ok(steps(from, to).map(|d| near + i32::from(d)))
}
