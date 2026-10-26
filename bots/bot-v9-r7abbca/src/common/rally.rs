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
// Gate siege (collect::GATE_SIEGE): a group that gates an altar is rallied for from
// GATE_RALLY_FROM_ROUND (other monoliths from MONOLITH_RALLY_FROM_ROUND), and a rally on it is
// joined up to GATE_JOIN_EXTRA_STEPS further than our nearest camp: until it falls nobody tributes.
pub const GATE_RALLY_FROM_ROUND: u32 = 150;
pub const GATE_JOIN_EXTRA_STEPS: i32 = 30;
// Waiting for allies at a rally counts as this many steps of extra walking: a rally is only joined
// when that's no further than the nearest camp we'd farm instead (farm_plan). 0: always join.
pub const RALLY_WAIT_STEPS: i32 = 10;
// Don't join with less than this share of our health.
pub const RALLY_MIN_HEALTH: f32 = 0.6;
const OFFSET: usize = 0;
const PARTICIPANTS_OFFSET: usize = 5;

// Profiling: print rally calls, needs, ends and expiries ("RALLY ..." lines; debug builds only).
pub const RALLY_LOG: bool = false;

pub fn log(event: &str, index: usize) {
    if !RALLY_LOG {
        return;
    }
    cambc::debug_block! {
        let who = super::me().and_then(|m| m.champion_name()).ok();
        println!("RALLY {} r{} g{} {:?}", event, cambc::round_number().unwrap_or(0), index, who);
    }
}

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
    if let Some(index) = active()? {
        log(
            if extra == 1 {
                "need1"
            } else if extra == 2 {
                "need2"
            } else if extra == 3 {
                "need3"
            } else {
                "need4+"
            },
            index,
        );
    }
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
    crate::opening::team()
        .iter()
        .position(|&c| c == champion)
        .filter(|&s| s < 8)
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
        if let Some(index) = active()? {
            if super::trace::TRACE {
                cambc::debug_block! {
                    let here = super::me()?.position()?;
                    let gap = super::world::camp_groups()?[index]
                        .camps()
                        .iter()
                        .map(|c| super::world::distance(c.spawn_position(), here))
                        .min()
                        .unwrap_or(0);
                    let hp = super::me()?.health()? / super::me()?.max_health()?.max(1.0);
                    println!("RALLY join r{} g{} gap{} hp{:.2}", cambc::round_number().unwrap_or(0), index, gap, hp);
                }
            }
        }
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
    if RALLY_REPLIES && !RALLY_AVAIL {
        cambc::shared_memory().write(DECLINED_OFFSET, &[0])?;
    }
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
pub fn max_helpers(index: usize, round: u32) -> Result<Option<usize>> {
    let group = &super::world::camp_groups()?[index];
    let has = |kind| group.camps().iter().any(|c| c.kind() == kind);
    if super::collect::GATE_SIEGE
        && round >= GATE_RALLY_FROM_ROUND
        && super::base::is_gate_group(index)?
    {
        return Ok(Some(MONOLITH_MAX_HELPERS));
    }
    if MONOLITH_RALLY && round >= MONOLITH_RALLY_FROM_ROUND && has(cambc::CampType::Monolith) {
        return Ok(Some(MONOLITH_MAX_HELPERS));
    }
    // The Group opening (opening.rs) rallies for elk on any map.
    if ELK_RALLY && has(cambc::CampType::Elk) && (map_has_elk()? || crate::opening::elk_rallies()?)
    {
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
    Ok(super::world::camp_groups()?.get(index).is_some_and(|g| {
        g.camps()
            .iter()
            .any(|c| c.kind() == cambc::CampType::Monolith)
    }))
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
    let group = byte & !HELP_FLAG & !DROP_FLAG;
    // RALLY_HANDSHAKE 1: a responder dropped out and called the rally off (the caller clears it).
    let dropped = RALLY_HANDSHAKE == 1 && byte & DROP_FLAG != 0;
    let mut active = (group > 0 && !dropped).then(|| group as usize - 1);
    // RALLY_HANDSHAKE 3: the caller restates the round every turn; only a silent caller ends it.
    let limit = if RALLY_HANDSHAKE == 3 {
        HEARTBEAT_ROUNDS
    } else {
        RALLY_ROUNDS
    };
    if active.is_some() && (round as u8).wrapping_sub(called) as u32 >= limit {
        if let Some(index) = active {
            log("expire", index);
        }
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
    if active()?.is_some() || index + 1 >= DROP_FLAG as usize {
        return Ok(());
    }
    super::trace::trace("rally-call");
    log("call", index);
    let round = cambc::round_number()?;
    write([index as u8 + 1, round as u8])?;
    OUR_RALLY.with(|o| o.set(Some((index, round))));
    CALLER.with(|c| c.set(Some((index, round, 0))));
    REPLY.with(|r| r.set(Some((index, round as u8, true, false))));
    join()
}

// Call a rally at jackpot `index` in place of the one on (farm.rs: only over a non-jackpot rally).
pub fn call_over(index: usize) -> Result<()> {
    if index + 1 >= DROP_FLAG as usize {
        return Ok(());
    }
    super::trace::trace("rally-call");
    let round = cambc::round_number()?;
    write([index as u8 + 1, round as u8])?;
    OUR_RALLY.with(|o| o.set(Some((index, round))));
    CALLER.with(|c| c.set(Some((index, round, 0))));
    REPLY.with(|r| r.set(Some((index, round as u8, true, false))));
    join()
}

// Rally replies (v8, the user's design, 2026-10-24). Every champion answers a new rally on its
// first turn after the call, whatever else it's doing (answer(), from bot.rs, so a kept or reused
// farm plan doesn't delay it): if the rally already has the allies it needs it says no, else it
// goes if it can get there within RALLY_REPLY_MAX_STEPS steps, on foot (with RALLY_MIN_HEALTH of
// its health) or by recalling first (RECALL_TURNS, then the walk from our spawn: a rally near our
// base is cheap to answer from anywhere). Going sets its bit in byte 5 (as joining did); no sets
// it in byte 4. Only champions that said yes head there (farm_plan); the rest ignore it: no
// waiting charge against their nearest camp, no decision kept for RALLY_DECISION_ROUNDS. The caller
// cancels the rally if fewer than the allies it needs said yes within RALLY_REPLY_ROUNDS rounds
// (every ally has had a turn by its next one), or as soon as everyone has answered and too few
// said yes, and gives the group up for UNANSWERED_RALLY_ROUNDS. Traced before: 47% of rallies had
// nobody join, 82% expired unhit (11% of champion turns), and the most common reason was
// "rally-skip-closer-camp". On the way, a champion that said yes only hits camps of the rally
// group (or awake ones, fighting it already), and a kept farm plan (farm::FARM_PLAN_MIN_FUEL,
// FIT_STALE_ROUNDS) is only used if it was a rally plan: in 6 traced games, champions that said
// yes spent 14% of the rally's rounds hitting other camps on the way and 26% of their farming turns
// on a plan from before the answer. Off: the old join rule (steps plus RALLY_WAIT_STEPS against the
// nearest camp) and check at the end of the rally.
// 2026-10-24, 4 seed pairs x 50 games against same-game controls (102/200): without the two
// on-the-way rules 99/200 (+40/-43), with them 96/200 (+46/-52); fuel-exhausted turns 118 and 122
// a 1,000 rounds against 138 (-12 to -15%), gold ratio a little lower (0.97 against 0.99). In 6
// traced games 53-62% of rallies were cancelled at once instead of waiting out 20 rounds;
// helpers that said yes spent 48% of the rally on it (42% without the on-the-way rules), but most
// answered rallies still ran out unwon (17 of 251 cleared). On: a fuel saver that tests neutral.
pub const RALLY_REPLIES: bool = true;
const RALLY_REPLY_ROUNDS: u32 = 1;
// v9 (2026-10-25), 120 games each against bot-v8 with same-seed controls: 26 steps 58/120 against
// 55 (gems submitted +4.6%, deaths 10.3 against 10.9); 34 steps on fresh seeds 60/120 against 55
// (+12/-7, gems +2.5%). Neither moved big captures. 26.
const RALLY_REPLY_MAX_STEPS: i32 = 26;
const RECALL_TURNS: i32 = 5;
// Who said no (one bit per champion, like byte 5). Byte 4 is the capture claim's when that's on.
const DECLINED_OFFSET: usize = 4;
const _: () = assert!(!(RALLY_REPLIES && super::capclaim::SHARED_CAPTURE_CLAIM));

// v8 (2026-10-24, the user's design): byte 4 holds who can't come to any rally right now (dead,
// or fighting enemies this turn), kept up by every champion every turn (set_unavailable), instead
// of who said no to the current one. A rally is only called if enough allies are available for
// the need (can_staff), and the caller then acts last in the next round (priority 0, bot.rs) so
// by its turn every ally has answered and it can keep or cancel the rally at once. Champions that
// stood waiting at a rally nobody could answer did nothing for the rally's first round. Off: byte
// 4 is the no answers, and the caller's next turn may come before some allies have answered.
// 2026-10-24, 4 seed pairs x 50 games against same-game controls: 119/200 against 101 (seeds
// 2501-2508), then 96/200 against 95 on fresh seeds (2601-2608); together 215/400 against 196
// (+101/-82), gold +2-3%, gems submitted +7%, kills 11.6-12.1 against 10.9-11.1, deaths a little
// down, fuel-exhausted turns about the same. On.
pub const RALLY_AVAIL: bool = true;

thread_local! {
    // The availability bit we last wrote (RALLY_AVAIL), to skip writing an unchanged one.
    static AVAIL_WRITTEN: Cell<Option<bool>> = const { Cell::new(None) };
}

// Say whether we can come to a rally now (RALLY_AVAIL): `unavailable` when dead or fighting.
pub fn set_unavailable(unavailable: bool) -> Result<()> {
    if unavailable {
        drop_out()?;
    }
    if !RALLY_AVAIL || AVAIL_WRITTEN.with(|a| a.get()) == Some(unavailable) {
        return Ok(());
    }
    let Some(slot) = slot_of(super::me()?.champion_name()?) else {
        return Ok(());
    };
    let mut byte = [0u8];
    cambc::shared_memory().read(DECLINED_OFFSET, &mut byte)?;
    let b = if unavailable {
        byte[0] | (1 << slot)
    } else {
        byte[0] & !(1 << slot)
    };
    cambc::shared_memory().write(DECLINED_OFFSET, &[b])?;
    AVAIL_WRITTEN.with(|a| a.set(Some(unavailable)));
    Ok(())
}

// Whether enough allies (not us) are available to answer a rally needing `extra` of them.
pub fn can_staff(extra: usize) -> Result<bool> {
    if !RALLY_AVAIL {
        return Ok(true);
    }
    let mut byte = [0u8];
    cambc::shared_memory().read(DECLINED_OFFSET, &mut byte)?;
    let ours = slot_of(super::me()?.champion_name()?).map_or(0, |s| 1u8 << s);
    let team = (crate::opening::team().len().min(5) as u32).saturating_sub(1);
    let away = (byte[0] & 0x1f & !ours).count_ones();
    Ok(team.saturating_sub(away) as usize >= extra)
}

// Whether we called a rally and wait for the answers (RALLY_AVAIL: we act last next round).
pub fn awaiting_replies() -> bool {
    RALLY_AVAIL && OUR_RALLY.with(|o| o.get()).is_some()
}

thread_local! {
    // Our answer to a rally: (group, round it was called (low byte), going, going by recall).
    static REPLY: Cell<Option<(usize, u8, bool, bool)>> = const { Cell::new(None) };
}

// Answer the active rally if we haven't yet (RALLY_REPLIES). One shared-memory read on a round
// with nothing new; deciding costs a walking-distance lookup, once per rally.
pub fn answer() -> Result<()> {
    if !RALLY_REPLIES {
        return Ok(());
    }
    let Some(index) = active()? else {
        return Ok(());
    };
    let [_, called] = read()?;
    let me = super::me()?;
    let Some(slot) = slot_of(me.champion_name()?) else {
        return Ok(());
    };
    let mut bytes = [0u8; 2]; // byte 4 (said no, or with RALLY_AVAIL can't come), byte 5 (going, need)
    cambc::shared_memory().read(DECLINED_OFFSET, &mut bytes)?;
    let [declined, joined] = bytes;
    if RALLY_AVAIL {
        // No answers aren't written: remember ours. Unavailable: answer once we're free again.
        // RALLY_HANDSHAKE 3 restates the round every turn: an answer stands for the group, a no
        // for RECONSIDER_ROUNDS.
        let round = cambc::round_number()?;
        let answered = REPLY.with(|r| r.get()).is_some_and(|(g, c, going, _)| {
            g == index
                && if RALLY_HANDSHAKE == 3 {
                    going || round < REPLIED.with(|r| r.get()) + RECONSIDER_ROUNDS
                } else {
                    c == called
                }
        });
        if joined & (1 << slot) != 0 || declined & (1 << slot) != 0 || answered {
            return Ok(());
        }
    } else if (declined | joined) & (1 << slot) != 0 {
        return Ok(()); // answered already (a new rally clears both bytes)
    }
    let need = u32::from(joined >> NEED_SHIFT).max(1);
    let enough = (joined & 0x1f).count_ones() > need; // the caller and its helpers
    let mut going = false;
    let mut by_recall = false;
    // The gate guard holds its tile (base::GATE_GUARD): it can't come.
    if !enough
        && me.is_alive()?
        && !me.is_recalling()?
        && !super::collect::capturing()?
        && super::base::gate_guard_tile()?.is_none()
    {
        let tiles: Vec<cambc::Vec2> = super::world::camp_groups()?[index]
            .camps()
            .iter()
            .map(|c| c.spawn_position())
            .collect();
        let steps = super::walk::StepsFrom::new(me.position()?);
        let healthy = me.health()? >= RALLY_MIN_HEALTH * me.max_health()?;
        let mut on_foot = match steps.nearest(&tiles) {
            Some(t) if healthy => steps.steps(t),
            _ => i32::MAX,
        };
        // RALLY_DASH_AWARE: with a dash, holes are a step (a dash) instead of a detour.
        if RALLY_DASH_AWARE && healthy && me.abilities().iter().any(|a| a.is_dash()) {
            on_foot = on_foot.min(steps_over_holes(me.position()?, &tiles)?.saturating_add(1));
        }
        // RALLY_DASH_GAPS: or walk to a gap, dash over it once the dash is ready, walk on.
        if RALLY_DASH_GAPS && healthy {
            if let Some(dash) = me.abilities().iter().find(|a| a.is_dash()) {
                on_foot = on_foot.min(steps_via_gap(&steps, &tiles, dash.cooldown()?.max(0))?);
            }
        }
        let recall = tiles
            .iter()
            .filter_map(|&t| super::walk::spawn_steps(t))
            .min()
            .map_or(i32::MAX, |s| s + RECALL_TURNS);
        if on_foot.min(recall) <= RALLY_REPLY_MAX_STEPS {
            going = true;
            by_recall = recall < on_foot;
        }
    }
    log(if going { "yes" } else { "no" }, index);
    if going {
        super::trace::trace("rally-reply-yes");
        cambc::shared_memory().write(PARTICIPANTS_OFFSET, &[joined | (1 << slot)])?;
    } else {
        super::trace::trace("rally-reply-no");
        if !RALLY_AVAIL {
            cambc::shared_memory().write(DECLINED_OFFSET, &[declined | (1 << slot)])?;
        }
    }
    REPLY.with(|r| r.set(Some((index, called, going, by_recall))));
    REPLIED.with(|r| r.set(cambc::round_number().unwrap_or(0)));
    Ok(())
}

// v8 (2026-10-24): champions now dash across holes on the way (movement::DASH_OVER_HOLES), but the
// answer still judged the trip on foot, round the holes. With this on, a champion with a dash also
// counts the trip with holes as passable, plus a step for the dash, and takes the shorter. Off: on
// foot only.
// 2026-10-24, 2 seeds x 25 maps against the same games without it (26/50): 22/50 (+4/-8), gold
// -3.2%, deaths 12.5 against 11.3: more yes answers to rallies across holes that the dash (8-turn
// cooldown, one crossing) doesn't really shorten. Off.
pub const RALLY_DASH_AWARE: bool = false;

// v8 (2026-10-24, the user's idea): like RALLY_DASH_AWARE, but only a real crossing counts: one
// dash over a one-tile strip of holes on the way (from a free tile beside it straight across to
// the free tile opposite), taken once the dash is off cooldown (we wait at the gap if it isn't).
// Off: on foot only.
// 2026-10-24, the same 50 games as RALLY_DASH_AWARE (control 26/50): 24/50 (+6/-8), gold -4.2%,
// deaths 12.0 against 11.3, fuel-exhausted turns 141 against 133. Counting a dash shortcut, either
// way, gets more yes answers to rallies across gaps, and those cost gold. Off.
pub const RALLY_DASH_GAPS: bool = false;

// One-tile hole strips a dash can cross: (launch, landing), both free terrain with a hole between,
// both ways round. Worked out once per champion from the hole tiles.
pub(super) fn gap_crossings() -> Result<&'static [(cambc::Vec2, cambc::Vec2)]> {
    thread_local! {
        static GAPS: std::cell::OnceCell<&'static [(cambc::Vec2, cambc::Vec2)]> =
            const { std::cell::OnceCell::new() };
    }
    if let Some(gaps) = GAPS.with(|g| g.get().copied()) {
        return Ok(gaps);
    }
    let (w, h) = (cambc::map::width()?, cambc::map::height()?);
    // 0: free, 1: hole, 2: can't land (wall, altar, enemy spawn).
    let mut grid = vec![0u8; (w * h) as usize];
    let mut mark = |b: &cambc::Bitboard, v: u8| -> Result<()> {
        for p in b.positions()? {
            grid[(p.y * w + p.x) as usize] = v;
        }
        Ok(())
    };
    mark(cambc::map::holes()?, 1)?;
    mark(cambc::map::walls()?, 2)?;
    mark(cambc::map::altars()?, 2)?;
    mark(cambc::map::spawn_region(cambc::Team::Enemy)?, 2)?;
    let at = |x: i32, y: i32| {
        if (0..w).contains(&x) && (0..h).contains(&y) {
            grid[(y * w + x) as usize]
        } else {
            2
        }
    };
    let mut gaps = Vec::new();
    for hole in cambc::map::holes()?.positions()? {
        for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
            let a = cambc::Vec2::new(hole.x - dx, hole.y - dy);
            let b = cambc::Vec2::new(hole.x + dx, hole.y + dy);
            if at(a.x, a.y) == 0 && at(b.x, b.y) == 0 {
                gaps.push((a, b));
                gaps.push((b, a));
            }
        }
    }
    let gaps: &'static [(cambc::Vec2, cambc::Vec2)] = Vec::leak(gaps);
    GAPS.with(|g| {
        let _ = g.set(gaps);
    });
    Ok(gaps)
}

