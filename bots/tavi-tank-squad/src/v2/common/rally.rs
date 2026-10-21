// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
// Rallies, for elk and monoliths. Two elk have 4,800 health and hit for 120 a round, so a champion alone (or with
// the one ally who happens to be near) loses the fight and nobody starts it; on ambush the six
// groups with elk were cleared 3 times in 1,916 rounds. A champion that sees an elk group it
// would beat with a few more allies calls a rally in the team's shared memory; everyone fit to
// join makes that group their farm goal. Camps only fight back once attacked, so champions gather
// next to it, and nobody attacks until the camp-fight check, counting the allies actually there,
// says we'd win (collect::camp_fight_winnable judges a rally group afresh every turn).
//
// Monoliths (20,000 health, hit for 200, worth 300 gold and 200 gems: a fifth of the target) are
// rallied for from MONOLITH_RALLY_FROM_ROUND, once us plus up to MONOLITH_MAX_HELPERS allies like
// us would win: the stand-in check uses our current items, so it passes as the team gets stronger.
//
// Shared memory: byte 0 = rally group index + 1 (0: none), byte 1 = round it was called (low
// byte), byte 5 = who has joined it (one bit per champion, in draft order; cleared with every
// change to bytes 0-1). One rally at a time; it ends when the group is cleared or respawning, or after
// RALLY_ROUNDS rounds.
use cambc::Result;
use std::cell::Cell;

// Off: no rallies; elk groups are only fought when the allies already near are enough.
pub const ELK_RALLY: bool = true;
// Rallies only on maps where at least this share of the camp groups we can reach have elk
// (worked out once per champion). Rallies won where elk dominate (ambush 12/12, elkring 6/6,
// corners 5/6: 45-100% of groups) and lost where other camps are plentiful and close (collision
// 0/6, spokes, hedgerows, fragment 1/6: 10-33%): waiting for allies there wastes cheap farming.
// Lowered from 0.45 for spine (8 of 18 groups: 44%), where without rallies a lone champion judged
// an elk unwinnable and walked off 29 times in one game. No other map is between 40% and 45%.
pub const ELK_RALLY_MIN_SHARE: f32 = 0.40;
const RALLY_ROUNDS: u32 = 20;
// Most extra allies an elk rally may count on: we call one only if us plus this many would win.
pub const RALLY_MAX_HELPERS: usize = 3;
// Monolith rallies, on any map with one, from this round, counting on up to this many allies.
pub const MONOLITH_RALLY: bool = true;
pub const MONOLITH_RALLY_FROM_ROUND: u32 = 250;
pub const MONOLITH_MAX_HELPERS: usize = 4;
// Waiting for allies at a rally counts as this many steps of extra walking: a rally is only joined
// when that's no further than the nearest camp we'd farm instead (farm_plan). 0: always join.
pub const RALLY_WAIT_STEPS: i32 = 10;
// Don't join with less than this share of our health.
pub const RALLY_MIN_HEALTH: f32 = 0.6;
const OFFSET: usize = 0;
const PARTICIPANTS_OFFSET: usize = 5;

// Only allies that have joined the rally count as helpers in its camp fight (collect.rs): counting
// every ally in reach let one just walking past make the fight look winnable, so the champions
// waiting attacked and woke the group. Off: every ally in reach counts.
pub const RALLY_PARTICIPANTS_ONLY: bool = true;

fn slot_of(champion: cambc::ChampionType) -> Option<usize> {
    crate::v2::opening::team().iter().position(|&c| c == champion).filter(|&s| s < 8)
}

// Mark us as having joined the active rally (we're heading to it this turn).
pub fn join() -> Result<()> {
    if !RALLY_PARTICIPANTS_ONLY {
        return Ok(());
    }
    let Some(slot) = slot_of(super::me()?.champion_name()?) else {
        return Ok(());
    };
    let mut byte = [0u8];
    crate::v2::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
    if byte[0] & (1 << slot) == 0 {
        crate::v2::shared_memory().write(PARTICIPANTS_OFFSET, &[byte[0] | (1 << slot)])?;
    }
    Ok(())
}

// Whether `champion` has joined the active rally.
pub fn joined(champion: cambc::ChampionType) -> Result<bool> {
    let Some(slot) = slot_of(champion) else {
        return Ok(false);
    };
    let mut byte = [0u8];
    crate::v2::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
    Ok(byte[0] & (1 << slot) != 0)
}

