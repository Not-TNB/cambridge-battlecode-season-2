// Pressing a lead. Farming takes the nearest camps, so on a wide map each team farms its own half
// whatever the fights say: on the ladder (00lanes, 2026-10-22) we had 3,000 more gold and 17 kills
// to 3, and still lost the gem race, taking 0 gems from their half while they took 511 there
// after round 800. When we're clearly the stronger team, groups on their side of the map (about as
// close to their spawn as to ours, farm::GroupInfo::contested) count PRESS_PULL_STEPS steps closer
// and lose the ranged contested-group penalty: we take their camps and deny them.
//
// Who's stronger, from what champions see: a champion's shards bought, estimated from its stats
// against its design (health, Strength and Focus above base, over each one's growth). Every
// champion remembers the last estimate for each champion it has seen (allies and enemies, itself
// included) for PRESS_MEMORY_ROUNDS; from PRESS_FROM_ROUND, with at least PRESS_MIN_SEEN enemies
// remembered, we press while our average is at least PRESS_RATIO times theirs.
use super::me;
use cambc::{Champion, ChampionType, Result};
use std::cell::RefCell;

// 2026-10-22, one game a map against off (one seed): ratio 1.2 / pull 15: 13/40 (gems lost on death
// 172 a game against 120); 1.1 / 30: 15/40 (lost 232); 1.1 / 30 with at most 20 gems carried
// (PRESS_MAX_GEMS): 16/40 (more gems collected, but 137 still carried at the end against 48). Off.
pub const PRESS: bool = false;
const PRESS_FROM_ROUND: u32 = 400;
const PRESS_MEMORY_ROUNDS: u32 = 200;
const PRESS_MIN_SEEN: usize = 2;
const PRESS_RATIO: f32 = 1.2;
pub const PRESS_PULL_STEPS: i32 = 15;
// Only while carrying at most this many gems: a bag carried into their half is theirs when we die
// there. u32::MAX: whatever we carry.
const PRESS_MAX_GEMS: u32 = u32::MAX;

thread_local! {
    // (champion, enemy?, round seen, shards)
    static SEEN: RefCell<Vec<(ChampionType, bool, u32, f32)>> = const { RefCell::new(Vec::new()) };
    // (round worked out, pressing)
    static VERDICT: std::cell::Cell<Option<(u32, bool)>> = const { std::cell::Cell::new(None) };
}

fn shards(c: &Champion) -> Result<f32> {
    let b = c.base_stats()?;
    let part = |now: f32, base: f32, growth: f32| ((now - base) / growth.max(0.1)).max(0.0);
    Ok(part(c.max_health()?, b.base_health, b.health_growth)
        + part(c.strength()?, b.base_strength, b.strength_growth)
        + part(c.focus()?, b.base_focus, b.focus_growth))
}

fn remember(c: &Champion, enemy: bool, round: u32) -> Result<()> {
    let (name, s) = (c.champion_name()?, shards(c)?);
    SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();
        seen.retain(|&(n, e, _, _)| !(n == name && e == enemy));
        seen.push((name, enemy, round, s));
    });
    Ok(())
}

// Whether we're pressing this round (worked out once a round).
pub fn pressing() -> Result<bool> {
    if !PRESS {
        return Ok(false);
    }
    let round = cambc::round_number()?;
    if let Some((r, on)) = VERDICT.with(|v| v.get()) {
        if r == round {
            return Ok(on);
        }
    }
    let sight = super::sight()?;
    remember(me()?, false, round)?;
    for ally in &sight.allies {
        remember(ally, false, round)?;
    }
    for enemy in &sight.enemies {
        remember(enemy, true, round)?;
    }
    let on = round >= PRESS_FROM_ROUND
        && me()?.gem_count()? <= PRESS_MAX_GEMS
        && SEEN.with(|seen| {
            let seen = seen.borrow();
            let avg = |enemy: bool| {
                let v: Vec<f32> = seen
                    .iter()
                    .filter(|&&(_, e, r, _)| e == enemy && round < r + PRESS_MEMORY_ROUNDS)
                    .map(|&(_, _, _, s)| s)
                    .collect();
                (v.len(), v.iter().sum::<f32>() / v.len().max(1) as f32)
            };
            let ((_, ours), (n, theirs)) = (avg(false), avg(true));
            n >= PRESS_MIN_SEEN && ours >= PRESS_RATIO * theirs.max(1.0)
        });
    if on {
        super::trace::trace("press");
    }
    VERDICT.with(|v| v.set(Some((round, on))));
    Ok(on)
}