// Steps to the nearest of `tiles` through one gap crossing (RALLY_DASH_GAPS): walk to its launch
// tile (`steps`), wait out `cooldown` there if needed, dash (one turn), walk on. i32::MAX if none.
fn steps_via_gap(
    steps: &super::walk::StepsFrom,
    tiles: &[cambc::Vec2],
    cooldown: i32,
) -> Result<i32> {
    let here = super::me()?.position()?;
    let near = |p: cambc::Vec2| {
        tiles
            .iter()
            .map(|&t| super::world::distance(t, p))
            .min()
            .unwrap_or(i32::MAX)
    };
    let mut best = i32::MAX;
    let mut field: Option<cambc::DistanceField> = None;
    for &(launch, landing) in gap_crossings()? {
        // Straight-line lower bound first: only crossings that could matter cost a lookup.
        let bound = super::world::distance(here, launch).max(cooldown) + 1 + near(landing);
        if bound > RALLY_REPLY_MAX_STEPS || bound >= best {
            continue;
        }
        if field.is_none() {
            let terrain = cambc::map::walls()?
                .union(cambc::map::holes()?)?
                .union(cambc::map::altars()?)?;
            let mut goals = cambc::Bitboard::new()?;
            for &t in tiles {
                goals.set(t, true)?;
            }
            let mut f = cambc::DistanceField::new()?;
            f.compute(
                &terrain,
                &goals,
                &mut cambc::Bitboard::new()?,
                RALLY_REPLY_MAX_STEPS as u16,
            )?;
            field = Some(f);
        }
        let Some(f) = field.as_ref() else {
            continue;
        };
        let after = f.get(landing)?;
        if after == cambc::DistanceField::UNREACHABLE {
            continue;
        }
        let total = steps.steps(launch).max(cooldown) + 1 + i32::from(after);
        best = best.min(total);
    }
    Ok(best)
}

