// A* fuel stress test (Rust). Drop in as src/bot.rs; main.rs stays as-is.
//
// Fuel exhaustion does NOT crash a bot: it is suspended and resumes next round with a fresh
// budget. So A* checks fuel every CHECK_EVERY node expansions and aborts before the budget runs
// out, which keeps every turn's numbers clean. If a search is suspended anyway, it is flagged.
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::hint::black_box;

const RESERVE: u64 = 10_000; // fuel kept back for the final summary print
const CHECK_EVERY: u32 = 8; // node expansions between in-search fuel checks
const MARGIN: u64 = 2; // abort when fuel < RESERVE + MARGIN * worst fuel burned between checks
const LOG_EVERY_SEARCH: bool = true; // one line per search (costs fuel itself)
const SEED: u64 = 12345;
const DIRS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Ok,
    NoPath,
    Abort,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::NoPath => "nopath",
            Status::Abort => "ABORT",
        }
    }
}

#[derive(Default)]
pub struct Bot {
    ready: bool,
    w: i32,
    h: i32,
    blocked: Vec<u8>,
    walkable: Vec<u32>,
    g: Vec<u32>,
    parent: Vec<u32>,
    seen: Vec<u32>,
    stamp: u32,
    rng: u64,
    worst: u64, // worst fuel burned over one CHECK_EVERY interval so far
    searches: u64, // every attempted search, completed or aborted
    completed: u64,
    aborted: u64,
    turn: u64,
    sink: u64,
    summary_cost: u64, // fuel the previous summary print cost, to help tune RESERVE
}

