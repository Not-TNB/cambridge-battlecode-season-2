// Dispatch draft and gameplay. Rust propagates SDK errors explicitly.
mod draft;
mod bot;
mod champions;
mod common;
mod constants;
mod hotspots;
use std::cell::Cell;

thread_local! {
    static RANDOM: Cell<Option<u64>> = const { Cell::new(None) };
}

// WASI randomness is always zero in matches, so seed from our spawn and this champion's start.
fn random_seed() -> u64 {
    // Fixed per build: change it (or run scripts/reseed.sh) to change this bot's random choices.
    const SEED: u64 = 0x72bffab2db81887d;
    let spawn = cambc::map::spawn_region(cambc::Team::Ally).ok().and_then(|region| region.positions().ok()?.first().copied());
    let start = cambc::self_champion().ok().flatten().and_then(|me| me.position().ok());
    [spawn, start].into_iter().flatten()
        .fold(SEED, |seed, p| seed.wrapping_mul(1_000_003).wrapping_add((p.x as u64) << 16 | p.y as u64))
}

fn random_number(limit: u32) -> u32 {
    RANDOM.with(|random| {
        let state = random.get().unwrap_or_else(random_seed)
            .wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        random.set(Some(state));
        ((state >> 33) % u64::from(limit)) as u32
    })
}

fn dump_map(turn: usize) -> cambc::Result<()> {
    use cambc::{Team, Vec2};
    let (w, h) = (cambc::map::width()?, cambc::map::height()?);
    let mut grid = vec![vec!['.'; w as usize]; h as usize];
    let put = |grid: &mut Vec<Vec<char>>, b: &cambc::Bitboard, c: char| -> cambc::Result<()> {
        for p in b.positions()? { grid[p.y as usize][p.x as usize] = c; } Ok(())
    };
    put(&mut grid, cambc::map::bushes()?, '"')?;
    put(&mut grid, cambc::map::walls()?, '#')?;
    put(&mut grid, cambc::map::holes()?, 'o')?;
    put(&mut grid, cambc::map::spawn_region(Team::Ally)?, 'A')?;
    put(&mut grid, cambc::map::spawn_region(Team::Enemy)?, 'B')?;
    put(&mut grid, cambc::map::altars()?, '@')?;
    let mut kinds = std::collections::BTreeMap::new();
    for g in common::camp_groups()? {
        for c in g.camps() {
            let ch = if c.is_wolf() { 'w' } else if c.is_elk() { 'e' } else if c.is_gem_cluster() { 'G' } else if c.is_gold_cluster() { '$' } else { 'M' };
            *kinds.entry(ch).or_insert(0) += 1;
            let Vec2 { x, y } = c.spawn_position();
            grid[y as usize][x as usize] = ch;
        }
    }
    if turn == 0 { println!("MAPHEAD {}x{} groups={} camps={:?}", w, h, common::camp_groups()?.len(), kinds); }
    let start = (turn * 3) % (h as usize);
    for by in start..(start + 3).min(h as usize) {
        println!("MAPROW {} |{}", by, grid[by].iter().collect::<String>());
    }
    Ok(())
}

fn main() -> cambc::Result<()> {
    let mut bot = bot::Bot::default();
    let mut dumped = 0usize;
    loop {
        let result = if cambc::is_draft() {
            draft::play()
        } else {
            if dumped < 200 {
                let r = (|| -> cambc::Result<()> {
                    let me = cambc::self_champion()?.expect("play").champion_name()? as u8 + 1;
                    let mut claim = [0u8; 1];
                    cambc::shared_memory().read(7, &mut claim)?;
                    if claim[0] == 0 { cambc::shared_memory().write(7, &[me])?; claim[0] = me; }
                    if claim[0] == me { dump_map(dumped)?; }
                    Ok(())
                })();
                if let Err(e) = r { println!("DUMPERR {e}"); }
                dumped += 1;
            }
            bot.play_turn()
        };
        if let Err(error) = result {
            println!("bot error: {error}");
        }
        cambc::end_turn();
    }
}