// Steps from `from` to the nearest of `tiles` with holes passable (walls and altars still block).
fn steps_over_holes(from: cambc::Vec2, tiles: &[cambc::Vec2]) -> Result<i32> {
    let blocked = cambc::map::walls()?.union(cambc::map::altars()?)?;
    let mut goals = cambc::Bitboard::new()?;
    for &t in tiles {
        goals.set(t, true)?;
    }
    let mut field = cambc::DistanceField::new()?;
    field.compute(&blocked, &goals, &mut cambc::Bitboard::new()?, 40)?;
    let d = field.get(from)?;
    Ok(if d == cambc::DistanceField::UNREACHABLE {
        i32::MAX
    } else {
        i32::from(d)
    })
}

// Whether we said yes to the active rally (RALLY_REPLIES): our bit in byte 5.
pub fn going() -> Result<bool> {
    if active()?.is_none() {
        return Ok(false);
    }
    let Some(slot) = slot_of(super::me()?.champion_name()?) else {
        return Ok(false);
    };
    let mut byte = [0u8];
    cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
    Ok(byte[0] & (1 << slot) != 0)
}

// v8 (overnight tests, 2026-10-24): a rally ran out (RALLY_ROUNDS from the call) while its helpers
// were still walking or fighting: of 117 answered rallies in 6 traced games, 37 had the helpers
// there and the fight judged winnable and still expired, and only 17 were cleared. With this on,
// the rally's clock restarts once enough allies said yes and every time one of us hits the group,
// so it lasts while the fight is on (it still ends when the group is cleared). Off: 20 rounds from
// the call.
// Overnight 2026-10-24 (see RALLY_COMMIT): 108/200 against 97 alone, no gain on top of
// RALLY_COMMIT (117 against 137 without it on fresh seeds). Off.
pub const RALLY_REFRESH: bool = false;

