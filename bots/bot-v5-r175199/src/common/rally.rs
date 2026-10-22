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
// Rallies for any group with fighting camps (wolves, elk, golems) we'd beat with a few more
// allies, on any map: the elk-share rule above came before RALLY_WAIT_STEPS (an ally only joins a
// rally that's no further than its nearest ordinary camp), and without it 01corridors (every group
// four wolves or elk plus clusters) left us walking between groups judged unwinnable, taking 5% of
// its camp gold. Off: elk only, on elk maps.
pub const ANY_GROUP_RALLY: bool = true;
// A champion that keeps finding the groups it walks to unwinnable (collect::struggling) joins a
// rally up to this many steps further than its nearest camp: on 01corridors (every group needs a
// few of us) champions walked up one corridor and down the next to group after group they then
// judged lost, skipping rallies for the next "closer" camp. Negative: off.
pub const STRUGGLE_JOIN_EXTRA_STEPS: i32 = 30;

// Most extra allies an elk rally may count on: we call one only if us plus this many would win.
pub const RALLY_MAX_HELPERS: usize = 3;
// Monolith rallies, on any map with one, from this round, counting on up to this many allies.
pub const MONOLITH_RALLY: bool = true;
pub const MONOLITH_RALLY_FROM_ROUND: u32 = 250;
pub const MONOLITH_MAX_HELPERS: usize = 4;
// A monolith rally is joined when it's at most this many steps further than the nearest camp we'd
// farm instead (instead of RALLY_WAIT_STEPS closer): 200 gems and 300 gold split between a few of
// us are worth many rounds of ordinary farming, but on 00contest (the monoliths are its only gems)
// allies skipped every one of 32 monolith rallies for a closer camp. Negative: off (use
// RALLY_WAIT_STEPS).
// 2026-10-21 (8 monolith maps, 96 games): 44-52 against off; more monolith gems on 00contest (383 vs
// 217) but more deaths and less gold, and 01boss-battle (its monoliths guard the altar) 4-8.
pub const MONOLITH_JOIN_EXTRA_STEPS: i32 = -1;
// Waiting for allies at a rally counts as this many steps of extra walking: a rally is only joined
// when that's no further than the nearest camp we'd farm instead (farm_plan). 0: always join.
pub const RALLY_WAIT_STEPS: i32 = 10;
// Don't join with less than this share of our health.
pub const RALLY_MIN_HEALTH: f32 = 0.6;
const OFFSET: usize = 0;
const PARTICIPANTS_OFFSET: usize = 5;

// Help calls: whoever hits a group with elk or golems calls a rally on it if none is on, flagged as
// a fight in progress (HELP_FLAG in byte 0). Champions that started such a fight alone mostly lost
// it (cleared 27-34% of them, against 77-79% with an ally; 42 traced games, 2026-10-21): the fight
// check counted allies who could walk over (collect::ALLY_JOIN_TURNS), but they had targets of
// their own. An ally answers a help call within HELP_CALL_MAX_STEPS when it's at most
// HELP_CALL_EXTRA_STEPS further than its nearest camp (no waiting: the fight is on). Off: no calls.
// 2026-10-21, 132 games: 62-70 against none (calls were rare: the one rally slot was usually taken).
pub const HELP_CALLS: bool = false;
pub const HELP_CALL_EXTRA_STEPS: i32 = 6;
pub const HELP_CALL_MAX_STEPS: i32 = 16;
const HELP_FLAG: u8 = 0x80;

// Only allies that have joined the rally count as helpers in its camp fight (collect.rs): counting
// every ally in reach let one just walking past make the fight look winnable, so the champions
// waiting attacked and woke the group. Off: every ally in reach counts.
pub const RALLY_PARTICIPANTS_ONLY: bool = true;

// Rally cap: the caller knows how many allies it needs (the fewest stand-ins that make the fight
// winnable); that number goes in the top three bits of byte 5 (the joined bits use the low five),
// and once the joined champions (the caller included) reach it plus RALLY_CAP_SPARE, nobody else
// joins: the rest farm on. Every champion fit to join used to join, four of them for a group one
// helper would beat. Off: no cap. 2026-10-22, one game a map against off, two seeds: 18/40 and
// 19/40, but gold 5.55 and 5.72 a round against 5.21, tributes +5-8%, deaths 12.3 and 12.8 against
// 17.2 and 14.9.
pub const RALLY_CAP: bool = true;
const RALLY_CAP_SPARE: u32 = 1;
const NEED_SHIFT: u32 = 5;

