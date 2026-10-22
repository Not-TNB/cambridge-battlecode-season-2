// Target claims: a champion farming a camp group one champion can clear alone (gold and gem
// clusters, wolves) claims it in shared memory, and allies leave it alone. Vision isn't shared, so
// allies kept heading for the same small groups: 23% of the time spent walking to cluster and wolf
// groups ended with an ally taking them (2026-10-21, 42 games).
//
// Shared memory bytes 2-4: one bit per camp group index 0..23 (groups past that can't be claimed).
// The opening uses those bytes (opening.rs), so claims are on when it's Normal, or from
// OPENING_MAX_ROUND + 2 on (any champion clears the bytes the round before). A claim is only made
// within CLAIM_MAX_STEPS of the group, so a far champion doesn't keep a near one away, and it's
// dropped as soon as we stop farming it (another target, banking, recalling, dead).
use super::me;
use super::world::{camp_groups, distance};
use cambc::{Result, Vec2};
use std::cell::{Cell, OnceCell};

// Off: no claims (allies may all head for the same group).
pub const CLAIMS: bool = false;
const OFFSET: usize = 2;
const BITS: usize = 24;
// Claim only a group this close (straight line); further away, an ally nearer to it may take it.
const CLAIM_MAX_TILES: i32 = 12;
// A group claimed by an ally is still ours to take when we're this close to it.
pub const CLAIM_IGNORE_TILES: i32 = 4;

thread_local! {
    static MINE: Cell<Option<usize>> = const { Cell::new(None) };
    static FARMED_ROUND: Cell<Option<u32>> = const { Cell::new(None) };
    static CLEARED: Cell<bool> = const { Cell::new(false) };
    static SOLO: OnceCell<Vec<bool>> = const { OnceCell::new() };
}

fn on() -> Result<bool> {
    Ok(CLAIMS
        && (crate::opening::mode() == crate::opening::Mode::Normal
            || cambc::round_number()? >= crate::opening::OPENING_MAX_ROUND + 2))
}

// Whether claims are in use (the opening's team target and fed bytes aren't).
pub fn active() -> bool {
    on().unwrap_or(false)
}

fn read() -> Result<u32> {
    let mut b = [0u8; 3];
    cambc::shared_memory().read(OFFSET, &mut b)?;
    Ok(b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16)
}

fn write(bits: u32) -> Result<()> {
    cambc::shared_memory().write(OFFSET, &[bits as u8, (bits >> 8) as u8, (bits >> 16) as u8])
}

// The claim bits as they stand (0 when claims are off), for plan caches' keys.
pub fn bits() -> Result<u32> {
    if !on()? {
        return Ok(0);
    }
    read()
}

// Whether group `index` is one champion's job: only clusters and wolves.
fn solo_group(index: usize) -> bool {
    SOLO.with(|s| {
        s.get_or_init(|| {
            camp_groups().map_or(Vec::new(), |groups| {
                groups
                    .iter()
                    .map(|g| {
                        g.camps().iter().all(|c| {
                            matches!(
                                c.kind(),
                                cambc::CampType::GoldCluster
                                    | cambc::CampType::GemCluster
                                    | cambc::CampType::Wolf
                            )
                        })
                    })
                    .collect()
            })
        })
        .get(index)
        .copied()
        .unwrap_or(false)
    })
}

// Whether an ally has claimed group `index` (and we're not close enough to take it anyway).
pub fn claimed_by_ally(index: usize, gap: i32) -> Result<bool> {
    if index >= BITS || gap <= CLAIM_IGNORE_TILES || MINE.with(|m| m.get()) == Some(index) || !on()? {
        return Ok(false);
    }
    Ok(read()? & (1 << index) != 0)
}

// farm::farm_camps, every turn it runs: we're farming this round.
pub fn farmed() {
    FARMED_ROUND.with(|f| f.set(cambc::round_number().ok()));
}

// End of every turn (bot.rs): hold a claim on the group we're farming, if it's a solo group close
// by; otherwise drop ours.
pub fn sync(target: Option<Vec2>) -> Result<()> {
    if !CLAIMS {
        return Ok(());
    }
    let round = cambc::round_number()?;
    // The opening's bytes, cleared for the claims (opening.rs uses them until OPENING_MAX_ROUND).
    if crate::opening::mode() != crate::opening::Mode::Normal
        && round == crate::opening::OPENING_MAX_ROUND + 1
        && !CLEARED.with(|c| c.get())
    {
        CLEARED.with(|c| c.set(true));
        write(0)?;
        MINE.with(|m| m.set(None));
        return Ok(());
    }
    if !on()? {
        return Ok(());
    }
    let me = me()?;
    let farming = me.is_alive()? && !me.is_recalling()? && FARMED_ROUND.with(|f| f.get()) == Some(round);
    let want = if farming {
        target.and_then(|t| {
            let index = super::world::group_at(t).ok().flatten()?;
            let close = me.position().is_ok_and(|p| distance(p, t) <= CLAIM_MAX_TILES);
            (index < BITS && solo_group(index) && close).then_some(index)
        })
    } else {
        None
    };
    let mine = MINE.with(|m| m.get());
    if want == mine {
        return Ok(());
    }
    let mut bits = read()?;
    if let Some(old) = mine {
        bits &= !(1 << old);
    }
    if let Some(new) = want {
        bits |= 1 << new;
    }
    write(bits)?;
    MINE.with(|m| m.set(want));
    super::trace::trace(if want.is_some() { "claim" } else { "claim-release" });
    Ok(())
}