// Restart the active rally's clock if it's on group `index` (RALLY_REFRESH).
pub fn refresh(index: usize) -> Result<()> {
    if !RALLY_REFRESH || active()? != Some(index) {
        return Ok(());
    }
    cambc::shared_memory().write(OFFSET + 1, &[cambc::round_number()? as u8])
}

// v8 (overnight tests): at the rally, everyone judges the fight again with only the allies who
// joined (RALLY_PARTICIPANTS_ONLY), and in 28 of those 117 rallies the helpers were there and the
// verdict stayed "lose": the caller sized the rally with stand-ins (collect::winnable_with_standins)
// that the real helpers didn't match. With this on, once the caller and the allies it needed
// (joined, within RALLY_ARRIVE_TILES of the group) are all there, the fight is on. Off: judged.
// 2026-10-24, 4 seed pairs x 50 games against same-game controls: overnight (seeds 2101-2108,
// control 97/200) RALLY_COMMIT 111, RALLY_REFRESH 108, both 108, HANDOFF_MODEL 1 105, 2 99,
// FUNNEL_MELEE off 109, TANK_FUNNEL off 88, HOME_STAT_GAIN 0.15/0.25/0.40 109/108/107 (deaths up
// with the threshold). Confirmed on fresh seeds 2201-2208 (control 102/200): RALLY_COMMIT +
// HANDOFF_MODEL 1 + HOME_STAT_GAIN 0.15 137/200 (+71/-36), gold +6.9%, gems submitted +10%;
// RALLY_COMMIT alone 107; with FUNNEL_MELEE off instead of model 1 112, plus RALLY_REFRESH 117.
// On (with HANDOFF_MODEL 1 and HOME_STAT_GAIN 0.15).
pub const RALLY_COMMIT: bool = true;
const RALLY_ARRIVE_TILES: i32 = 3;

