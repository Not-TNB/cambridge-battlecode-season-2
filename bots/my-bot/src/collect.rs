// Collecting: which camp groups we'd beat with nearby allies, holding a capture until it pays out, and support
// champions (tanks, Willow) sticking with the damage dealers instead of fighting on their own.
use crate::common::{camp_groups, distance, group_at, is_damaging, me, sight, tiles_around};
use crate::constants::{class_of, ClassKind};
use cambc::{Bitboard, Camp, Champion, Result, Vec2};

// Camp fights, simulated turn by turn from where we stand: attacking any camp wakes the whole
// group. We focus its weakest camp; allies in support range split their expected damage across
// living camps. Camp damage is shared among us and nearby visible allies. Groups we expect to lose
// are skipped.
pub const CAMP_FIGHT: bool = true;
pub const CAMP_FIGHT_HP_FLOOR: f32 = 0.35;
// Assume visible allies within this distance can join a camp fight, even before they are in attack range.
pub const CAMP_ALLY_SUPPORT_RADIUS: i32 = 7;

// What every camp-fight check in a turn shares, built once per round and position: looking things
// up per group (~20 groups) cost ~60k fuel.
struct FightContext {
    round: u32,
    here: Vec2,
    live: Vec<(usize, Camp)>, // visible live camps, by group index
    sight: std::rc::Rc<crate::common::Sight>,
    outgoing: f32,                            // our damage per turn against camps
    helpers: Vec<(Vec2, i32, f32, f32, f32)>, // position, attack range, damage, hp, retreat floor
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
        if let Some(index) = group_at(camp.spawn_position())? {
            live.push((index, camp.clone()));
        }
    }
    let outgoing = expected_camp_damage(&me)?;
    let mut helpers = Vec::new();
    for ally in &sight.allies {
        if ally.is_alive()? {
            helpers.push((
                ally.position()?,
                ally.attack_range()?,
                expected_camp_damage(ally)?,
                ally.health()?,
                CAMP_FIGHT_HP_FLOOR * ally.max_health()?,
            ));
        }
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
        let verdict = judge(ctx, index)?;
        ctx.verdicts[index] = Some(verdict);
        Ok(verdict)
    })
}

fn judge(ctx: &FightContext, index: usize) -> Result<Option<bool>> {
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
            camps.push((
                member.spawn_position(),
                c.health()?,
                c.strength()?,
                c.attack_range()?,
            ));
        } else if !ctx.sight.visible.is_set(member.spawn_position())? {
            camps.push(match live.iter().find(|c| c.kind() == member.kind()) {
                Some(c) => (
                    member.spawn_position(),
                    c.max_health()?,
                    c.strength()?,
                    c.attack_range()?,
                ),
                None => {
                    let stats = member.base_stats()?;
                    (
                        member.spawn_position(),
                        stats.base_health,
                        stats.base_strength,
                        stats.base_attack_range,
                    )
                }
            });
        }
    }
    if camps.iter().all(|&(_, _, s, _)| s <= 0.0) {
        return Ok(Some(true));
    } // clusters don't hit back
    camps.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut damage_rates = vec![0.0; camps.len()];
    let helpers = ctx.helpers.clone();
    let mut combatants = Vec::with_capacity(ctx.helpers.len() + 1);
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
            if health <= 0.0 || distance(position, ctx.here) > range {
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
    Ok(Some(winnable))
}

// Whether group `index` is safe to take: only clusters (they don't hit back), or a camp fight we'd
// win. Unknown (not in sight) counts as not safe.
pub fn safe_group(index: usize) -> Result<bool> {
    let group = &camp_groups()?[index];
    if group
        .camps()
        .iter()
        .all(|c| c.is_gem_cluster() || c.is_gold_cluster())
    {
        return Ok(true);
    }
    Ok(GREEDY_CAMPS && camp_fight_winnable(index)? == Some(true))
}

// Greedy collecting: with no enemy about to reach us, safe groups (see safe_group) and uncaptured
// rewards within GREEDY_RADIUS come first: before chasing enemies, and whatever the gold/gem phase.
pub const GREEDY: bool = true;
pub const GREEDY_RADIUS: i32 = 8;
// Whether camp groups we'd win count as greedy targets, or only clusters and uncaptured rewards.
pub const GREEDY_CAMPS: bool = false;
// Whether a greedy target nearby stops us chasing enemies, or only steers farming.
pub const GREEDY_BEFORE_CHASE: bool = true;

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

// Escort: tanks and Willow stick with the damage dealers. They only chase an enemy a damage dealer
// is close to, walk back to the nearest one when more than ESCORT_RADIUS away, and recall when
// every damage dealer in sight is recalling.
pub const ESCORT: bool = true;
pub const ESCORT_RADIUS: i32 = 4;
pub const ESCORT_RECALL: bool = false;
pub const ESCORT_CHASE: bool = true;
pub const ESCORT_FOLLOW: bool = false;

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

pub fn escort_recalls() -> Result<bool> {
    if !ESCORT_RECALL || !is_escort()? {
        return Ok(false);
    }
    let dealers = damage_dealers()?;
    if dealers.is_empty() {
        return Ok(false);
    }
    for dealer in &dealers {
        if !dealer.is_recalling()? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn escort_allows_chase(target: Vec2) -> Result<bool> {
    if !ESCORT_CHASE || !is_escort()? {
        return Ok(true);
    }
    for dealer in damage_dealers()? {
        if distance(dealer.position()?, target) <= ESCORT_RADIUS + dealer.attack_range()? {
            return Ok(true);
        }
    }
    Ok(false)
}

// Tiles next to the nearest damage dealer, if it's more than ESCORT_RADIUS away.
pub fn escort_goal() -> Result<Option<Bitboard>> {
    if !ESCORT_FOLLOW || !is_escort()? {
        return Ok(None);
    }
    let here = me()?.position()?;
    // Finish the camp we're fighting first.
    let range = me()?.attack_range()?;
    if sight()?
        .camps
        .iter()
        .any(|c| distance(c.spawn_position(), here) <= range)
    {
        return Ok(None);
    }
    let mut nearest: Option<(Vec2, i32)> = None;
    for dealer in damage_dealers()? {
        if dealer.is_recalling()? {
            continue;
        }
        let p = dealer.position()?;
        if nearest.is_none_or(|(_, d)| distance(p, here) < d) {
            nearest = Some((p, distance(p, here)));
        }
    }
    match nearest {
        Some((p, d)) if d > ESCORT_RADIUS => Ok(Some(tiles_around(p, 2)?)),
        _ => Ok(None),
    }
}
