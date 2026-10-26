// Fuel benchmark (2026-10-25): the plan's hot loops, timed in fuel, to compare with the same loops
// in C++ (bots/xbench-cpp). Each champion runs them once, on its first turn.
use cambc::{ChampionDraft as D, Team, Vec2};
use std::cell::RefCell;

fn fuel() -> u64 {
    cambc::fuel_remaining().unwrap_or(0)
}

fn report(name: &str, spent: u64, units: usize) {
    println!(
        "BENCH rust {name} fuel {spent} units {units} per {:.1}",
        spent as f64 / units.max(1) as f64
    );
}

thread_local! {
    static MEMO: RefCell<Vec<(i32, u32)>> = const { RefCell::new(Vec::new()) };
}

fn bench(step: u32) -> cambc::Result<()> {
    let (w, h) = (cambc::map::width()?, cambc::map::height()?);
    let walls = cambc::map::walls()?;
    let blocked: Vec<Vec2> = walls
        .union(cambc::map::holes()?)?
        .positions()?;
    let mut camps: Vec<Vec2> = Vec::new();
    for group in cambc::map::camp_groups()? {
        for camp in group.camps() {
            camps.push(camp.spawn_position());
        }
    }
    let here = cambc::self_champion()?.expect("in play").position()?;

    if step == 0 {
    let a = fuel();
    let b = fuel();
    report("fuel-read", a - b, 1);
    }
    if step == 1 {

    // B1: flood fill over a padded grid (like common/flood.rs).
    let a = fuel();
    let width = (w + 2) as isize;
    let size = ((w + 2) * (h + 2)) as usize;
    let at = |p: Vec2| ((p.y + 1) as isize * width + (p.x + 1) as isize) as usize;
    let mut cell = vec![0u8; size];
    for x in 0..width as usize {
        cell[x] = 1;
        cell[size - 1 - x] = 1;
    }
    for y in 0..(h + 2) as usize {
        cell[y * width as usize] = 1;
        cell[y * width as usize + width as usize - 1] = 1;
    }
    for &p in &blocked {
        cell[at(p)] = 1;
    }
    let b = fuel();
    report("grid-init", a - b, size);
    let a = fuel();
    let mut dist = vec![0u16; size];
    let mut queue: Vec<u32> = Vec::with_capacity(size);
    let start = at(here);
    cell[start] = 2;
    queue.push(start as u32);
    let offsets = [-width - 1, -width, -width + 1, -1, 1, width - 1, width, width + 1];
    let mut head = 0;
    while head < queue.len() && head < 100 {
        let i = queue[head] as isize;
        head += 1;
        let d = dist[i as usize];
        for off in offsets {
            let j = (i + off) as usize;
            if cell[j] == 0 {
                cell[j] = 2;
                dist[j] = d + 1;
                queue.push(j as u32);
            }
        }
    }
    std::hint::black_box(&dist);
    let b = fuel();
    report("flood-pop", a - b, head);
    std::hint::black_box(head);
    }
    if step == 2 {

    // B2: nearest candidate in a straight line, after a cooldown filter (farm target pick).
    let abandoned = [(Vec2::new(1, 1), 5u32), (Vec2::new(2, 3), 5), (Vec2::new(7, 7), 5)];
    let a = fuel();
    let mut found = 0;
    for _ in 0..20 {
        let available: Vec<Vec2> = camps
            .iter()
            .copied()
            .filter(|&t| !abandoned.iter().any(|&(o, _)| o == t))
            .collect();
        if let Some(t) = available.iter().copied().min_by_key(|&t| t.chebyshev_distance(here)) {
            found += t.x;
        }
    }
    let b = fuel();
    report("nearest-tile", a - b, 20 * camps.len());
    std::hint::black_box(found);
    }
    if step == 3 {

    // B3: host bitboard lookups.
    let a = fuel();
    let mut n = 0;
    for i in 0..200 {
        let p = Vec2::new(i % w, (i / w) % h);
        n += walls.is_set(p)? as i32;
    }
    let b = fuel();
    report("is-set", a - b, 200);
    std::hint::black_box(n);
    }
    if step == 4 {

    // B4: linear find in a list of (group, tile) (the fight context's live camps).
    let live: Vec<(usize, Vec2)> = camps.iter().take(18).enumerate().map(|(i, &t)| (i, t)).collect();
    let a = fuel();
    let mut hits = 0;
    for q in camps.iter().take(30) {
        if live.iter().any(|(_, t)| t == q) {
            hits += 1;
        }
    }
    let b = fuel();
    report("find-18", a - b, 30.min(camps.len()));
    std::hint::black_box(hits);
    }
    if step == 5 {

    // B5: thread-local memo lookups (the bot's caches).
    MEMO.with(|m| *m.borrow_mut() = (0..8).map(|i| (i, i as u32 * 3)).collect());
    let a = fuel();
    let mut sum = 0u32;
    for i in 0..200 {
        sum += MEMO.with(|m| m.borrow().iter().find(|e| e.0 == i % 8).map_or(0, |e| e.1));
    }
    let b = fuel();
    report("tls-memo", a - b, 200);
    std::hint::black_box(sum);
    }
    Ok(())
}

fn main() -> cambc::Result<()> {
    let mut step = 0u32;
    loop {
        if cambc::is_draft() {
            let mine = cambc::draft::current_draft(Team::Ally).unwrap_or_default();
            for pick in [D::Ryth, D::VesperVale, D::Vendetta, D::Marcella, D::HeartOfRuin, D::Tavi, D::Varg] {
                if !mine.contains(&pick) {
                    let _ = cambc::draft::pick(pick);
                    break;
                }
            }
        } else if step < 6 {
            step += 1;
            if let Err(e) = bench(step - 1) {
                println!("bench error: {e}");
            }
        }
        cambc::end_turn();
    }
}