// Whether the rally on group `index` has everyone it needs at the group (RALLY_COMMIT).
pub fn arrived(index: usize) -> Result<bool> {
    if !RALLY_COMMIT || active()? != Some(index) {
        return Ok(false);
    }
    let mut byte = [0u8];
    cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
    let need = u32::from(byte[0] >> NEED_SHIFT).max(1);
    let tiles: Vec<cambc::Vec2> = super::world::camp_groups()?[index]
        .camps()
        .iter()
        .map(|c| c.spawn_position())
        .collect();
    let near = |p: cambc::Vec2| {
        tiles
            .iter()
            .any(|&t| super::world::distance(t, p) <= RALLY_ARRIVE_TILES)
    };
    let me = super::me()?;
    let joined_bit = |name| slot_of(name).is_some_and(|s| byte[0] & (1 << s) != 0);
    if !joined_bit(me.champion_name()?) || !near(me.position()?) {
        return Ok(false);
    }
    let mut there = 1;
    for ally in &super::world::sight()?.allies {
        if ally.is_alive()? && joined_bit(ally.champion_name()?) && near(ally.position()?) {
            there += 1;
        }
    }
    Ok(there > need)
}

// Whether we said yes by recalling first and haven't been home since (RALLY_REPLIES).
pub fn wants_recall() -> Result<bool> {
    if !RALLY_REPLIES || !going()? {
        return Ok(false);
    }
    let Some((g, c, yes, by_recall)) = REPLY.with(|r| r.get()) else {
        return Ok(false);
    };
    if !by_recall || active()? != Some(g) {
        return Ok(false);
    }
    let me = super::me()?;
    if cambc::map::spawn_region(cambc::Team::Ally)?.is_set(me.position()?)? {
        REPLY.with(|r| r.set(Some((g, c, yes, false))));
        return Ok(false);
    }
    Ok(true)
}

