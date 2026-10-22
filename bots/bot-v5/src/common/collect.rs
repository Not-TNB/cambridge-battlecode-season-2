// Collecting: which camp groups we'd beat with nearby allies, holding a capture until it pays out,
// and support champions (tanks, Willow) only chasing enemies a damage dealer is close to.
use super::attack::is_damaging;
use super::me;
use super::world::{camp_groups, distance, group_at, sight, Sight};
use cambc::{Camp, Champion, Result, Vec2};

use crate::constants::{class_of, ClassKind};

// Camp fights, simulated turn by turn from where we stand: attacking any camp wakes the whole
// group. We focus its weakest camp; allies in support range split their expected damage across
// living camps. Camp damage is shared among us and nearby visible allies. Groups we expect to lose
// are skipped.
pub const CAMP_FIGHT: bool = true;
pub const CAMP_FIGHT_HP_FLOOR: f32 = 0.35;
// The fight is fought down to this many hits of the group's hardest-hitting camp, when that's
// below CAMP_FIGHT_HP_FLOOR (the low-health recall waits as long, safety::low_hp_recall).
pub const CAMP_FIGHT_HITS: f32 = 3.0;
// v5: each fighter also keeps what walking out of the group's range costs (every camp of the group
// hitting it all the way, from its attack range out), since that's when safety::low_hp_recall now
// leaves (ESCAPE_ANY_HP): a fight planned down to three elk hits (180) was left at the escape cost
// (480 for a melee champion next to two elk), half fought, and champions starting a double elk at
// half health died on the way out (123 of 443 deaths to camps on the retreat came straight after
// hitting the camp). Off: three hits.
pub const ESCAPE_RESERVE: bool = true;
// Assume visible allies within this distance can join a camp fight, even before they are in attack range.
pub const CAMP_ALLY_SUPPORT_RADIUS: i32 = 7;
// A group judged lost stays lost for this many rounds (forgotten on our spawn, where we heal). The
// verdict used to be redone every turn, and it changed with every step: allies walking in and out
// of support range flipped the target back and forth (~280 flips in 24 traced games).
const CAMP_LOSS_MEMORY_ROUNDS: u32 = 10;