thread_local! {
    // This round's reading of the shared memory: (round, active rally group).
    static ACTIVE: Cell<Option<(u32, Option<usize>)>> = const { Cell::new(None) };
}

fn read() -> Result<[u8; 2]> {
    let mut bytes = [0u8; 2];
    crate::v2::shared_memory().read(OFFSET, &mut bytes)?;
    Ok(bytes)
}

fn write(bytes: [u8; 2]) -> Result<()> {
    crate::v2::shared_memory().write(OFFSET, &bytes)?;
    crate::v2::shared_memory().write(PARTICIPANTS_OFFSET, &[0])?;
    ACTIVE.with(|a| a.set(None));
    Ok(())
}

// Whether this map has enough elk for rallies (see ELK_RALLY_MIN_SHARE).
fn map_has_elk() -> Result<bool> {
    thread_local! {
        static ELK_MAP: std::cell::OnceCell<bool> = const { std::cell::OnceCell::new() };
    }
    if let Some(yes) = ELK_MAP.with(|e| e.get().copied()) {
        return Ok(yes);
    }
    let (mut reachable, mut elk) = (0, 0);
    for (index, group) in super::world::camp_groups()?.iter().enumerate() {
        if !crate::v2::regions::reachable_for_me(index)? {
            continue;
        }
        reachable += 1;
        if group
            .camps()
            .iter()
            .any(|c| c.kind() == cambc::CampType::Elk)
        {
            elk += 1;
        }
    }
    let yes = reachable > 0 && elk as f32 >= ELK_RALLY_MIN_SHARE * reachable as f32;
    ELK_MAP.with(|e| {
        let _ = e.set(yes);
    });
    Ok(yes)
}

// Whether group `group` is worth a rally now, and how many extra allies one may count on: elk
// groups on maps with enough elk, monolith groups from MONOLITH_RALLY_FROM_ROUND.
pub fn max_helpers(group: &cambc::CampGroup, round: u32) -> Result<Option<usize>> {
    let has = |kind| group.camps().iter().any(|c| c.kind() == kind);
    if MONOLITH_RALLY && round >= MONOLITH_RALLY_FROM_ROUND && has(cambc::CampType::Monolith) {
        return Ok(Some(MONOLITH_MAX_HELPERS));
    }
    // The Group opening (opening.rs) rallies for elk on any map.
    if ELK_RALLY && has(cambc::CampType::Elk) && (map_has_elk()? || crate::v2::opening::elk_rallies()?) {
        return Ok(Some(RALLY_MAX_HELPERS));
    }
    Ok(None)
}

// The group of the active rally, if any (an expired one is cleared). Read once per round.
pub fn active() -> Result<Option<usize>> {
    if !ELK_RALLY && !MONOLITH_RALLY {
        return Ok(None);
    }
    let round = cambc::round_number()?;
    if let Some((r, group)) = ACTIVE.with(|a| a.get()) {
        if r == round {
            return Ok(group);
        }
    }
    let [group, called] = read()?;
    let mut active = (group > 0).then(|| group as usize - 1);
    if active.is_some() && (round as u8).wrapping_sub(called) as u32 >= RALLY_ROUNDS {
        write([0, 0])?;
        active = None;
    }
    ACTIVE.with(|a| a.set(Some((round, active))));
    Ok(active)
}

// Call a rally at group `index`, unless one is already on.
pub fn call(index: usize) -> Result<()> {
    if active()?.is_some() || index >= u8::MAX as usize {
        return Ok(());
    }
    super::trace::trace("rally-call");
    write([index as u8 + 1, cambc::round_number()? as u8])?;
    join()
}

// Groups found not worth a rally (we'd lose even with RALLY_MAX_HELPERS more allies) aren't
// checked again for a while: each check simulates the fight up to three times.
const RECHECK_ROUNDS: u32 = 10;

thread_local! {
    static CHECKED_UNTIL: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn worth_checking(index: usize, round: u32) -> bool {
    CHECKED_UNTIL.with(|c| c.borrow().get(index).is_none_or(|&until| round >= until))
}

pub fn checked(index: usize, round: u32) {
    CHECKED_UNTIL.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() <= index {
            c.resize(index + 1, 0);
        }
        c[index] = round + RECHECK_ROUNDS;
    });
}

// End the rally at group `index` (cleared, or respawning).
pub fn end(index: usize) -> Result<()> {
    if active()? == Some(index) {
        super::trace::trace("rally-end");
        write([0, 0])?;
    }
    Ok(())
}