// The caller's side of RALLY_REPLIES: cancel the rally once it's clear too few said yes.
fn check_replies(index: usize, called: u32, round: u32) -> Result<()> {
    let mut bytes = [0u8; 2];
    cambc::shared_memory().read(DECLINED_OFFSET, &mut bytes)?;
    let [declined, joined] = bytes;
    let need = u32::from(joined >> NEED_SHIFT).max(1);
    let yes = (joined & 0x1f).count_ones().saturating_sub(1); // without us
    if yes >= need {
        OUR_RALLY.with(|o| o.set(None)); // answered: it runs its course
        return refresh(index);
    }
    let team = crate::opening::team().len().min(5) as u32;
    let answered = ((declined | joined) & 0x1f).count_ones();
    // RALLY_AVAIL: we act last in the round after the call, so everyone has answered by then.
    if round < called + RALLY_REPLY_ROUNDS && (RALLY_AVAIL || answered < team) {
        return Ok(());
    }
    cancel_and_skip(index, round)
}

// Call our rally off and give its group up for UNANSWERED_RALLY_ROUNDS (judged lost, no new rally).
fn cancel_and_skip(index: usize, round: u32) -> Result<()> {
    OUR_RALLY.with(|o| o.set(None));
    CALLER.with(|c| c.set(None));
    super::trace::trace("rally-cancelled");
    log("cancelled", index);
    // end() reads the rally through state(), which hides a dropped one: clear it directly.
    if read()?[0] & !HELP_FLAG & !DROP_FLAG == index as u8 + 1 {
        write([0, 0])?;
    }
    super::collect::extend_loss(index, round + UNANSWERED_RALLY_ROUNDS);
    CHECKED_UNTIL.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() <= index {
            c.resize(index + 1, 0);
        }
        c[index] = c[index].max(round + UNANSWERED_RALLY_ROUNDS);
    });
    Ok(())
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
// v8: the caller checks for an answer RALLY_ANSWER_ROUNDS after calling instead of when the rally
// runs out, and ends an unanswered rally then: in 10 traced games 171 of 366 rallies had no ally
// join and the caller waited out all 20 rounds (joiners join a median 1.5 rounds after the call).
// Off: checked at the end of the rally.
// 2026-10-23, 50 games (2 seeds x 25 maps) against bot-v8 without it, read game by game against a
// control run of the same games with no change (21/50): 24/50, 7 games flipped to wins and 4 to losses,
// gold -7% and deaths 14.5 against 11.9. Off.
pub const RALLY_FAIL_FAST: bool = false;
const RALLY_ANSWER_ROUNDS: u32 = 6;