// Record that the rally we just called needs `extra` allies besides us (RALLY_CAP).
pub fn set_need(extra: usize) -> Result<()> {
    if !RALLY_CAP {
        return Ok(());
    }
    let mut byte = [0u8];
    cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
    let need = (extra.min(7) as u8) << NEED_SHIFT;
    cambc::shared_memory().write(PARTICIPANTS_OFFSET, &[(byte[0] & 0x1f) | need])
}

// Whether the active rally already has as many joined as it needs (RALLY_CAP), not counting us.
pub fn full() -> Result<bool> {
    if !RALLY_CAP || active()?.is_none() {
        return Ok(false);
    }
    let mut byte = [0u8];
    cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
    let need = (byte[0] >> NEED_SHIFT) as u32;
    if need == 0 {
        return Ok(false);
    }
    if slot_of(super::me()?.champion_name()?).is_some_and(|s| byte[0] & (1 << s) != 0) {
        return Ok(false);
    }
    Ok((byte[0] & 0x1f).count_ones() >= need + 1 + RALLY_CAP_SPARE)
}

fn slot_of(champion: cambc::ChampionType) -> Option<usize> {
    crate::opening::team().iter().position(|&c| c == champion).filter(|&s| s < 8)
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
    cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
    if byte[0] & (1 << slot) == 0 {
        cambc::shared_memory().write(PARTICIPANTS_OFFSET, &[byte[0] | (1 << slot)])?;
    }
    Ok(())
}

// Whether `champion` has joined the active rally.
pub fn joined(champion: cambc::ChampionType) -> Result<bool> {
    let Some(slot) = slot_of(champion) else {
        return Ok(false);
    };
    let mut byte = [0u8];
    cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
    Ok(byte[0] & (1 << slot) != 0)
}

thread_local! {
    // This round's reading of the shared memory: (round, active rally group, whether it's a help call).
    static ACTIVE: Cell<Option<(u32, Option<usize>, bool)>> = const { Cell::new(None) };
}

fn read() -> Result<[u8; 2]> {
    let mut bytes = [0u8; 2];
    cambc::shared_memory().read(OFFSET, &mut bytes)?;
    Ok(bytes)
}

