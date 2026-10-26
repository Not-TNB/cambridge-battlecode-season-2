// The camp neighbour graph (v9, 2026-10-25, the user's idea, "method 2"): one flood from every camp
// group (and our spawn) at once over the static terrain, each tile taken by the node that reaches
// it first. Two nodes whose territories touch are neighbours, at the walking steps across that
// border (the walking-distance Delaunay graph: no crossing edges, following walls and holes). One
// pass over the map instead of a distance field per node.
//
// Built in the draft (resumable across draft turns), stored as an edge list after the region table.
// In play, steps between two nodes are the shortest path along edges (a Dijkstra per source node,
// on first use).
use super::world::{camp_groups, distance};
use cambc::{Result, Vec2};
use std::cell::RefCell;

// Off: no neighbour graph.
pub const FLOOD_GRAPH: bool = false; // parked 2026-10-25: ~400 fuel a tile, too slow for most maps
                                     // StepsFrom (walk.rs) answers from the graph when it can (see steps_from).
pub const FLOOD_STEPS: bool = true;
// Fuel to leave in hand in a draft turn, and the most one draft turn spends on the flood.
const DRAFT_RESERVE: u64 = 100_000;
const DRAFT_BUDGET: u64 = 400_000;
// Pops between fuel checks.
const CHUNK: usize = 64;
// A champion this close to a group (or our spawn) counts as standing at it (steps_from).
const ANCHOR_TILES: i32 = 2;
const MAGIC: u8 = b'N';
const VERSION: u8 = 1;
const NONE: u8 = u8::MAX;

// Draft: the flood in progress.
struct Flood {
    n: usize,
    width: isize, // the padded grid's width (map width + 2: a blocked border, no bounds checks)
    cell: Vec<u8>, // BLOCKED, NONE (not reached yet) or the node that reached it
    dist: Vec<u16>, // steps from that node
    queue: Vec<u32>,
    head: usize,
    best: Vec<u16>, // n x n, the shortest border crossing seen so far
    done: bool,
}
const BLOCKED: u8 = u8::MAX - 1;

// Play: the graph.
struct Graph {
    n: usize,
    tiles: Vec<Vec<Vec2>>,
    adj: Vec<Vec<(usize, u16)>>,
    rows: Vec<Option<Vec<u16>>>, // shortest paths from a node, on first use
}

