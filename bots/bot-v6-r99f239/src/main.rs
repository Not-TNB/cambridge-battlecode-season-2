// Dispatch draft and gameplay. Rust propagates SDK errors explicitly.
mod alloc;
mod bot;
mod champions;
mod common;
mod constants;
mod draft;
mod opening;
mod regions;
use std::cell::Cell;

#[global_allocator]
static ALLOC: alloc::SizeClassAlloc = alloc::SizeClassAlloc::new();

thread_local! {
    static RANDOM: Cell<Option<u64>> = const { Cell::new(None) };
}

// WASI randomness is always zero in matches, so seed from our spawn and this champion's start, the
// map's size and its altars, and (MIX_DRAFTS) the picks made so far.
fn random_seed() -> u64 {
    // Fixed per build: change it (or run scripts/reseed.sh) to change this bot's random choices.
    const SEED: u64 = 0x99f239a5eef2229e;
    let spawn = cambc::map::spawn_region(cambc::Team::Ally)
        .ok()
        .and_then(|region| region.positions().ok()?.first().copied());
    let start = cambc::self_champion()
        .ok()
        .flatten()
        .and_then(|me| me.position().ok());
    let size = cambc::map::width()
        .ok()
        .zip(cambc::map::height().ok())
        .map(|(w, h)| cambc::Vec2::new(w, h));
    let altars = cambc::map::altars()
        .ok()
        .and_then(|a| a.positions().ok())
        .unwrap_or_default();
    let seed = [spawn, start, size]
        .into_iter()
        .flatten()
        .chain(altars)
        .fold(SEED, |seed, p| {
            seed.wrapping_mul(1_000_003)
                .wrapping_add((p.x as u64) << 16 | p.y as u64)
        });
    if MIX_DRAFTS && start.is_some() {
        // In a game the whole draft is known: the enemy's picks vary the game's random choices
        // (shop stat, move priority) by opponent too.
        return mix(seed, draft_key().unwrap_or(0));
    }
    seed
}

// Mix the picks so far (both teams', and the team size) into the random state before every draft
// pick and into the game's seed. The only randomness is this seed, so without it the draft was the
// same for every opponent on a map and side whenever their picks left the class odds alone: on
// 01broken_curve we drafted Marcella, Vesper and Ryth in all three ladder games (0-3). Opponents
// that pick differently now get different draws from us. Off: spawn and map only.
pub const MIX_DRAFTS: bool = true;

fn draft_key() -> cambc::Result<u64> {
    let mut key = cambc::draft::team_size()? as u64;
    for team in [cambc::Team::Enemy, cambc::Team::Ally] {
        for pick in cambc::draft::current_draft(team)? {
            key = key.wrapping_mul(31).wrapping_add(pick as u64 + 1);
        }
        key = key.wrapping_mul(97);
    }
    Ok(key)
}

// SplitMix64-style mix of `value` into `seed`.
fn mix(seed: u64, value: u64) -> u64 {
    let mut z = seed ^ value.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

// Before each draft pick (MIX_DRAFTS): fold the picks so far into the random state.
fn mix_draft_into_random() {
    if !MIX_DRAFTS {
        return;
    }
    let key = draft_key().unwrap_or(0);
    RANDOM.with(|random| {
        let state = random.get().unwrap_or_else(random_seed);
        random.set(Some(mix(state, key)));
    });
}

fn random_number(limit: u32) -> u32 {
    RANDOM.with(|random| {
        let state = random
            .get()
            .unwrap_or_else(random_seed)
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        random.set(Some(state));
        ((state >> 33) % u64::from(limit)) as u32
    })
}

fn main() -> cambc::Result<()> {
    let mut bot = bot::Bot::default();
    loop {
        let result = if cambc::is_draft() {
            draft::play()
        } else {
            bot.play_turn()
        };
        if let Err(error) = result {
            println!("bot error: {error}");
        }
        cambc::end_turn();
    }
}