thread_local! {
    static LOST_UNTIL: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn forget_lost_camps() {
    LOST_UNTIL.with(|l| l.borrow_mut().clear());
}

// A camp group we hit within the last CAMP_COMMIT_ROUNDS rounds counts as winnable: we're in that
// fight. Judged afresh every turn, the verdict flipped as allies walked in and out of support range,
// so we stepped out of the group's range to wait (farm::WAIT_OUTSIDE_AWAKE_CAMPS), it went back to
// sleep after a round, we stepped back in and woke it again (22 such in-and-out cycles in 8 games,
// half of them waiting outside). Leaving at low health (safety::low_hp_recall) still comes first.
// 0: judge every turn. 2026-10-21, 132 games: off lost 50 to 82 against on (the biggest of the
// three changes tested together with threats::CAMPS_HIT_CLOSEST and movement::CENTRE_TIE_BREAK).
pub const CAMP_COMMIT_ROUNDS: u32 = 3;

thread_local! {
    static ENGAGED: std::cell::RefCell<Vec<(usize, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Note that we just hit a camp of group `index` (camps::attack_camp_in_range).
pub fn note_camp_hit(index: usize) {
    if CAMP_COMMIT_ROUNDS == 0 {
        return;
    }
    let round = cambc::round_number().unwrap_or(0);
    ENGAGED.with(|e| {
        let mut e = e.borrow_mut();
        e.retain(|&(g, r)| g != index && round < r + CAMP_COMMIT_ROUNDS);
        e.push((index, round));
    });
}

// Whether we hit a camp within the last CAMP_COMMIT_ROUNDS rounds: we're in a camp fight.
pub fn in_camp_fight() -> Result<bool> {
    let round = cambc::round_number()?;
    Ok(ENGAGED.with(|e| {
        e.borrow()
            .iter()
            .any(|&(_, r)| round < r + CAMP_COMMIT_ROUNDS)
    }))
}

fn committed(index: usize) -> Result<bool> {
    if CAMP_COMMIT_ROUNDS == 0 {
        return Ok(false);
    }
    let round = cambc::round_number()?;
    Ok(ENGAGED.with(|e| {
        e.borrow()
            .iter()
            .any(|&(g, r)| g == index && round < r + CAMP_COMMIT_ROUNDS)
    }))
}

// Whether group `index` is partly cleared as far as we can see: a member we can see is dead, or a
// live one is damaged. (PARTIAL_GROUPS; off: never.)
pub const PARTIAL_GROUPS: bool = true;

fn partly_cleared(ctx: &FightContext, index: usize) -> Result<bool> {
    if !PARTIAL_GROUPS {
        return Ok(false);
    }
    for member in camp_groups()?[index].camps() {
        let spawn = member.spawn_position();
        match ctx
            .live
            .iter()
            .find(|(g, c)| *g == index && c.spawn_position() == spawn)
        {
            Some((_, c)) => {
                if c.health()? < c.max_health()? {
                    return Ok(true);
                }
            }
            None => {
                if ctx.sight.visible.is_set(spawn)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

// The camp fight for group `index` as if we were there: we stand at its nearest camp, allies within
// TRAVEL_TILES of us arrive with us, and the rest join from where they are (ALLY_JOIN_TURNS).
// Camps we can't see count at base stats. For groups we can't see (farm::PRESUME_FAR_TARGETS):
// judged from where we stand, a group a walk away looked lost even with the whole team beside us.
const TRAVEL_TILES: i32 = 4;

pub fn presumed_winnable(index: usize) -> Result<bool> {
    if !CAMP_FIGHT || committed(index)? {
        return Ok(true);
    }
    // Only the context: judging from here would remember a loss for a group a walk away.
    ensure_context()?;
    CONTEXT.with(|c| {
        let mut c = c.borrow_mut();
        let ctx = c.as_mut().expect("built above");
        let camps = group_camps(ctx, index)?;
        let Some(at) = camps
            .iter()
            .map(|c| c.0)
            .min_by_key(|&p| distance(p, ctx.here))
        else {
            return Ok(true);
        };
        let helpers: Vec<Helper> = ctx
            .helpers
            .iter()
            .map(|&(p, range, damage, hp, floor)| {
                let p = if distance(p, ctx.here) <= TRAVEL_TILES {
                    at
                } else {
                    p
                };
                (p, range, damage, hp, floor)
            })
            .collect();
        // Judged from `at` in place (copying the context cost more than the judgment).
        let here = std::mem::replace(&mut ctx.here, at);
        let winnable = fight_winnable(ctx, camps, &helpers);
        ctx.here = here;
        winnable
    })
}

// Struggling: we judged at least STRUGGLE_LOSSES different groups lost within the last
// STRUGGLE_ROUNDS rounds (rally::STRUGGLE_JOIN_EXTRA_STEPS).
const STRUGGLE_ROUNDS: u32 = 40;
const STRUGGLE_LOSSES: usize = 2;

thread_local! {
    static RECENT_LOSSES: std::cell::RefCell<Vec<(usize, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn struggling(round: u32) -> bool {
    super::rally::STRUGGLE_JOIN_EXTRA_STEPS >= 0
        && RECENT_LOSSES.with(|r| {
            r.borrow()
                .iter()
                .filter(|&&(_, at)| round < at + STRUGGLE_ROUNDS)
                .count()
        }) >= STRUGGLE_LOSSES
}

// Gold of the camp groups in sight within `tiles` of us that we'd clear (live camps, reachable, not
// judged lost): what a shopping trip would leave behind (bot.rs HOME_NEAR_GOLD).
pub fn nearby_camp_gold(tiles: i32) -> Result<u32> {
    let sight = sight()?;
    let here = me()?.position()?;
    let mut counted: Vec<usize> = Vec::new();
    let mut gold = 0;
    for camp in &sight.camps {
        if distance(camp.spawn_position(), here) > tiles {
            continue;
        }
        let Some(index) = group_at(camp.spawn_position())? else {
            continue;
        };
        if counted.contains(&index) {
            continue;
        }
        counted.push(index);
        // Losses judged already (recently_lost, no host calls), not camp_fight_winnable: judging
        // every group in reach here cost up to 52k fuel on 00lanes, leaving none to walk, so a
        // champion that wanted home but saw gold near stood still for 118 turns.
        if !crate::regions::reachable_for_me(index)? || recently_lost(index, cambc::round_number()?)
        {
            continue;
        }
        gold += camp_groups()?[index].gold_value();
    }
    Ok(gold)
}

// Remember group `index` as lost until at least round `until`.
pub fn extend_loss(index: usize, until: u32) {
    LOST_UNTIL.with(|l| {
        let mut l = l.borrow_mut();
        if l.len() <= index {
            l.resize(index + 1, 0);
        }
        l[index] = l[index].max(until);
    });
}

// The round group `index`'s loss memory runs out (0: none).
pub fn lost_until(index: usize) -> u32 {
    LOST_UNTIL.with(|l| l.borrow().get(index).copied().unwrap_or(0))
}

// Judged lost within the last CAMP_LOSS_MEMORY_ROUNDS rounds (no host calls, so it's also checked
// for groups out of sight: stepping out of sight of a lost group made it a goal again).
pub fn recently_lost(index: usize, round: u32) -> bool {
    CAMP_FIGHT && LOST_UNTIL.with(|l| l.borrow().get(index).is_some_and(|&until| round < until))
}

// An ally that can join a camp fight: position, attack range, damage, hp, retreat floor.
type Helper = (Vec2, i32, f32, f32, f32);

// What every camp-fight check in a turn shares, built once per round and position: looking things
// up per group (~20 groups) cost ~60k fuel.
#[derive(Clone)]
struct FightContext {
    round: u32,
    here: Vec2,
    live: Vec<(usize, Camp)>, // visible live camps, by group index
    sight: std::rc::Rc<Sight>,
    outgoing: f32, // our damage per turn against camps
    range: i32,    // our attack range
    helpers: Vec<Helper>,
    in_sight: Vec<cambc::ChampionType>, // the allies in `helpers`
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
    let round = cambc::round_number()?;
    let mut live = Vec::new();
    for camp in &sight.camps {
        if let Some(index) = group_at(camp.spawn_position())? {
            live.push((index, camp.clone()));
        }
    }
    let outgoing = cached_camp_damage(me, round)?;
    let mut helpers = Vec::new();
    let mut in_sight = Vec::new();
    for ally in &sight.allies {
        if ally.is_alive()? {
            let (range, damage, max_health) = (
                ally.attack_range()?,
                cached_camp_damage(ally, round)?,
                ally.max_health()?,
            );
            // Camp damage through its Jungle and Durability emblems (mitigation.rs): its spare
            // health counts for that much more.
            let toughness = 1.0 / super::mitigation::from_camps(ally);
            helpers.push((
                ally.position()?,
                range,
                damage,
                ally.health()? * toughness,
                CAMP_FIGHT_HP_FLOOR * max_health * toughness,
            ));
            let name = ally.champion_name()?;
            in_sight.push(name);
            remember_teammate(name, (range, damage, max_health));
        }
    }
    Ok(FightContext {
        round,
        here: me.position()?,
        live,
        sight: sight.clone(),
        outgoing,
        range: me.attack_range()?,
        helpers,
        in_sight,
        hp: me.health()? / super::mitigation::from_camps(me),
        floor: CAMP_FIGHT_HP_FLOOR * me.max_health()? / super::mitigation::from_camps(me),
        verdicts: vec![None; camp_groups()?.len()],
    })
}

// expected_camp_damage reads every ability through the host (~1k fuel a champion, for us and each
// ally in sight, on every turn with a camp in sight). It changes only when items do, so it's kept
// per champion for this many rounds.
const DAMAGE_ESTIMATE_ROUNDS: u32 = 10;

thread_local! {
    static DAMAGE_ESTIMATES: std::cell::RefCell<Vec<(cambc::ChampionType, u32, f32)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn cached_camp_damage(champion: &Champion, round: u32) -> Result<f32> {
    let name = champion.champion_name()?;
    let cached = DAMAGE_ESTIMATES.with(|d| {
        d.borrow()
            .iter()
            .find(|&&(n, at, _)| n == name && round < at + DAMAGE_ESTIMATE_ROUNDS)
            .map(|&(_, _, damage)| damage)
    });
    if let Some(damage) = cached {
        return Ok(damage);
    }
    let damage = expected_camp_damage(champion)?;
    DAMAGE_ESTIMATES.with(|d| {
        let mut d = d.borrow_mut();
        d.retain(|&(n, _, _)| n != name);
        d.push((name, round, damage));
    });
    Ok(damage)
}

fn expected_camp_damage(champion: &Champion) -> Result<f32> {
    let strength = champion.strength()?;
    let mut damage = strength;
    for ability in champion.abilities() {
        let Some(info) = ability.info()? else {
            continue;
        };
        let per_turn = 1.0 / info.as_dyn().max_cooldown().max(1) as f32;
        if ability.is_attack_reset() {
            damage += strength * per_turn;
        } else if is_damaging(ability) {
            damage += ability.damage_amount()? * per_turn;
        }
    }
    Ok(damage)
}

// This turn's fight context, built once per round and position.
fn ensure_context() -> Result<()> {
    let key = (cambc::round_number()?, me()?.position()?);
    let fresh = CONTEXT.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|ctx| (ctx.round, ctx.here) == key)
    });
    if !fresh {
        let ctx = fight_context()?;
        CONTEXT.with(|c| *c.borrow_mut() = Some(ctx));
        super::trace::checkpoint("camp-fight-context");
    }
    Ok(())
}

// Whether we'd win the camp fight for group `index`. Unseen camps are estimated at base stats so
// farm planning can compare groups beyond current vision.
pub fn camp_fight_winnable(index: usize) -> Result<Option<bool>> {
    if !CAMP_FIGHT {
        return Ok(Some(true));
    }
    if committed(index)? {
        return Ok(Some(true));
    }
    ensure_context()?;
    CONTEXT.with(|c| {
        let mut c = c.borrow_mut();
        let ctx = c.as_mut().expect("set above");
        if let Some(verdict) = ctx.verdicts[index] {
            return Ok(verdict);
        }
        // A rally group (rally.rs) or the team target (opening.rs) is judged afresh every turn:
        // allies arriving should flip it.
        let rally =
            super::rally::active()? == Some(index) || crate::opening::team_target()? == Some(index);
        // A partly cleared group (a member dead or damaged) is judged afresh: a loss judged at full
        // strength kept it dropped for CAMP_LOSS_MEMORY_ROUNDS while allies cleared half of it.
        let lost = !rally && recently_lost(index, ctx.round) && !partly_cleared(ctx, index)?;
        let verdict = if lost {
            Some(false)
        } else {
            judge(ctx, index)?
        };
        if verdict == Some(false) && !lost && !rally {
            RECENT_LOSSES.with(|r| {
                let mut r = r.borrow_mut();
                r.retain(|&(g, at)| g != index && ctx.round < at + STRUGGLE_ROUNDS);
                r.push((index, ctx.round));
            });
            LOST_UNTIL.with(|l| {
                let mut l = l.borrow_mut();
                l.resize(ctx.verdicts.len(), 0);
                l[index] = ctx.round + CAMP_LOSS_MEMORY_ROUNDS;
            });
        }
        ctx.verdicts[index] = Some(verdict);
        Ok(verdict)
    })
}

// A kind of camp's (max health, strength, attack range): camps respawn with their base values, so
// they're read once per kind (from a live camp when we've seen one, else the base stats) instead
// of three host calls per camp every turn.
thread_local! {
    static CAMP_STATS: std::cell::RefCell<Vec<(cambc::CampType, bool, (f32, f32, i32))>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn camp_stats(
    kind: cambc::CampType,
    live: Option<&Camp>,
    member: &Camp,
) -> Result<(f32, f32, i32)> {
    let known = CAMP_STATS.with(|c| {
        c.borrow()
            .iter()
            .find(|&&(k, from_live, _)| k == kind && (from_live || live.is_none()))
            .map(|&(_, _, stats)| stats)
    });
    if let Some(stats) = known {
        return Ok(stats);
    }
    let (stats, from_live) = match live {
        Some(c) => ((c.max_health()?, c.strength()?, c.attack_range()?), true),
        None => {
            let base = member.base_stats()?;
            (
                (base.base_health, base.base_strength, base.base_attack_range),
                false,
            )
        }
    };
    CAMP_STATS.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|&(k, _, _)| k != kind);
        c.push((kind, from_live, stats));
    });
    Ok(stats)
}

// The inputs a group's verdict depends on, to reuse it while they don't change: each camp's
// health and whether it can hit us, our health, damage and floor, and the allies in sight.
type VerdictKey = (Vec<(i32, bool)>, [i32; 3], Vec<(Vec2, i32, i32, i32)>);

thread_local! {
    static VERDICTS: std::cell::RefCell<Vec<Option<(VerdictKey, bool)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn judge(ctx: &FightContext, index: usize) -> Result<Option<bool>> {
    // The rally's camp fight only counts allies that joined it (rally::RALLY_PARTICIPANTS_ONLY).
    if super::rally::RALLY_PARTICIPANTS_ONLY && super::rally::active()? == Some(index) {
        let mut helpers = Vec::with_capacity(ctx.helpers.len());
        for (i, h) in ctx.helpers.iter().enumerate() {
            if super::rally::joined(ctx.in_sight[i])? {
                helpers.push(*h);
            }
        }
        if helpers.len() != ctx.helpers.len() {
            let mut own = ctx.clone();
            own.helpers = helpers;
            return judge_with(&own, index);
        }
    }
    judge_with(ctx, index)
}

fn judge_with(ctx: &FightContext, index: usize) -> Result<Option<bool>> {
    let camps = group_camps(ctx, index)?;
    if camps.iter().all(|&(_, _, s, _)| s <= 0.0) {
        return Ok(Some(true));
    } // clusters don't hit back
    let key: VerdictKey = (
        camps
            .iter()
            .map(|&(position, health, _, range)| {
                let hits_us = range >= ctx.range || distance(position, ctx.here) <= range;
                (health as i32, hits_us)
            })
            .collect(),
        [ctx.hp as i32, ctx.floor as i32, ctx.outgoing as i32],
        ctx.helpers
            .iter()
            .map(|&(p, range, damage, hp, _)| (p, range, damage as i32, hp as i32))
            .collect(),
    );
    let reused = VERDICTS.with(|v| {
        v.borrow()
            .get(index)
            .and_then(|slot| slot.as_ref())
            .filter(|(k, _)| *k == key)
            .map(|&(_, winnable)| winnable)
    });
    if let Some(winnable) = reused {
        return Ok(Some(winnable));
    }
    // Fuel fix 3: the exact key above changes whenever an ally moves a tile, so the fight was
    // simulated again nearly every turn. A coarser key (health in 100s, damage in 5s, allies by
    // when they'd join rather than where they stand) keeps a verdict CAMP_VERDICT_KEEP_ROUNDS rounds.
    let coarse = (CAMP_VERDICT_KEEP_ROUNDS > 0).then(|| coarse_key(ctx, &camps));
    let kept = coarse.as_ref().and_then(|coarse| {
        COARSE_VERDICTS.with(|v| {
            v.borrow()
                .get(index)
                .and_then(|slot| slot.as_ref())
                .filter(|(stored, k, _)| {
                    ctx.round < stored + CAMP_VERDICT_KEEP_ROUNDS && k == coarse
                })
                .map(|&(_, _, winnable)| winnable)
        })
    });
    if let Some(winnable) = kept {
        super::trace::trace("verdict-kept");
        if super::trace::VERIFY_FUEL_CACHES
            && fight_winnable(ctx, camps.clone(), &ctx.helpers)? != winnable
        {
            super::trace::trace("verdict-cache-mismatch");
        }
        return Ok(Some(winnable));
    }
    let winnable = fight_winnable(ctx, camps, &ctx.helpers)?;
    VERDICTS.with(|v| {
        let mut v = v.borrow_mut();
        v.resize(camp_groups().map_or(0, |g| g.len()), None);
        if let Some(slot) = v.get_mut(index) {
            *slot = Some((key, winnable));
        }
    });
    if let Some(coarse) = coarse {
        COARSE_VERDICTS.with(|v| {
            let mut v = v.borrow_mut();
            v.resize(camp_groups().map_or(0, |g| g.len()), None);
            if let Some(slot) = v.get_mut(index) {
                *slot = Some((ctx.round, coarse, winnable));
            }
        });
    }
    Ok(Some(winnable))
}

// Fuel fix 3 (see judge). 0: only exact repeats are reused. Off: alone on 6 camp-heavy maps it
// didn't reduce fuel-exhausted turns (+2%); kept verdicts were right 99.4% of the time. 3 turns it on.
pub const CAMP_VERDICT_KEEP_ROUNDS: u32 = 0;

type CoarseKey = (Vec<(i32, bool)>, [i32; 3], Vec<(i32, i32, i32, i32)>);

thread_local! {
    static COARSE_VERDICTS: std::cell::RefCell<Vec<Option<(u32, CoarseKey, bool)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn coarse_key(ctx: &FightContext, camps: &[(Vec2, f32, f32, i32)]) -> CoarseKey {
    let mut helpers: Vec<(i32, i32, i32, i32)> = ctx
        .helpers
        .iter()
        .map(|&(p, range, damage, hp, _)| {
            let gap = camps
                .iter()
                .filter(|c| c.1 > 0.0)
                .map(|c| distance(c.0, p))
                .min()
                .unwrap_or(0)
                - range.max(CAMP_ALLY_SUPPORT_RADIUS);
            (
                range,
                damage as i32 / 5,
                hp as i32 / 100,
                gap.clamp(0, ALLY_JOIN_TURNS + 1),
            )
        })
        .collect();
    helpers.sort_unstable();
    (
        camps
            .iter()
            .map(|&(position, health, _, range)| {
                (
                    health as i32 / 100,
                    range >= ctx.range || distance(position, ctx.here) <= range,
                )
            })
            .collect(),
        [
            ctx.hp as i32 / 50,
            ctx.floor as i32 / 50,
            ctx.outgoing as i32 / 5,
        ],
        helpers,
    )
}

// Group `index`'s camps as the fight sees them: (position, health, strength, attack range). Members
// not in sight count as alive at full health, like the visible camp of the same kind.
fn group_camps(ctx: &FightContext, index: usize) -> Result<Vec<(Vec2, f32, f32, i32)>> {
    let group = &camp_groups()?[index];
    let live: Vec<&Camp> = ctx
        .live
        .iter()
        .filter(|(g, _)| *g == index)
        .map(|(_, c)| c)
        .collect();
    // Members not in sight: assume alive at base health, like the visible camp of the same kind.
    let mut camps = Vec::new(); // (position, health, strength, attack range)
    for member in group.camps() {
        if let Some(c) = live
            .iter()
            .find(|c| c.spawn_position() == member.spawn_position())
        {
            let (_, strength, range) = camp_stats(member.kind(), Some(c), member)?;
            camps.push((member.spawn_position(), c.health()?, strength, range));
        } else if !ctx.sight.visible.is_set(member.spawn_position())? {
            let same_kind = live.iter().find(|c| c.kind() == member.kind()).copied();
            let (max_health, strength, range) = camp_stats(member.kind(), same_kind, member)?;
            camps.push((member.spawn_position(), max_health, strength, range));
        }
    }
    Ok(camps)
}

// The camp fight itself (see judge): whether we and the allies in support range clear `camps`
// before falling to our retreat floors. Allies further away join once they could have walked into
// support range (a step a round), if that's within ALLY_JOIN_TURNS: counting only the allies
// already in range made the first champion to reach an elk judge it lost (and remember that for
// CAMP_LOSS_MEMORY_ROUNDS) while the rest of the team was 8-9 tiles behind; on spine all five
// walked up to a lone elk they'd have killed in ~9 rounds and every one gave up on it.
pub const ALLY_JOIN_TURNS: i32 = 6;

// The camp-fight check in closed form instead of simulated. Focusing the weakest camp first with a
// team damage rate D, camp i (in that order) is alive until C_i / D, C_i being the health of camps
// 1..=i, so the team takes K / D damage in all, with K = sum of strength_i * C_i. The fight is won
// if that fits in the health the fighters can spend before their retreat floors: D * spare >= K.
// Allies still walking over count for the share of the fight they'd be there for. It's the even
// damage split the simulation assumed, without simulating it (the simulation was the biggest cost
// in the turns that ran out of fuel on 01boss-battle). Off: simulate.
pub const CAMP_FIGHT_CLOSED_FORM: bool = true;

fn fight_winnable(
    ctx: &FightContext,
    camps: Vec<(Vec2, f32, f32, i32)>,
    helpers: &[Helper],
) -> Result<bool> {
    if !CAMP_FIGHT_CLOSED_FORM {
        return simulate(ctx, camps, helpers);
    }
    let closed = closed_form(ctx, &camps, helpers);
    if super::trace::VERIFY_FUEL_CACHES {
        let simulated = simulate(ctx, camps, helpers)?;
        super::trace::trace(match (closed, simulated) {
            (true, false) => "closed-form-mismatch-optimistic",
            (false, true) => "closed-form-mismatch-cautious",
            _ => "closed-form-agrees",
        });
    }
    Ok(closed)
}

fn closed_form(ctx: &FightContext, camps: &[(Vec2, f32, f32, i32)], helpers: &[Helper]) -> bool {
    let mut live: Vec<(f32, f32, Vec2, i32)> = camps
        .iter()
        .filter(|c| c.1 > 0.0)
        .map(|&(p, health, strength, range)| (health, strength, p, range))
        .collect();
    if live.is_empty() {
        return true;
    }
    live.sort_by(|a, b| a.0.total_cmp(&b.0));
    let hardest_hit = live.iter().map(|c| c.1).fold(0.0f32, f32::max);
    // ESCAPE_RESERVE: what walking out of the group's range costs, from where a fighter with attack
    // range `range` stands (every camp of the group hitting it all the way).
    let total_strength: f32 = live.iter().map(|c| c.1).sum();
    let reach = live.iter().map(|c| c.3).max().unwrap_or(0);
    let floor_of = |floor: f32, range: i32| {
        let floor = if CAMP_FIGHT_HITS > 0.0 && hardest_hit > 0.0 {
            floor.min(CAMP_FIGHT_HITS * hardest_hit)
        } else {
            floor
        };
        if ESCAPE_RESERVE {
            floor.max((reach + 1 - range).max(1) as f32 * total_strength)
        } else {
            floor
        }
    };
    // When each helper would be in the fight (as in simulate).
    let joins: Vec<Option<f32>> = helpers
        .iter()
        .map(|&(position, range, ..)| {
            let gap = live
                .iter()
                .map(|c| distance(c.2, position))
                .min()
                .unwrap_or(0)
                - range.max(CAMP_ALLY_SUPPORT_RADIUS);
            if gap <= 0 {
                Some(0.0)
            } else if gap <= ALLY_JOIN_TURNS {
                Some(gap as f32)
            } else {
                None
            }
        })
        .collect();
    let total: f32 = live.iter().map(|c| c.0).sum();
    let full: f32 = ctx.outgoing
        + helpers
            .iter()
            .zip(&joins)
            .filter(|(h, j)| j.is_some() && h.3 > h.4)
            .map(|(h, _)| h.2)
            .sum::<f32>();
    if full <= 0.0 {
        return false;
    }
    // A helper arriving after `join` of the fight's T rounds adds (1 - join / T) of its damage and
    // of its spare health.
    let rounds = total / full;
    let share = |join: f32| (1.0 - join / rounds.max(1.0)).clamp(0.0, 1.0);
    let mut damage = ctx.outgoing;
    let mut spare = (ctx.hp - floor_of(ctx.floor, ctx.range)).max(0.0);
    let mut any_helper = false;
    for (h, join) in helpers.iter().zip(&joins) {
        if let Some(join) = join {
            if h.3 > h.4 {
                let s = share(*join);
                damage += h.2 * s;
                spare += (h.3 - floor_of(h.4, h.1)).max(0.0) * s;
                any_helper |= s > 0.0;
            }
        }
    }
    if damage <= 0.0 {
        return false;
    }
    // K: only camps that hit someone count (a camp that can't reach us hits a helper, if any).
    let mut k = 0.0;
    let mut cumulative = 0.0;
    for &(health, strength, position, range) in &live {
        cumulative += health;
        let hits_us = range >= ctx.range || distance(position, ctx.here) <= range;
        if hits_us || any_helper {
            k += strength * cumulative;
        }
    }
    k <= damage * spare
}

fn simulate(
    ctx: &FightContext,
    mut camps: Vec<(Vec2, f32, f32, i32)>,
    helpers: &[Helper],
) -> Result<bool> {
    camps.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut damage_rates = vec![0.0; camps.len()];
    let mut combatants = Vec::with_capacity(helpers.len() + 1);
    combatants.push((ctx.hp, ctx.floor));
    combatants.extend(helpers.iter().map(|helper| (helper.3, helper.4)));
    let hardest_hit = camps.iter().map(|camp| camp.2).fold(0.0f32, f32::max);
    if CAMP_FIGHT_HITS > 0.0 && hardest_hit > 0.0 {
        for combatant in &mut combatants {
            combatant.1 = combatant.1.min(CAMP_FIGHT_HITS * hardest_hit);
        }
    }
    let mut active: Vec<bool> = combatants.iter().map(|&(hp, floor)| hp > floor).collect();
    // When each combatant is in the fight (we are from the start).
    let mut join_at = vec![0.0f32; combatants.len()];
    for (i, &(position, range, ..)) in helpers.iter().enumerate() {
        let gap = camps
            .iter()
            .filter(|camp| camp.1 > 0.0)
            .map(|camp| distance(camp.0, position))
            .min()
            .unwrap_or(0)
            - range.max(CAMP_ALLY_SUPPORT_RADIUS);
        join_at[i + 1] = if gap <= 0 {
            0.0
        } else if gap <= ALLY_JOIN_TURNS {
            gap as f32
        } else {
            f32::INFINITY
        };
    }
    let mut now = 0.0f32;
    let mut winnable = camps.iter().all(|&(_, health, _, _)| health <= 0.0);

    // Each iteration advances to the next camp death, champion retreat threshold or ally arrival,
    // rather than simulating every round. Allies that retreat stop contributing damage.
    while !winnable {
        let joined: Vec<bool> = (0..combatants.len())
            .map(|i| active[i] && join_at[i] <= now + 0.001)
            .collect();
        damage_rates.fill(0.0);
        if joined[0] && ctx.outgoing > 0.0 {
            if let Some((index, _)) = camps
                .iter()
                .enumerate()
                .filter(|(_, camp)| camp.1 > 0.0)
                .min_by(|(_, a), (_, b)| a.1.total_cmp(&b.1))
            {
                damage_rates[index] += ctx.outgoing;
            }
        }

        // A helper that walked in counts as in support range of every camp of the group.
        let in_support = |helper_index: usize, camp: Vec2| {
            let (position, range, ..) = helpers[helper_index];
            join_at[helper_index + 1] > 0.0
                || distance(camp, position) <= range.max(CAMP_ALLY_SUPPORT_RADIUS)
        };
        for (helper_index, &(_, _, damage, _, _)) in helpers.iter().enumerate() {
            if !joined[helper_index + 1] {
                continue;
            }
            let targets = camps
                .iter()
                .filter(|camp| camp.1 > 0.0 && in_support(helper_index, camp.0))
                .count();
            if targets == 0 {
                continue;
            }
            let per_target = damage / targets as f32;
            for (i, camp) in camps.iter().enumerate() {
                if camp.1 > 0.0 && in_support(helper_index, camp.0) {
                    damage_rates[i] += per_target;
                }
            }
        }

        let next_death = camps
            .iter()
            .enumerate()
            .filter(|(i, camp)| camp.1 > 0.0 && damage_rates[*i] > 0.0)
            .map(|(i, camp)| (i, camp.1 / damage_rates[i]))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let mut incoming = vec![0.0; combatants.len()];
        for &(position, health, strength, range) in &camps {
            // To fight it we stand within our attack range of the group, so a camp whose range is
            // at least ours hits us wherever we stand now. Counting only camps whose range covers
            // our current tile judged a group winnable from just outside it and lost one step in.
            if health <= 0.0 || (range < ctx.range && distance(position, ctx.here) > range) {
                continue;
            }
            let targets: Vec<usize> =
                std::iter::once(0)
                    .chain((0..helpers.len()).filter_map(|i| {
                        (joined[i + 1] && in_support(i, position)).then_some(i + 1)
                    }))
                    .filter(|&i| joined[i])
                    .collect();
            if !targets.is_empty() {
                let damage = strength / targets.len() as f32;
                for target in targets {
                    incoming[target] += damage;
                }
            }
        }
        let next_retreat = combatants
            .iter()
            .enumerate()
            .filter(|(i, _)| joined[*i] && incoming[*i] > 0.0)
            .map(|(i, &(hp, floor))| (i, (hp - floor) / incoming[i]))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let next_join = (0..combatants.len())
            .filter(|&i| active[i] && join_at[i] > now + 0.001 && join_at[i].is_finite())
            .map(|i| join_at[i] - now)
            .min_by(|a, b| a.total_cmp(b));
        let time_to_retreat = next_retreat.map_or(f32::INFINITY, |(_, time)| time);
        let time_to_death = next_death.map_or(f32::INFINITY, |(_, time)| time);
        let time_to_join = next_join.unwrap_or(f32::INFINITY);
        let step = time_to_retreat.min(time_to_death).min(time_to_join);
        if !step.is_finite() {
            break; // nobody can hurt the camps and nobody else is coming
        }
        for (i, camp) in camps.iter_mut().enumerate() {
            if camp.1 > 0.0 {
                camp.1 -= damage_rates[i] * step;
            }
        }
        for (i, combatant) in combatants.iter_mut().enumerate() {
            combatant.0 -= incoming[i] * step;
            if active[i] && joined[i] && combatant.0 <= combatant.1 + 0.001 {
                active[i] = false;
            }
        }
        if let Some((index, time)) = next_death {
            if time <= step {
                camps[index].1 = 0.0;
            }
        }
        now += step;
        winnable = camps.iter().all(|&(_, health, _, _)| health <= 0.0);
    }
    Ok(winnable)
}

// Teammates we've seen: (attack range, damage per turn, max health), as of the last sighting.
// Rallies count on them, not on copies of ourselves: a tank calling a monolith rally assumed four
// more tanks, and the real team (two squishy carries among them) never had the fight.
thread_local! {
    static TEAMMATES: std::cell::RefCell<Vec<(cambc::ChampionType, (i32, f32, f32))>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

// How many champions our team has (Cassi & Issac are one pick, two champions).
fn team_size() -> usize {
    thread_local! {
        static SIZE: std::cell::OnceCell<usize> = const { std::cell::OnceCell::new() };
    }
    SIZE.with(|s| {
        *s.get_or_init(|| match cambc::draft::current_draft(cambc::Team::Ally) {
            Ok(picks) if !picks.is_empty() => {
                picks.len()
                    + picks
                        .iter()
                        .filter(|&&p| p == cambc::ChampionDraft::CassiAndIssac)
                        .count()
            }
            _ => cambc::draft::team_size().unwrap_or(5),
        })
    })
}

fn remember_teammate(name: cambc::ChampionType, stats: (i32, f32, f32)) {
    TEAMMATES.with(|t| {
        let mut t = t.borrow_mut();
        match t.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = stats,
            None => t.push((name, stats)),
        }
    });
}

// Whether we'd win the fight for group `index` (which must have a camp in sight) with `extra` more
// allies at full health next to it: whether calling a rally (rally.rs) is worth it. The extra
// allies are the strongest teammates we've seen who aren't here, then copies of us for teammates
// never seen; never more than the teammates not already in sight (counting the ones here twice
// once had a five-champion team planning with eight).
pub fn winnable_with_standins(index: usize, extra: usize) -> Result<bool> {
    camp_fight_winnable(index)?; // builds this turn's context
    CONTEXT.with(|c| {
        let c = c.borrow();
        let ctx = c.as_ref().expect("built above");
        let camps = group_camps(ctx, index)?;
        let Some(&(at, ..)) = camps.first() else {
            return Ok(false);
        };
        let full = ctx.floor / CAMP_FIGHT_HP_FLOOR;
        let mut helpers = ctx.helpers.clone();
        let mut absent: Vec<(i32, f32, f32)> = TEAMMATES.with(|t| {
            t.borrow()
                .iter()
                .filter(|(name, _)| !ctx.in_sight.contains(name))
                .map(|&(_, stats)| stats)
                .collect()
        });
        absent.sort_by(|a, b| b.1.total_cmp(&a.1));
        let known = TEAMMATES.with(|t| t.borrow().len());
        let never_seen = (team_size() - 1).saturating_sub(known);
        if extra > absent.len() + never_seen {
            return Ok(false);
        }
        for k in 0..extra {
            let (range, damage, max_health) =
                absent
                    .get(k)
                    .copied()
                    .unwrap_or((ctx.range, ctx.outgoing, full));
            helpers.push((
                at,
                range,
                damage,
                max_health,
                CAMP_FIGHT_HP_FLOOR * max_health,
            ));
        }
        fight_winnable(ctx, camps, &helpers)
    })
}

// Whether we alone, at full health and standing next to it, would clear all of group `index` at
// full health: whether we're strong enough to farm without the team (opening.rs).
pub fn could_solo(index: usize) -> Result<bool> {
    let mut ctx = fight_context()?;
    let mut camps = Vec::new();
    for member in camp_groups()?[index].camps() {
        let (max_health, strength, range) = camp_stats(member.kind(), None, member)?;
        camps.push((member.spawn_position(), max_health, strength, range));
    }
    let Some(&(at, ..)) = camps.first() else {
        return Ok(true);
    };
    ctx.hp = ctx.floor / CAMP_FIGHT_HP_FLOOR;
    ctx.here = at;
    fight_winnable(&ctx, camps, &[])
}

// Hold a capture: standing on a cleared group's spawn tile, stay until the reward is ours (5
// rounds) rather than leave to chase, bank or go home. Retreating from a losing fight and the
// low-health recall still come first.
pub const CAPTURE_HOLD: bool = true;

// Before going home to shop, capture a reward lying close by if it's safe (not before the
// low-health recall: standing 5 rounds on a capture at low health raised gems lost on death by 20%
// and cost 4 of 126 games): a group we see cleared with its reward waiting, a camp tile within REWARD_BEFORE_RECALL_TILES,
// no ally already on it, no enemy champion in sight and no awake camp able to hit us. Walking home
// past it threw the reward away (it waits for a capture, and nobody respawns it). Returns whether
// that used the turn (a step towards it, or standing on it). 0 turns it off.
// Off: before going home only, it was neutral on wins (68 against 67 of 126) and still raised gems
// lost on death by 11%.
pub const REWARD_BEFORE_RECALL_TILES: i32 = 0;

pub fn grab_reward_before_recall() -> Result<bool> {
    if REWARD_BEFORE_RECALL_TILES <= 0 {
        return Ok(false);
    }
    let me = me()?;
    let here = me.position()?;
    let sight = sight()?;
    if !sight.enemies.is_empty() {
        return Ok(false);
    }
    for camp in &sight.camps {
        if camp.is_awake().unwrap_or(true)
            && distance(camp.spawn_position(), here) <= camp.attack_range()?
        {
            return Ok(false);
        }
    }
    let mut goals = cambc::Bitboard::new()?;
    let mut any = false;
    for (index, group) in camp_groups()?.iter().enumerate() {
        let near = group
            .camps()
            .iter()
            .any(|c| distance(c.spawn_position(), here) <= REWARD_BEFORE_RECALL_TILES);
        let seen = group
            .camps()
            .iter()
            .any(|c| sight.visible.is_set(c.spawn_position()).unwrap_or(false));
        if !near
            || !seen
            || sight
                .camps
                .iter()
                .any(|c| group_at(c.spawn_position()).ok().flatten() == Some(index))
        {
            continue;
        }
        if !matches!(group.state(), Ok(cambc::CampGroupState::RewardAvailable))
            || super::camps::ally_on_group(group, false)?
        {
            continue;
        }
        if group.camps().iter().any(|c| c.spawn_position() == here) {
            super::trace::trace("grab-reward-hold");
            return Ok(true);
        }
        for camp in group.camps() {
            goals.set(camp.spawn_position(), true)?;
        }
        any = true;
    }
    if !any {
        return Ok(false);
    }
    super::trace::trace("grab-reward");
    super::movement::move_towards_closest(goals)
}

// Helping instead of going home: when we'd go home to shop and heal (not the low-health recall),
// but an ally within HELP_ALLY_RADIUS is fighting a camp group (a camp awake and damaged, the ally
// in range of it) that the camp-fight check says we'd win together, and it's safe (at least
// HELP_MIN_HEALTH of our health, no enemy champion within HELP_ENEMY_CLEAR tiles), help clear it
// first. Also cancels a go-home recall in progress (bot.rs). Returns that group's live camps (the
// goals), nearest group first. 0 radius: off.
pub const HELP_ALLY_RADIUS: i32 = 8;
pub const HELP_MIN_HEALTH: f32 = 0.6;
pub const HELP_ENEMY_CLEAR: i32 = 6;
// Only while carrying fewer gems than this: a champion that wants to go home has usually been out a
// while with a full bag, and keeping it out (3 seeds, no gem limit) lost 32% more gems on death for
// the same wins (65/126 both ways).
pub const HELP_MAX_GEMS: u32 = 20;

pub fn ally_camp_fight_to_join() -> Result<Option<Vec<Vec2>>> {
    ally_camp_fight_to_join_with(HELP_MAX_GEMS)
}

// On the way to bank, help an ally's camp fight we pass whatever we carry (HELP_BEFORE_BANK):
// walking past made the ally's camp-fight check count us in (ALLY_JOIN_TURNS), so it started a
// fight we then left it to lose. Off: walk on to the altar.
pub const HELP_BEFORE_BANK: bool = true;

pub fn ally_camp_fight_to_join_while_banking() -> Result<Option<Vec<Vec2>>> {
    if !HELP_BEFORE_BANK {
        return Ok(None);
    }
    ally_camp_fight_to_join_with(u32::MAX)
}

fn ally_camp_fight_to_join_with(max_gems: u32) -> Result<Option<Vec<Vec2>>> {
    if HELP_ALLY_RADIUS <= 0 {
        return Ok(None);
    }
    let me = me()?;
    if me.health()? < HELP_MIN_HEALTH * me.max_health()? || me.gem_count()? >= max_gems {
        return Ok(None);
    }
    let here = me.position()?;
    let sight = sight()?;
    if sight.allies.is_empty() || sight.camps.is_empty() {
        return Ok(None);
    }
    for enemy in &sight.enemies {
        if distance(enemy.position()?, here) <= HELP_ENEMY_CLEAR {
            return Ok(None);
        }
    }
    let mut best: Option<(i32, usize)> = None;
    let mut checked = Vec::new();
    for camp in &sight.camps {
        let position = camp.spawn_position();
        let Some(index) = group_at(position)? else {
            continue;
        };
        if checked.contains(&index)
            || !camp.is_awake().unwrap_or(false)
            || camp.health()? >= camp.max_health()?
        {
            continue;
        }
        let mut fought = false;
        for ally in &sight.allies {
            let p = ally.position()?;
            if distance(p, here) <= HELP_ALLY_RADIUS
                && distance(p, position) <= ally.attack_range()?.max(camp.attack_range()?)
            {
                fought = true;
                break;
            }
        }
        if !fought {
            continue;
        }
        checked.push(index);
        if camp_fight_winnable(index)? != Some(true) {
            continue;
        }
        let gap = distance(here, position);
        if best.is_none_or(|(d, _)| gap < d) {
            best = Some((gap, index));
        }
    }
    let Some((_, index)) = best else {
        return Ok(None);
    };
    Ok(Some(
        sight
            .camps
            .iter()
            .map(|c| c.spawn_position())
            .filter(|&p| group_at(p).ok().flatten() == Some(index))
            .collect(),
    ))
}

pub fn capturing() -> Result<bool> {
    if !CAPTURE_HOLD {
        return Ok(false);
    }
    let Some(index) = group_at(me()?.position()?)? else {
        return Ok(false);
    };
    let group = &camp_groups()?[index];
    Ok(
        matches!(group.state(), Ok(cambc::CampGroupState::RewardAvailable))
            && !(super::camps::CAPTURE_CLAIM && super::camps::claimed_by_other(group)?),
    )
}

// Escort: tanks and Willow only chase an enemy a damage dealer (within ESCORT_RADIUS of it) is
// close to, instead of fighting on their own.
pub const ESCORT: bool = true;
pub const ESCORT_RADIUS: i32 = 4;

pub fn is_escort() -> Result<bool> {
    Ok(ESCORT
        && matches!(
            class_of(me()?.champion_name()?),
            ClassKind::Tank | ClassKind::Healer
        ))
}

// Damage dealers in sight: allies that aren't tanks or healers.
fn damage_dealers() -> Result<Vec<Champion>> {
    let mut dealers = Vec::new();
    for ally in &sight()?.allies {
        if ally.is_alive()?
            && !matches!(
                class_of(ally.champion_name()?),
                ClassKind::Tank | ClassKind::Healer
            )
        {
            dealers.push(ally.clone());
        }
    }
    Ok(dealers)
}

// Follow-only healers: Willow never picks camps of her own. She answers rallies and the team target
// (rally.rs, opening.rs) and otherwise follows the nearest damage dealer in sight, or the lead
// damage dealer's published tile (see publish_lead), to within ESCORT_FOLLOW_RADIUS. She only hits
// camps someone has woken, or the rally / team-target group's. Off: she farms like everyone.
// Tanks farm normally: with tanks following too (2026-10-20, 126 games against it off) the team
// won 49 and lost 77. Following Willow healed 24% more and took half the camp damage, but Ryth
// stopped tanking camps, so the carries took 14% more camp damage, and both lost ~40% of their gold.
pub const ESCORT_FOLLOW_ONLY: bool = true;
pub const ESCORT_FOLLOW_RADIUS: i32 = 2;

pub fn follow_only() -> Result<bool> {
    Ok(ESCORT_FOLLOW_ONLY && class_of(me()?.champion_name()?) == ClassKind::Healer)
}

// Groups we can't get to (a pocket only a dash reaches, for a champion without one; a sealed room)
// are never captured by us, so we leave their camps alone unless an ally in range of the camp is
// fighting it: champions without a dash were hitting camps across holes (01islands, 01big-world:
// 177 hits in 44 games), waking them for nothing. Off: hit them.
pub const SKIP_UNREACHABLE_CAMPS: bool = true;

fn ally_fighting(camp: &Camp) -> Result<bool> {
    if !camp.is_awake().unwrap_or(false) {
        return Ok(false);
    }
    let at = camp.spawn_position();
    for ally in &sight()?.allies {
        if ally.is_alive()? && distance(ally.position()?, at) <= ally.attack_range()?.max(1) {
            return Ok(true);
        }
    }
    Ok(false)
}

// Whether we may hit `camp` (of group `index`): not if we can't get to its group (see
// SKIP_UNREACHABLE_CAMPS), and a follow-only escort leaves sleeping camps to the damage dealers
// unless the camp's group is the rally or team target.
pub fn may_hit_camp(camp: &Camp, index: usize) -> Result<bool> {
    if SKIP_UNREACHABLE_CAMPS && !crate::regions::reachable_for_me(index)? && !ally_fighting(camp)?
    {
        return Ok(false);
    }
    if !follow_only()? || camp.is_awake().unwrap_or(true) {
        return Ok(true);
    }
    Ok(super::rally::active()? == Some(index) || crate::opening::team_target()? == Some(index))
}

// Where the lead damage dealer is, for escorts that can't see it (vision isn't shared): shared
// memory bytes 6-7 (x, y; 0, 0 for none). A damage dealer that finds the slot empty takes it and
// writes its tile every turn; it empties the slot when it dies or recalls, and another takes over.
const LEAD_OFFSET: usize = 6;

thread_local! {
    static LEAD_WRITTEN: std::cell::Cell<Option<Vec2>> = const { std::cell::Cell::new(None) };
}

// The lead damage dealer's tile, unless we're the lead (farm::PACK_WEIGHT).
pub fn pack_anchor() -> Result<Option<Vec2>> {
    let lead = read_lead()?;
    let ours = LEAD_WRITTEN.with(|w| w.get());
    Ok(lead.filter(|_| ours.is_none() || ours != lead))
}

fn read_lead() -> Result<Option<Vec2>> {
    let mut bytes = [0u8; 2];
    cambc::shared_memory().read(LEAD_OFFSET, &mut bytes)?;
    Ok((bytes != [0, 0]).then(|| Vec2::new(bytes[0] as i32, bytes[1] as i32)))
}

// Called every turn by every champion, dead or alive (bot.rs).
pub fn publish_lead() -> Result<()> {
    if !ESCORT_FOLLOW_ONLY || is_escort()? {
        return Ok(());
    }
    let me = me()?;
    let slot = read_lead()?;
    let written = LEAD_WRITTEN.with(|w| w.get());
    let owns = written.is_some() && slot == written;
    if me.is_alive()? && !me.is_recalling()? {
        if owns || slot.is_none() {
            let here = me.position()?;
            let (Ok(x), Ok(y)) = (u8::try_from(here.x), u8::try_from(here.y)) else {
                return Ok(());
            };
            if (x, y) == (0, 0) {
                return Ok(()); // can't be told from "none"; the old tile stays
            }
            cambc::shared_memory().write(LEAD_OFFSET, &[x, y])?;
            LEAD_WRITTEN.with(|w| w.set(Some(here)));
        } else {
            LEAD_WRITTEN.with(|w| w.set(None));
        }
    } else if owns {
        cambc::shared_memory().write(LEAD_OFFSET, &[0, 0])?;
        LEAD_WRITTEN.with(|w| w.set(None));
    }
    Ok(())
}

// Tiles a follow-only escort heads for when there's no rally or team target: around the nearest
// damage dealer in sight, else the lead's published tile, else our spawn (dealers come out of it).
// Where we already are if we're close enough.
// A follow-only healer goes to the most hurt ally in sight (below FOLLOW_HURT_BELOW of its health)
// first: whoever is tanking camps, where heals keep it out longer. 0: damage dealers only.
// 2026-10-21, 132 games: 64-68 against off.
pub const FOLLOW_HURT_BELOW: f32 = 0.0;

pub fn escort_follow_goals() -> Result<cambc::Bitboard> {
    let here = me()?.position()?;
    let mut target = None;
    if FOLLOW_HURT_BELOW > 0.0 && class_of(me()?.champion_name()?) == ClassKind::Healer {
        let mut worst: Option<(f32, Vec2)> = None;
        for ally in &sight()?.allies {
            if !ally.is_alive()? || ally.is_recalling()? {
                continue;
            }
            let fraction = ally.health()? / ally.max_health()?.max(1.0);
            if fraction < FOLLOW_HURT_BELOW && worst.is_none_or(|(f, _)| fraction < f) {
                worst = Some((fraction, ally.position()?));
            }
        }
        target = worst.map(|(_, p)| p);
    }
    let following_hurt = target.is_some();
    for dealer in damage_dealers()?.iter().filter(|_| !following_hurt) {
        if dealer.is_recalling()? {
            continue;
        }
        let p = dealer.position()?;
        if target.is_none_or(|t| distance(p, here) < distance(t, here)) {
            target = Some(p);
        }
    }
    let target = match target {
        Some(t) => t,
        None => match read_lead()? {
            Some(t) => t,
            None => super::base::ally_spawn_centre()?,
        },
    };
    let mut goals = cambc::Bitboard::new()?;
    if distance(here, target) <= ESCORT_FOLLOW_RADIUS {
        goals.set(here, true)?;
    } else {
        goals.draw_chebyshev(target, ESCORT_FOLLOW_RADIUS as f32)?;
    }
    Ok(goals)
}

pub fn escort_allows_chase(target: Vec2) -> Result<bool> {
    if !is_escort()? {
        return Ok(true);
    }
    for dealer in damage_dealers()? {
        if distance(dealer.position()?, target) <= ESCORT_RADIUS + dealer.attack_range()? {
            return Ok(true);
        }
    }
    Ok(false)
}
