// Capture claims in shared memory. A cleared group's capture was left by one ally because another
// stood on it or could cover it, and that other ally often had plans of its own, so the first one
// came back a few steps later (the user's ladder replays; ~3 times a game in self-play). Ranking
// who should take it on every champion separately (camps::CAPTURE_CLAIM) made it worse: the chosen
// one didn't always go. Here only a champion actually heading for the capture (its plan's goal) or
// holding it claims it: it writes the group's index + 1 into byte CLAIM_OFFSET, and while that
// stands the others leave that reward alone. It's cleared when the claimant heads elsewhere; a
// claim another champion has held for CLAIM_STALE_ROUNDS rounds with the reward still waiting
// (the claimant died, or went off fighting) is ignored. One claim a team at a time.
// Byte 4 is shared with the opening plan and claims.rs, both off; this stays off while either is on.
use std::cell::Cell;

// 2026-10-22, 320 games each against off: 154-166 (coming back to a left capture 8.2 -> 6.8 a game,
// but tributes 27.4 against 29.2 and deaths 15.9 against 13.9); claiming only within 4 tiles
// (CLAIM_NEAR_TILES) 159-161, tributes 27.0 against 29.4. Off.
pub const SHARED_CAPTURE_CLAIM: bool = false;
const CLAIM_OFFSET: usize = 4;
const CLAIM_STALE_ROUNDS: u32 = 15;
// Only claim once our goal is this close (allies near it shouldn't leave it to one still walking
// over). 0: from any distance.
pub const CLAIM_NEAR_TILES: i32 = 0;

thread_local! {
    // The group whose capture we claimed.
    static MINE: Cell<Option<usize>> = const { Cell::new(None) };
    // Another champion's claim and the round we first saw it.
    static SEEN: Cell<Option<(usize, u32)>> = const { Cell::new(None) };
}

fn usable() -> bool {
    SHARED_CAPTURE_CLAIM && !crate::opening::OPENING && !super::claims::CLAIMS
}

fn read() -> cambc::Result<Option<usize>> {
    let mut byte = [0u8; 1];
    cambc::shared_memory().read(CLAIM_OFFSET, &mut byte)?;
    Ok(byte[0].checked_sub(1).map(usize::from))
}

fn write(group: Option<usize>) -> cambc::Result<()> {
    let byte = group.map_or(0, |g| (g + 1).min(255) as u8);
    cambc::shared_memory().write(CLAIM_OFFSET, &[byte])
}

// Whether another champion holds a fresh claim on group `index`'s capture.
pub fn claimed_by_other(index: usize, round: u32) -> cambc::Result<bool> {
    if !usable() {
        return Ok(false);
    }
    let Some(group) = read()? else {
        SEEN.with(|s| s.set(None));
        return Ok(false);
    };
    if group != index || MINE.with(|m| m.get()) == Some(index) {
        return Ok(false);
    }
    let first = match SEEN.with(|s| s.get()) {
        Some((g, r)) if g == group => r,
        _ => {
            SEEN.with(|s| s.set(Some((group, round))));
            round
        }
    };
    Ok(round < first + CLAIM_STALE_ROUNDS)
}

// The capture we're heading for or holding this turn (None: none): claim it, or give up ours.
pub fn heading_for(group: Option<usize>) -> cambc::Result<()> {
    if !usable() {
        return Ok(());
    }
    let mine = MINE.with(|m| m.get());
    match group {
        Some(g) if mine != Some(g) || read()? != Some(g) => {
            write(Some(g))?;
            MINE.with(|m| m.set(Some(g)));
        }
        Some(_) => {}
        None => {
            if let Some(m) = mine {
                if read()? == Some(m) {
                    write(None)?;
                }
                MINE.with(|c| c.set(None));
            }
        }
    }
    Ok(())
}