thread_local! {
    static FLOOD: RefCell<Option<Flood>> = const { RefCell::new(None) };
    static PACKED: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    static GRAPH: RefCell<Option<Graph>> = const { RefCell::new(None) };
    static LOADED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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

fn start() -> Result<Flood> {
    let (w, h) = (cambc::map::width()?, cambc::map::height()?);
    let width = w + 2;
    let size = (width * (h + 2)) as usize;
    let at = |p: Vec2| ((p.y + 1) * width + p.x + 1) as usize;
    let mut cell = vec![NONE; size];
    for x in 0..width {
        cell[x as usize] = BLOCKED;
        cell[((h + 1) * width + x) as usize] = BLOCKED;
    }
    for y in 0..h + 2 {
        cell[(y * width) as usize] = BLOCKED;
        cell[(y * width + width - 1) as usize] = BLOCKED;
    }
    for b in [
        cambc::map::walls()?.positions()?,
        cambc::map::holes()?.positions()?,
        cambc::map::altars()?.positions()?,
        cambc::map::spawn_region(cambc::Team::Enemy)?.positions()?,
    ] {
        for p in b {
            cell[at(p)] = BLOCKED;
        }
    }
    let tiles = node_tiles()?;
    let n = tiles.len().min(BLOCKED as usize);
    let mut dist = vec![0u16; size];
    let mut queue = Vec::with_capacity(size / 2);
    for (node, ts) in tiles.iter().enumerate().take(n) {
        for &p in ts {
            let i = at(p);
            if cell[i] == NONE || cell[i] == BLOCKED {
                cell[i] = node as u8; // a camp tile is walkable once the camp is dead
                dist[i] = 0;
                queue.push(i as u32);
            }
        }
    }
    Ok(Flood {
        n,
        width: width as isize,
        cell,
        dist,
        queue,
        head: 0,
        best: vec![u16::MAX; n * n],
        done: false,
    })
}

// Draft: carry the flood on while the fuel lasts.
pub fn draft_step() -> Result<()> {
    if !FLOOD_GRAPH {
        return Ok(());
    }
    if FLOOD.with(|f| f.borrow().is_none()) {
        let flood = start()?;
        FLOOD.with(|f| *f.borrow_mut() = Some(flood));
    }
    let start_fuel = cambc::fuel_remaining()?;
    loop {
        let fuel = cambc::fuel_remaining()?;
        if fuel < DRAFT_RESERVE || start_fuel - fuel > DRAFT_BUDGET {
            break;
        }
        let finished = FLOOD.with(|f| {
            let mut f = f.borrow_mut();
            let f = f.as_mut().expect("set above");
            if f.done {
                return true;
            }
            let n = f.n;
            let wd = f.width;
            let offsets = [-wd - 1, -wd, -wd + 1, -1, 1, wd - 1, wd, wd + 1];
            for _ in 0..CHUNK {
                if f.head >= f.queue.len() {
                    f.done = true;
                    return true;
                }
                let i = f.queue[f.head] as isize;
                f.head += 1;
                let (li, di) = (f.cell[i as usize], f.dist[i as usize]);
                for off in offsets {
                    let j = (i + off) as usize; // the blocked border keeps j in the grid
                    let lj = f.cell[j];
                    if lj == NONE {
                        f.cell[j] = li;
                        f.dist[j] = di + 1;
                        f.queue.push(j as u32);
                    } else if lj != li && lj != BLOCKED {
                        let d = di.saturating_add(f.dist[j]).saturating_add(1);
                        let (a, b) = (li as usize, lj as usize);
                        if d < f.best[a * n + b] {
                            f.best[a * n + b] = d;
                            f.best[b * n + a] = d;
                        }
                    }
                }
            }
            false
        });
        if finished {
            break;
        }
    }
    if super::trace::TRACE {
        FLOOD.with(|f| {
            if let Some(f) = f.borrow().as_ref() {
                println!(
                    "FLOOD draft {}/{} tiles done {} fuel spent {}",
                    f.head,
                    f.queue.len(),
                    f.done,
                    start_fuel.saturating_sub(cambc::fuel_remaining().unwrap_or(0))
                );
            }
        });
    }
    Ok(())
}

// Draft: the edge list once the flood is done (header: magic, version, nodes, edges; then a, b,
// steps per edge, steps capped at 255), if it fits in `room` bytes.
pub fn draft_bytes(room: usize) -> Vec<u8> {
    if let Some(bytes) = PACKED.with(|p| p.borrow().clone()) {
        return if bytes.len() <= room {
            bytes
        } else {
            Vec::new()
        };
    }
    let bytes = FLOOD.with(|f| {
        let f = f.borrow();
        let Some(f) = f.as_ref().filter(|f| f.done) else {
            return None;
        };
        let n = f.n;
        let mut edges = Vec::new();
        for a in 0..n {
            for b in (a + 1)..n {
                let d = f.best[a * n + b];
                if d != u16::MAX {
                    edges.push([a as u8, b as u8, d.min(255) as u8]);
                }
            }
        }
        let mut bytes = vec![MAGIC, VERSION, n as u8, edges.len().min(255) as u8];
        for e in edges.iter().take(255) {
            bytes.extend_from_slice(e);
        }
        Some(bytes)
    });
    let Some(bytes) = bytes else {
        return Vec::new();
    };
    PACKED.with(|p| *p.borrow_mut() = Some(bytes.clone()));
    if bytes.len() <= room {
        bytes
    } else {
        Vec::new()
    }
}

// Play: read the edge list from the draft memory (once), right after the region table (and any
// row graph, common::graph, which is off when this is on).
fn load() -> Result<()> {
    if LOADED.with(|l| l.replace(true)) {
        return Ok(());
    }
    let bytes = cambc::draft_memory().bytes();
    let start = crate::regions::table_len(&bytes);
    let Some(block) = [start, start + 3]
        .into_iter()
        .filter_map(|s| bytes.get(s..))
        .find(|b| b.len() >= 4 && b[0] == MAGIC && b[1] == VERSION)
    else {
        return Ok(());
    };
    let (n, m) = (block[2] as usize, block[3] as usize);
    let tiles = node_tiles()?;
    if tiles.len() != n || block.len() < 4 + 3 * m {
        return Ok(());
    }
    let mut adj = vec![Vec::new(); n];
    for e in block[4..4 + 3 * m].chunks_exact(3) {
        let (a, b, d) = (e[0] as usize, e[1] as usize, u16::from(e[2]));
        if a < n && b < n {
            adj[a].push((b, d));
            adj[b].push((a, d));
        }
    }
    GRAPH.with(|g| {
        *g.borrow_mut() = Some(Graph {
            n,
            tiles,
            adj,
            rows: vec![None; n],
        })
    });
    super::trace::trace("flood-from-draft");
    Ok(())
}

// Shortest steps along edges from node `from` to every node (Dijkstra, cached per source).
fn row(from: usize) -> Option<Vec<u16>> {
    GRAPH.with(|g| {
        let mut g = g.borrow_mut();
        let g = g.as_mut()?;
        if from >= g.n {
            return None;
        }
        if let Some(r) = &g.rows[from] {
            return Some(r.clone());
        }
        let n = g.n;
        let mut dist = vec![u16::MAX; n];
        let mut heap = std::collections::BinaryHeap::new();
        dist[from] = 0;
        heap.push(std::cmp::Reverse((0u16, from)));
        while let Some(std::cmp::Reverse((d, u))) = heap.pop() {
            if d > dist[u] {
                continue;
            }
            for &(v, w) in &g.adj[u] {
                let nd = d.saturating_add(w);
                if nd < dist[v] {
                    dist[v] = nd;
                    heap.push(std::cmp::Reverse((nd, v)));
                }
            }
        }
        g.rows[from] = Some(dist.clone());
        Some(dist)
    })
}

// Steps from `origin` to the camp tile `tile` along the graph, when we stand within ANCHOR_TILES of
// a group (or our spawn). None when the graph can't say.
pub fn steps_from(origin: Vec2, tile: Vec2) -> Result<Option<i32>> {
    if !FLOOD_GRAPH || !FLOOD_STEPS {
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
    super::trace::trace("flood-steps");
    if from == to {
        return Ok(Some(distance(origin, tile)));
    }
    Ok(row(from)
        .and_then(|r| r.get(to).copied())
        .filter(|&d| d != u16::MAX)
        .map(|d| near + i32::from(d)))
}