impl Bot {
    fn rnd(&mut self, limit: usize) -> usize {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.rng >> 33) % limit as u64) as usize
    }

    fn setup(&mut self) -> cambc::Result<()> {
        self.w = cambc::map::width()?;
        self.h = cambc::map::height()?;
        let mask = cambc::map::walls()?
            .union(cambc::map::holes()?)?
            .union(cambc::map::altars()?)?;
        let n = (self.w * self.h) as usize;
        // export_linear_buffer() came back bit-packed (len = tiles/8), so read tiles with is_set instead.
        self.blocked = vec![0u8; n];
        for y in 0..self.h {
            for x in 0..self.w {
                if mask.is_set(cambc::Vec2 { x, y })? {
                    self.blocked[(y * self.w + x) as usize] = 1;
                }
            }
        }
        self.walkable = (0..n as u32).filter(|&i| self.blocked[i as usize] == 0).collect();
        self.g = vec![0; n];
        self.parent = vec![0; n];
        self.seen = vec![0; n];
        self.rng = SEED;
        self.ready = true;
        Ok(())
    }

    /// 8-directional A*, uniform cost, Chebyshev heuristic.
    /// Returns (path_length, nodes_expanded, status). f0 = fuel when the search starts.
    fn astar(&mut self, s: u32, t: u32, f0: u64) -> cambc::Result<(i64, u32, Status)> {
        self.stamp += 1;
        let stamp = self.stamp;
        let (w, h) = (self.w, self.h);
        let (tx, ty) = (t as i32 % w, t as i32 / w);
        let (sx, sy) = (s as i32 % w, s as i32 / w);
        let h0 = (sx - tx).abs().max((sy - ty).abs()) as u64;
        self.g[s as usize] = 0;
        self.seen[s as usize] = stamp;
        self.parent[s as usize] = s;
        let mut heap: BinaryHeap<Reverse<u64>> = BinaryHeap::new();
        heap.push(Reverse((h0 << 22) | (h0 << 14) | s as u64)); // key = f<<22 | h<<14 | idx
        let mut nodes = 0u32;
        let mut last = f0;
        while let Some(Reverse(key)) = heap.pop() {
            let idx = (key & 0x3FFF) as u32;
            let hh = (key >> 14) & 0xFF;
            let gc = ((key >> 22) - hh) as u32;
            if gc > self.g[idx as usize] {
                continue; // stale entry
            }
            nodes += 1;
            if idx == t {
                let mut steps = 0i64;
                let mut cur = t;
                while cur != s {
                    cur = self.parent[cur as usize];
                    steps += 1;
                }
                return Ok((steps, nodes, Status::Ok));
            }
            if nodes % CHECK_EVERY == 0 {
                let f = cambc::fuel_remaining()?;
                if f <= last && last - f > self.worst {
                    self.worst = last - f;
                }
                last = f;
                if f < RESERVE + MARGIN * self.worst {
                    return Ok((-1, nodes, Status::Abort));
                }
            }
            let (x, y) = (idx as i32 % w, idx as i32 / w);
            let ng = gc + 1;
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let ni = (ny * w + nx) as usize;
                if self.blocked[ni] != 0 {
                    continue;
                }
                if self.seen[ni] == stamp && self.g[ni] <= ng {
                    continue;
                }
                self.seen[ni] = stamp;
                self.g[ni] = ng;
                self.parent[ni] = idx;
                let nh = (nx - tx).abs().max((ny - ty).abs()) as u64;
                heap.push(Reverse(((ng as u64 + nh) << 22) | (nh << 14) | ni as u64));
            }
        }
        Ok((-1, nodes, Status::NoPath))
    }

    pub fn play_turn(&mut self) -> cambc::Result<()> {
        self.turn += 1;
        let turn_start = cambc::fuel_remaining()?;
        if !self.ready {
            self.setup()?;
            let after = cambc::fuel_remaining()?;
            println!(
                "[stress] setup {}x{}, {} walkable, setup fuel cost {} (turn started with {})",
                self.w, self.h, self.walkable.len(), turn_start.saturating_sub(after), turn_start
            );
        }
        let loop_start = cambc::fuel_remaining()?;

        let (mut done, mut nopath, mut aborted, mut suspended) = (0u64, 0u64, 0u64, 0u64);
        let (mut cost_sum, mut cost_min, mut cost_max, mut nodes_sum, mut wasted) = (0u64, 0u64, 0u64, 0u64, 0u64);
        loop {
            let n = self.walkable.len();
            let ia = self.rnd(n);
            let ib = self.rnd(n);
            let a = self.walkable[ia];
            let b = self.walkable[ib];
            let f0 = cambc::fuel_remaining()?;
            if f0 < RESERVE + MARGIN * self.worst {
                break;
            }
            self.searches += 1;
            let (len, nodes, status) = self.astar(a, b, f0)?;
            let f1 = cambc::fuel_remaining()?;
            if f1 > f0 {
                // fuel went UP: suspended and refilled mid-search
                suspended += 1;
                if LOG_EVERY_SEARCH {
                    println!("search {}: fuel_before={} SUSPENDED mid-search (cost unknown) nodes={}", self.searches, f0, nodes);
                }
                continue;
            }
            let cost = f0 - f1;
            if status == Status::Abort {
                aborted += 1;
                self.aborted += 1;
                wasted += cost;
            } else {
                done += 1;
                self.completed += 1;
                if status == Status::NoPath {
                    nopath += 1;
                }
                cost_sum += cost;
                nodes_sum += nodes as u64;
                if done == 1 || cost < cost_min {
                    cost_min = cost;
                }
                if cost > cost_max {
                    cost_max = cost;
                }
                self.sink = self.sink.wrapping_add(black_box(len) as u64); // keep the optimizer honest
            }
            if LOG_EVERY_SEARCH {
                println!(
                    "search {}: fuel_before={} cost={} nodes={} len={} {}",
                    self.searches, f0, cost, nodes, len, status.name()
                );
            }
        }

        let end = cambc::fuel_remaining()?;
        let avg = if done > 0 { cost_sum / done } else { 0 };
        let per_node = if nodes_sum > 0 { cost_sum / nodes_sum } else { 0 };
        let attempts = done + aborted + suspended;
        let all_in = if attempts > 0 && end <= loop_start { (loop_start - end) / attempts } else { 0 };
        let flag = if suspended > 0 || end > loop_start {
            " !! SUSPENDED: turn spanned rounds, totals invalid"
        } else {
            ""
        };
        let before = cambc::fuel_remaining()?;
        println!(
            "[stress] turn {}: completed {} ({} unreachable), aborted {} (wasted {}), total completed {}, total aborted {} | fuel {} -> {} | A*-only cost avg {} min {} max {}, {}/node | all-in avg {}/attempt incl. logging | worst interval {}, prev summary cost {}, checksum {}{}",
            self.turn, done, nopath, aborted, wasted, self.completed, self.aborted, loop_start, end,
            avg, cost_min, cost_max, per_node, all_in, self.worst, self.summary_cost, self.sink, flag
        );
        self.summary_cost = before.saturating_sub(cambc::fuel_remaining()?);
        Ok(())
    }
}