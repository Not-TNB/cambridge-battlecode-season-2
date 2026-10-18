// Collecting: which camp groups we'd beat alone, holding a capture until it pays out, and support
// champions (tanks, Willow) sticking with the damage dealers instead of fighting on their own.
use crate::common::{camp_groups, distance, group_at, is_damaging, me, sight, tiles_around};
use crate::constants::{class_of, ClassKind};
use cambc::{Bitboard, Camp, Champion, Result, Vec2};

// Camp fights, simulated turn by turn from where we stand: we and the allies in sight next to the
// group hit its weakest live camp; every live camp hits back (which champion a camp picks isn't
// documented, so each one is assumed to hit us). A group counts as winnable only if we'd clear it
// with at least CAMP_FIGHT_HP_FLOOR of our health left: below the low-health recall line we'd
// leave mid-fight. Losing groups are skipped. 0 floor with CAMP_FIGHT off = fight anything.
pub const CAMP_FIGHT: bool = false;
pub const CAMP_FIGHT_HP_FLOOR: f32 = 0.35;
const CAMP_FIGHT_MAX_TURNS: u32 = 80;
// Allies within this many tiles of a group's camps help in the fight.
const CAMP_HELP_RADIUS: i32 = 4;
// Assume the camps spread their attacks over us and those allies instead of all hitting us.
pub const CAMP_FIGHT_SHARE: bool = false;
// A group judged lost stays skipped for this many rounds while out of sight: judging it only when
// in sight would flip between walking to it and away from it. Forgotten whenever we're at spawn.
const CAMP_LOSS_MEMORY_ROUNDS: u32 = 30;