fn write(bytes: [u8; 2]) -> Result<()> {
    cambc::shared_memory().write(OFFSET, &bytes)?;
    cambc::shared_memory().write(PARTICIPANTS_OFFSET, &[0])?;
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
        if !crate::regions::reachable_for_me(index)? {
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
    if ELK_RALLY && has(cambc::CampType::Elk) && (map_has_elk()? || crate::opening::elk_rallies()?) {
        return Ok(Some(RALLY_MAX_HELPERS));
    }
    if ANY_GROUP_RALLY
        && (has(cambc::CampType::Elk) || has(cambc::CampType::Wolf) || has(cambc::CampType::Golem))
    {
        return Ok(Some(RALLY_MAX_HELPERS));
    }
    Ok(None)
}

// The group of the active rally, if any (an expired one is cleared). Read once per round.
pub fn active() -> Result<Option<usize>> {
    Ok(state()?.0)
}

// Whether group `index` has a monolith.
pub fn is_monolith(index: usize) -> Result<bool> {
    Ok(super::world::camp_groups()?
        .get(index)
        .is_some_and(|g| g.camps().iter().any(|c| c.kind() == cambc::CampType::Monolith)))
}

// Whether the active rally is a help call (a fight in progress).
pub fn is_help_call() -> Result<bool> {
    Ok(state()?.1)
}

fn state() -> Result<(Option<usize>, bool)> {
    if !ELK_RALLY && !MONOLITH_RALLY {
        return Ok((None, false));
    }
    let round = cambc::round_number()?;
    if let Some((r, group, help)) = ACTIVE.with(|a| a.get()) {
        if r == round {
            return Ok((group, help));
        }
    }
    let [byte, called] = read()?;
    let group = byte & !HELP_FLAG;
    let mut active = (group > 0).then(|| group as usize - 1);
    if active.is_some() && (round as u8).wrapping_sub(called) as u32 >= RALLY_ROUNDS {
        write([0, 0])?;
        active = None;
    }
    let help = active.is_some() && byte & HELP_FLAG != 0;
    ACTIVE.with(|a| a.set(Some((round, active, help))));
    Ok((active, help))
}

// We just hit a camp of group `index`: call for help if it has elk or golems (HELP_CALLS).
pub fn call_for_help(index: usize) -> Result<()> {
    if !HELP_CALLS || index + 1 >= HELP_FLAG as usize || active()?.is_some() {
        return Ok(());
    }
    let groups = super::world::camp_groups()?;
    let Some(group) = groups.get(index) else {
        return Ok(());
    };
    if !group
        .camps()
        .iter()
        .any(|c| matches!(c.kind(), cambc::CampType::Elk | cambc::CampType::Golem))
    {
        return Ok(());
    }
    super::trace::trace("help-call");
    write([(index as u8 + 1) | HELP_FLAG, cambc::round_number()? as u8])?;
    join()
}

// Call a rally at group `index`, unless one is already on.
pub fn call(index: usize) -> Result<()> {
    if active()?.is_some() || index + 1 >= HELP_FLAG as usize {
        return Ok(());
    }
    super::trace::trace("rally-call");
    let round = cambc::round_number()?;
    write([index as u8 + 1, round as u8])?;
    OUR_RALLY.with(|o| o.set(Some((index, round))));
    join()
}

// Call a rally at jackpot `index` in place of the one on (farm.rs: only over a non-jackpot rally).
pub fn call_over(index: usize) -> Result<()> {
    if index + 1 >= HELP_FLAG as usize {
        return Ok(());
    }
    super::trace::trace("rally-call");
    let round = cambc::round_number()?;
    write([index as u8 + 1, round as u8])?;
    OUR_RALLY.with(|o| o.set(Some((index, round))));
    join()
}

// A rally nobody answers: when ours runs out (RALLY_ROUNDS) with no other ally joined (heading
// there) and none within ANSWER_TILES of the group, we give the group up for
// UNANSWERED_RALLY_ROUNDS (judged lost, no new rally on it) and farm elsewhere. We called again the
// moment it ran out, so a champion at a group too far from the others waited there all game: Ren
// stood beside 01concentrated-wealth's corner elk for 210 rounds, calling a rally every 20. 0: off.
// 2026-10-21, 140 games: the first version (no joined check, and it ended the rally for allies
// already on their way) 59 against off.
pub const UNANSWERED_RALLY_ROUNDS: u32 = 40;
const ANSWER_TILES: i32 = 6;

thread_local! {
    static OUR_RALLY: Cell<Option<(usize, u32)>> = const { Cell::new(None) };
}

// Called every acting turn (farm_plan): give up our rally if it went unanswered.
pub fn check_unanswered() -> Result<()> {
    if UNANSWERED_RALLY_ROUNDS == 0 {
        return Ok(());
    }
    let Some((index, called)) = OUR_RALLY.with(|o| o.get()) else {
        return Ok(());
    };
    let round = cambc::round_number()?;
    if active()? != Some(index) && round < called + RALLY_ROUNDS {
        OUR_RALLY.with(|o| o.set(None)); // ended early: cleared, or someone else's now
        return Ok(());
    }
    if round + 1 < called + RALLY_ROUNDS {
        return Ok(());
    }
    OUR_RALLY.with(|o| o.set(None));
    // A jackpot (jackpot.rs) is worth the wait: allies come from further away for it.
    if super::jackpot::is_jackpot(index) {
        return Ok(());
    }
    let Some(group) = super::world::camp_groups()?.get(index) else {
        return Ok(());
    };
    let mut joined = [0u8];
    cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut joined)?;
    let ours = slot_of(super::me()?.champion_name()?).map_or(0, |s| 1u8 << s);
    let mut answered = joined[0] & !ours != 0;
    for ally in &super::world::sight()?.allies {
        let p = ally.position()?;
        if group.camps().iter().any(|c| super::world::distance(c.spawn_position(), p) <= ANSWER_TILES) {
            answered = true;
            break;
        }
    }
    if !answered {
        super::trace::trace("rally-unanswered");
        super::collect::extend_loss(index, round + UNANSWERED_RALLY_ROUNDS);
        CHECKED_UNTIL.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() <= index {
                c.resize(index + 1, 0);
            }
            c[index] = c[index].max(round + UNANSWERED_RALLY_ROUNDS);
        });
    }
    Ok(())
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