thread_local! {
    static OUR_RALLY: Cell<Option<(usize, u32)>> = const { Cell::new(None) };
}

// Called every acting turn (farm_plan): give up our rally if it went unanswered.
pub fn check_unanswered() -> Result<()> {
    watch_our_rally()?;
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
    if RALLY_REPLIES {
        return check_replies(index, called, round);
    }
    let deadline = if RALLY_FAIL_FAST {
        called + RALLY_ANSWER_ROUNDS
    } else {
        called + RALLY_ROUNDS - 1
    };
    if round < deadline {
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
        if group
            .camps()
            .iter()
            .any(|c| super::world::distance(c.spawn_position(), p) <= ANSWER_TILES)
        {
            answered = true;
            break;
        }
    }
    if !answered {
        super::trace::trace("rally-unanswered");
        if RALLY_FAIL_FAST {
            log("unanswered", index);
            end(index)?;
        }
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

thread_local! {
    // RALLY_LOG: the last round we hit a camp of the active rally's group.
    static RALLY_HIT: Cell<Option<u32>> = const { Cell::new(None) };
}

// RALLY_LOG: we just hit a camp of the rally group.
pub fn note_hit() {
    if RALLY_LOG {
        RALLY_HIT.with(|h| h.set(cambc::round_number().ok()));
    }
}

// RALLY_LOG, end of every turn (main.rs): a champion that said yes to the active rally prints what
// it did this turn: "RALLY turn r<round> g<group> <champion> near<0/1> hit<0/1>" (near: within
// RALLY_ARRIVE_TILES of the group). Debug builds only.
pub fn log_turn() {
    if !RALLY_LOG {
        return;
    }
    cambc::debug_block! {
        let Ok(Some(index)) = active() else { return };
        if !going().unwrap_or(false) {
            return;
        }
        let Ok(me) = super::me() else { return };
        if !me.is_alive().unwrap_or(false) {
            return;
        }
        let round = cambc::round_number().unwrap_or(0);
        let here = me.position().unwrap_or(cambc::Vec2::new(-99, -99));
        let near = super::world::camp_groups().is_ok_and(|g| {
            g[index].camps().iter().any(|c| super::world::distance(c.spawn_position(), here) <= RALLY_ARRIVE_TILES)
        });
        let hit = RALLY_HIT.with(|h| h.get()) == Some(round);
        println!(
            "RALLY turn r{} g{} {:?} near{} hit{}",
            round, index, me.champion_name().ok(), u8::from(near), u8::from(hit)
        );
    }
}

// v9 (2026-10-25, the user's handshake): with RALLY_LOG on, 57% of rallies ran out (RALLY_ROUNDS
// from the call) and 90% of the turns spent on rallies went to ones that ran out, were cancelled or
// were never hit (40 games: 210 champion turns a game on rallies, 113 of them walking to ones that
// failed). A helper that said yes can't always come any more (dead, fighting enemies, retreating
// at low health). What happens then:
//   1: it calls the rally off (DROP_FLAG in byte 0); the caller gives the group up
//      (UNANSWERED_RALLY_ROUNDS), as for a rally too few answered.
//   2: it takes its yes back (its bit in byte 5); the caller, short of helpers, posts the rally
//      again (REPOSTS times, the clock restarting) so others can answer, then gives it up.
//   3: as 2, and the rally has no fixed length: the caller restates it every turn (byte 1) and it
//      stays up while the caller does (gone after HEARTBEAT_ROUNDS of silence, RALLY_MAX_ROUNDS at
//      most), so helpers still walking don't find it gone. The caller alone decides to end it.
// A caller that can't go on itself ends the rally. 0: off (the rally runs out after RALLY_ROUNDS).
// 2026-10-25, 120 games each against bot-v8 with a same-seed control (59/120; rallies 8% cleared,
// 56% ran out, 91% of rally turns wasted): 1: 63/120, 8% cleared, 85% wasted; 2: 59/120, 10%,
// 82%; 3: 58/120, 22% cleared (220 against 78), 4% ran out, 46% wasted, hits on rally groups 50 a
// game against 36, deaths 10.8 against 11.5, fuel -4%; gold and big captures unchanged. 3, with
// RALLY_MAX_ROUNDS 40 (31 rallies sat out the 80-round cap).
pub const RALLY_HANDSHAKE: u8 = 3;
const DROP_FLAG: u8 = 0x40;
const REPOSTS: u32 = 1;
const HEARTBEAT_ROUNDS: u32 = 6;
const RALLY_MAX_ROUNDS: u32 = 40;
const RECONSIDER_ROUNDS: u32 = 10;

thread_local! {
    // The rally we called: (group, round first called, reposts so far), while it's up.
    static CALLER: Cell<Option<(usize, u32, u32)>> = const { Cell::new(None) };
    // The round of our last answer (RALLY_HANDSHAKE 3).
    static REPLIED: Cell<u32> = const { Cell::new(0) };
}

// A champion that said yes (or called) can't come now (RALLY_HANDSHAKE): set_unavailable(true)
// (dead or fighting) and the low-health retreat (bot.rs).
pub fn drop_out() -> Result<()> {
    if RALLY_HANDSHAKE == 0 {
        return Ok(());
    }
    let Some(index) = active()? else {
        return Ok(());
    };
    if CALLER.with(|c| c.get()).is_some_and(|(g, _, _)| g == index) {
        log("caller-out", index);
        CALLER.with(|c| c.set(None));
        OUR_RALLY.with(|o| o.set(None));
        return end(index);
    }
    if !going()? {
        return Ok(());
    }
    let Some(slot) = slot_of(super::me()?.champion_name()?) else {
        return Ok(());
    };
    if RALLY_HANDSHAKE == 1 {
        log("drop-cancel", index);
        let [byte, called] = read()?;
        cambc::shared_memory().write(OFFSET, &[byte | DROP_FLAG, called])?;
        ACTIVE.with(|a| a.set(None));
    } else {
        log("drop", index);
        let mut byte = [0u8];
        cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut byte)?;
        cambc::shared_memory().write(PARTICIPANTS_OFFSET, &[byte[0] & !(1 << slot)])?;
    }
    Ok(())
}

// The caller's turn (RALLY_HANDSHAKE): act on drop-outs, keep the rally up (3), end it at
// RALLY_MAX_ROUNDS.
fn watch_our_rally() -> Result<()> {
    if RALLY_HANDSHAKE == 0 {
        return Ok(());
    }
    let Some((index, first, reposts)) = CALLER.with(|c| c.get()) else {
        return Ok(());
    };
    let round = cambc::round_number()?;
    let [byte, _] = read()?;
    if byte & !HELP_FLAG & !DROP_FLAG != index as u8 + 1 {
        CALLER.with(|c| c.set(None)); // ended (cleared, ran out) or replaced
        return Ok(());
    }
    if byte & DROP_FLAG != 0 || round >= first + RALLY_MAX_ROUNDS {
        return cancel_and_skip(index, round);
    }
    if RALLY_HANDSHAKE == 3 {
        cambc::shared_memory().write(OFFSET + 1, &[round as u8])?;
        ACTIVE.with(|a| a.set(None));
    }
    // Still waiting for the first answers (check_replies): nothing to check yet.
    if OUR_RALLY.with(|o| o.get()).is_some() {
        return Ok(());
    }
    let mut joined = [0u8];
    cambc::shared_memory().read(PARTICIPANTS_OFFSET, &mut joined)?;
    let need = u32::from(joined[0] >> NEED_SHIFT).max(1);
    let helpers = (joined[0] & 0x1f).count_ones().saturating_sub(1);
    if helpers >= need {
        return Ok(());
    }
    if RALLY_HANDSHAKE >= 2 && reposts < REPOSTS {
        // Post it again (the joined bits stay): those who said no, or weren't free, answer anew,
        // and we act last next round to see them (check_replies).
        log("repost", index);
        cambc::shared_memory().write(OFFSET, &[index as u8 + 1, round as u8])?;
        ACTIVE.with(|a| a.set(None));
        OUR_RALLY.with(|o| o.set(Some((index, round))));
        REPLY.with(|r| r.set(Some((index, round as u8, true, false))));
        CALLER.with(|c| c.set(Some((index, first, reposts + 1))));
        return Ok(());
    }
    cancel_and_skip(index, round)
}

// End the rally at group `index` (cleared, or respawning).
pub fn end(index: usize) -> Result<()> {
    if active()? == Some(index) {
        super::trace::trace("rally-end");
        log("end", index);
        write([0, 0])?;
    }
    Ok(())
}
