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

// Judged lost within the last CAMP_LOSS_MEMORY_ROUNDS rounds (no host calls, so it's also checked
// for groups out of sight: stepping out of sight of a lost group made it a goal again).
pub fn recently_lost(index: usize, round: u32) -> bool {
    CAMP_FIGHT && LOST_UNTIL.with(|l| l.borrow().get(index).is_some_and(|&until| round < until))
}

// An ally that can join a camp fight: position, attack range, damage, hp, retreat floor.
type Helper = (Vec2, i32, f32, f32, f32);

// What every camp-fight check in a turn shares, built once per round and position: looking things
// up per group (~20 groups) cost ~60k fuel.
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
            helpers.push((
                ally.position()?,
                range,
                damage,
                ally.health()?,
                CAMP_FIGHT_HP_FLOOR * max_health,
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
        hp: me.health()?,
        floor: CAMP_FIGHT_HP_FLOOR * me.max_health()?,
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

// Whether we'd win the camp fight for group `index`. Unseen camps are estimated at base stats so
// farm planning can compare groups beyond current vision.
pub fn camp_fight_winnable(index: usize) -> Result<Option<bool>> {
    if !CAMP_FIGHT {
        return Ok(Some(true));
    }
    let key = (cambc::round_number()?, me()?.position()?);
    let fresh = CONTEXT.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|ctx| (ctx.round, ctx.here) == key)
    });
    if !fresh {
        let ctx = fight_context()?;
        CONTEXT.with(|c| *c.borrow_mut() = Some(ctx));
    }
    CONTEXT.with(|c| {
        let mut c = c.borrow_mut();
        let ctx = c.as_mut().expect("set above");
        if let Some(verdict) = ctx.verdicts[index] {
            return Ok(verdict);
        }
        // A rally group (rally.rs) is judged afresh every turn: allies arriving should flip it.
        let rally = super::rally::active()? == Some(index);
        let lost = !rally && recently_lost(index, ctx.round);
        let verdict = if lost {
            Some(false)
        } else {
            judge(ctx, index)?
        };
        if verdict == Some(false) && !lost && !rally {
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
    let winnable = simulate(ctx, camps, &ctx.helpers)?;
    VERDICTS.with(|v| {
        let mut v = v.borrow_mut();
        v.resize(camp_groups().map_or(0, |g| g.len()), None);
        if let Some(slot) = v.get_mut(index) {
            *slot = Some((key, winnable));
        }
    });
    Ok(Some(winnable))
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
// before falling to our retreat floors.
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
    let mut active: Vec<bool> = combatants.iter().map(|&(hp, floor)| hp > floor).collect();
    let mut winnable = camps.iter().all(|&(_, health, _, _)| health <= 0.0);

    // Each iteration advances to the next camp death or champion retreat threshold, rather than
    // simulating every round. Allies that retreat stop contributing damage.
    while !winnable {
        damage_rates.fill(0.0);
        if active[0] && ctx.outgoing > 0.0 {
            if let Some((index, _)) = camps
                .iter()
                .enumerate()
                .filter(|(_, camp)| camp.1 > 0.0)
                .min_by(|(_, a), (_, b)| a.1.total_cmp(&b.1))
            {
                damage_rates[index] += ctx.outgoing;
            }
        }

        for (helper_index, &(position, range, damage, _, _)) in helpers.iter().enumerate() {
            if !active[helper_index + 1] {
                continue;
            }
            let targets = camps
                .iter()
                .enumerate()
                .filter(|(_, camp)| {
                    camp.1 > 0.0
                        && distance(camp.0, position) <= range.max(CAMP_ALLY_SUPPORT_RADIUS)
                })
                .count();
            if targets == 0 {
                continue;
            }
            let per_target = damage / targets as f32;
            for (i, camp) in camps.iter().enumerate() {
                if camp.1 > 0.0 && distance(camp.0, position) <= range.max(CAMP_ALLY_SUPPORT_RADIUS)
                {
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
            let targets: Vec<usize> = std::iter::once(0)
                .chain(helpers.iter().enumerate().filter_map(|(i, helper)| {
                    (active[i + 1]
                        && distance(position, helper.0) <= range.max(CAMP_ALLY_SUPPORT_RADIUS))
                    .then_some(i + 1)
                }))
                .filter(|&i| active[i])
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
            .filter(|(i, _)| active[*i] && incoming[*i] > 0.0)
            .map(|(i, &(hp, floor))| (i, (hp - floor) / incoming[i]))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((next_death_index, next_death)) = next_death else {
            break;
        };
        let time_to_retreat = next_retreat.map_or(f32::INFINITY, |(_, time)| time);
        if time_to_retreat < next_death {
            for (i, camp) in camps.iter_mut().enumerate() {
                if camp.1 > 0.0 {
                    camp.1 -= damage_rates[i] * time_to_retreat;
                }
            }
            for (i, combatant) in combatants.iter_mut().enumerate() {
                combatant.0 -= incoming[i] * time_to_retreat;
                if active[i] && combatant.0 <= combatant.1 + 0.001 {
                    active[i] = false;
                }
            }
            continue;
        }

        for (i, camp) in camps.iter_mut().enumerate() {
            if camp.1 > 0.0 {
                camp.1 -= damage_rates[i] * next_death;
            }
        }
        camps[next_death_index].1 = 0.0;
        for (i, combatant) in combatants.iter_mut().enumerate() {
            combatant.0 -= incoming[i] * next_death;
        }
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
        simulate(ctx, camps, &helpers)
    })
}

// Hold a capture: standing on a cleared group's spawn tile, stay until the reward is ours (5
// rounds) rather than leave to chase, bank or go home. Retreating from a losing fight and the
// low-health recall still come first.
pub const CAPTURE_HOLD: bool = true;

pub fn capturing() -> Result<bool> {
    if !CAPTURE_HOLD {
        return Ok(false);
    }
    let Some(index) = group_at(me()?.position()?)? else {
        return Ok(false);
    };
    Ok(matches!(
        camp_groups()?[index].state(),
        Ok(cambc::CampGroupState::RewardAvailable)
    ))
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
