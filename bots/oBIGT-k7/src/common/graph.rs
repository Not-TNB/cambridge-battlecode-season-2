// The camp graph (v8, 2026-10-25, the user's idea): walking steps between every pair of camp groups,
// and between each group and our spawn, over the static terrain (walls, holes, altars and the
// enemy spawn block; units are ignored). Most farm decisions are made next to a group we just
// cleared, so the distance to every other group is then a lookup instead of a distance field.
//
// Built in rows, one capped distance field per node (sources: that group's camp tiles, or our spawn
// region), as many rows a turn as the spare fuel allows: the first turn (double fuel) does most of
// it. Groups further than GRAPH_CAP steps apart get their distance through other groups (a
// shortest-path pass once every row is in). Not used for decisions yet: built and measured.
use super::me;
use super::trace::checkpoint;
use cambc::{Bitboard, DistanceField, Result, Vec2};
use std::cell::RefCell;

// Off: no graph.
pub const CAMP_GRAPH: bool = false;
// Fuel to leave in hand after a row (the turn still has to end).
const GRAPH_RESERVE: u64 = 25_000;
// Field cap per row: the cost of a row grows with the area searched.
const GRAPH_CAP: u16 = 80;
// Budget guess for one row until one has been measured.
const ROW_COST_GUESS: u64 = 12_000;
pub const UNKNOWN: u16 = u16::MAX;

struct Graph {
    n: usize,              // groups + 1 (the last node is our spawn)
    dist: Vec<u16>,        // n x n, UNKNOWN until filled
    next_row: usize,       // rows built so far (in node order)
    tiles: Vec<Vec<Vec2>>, // each node's tiles
    row_cost: u64,         // the most a row has cost so far
    closed: bool,          // the all-pairs pass has run
}

thread_local! {
    static GRAPH: RefCell<Option<Graph>> = const { RefCell::new(None) };
}

fn init() -> Result<Graph> {
    let groups = super::world::camp_groups()?;
    let mut tiles: Vec<Vec<Vec2>> = groups
        .iter()
        .map(|g| g.camps().iter().map(|c| c.spawn_position()).collect())
        .collect();
    tiles.push(cambc::map::spawn_region(cambc::Team::Ally)?.positions()?);
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

// Build rows while the fuel lasts (call with spare fuel, e.g. at the end of a turn). Returns how
// many rows this call built.
pub fn build_step() -> Result<usize> {
    if !CAMP_GRAPH {
        return Ok(0);
    }
    let missing = GRAPH.with(|g| g.borrow().is_none());
    if missing {
        let graph = init()?;
        GRAPH.with(|g| *g.borrow_mut() = Some(graph));
    }
    let (n, done, row_cost) = GRAPH.with(|g| {
        let g = g.borrow();
        let g = g.as_ref().expect("set above");
        (g.n, g.next_row, g.row_cost)
    });
    if done >= n {
        return Ok(0);
    }
    let blocked = terrain()?;
    let mut built = 0;
    let mut row = done;
    while row < n && cambc::fuel_remaining()? >= row_cost.max(1) + GRAPH_RESERVE {
        let before = cambc::fuel_remaining()?;
        let sources = GRAPH.with(|g| g.borrow().as_ref().expect("set").tiles[row].clone());
        let mut roots = Bitboard::new()?;
        for &t in &sources {
            roots.set(t, true)?;
        }
        let mut field = DistanceField::new()?;
        field.compute(&blocked, &roots, &mut Bitboard::new()?, GRAPH_CAP)?;
        let targets = GRAPH.with(|g| g.borrow().as_ref().expect("set").tiles.clone());
        let mut out = vec![UNKNOWN; n];
        for (j, ts) in targets.iter().enumerate() {
            let mut best = UNKNOWN;
            for &t in ts {
                let d = field.get(t)?;
                if d != DistanceField::UNREACHABLE && d < best {
                    best = d;
                }
            }
            out[j] = if j == row { 0 } else { best };
        }
        let spent = before.saturating_sub(cambc::fuel_remaining()?);
        GRAPH.with(|g| {
            let mut g = g.borrow_mut();
            let g = g.as_mut().expect("set");
            for (j, d) in out.into_iter().enumerate() {
                g.dist[row * n + j] = d;
            }
            g.next_row = row + 1;
            g.row_cost = g.row_cost.max(spent);
        });
        row += 1;
        built += 1;
    }
    checkpoint("graph-rows");
    if row >= n {
        close()?;
    }
    if super::trace::TRACE {
        cambc::debug_block! {
            let cost = GRAPH.with(|g| g.borrow().as_ref().map_or(0, |g| g.row_cost));
            println!("GRAPH r{} rows {}/{} (+{}) max row cost {}", cambc::round_number()?, row, n, built, cost);
        }
    }
    Ok(built)
}

// Every row in: make it symmetric (each pair as the shorter way measured) and fill the pairs the
// cap left unknown through other nodes (Floyd-Warshall on ~30-60 nodes, once).
fn close() -> Result<()> {
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
                    if kj == UNKNOWN {
                        continue;
                    }
                    let via = ik.saturating_add(kj);
                    if via < g.dist[i * n + j] {
                        g.dist[i * n + j] = via;
                    }
                }
            }
        }
        g.closed = true;
    });
    let _ = me;
    Ok(())
}

// Walking steps between camp groups `a` and `b` (the spawn is node `groups`), once built.
#[allow(dead_code)]
pub fn steps(a: usize, b: usize) -> Option<u16> {
    GRAPH.with(|g| {
        let g = g.borrow();
        let g = g.as_ref()?;
        if !g.closed || a >= g.n || b >= g.n {
            return None;
        }
        let d = g.dist[a * g.n + b];
        (d != UNKNOWN).then_some(d)
    })
}