thread_local! {
    static LOST_UNTIL: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Judged lost within the last CAMP_LOSS_MEMORY_ROUNDS rounds (cheap: no host calls).
pub fn recently_lost(index: usize, round: u32) -> bool {
    CAMP_FIGHT && LOST_UNTIL.with(|l| l.borrow().get(index).is_some_and(|&until| round < until))
}

pub fn forget_lost_camps() {
    LOST_UNTIL.with(|l| l.borrow_mut().clear());
}

// What every camp-fight check in a turn shares, built once per round and position: looking things
// up per group (~20 groups) cost ~60k fuel.
struct FightContext {
    round: u32,
    here: Vec2,
    live: Vec<(usize, Camp)>,  // visible live camps, by group index
    sight: std::rc::Rc<crate::common::Sight>,
    outgoing: f32,             // our damage per turn against camps
    helpers: Vec<(Vec2, f32)>, // allies in sight: position, damage per turn
    hp: f32,
    floor: f32,
    verdicts: Vec<Option<Option<bool>>>, // per group, once judged this turn
}

thread_local! {
    static CONTEXT: std::cell::RefCell<Option<FightContext>> = const { std::cell::RefCell::new(None) };
}

fn fight_context() -> Result<FightContext> {
    let me = me()?;
    let sight = sight()?;
    let mut live = Vec::new();
    for camp in &sight.camps {
        if let Some(index) = group_at(camp.spawn_position())? { live.push((index, camp.clone())); }
    }
    // A basic attack plus damaging abilities averaged over their cooldown (a reset is a fraction of
    // an extra attack).
    let strength = me.strength()?;
    let mut outgoing = strength;
    for ability in me.abilities() {
        let Some(info) = ability.info()? else { continue };
        let per_turn = 1.0 / info.as_dyn().max_cooldown().max(1) as f32;
        if ability.is_attack_reset() {
            outgoing += strength * per_turn;
        } else if is_damaging(ability) {
            outgoing += ability.damage_amount()? * per_turn;
        }
    }
    let mut helpers = Vec::new();
    for ally in &sight.allies {
        if ally.is_alive()? { helpers.push((ally.position()?, ally.strength()?)); }
    }
    Ok(FightContext {
        round: cambc::round_number()?,
        here: me.position()?,
        live,
        sight: sight.clone(),
        outgoing,
        helpers,
        hp: me.health()?,
        floor: CAMP_FIGHT_HP_FLOOR * me.max_health()?,
        verdicts: vec![None; camp_groups()?.len()],
    })
}

// Whether we'd win the camp fight for group `index`, or None if none of its camps is in sight (and
// it wasn't judged lost recently). Remembers losing verdicts.
pub fn camp_fight_winnable(index: usize) -> Result<Option<bool>> {
    if !CAMP_FIGHT { return Ok(Some(true)); }
    let key = (cambc::round_number()?, me()?.position()?);
    let fresh = CONTEXT.with(|c| c.borrow().as_ref().is_some_and(|ctx| (ctx.round, ctx.here) == key));
    if !fresh {
        let ctx = fight_context()?;
        CONTEXT.with(|c| *c.borrow_mut() = Some(ctx));
    }
    CONTEXT.with(|c| {
        let mut c = c.borrow_mut();
        let ctx = c.as_mut().expect("set above");
        if let Some(verdict) = ctx.verdicts[index] { return Ok(verdict); }
        let verdict = judge(ctx, index)?;
        ctx.verdicts[index] = Some(verdict);
        Ok(verdict)
    })
}

fn judge(ctx: &FightContext, index: usize) -> Result<Option<bool>> {
    let group = &camp_groups()?[index];
    let live: Vec<&Camp> = ctx.live.iter().filter(|(g, _)| *g == index).map(|(_, c)| c).collect();
    if live.is_empty() { return Ok(if recently_lost(index, ctx.round) { Some(false) } else { None }); }
    // Members not in sight: assume alive, like the visible camp of the same kind.
    let mut camps = Vec::new(); // (health, strength)
    for member in group.camps() {
        if let Some(c) = live.iter().find(|c| c.spawn_position() == member.spawn_position()) {
            camps.push((c.health()?, c.strength()?));
        } else if !ctx.sight.visible.is_set(member.spawn_position())? {
            camps.push(match live.iter().find(|c| c.kind() == member.kind()) {
                Some(c) => (c.max_health()?, c.strength()?),
                None => { let stats = member.base_stats()?; (stats.base_health, stats.base_strength) }
            });
        }
    }
    if camps.iter().all(|&(_, s)| s <= 0.0) { return Ok(Some(true)); } // clusters don't hit back
    camps.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut outgoing = ctx.outgoing;
    let mut fighters = 1.0;
    for &(p, damage) in &ctx.helpers {
        if group.camps().iter().any(|c| distance(c.spawn_position(), p) <= CAMP_HELP_RADIUS) { outgoing += damage; fighters += 1.0; }
    }
    let share = if CAMP_FIGHT_SHARE { 1.0 / fighters } else { 1.0 };
    let (mut hp, mut next, mut winnable) = (ctx.hp, 0, false);
    if outgoing > 0.0 {
        for _ in 0..CAMP_FIGHT_MAX_TURNS {
            camps[next].0 -= outgoing;
            if camps[next].0 <= 0.0 { next += 1; }
            if next == camps.len() { winnable = true; break; }
            hp -= share * camps[next..].iter().map(|&(_, s)| s).sum::<f32>();
            if hp < ctx.floor { break; }
        }
    }
    LOST_UNTIL.with(|l| {
        let mut l = l.borrow_mut();
        l.resize(ctx.verdicts.len(), 0);
        l[index] = if winnable { 0 } else { ctx.round + CAMP_LOSS_MEMORY_ROUNDS };
    });
    Ok(Some(winnable))
}

// Whether group `index` is safe to take: only clusters (they don't hit back), or a camp fight we'd
// win. Unknown (not in sight) counts as not safe.
pub fn safe_group(index: usize) -> Result<bool> {
    let group = &camp_groups()?[index];
    if group.camps().iter().all(|c| c.is_gem_cluster() || c.is_gold_cluster()) { return Ok(true); }
    Ok(GREEDY_CAMPS && camp_fight_winnable(index)? == Some(true))
}

// Greedy collecting: with no enemy about to reach us, safe groups (see safe_group) and uncaptured
// rewards within GREEDY_RADIUS come first: before chasing enemies, and whatever the gold/gem phase.
pub const GREEDY: bool = false;
pub const GREEDY_RADIUS: i32 = 8;
// Whether camp groups we'd win count as greedy targets, or only clusters and uncaptured rewards.
pub const GREEDY_CAMPS: bool = true;
// Whether a greedy target nearby stops us chasing enemies, or only steers farming.
pub const GREEDY_BEFORE_CHASE: bool = true;

// Hold a capture: standing on a cleared group's spawn tile, stay until the reward is ours (5
// rounds) rather than leave to chase, bank or go home. Retreating from a losing fight and the
// low-health recall still come first.
pub const CAPTURE_HOLD: bool = true;

pub fn capturing() -> Result<bool> {
    if !CAPTURE_HOLD { return Ok(false); }
    let Some(index) = group_at(me()?.position()?)? else { return Ok(false) };
    Ok(matches!(camp_groups()?[index].state(), Ok(cambc::CampGroupState::RewardAvailable)))
}

// Escort: tanks and Willow stick with the damage dealers. They only chase an enemy a damage dealer
// is close to, walk back to the nearest one when more than ESCORT_RADIUS away, and recall when
// every damage dealer in sight is recalling.
pub const ESCORT: bool = true;
pub const ESCORT_RADIUS: i32 = 4;
pub const ESCORT_RECALL: bool = false;
pub const ESCORT_CHASE: bool = true;
pub const ESCORT_FOLLOW: bool = false;

pub fn is_escort() -> Result<bool> {
    Ok(ESCORT && matches!(class_of(me()?.champion_name()?), ClassKind::Tank | ClassKind::Healer))
}

// Damage dealers in sight: allies that aren't tanks or healers.
fn damage_dealers() -> Result<Vec<Champion>> {
    let mut dealers = Vec::new();
    for ally in &sight()?.allies {
        if ally.is_alive()? && !matches!(class_of(ally.champion_name()?), ClassKind::Tank | ClassKind::Healer) {
            dealers.push(ally.clone());
        }
    }
    Ok(dealers)
}

pub fn escort_recalls() -> Result<bool> {
    if !ESCORT_RECALL || !is_escort()? { return Ok(false); }
    let dealers = damage_dealers()?;
    if dealers.is_empty() { return Ok(false); }
    for dealer in &dealers {
        if !dealer.is_recalling()? { return Ok(false); }
    }
    Ok(true)
}

pub fn escort_allows_chase(target: Vec2) -> Result<bool> {
    if !ESCORT_CHASE || !is_escort()? { return Ok(true); }
    for dealer in damage_dealers()? {
        if distance(dealer.position()?, target) <= ESCORT_RADIUS + dealer.attack_range()? { return Ok(true); }
    }
    Ok(false)
}

// Tiles next to the nearest damage dealer, if it's more than ESCORT_RADIUS away.
pub fn escort_goal() -> Result<Option<Bitboard>> {
    if !ESCORT_FOLLOW || !is_escort()? { return Ok(None); }
    let here = me()?.position()?;
    // Finish the camp we're fighting first.
    let range = me()?.attack_range()?;
    if sight()?.camps.iter().any(|c| distance(c.spawn_position(), here) <= range) { return Ok(None); }
    let mut nearest: Option<(Vec2, i32)> = None;
    for dealer in damage_dealers()? {
        if dealer.is_recalling()? { continue; }
        let p = dealer.position()?;
        if nearest.is_none_or(|(_, d)| distance(p, here) < d) { nearest = Some((p, distance(p, here))); }
    }
    match nearest {
        Some((p, d)) if d > ESCORT_RADIUS => Ok(Some(tiles_around(p, 2)?)),
        _ => Ok(None),
    }
}
