// Respawn sharing: vision isn't shared, so a champion walked to a group an ally had just captured
// and only found out it was respawning on arrival (10% of the rounds champions spent heading for a
// target went to groups an ally took, 2026-10-21). Whoever starts capturing a group, or sees one
// respawning that it didn't know about, posts the round it comes back; allies skip it until then
// (farm.rs keeps the rounds, RESPAWN_READY). What a champion sees itself always wins.
//
// Shared memory bytes 2-3, once the opening is over (opening.rs uses bytes 2-4 until then; claims.rs
// when CLAIMS is on): byte 2 = FLAG | group index (FLAG clear: not a message, e.g. the opening's
// team target left behind), byte 3 = the round the group is back (low byte). One message at a
// time: a later one overwrites it, so two posts in the same round can lose the first.
use cambc::Result;
use std::cell::Cell;

// Off: champions only know about respawns they've seen.
pub const RESPAWN_SHARE: bool = true;
const OFFSET: usize = 2;
const FLAG: u8 = 0x80;
// A capture takes 5 capture phases; the respawn timer starts after the reward is claimed.
pub const CAPTURE_ROUNDS: u32 = 6;

thread_local! {
    // The message last read (or posted) by us, so each one is only taken in once.
    static LAST: Cell<[u8; 2]> = const { Cell::new([0, 0]) };
    // Bumped whenever a message changes what we know (plan caches are keyed on it).
    static NEWS: Cell<u32> = const { Cell::new(0) };
    // The capture we last posted: (group, round), so standing on it doesn't post every turn.
    static POSTED_CAPTURE: Cell<Option<(usize, u32)>> = const { Cell::new(None) };
}

fn on() -> Result<bool> {
    Ok(RESPAWN_SHARE
        && !super::claims::CLAIMS
        && (crate::opening::mode() == crate::opening::Mode::Normal || !crate::opening::active()?))
}

// How many messages have changed what we know so far.
pub fn news() -> u32 {
    NEWS.with(|n| n.get())
}

pub fn bump_news() {
    NEWS.with(|n| n.set(n.get() + 1));
}

// A new message from an ally, if any: (group, round it's back).
pub fn listen() -> Result<Option<(usize, u32)>> {
    if !on()? {
        return Ok(None);
    }
    let mut bytes = [0u8; 2];
    cambc::shared_memory().read(OFFSET, &mut bytes)?;
    if bytes == LAST.with(|l| l.get()) {
        return Ok(None);
    }
    LAST.with(|l| l.set(bytes));
    if bytes[0] & FLAG == 0 {
        return Ok(None);
    }
    let index = (bytes[0] & !FLAG) as usize;
    let groups = super::world::camp_groups()?;
    let Some(group) = groups.get(index) else {
        return Ok(None);
    };
    // The full round: the one with this low byte nearest to what a fresh message can say (from a
    // little before now up to a capture plus the group's respawn timer ahead).
    let round = cambc::round_number()?;
    let longest = group.max_respawn_timer().max(0) as u32 + CAPTURE_ROUNDS + 2;
    let mut ready = (round & !0xff) | bytes[1] as u32;
    while ready + 256 <= round + longest {
        ready += 256;
    }
    while ready > round + longest {
        ready = ready.saturating_sub(256);
    }
    Ok((ready > round).then_some((index, ready)))
}

// Tell allies group `index` is back at round `ready`.
pub fn post(index: usize, ready: u32) -> Result<()> {
    if !on()? || index >= FLAG as usize {
        return Ok(());
    }
    let bytes = [FLAG | index as u8, ready as u8];
    cambc::shared_memory().write(OFFSET, &bytes)?;
    LAST.with(|l| l.set(bytes));
    super::trace::trace("respawn-post");
    Ok(())
}

// We're on the capture of group `index` this round: post when it'll be back, once per capture.
pub fn post_capture(index: usize, round: u32) -> Result<()> {
    if POSTED_CAPTURE
        .with(|p| p.get())
        .is_some_and(|(g, r)| g == index && round < r + CAPTURE_ROUNDS * 2)
    {
        return Ok(());
    }
    POSTED_CAPTURE.with(|p| p.set(Some((index, round))));
    let respawn = super::world::camp_groups()?[index].max_respawn_timer().max(0) as u32;
    post(index, round + CAPTURE_ROUNDS + respawn)
}
