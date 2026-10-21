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
// The fight is fought down to this many hits of the group's hardest-hitting camp, when that's
// below CAMP_FIGHT_HP_FLOOR (the low-health recall waits as long, safety::low_hp_recall).
pub const CAMP_FIGHT_HITS: f32 = 3.0;
// Assume visible allies within this distance can join a camp fight, even before they are in attack range.
pub const CAMP_ALLY_SUPPORT_RADIUS: i32 = 7;
// Allies further away join once they could have walked into support range (a step a round), if
// that's within ALLY_JOIN_TURNS: counting only the allies already in range made the first champion
// to reach an elk judge it lost while the rest of the team was 8-9 tiles behind (bot-v2: 66/126
// against 53/126 without it, the biggest gain of its camp-fight changes).
pub const ALLY_JOIN_TURNS: i32 = 6;
// The camp-fight check in closed form instead of simulated. Focusing the weakest camp first with a
// team damage rate D, camp i (in that order) is alive until C_i / D, C_i being the health of camps
// 1..=i, so the team takes K / D damage in all, with K = sum of strength_i * C_i. The fight is won
// if that fits in the health the fighters can spend before their retreat floors: D * spare >= K.
// Allies still walking over count for the share of the fight they'd be there for. It's the even
// damage split the simulation assumed, without simulating it (in bot-v2 the simulation was the
// biggest cost in turns that ran out of fuel; the two agreed on 90% of checks). Off: simulate.
pub const CAMP_FIGHT_CLOSED_FORM: bool = true;
// A group whose visible members are dead but which isn't cleared has the rest alive out of sight:
// farm_plan then sends us to its unseen tiles rather than its dead ones. Off: dead tiles too.
pub const PARTIAL_GROUPS: bool = true;

// An ally that can join a camp fight: position, attack range, damage, hp, retreat floor.
type Helper = (Vec2, i32, f32, f32, f32);

// What every camp-fight check in a turn shares, built once per round and position: looking things
// up per group (~20 groups) cost ~60k fuel.
struct FightContext {
    round: u32,
    here: Vec2,
    live: Vec<(usize, Camp)>, // visible live camps, by group index
    sight: std::rc::Rc<crate::common::Sight>,
    outgoing: f32, // our damage per turn against camps
    range: i32,    // our attack range
    helpers: Vec<Helper>,
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
    let round = cambc::round_number()?;
    let outgoing = cached_camp_damage(me, round)?;
    let mut helpers = Vec::new();
    for ally in &sight.allies {
        if ally.is_alive()? {
            helpers.push((
                ally.position()?,
                ally.attack_range()?,
                cached_camp_damage(ally, round)?,
                ally.health()?,
                CAMP_FIGHT_HP_FLOOR * ally.max_health()?,
            ));
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
        let verdict = judge(ctx, index)?;
        ctx.verdicts[index] = Some(verdict);
        Ok(verdict)
    })
}

fn judge(ctx: &FightContext, index: usize) -> Result<Option<bool>> {
    let camps = group_camps(ctx, index)?;
    if camps.iter().all(|&(_, _, s, _)| s <= 0.0) {
        return Ok(Some(true));
    } // clusters don't hit back
    Ok(Some(fight_winnable(ctx, camps, &ctx.helpers)?))
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
    let mut camps = Vec::new();
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
    Ok(camps)
}

fn fight_winnable(
    ctx: &FightContext,
    camps: Vec<(Vec2, f32, f32, i32)>,
    helpers: &[Helper],
) -> Result<bool> {
    if CAMP_FIGHT_CLOSED_FORM {
        Ok(closed_form(ctx, &camps, helpers))
    } else {
        Ok(simulate(ctx, camps, helpers))
    }
}

// When each helper would be in the fight: rounds from now (0: already in support range), or None
// if it's more than ALLY_JOIN_TURNS away.
fn join_times(camps: &[(Vec2, f32, f32, i32)], helpers: &[Helper]) -> Vec<Option<f32>> {
    helpers
        .iter()
        .map(|&(position, range, ..)| {
            let gap = camps
                .iter()
                .filter(|camp| camp.1 > 0.0)
                .map(|camp| distance(camp.0, position))
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
        .collect()
}

// A combatant's retreat floor: CAMP_FIGHT_HP_FLOOR of its health, or CAMP_FIGHT_HITS hits of the
// hardest-hitting camp if that's lower.
fn floor_against(floor: f32, hardest_hit: f32) -> f32 {
    if CAMP_FIGHT_HITS > 0.0 && hardest_hit > 0.0 {
        floor.min(CAMP_FIGHT_HITS * hardest_hit)
    } else {
        floor
    }
}

fn closed_form(ctx: &FightContext, camps: &[(Vec2, f32, f32, i32)], helpers: &[Helper]) -> bool {
    let mut live: Vec<(Vec2, f32, f32, i32)> =
        camps.iter().copied().filter(|c| c.1 > 0.0).collect();
    if live.is_empty() {
        return true;
    }
    live.sort_by(|a, b| a.1.total_cmp(&b.1));
    let hardest_hit = live.iter().map(|c| c.2).fold(0.0f32, f32::max);
    let joins = join_times(&live, helpers);
    let total: f32 = live.iter().map(|c| c.1).sum();
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
    let mut spare = (ctx.hp - floor_against(ctx.floor, hardest_hit)).max(0.0);
    let mut any_helper = false;
    for (h, join) in helpers.iter().zip(&joins) {
        if let Some(join) = join {
            if h.3 > h.4 {
                let s = share(*join);
                damage += h.2 * s;
                spare += (h.3 - floor_against(h.4, hardest_hit)).max(0.0) * s;
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
    for &(position, health, strength, range) in &live {
        cumulative += health;
        let hits_us = range >= ctx.range || distance(position, ctx.here) <= range;
        if hits_us || any_helper {
            k += strength * cumulative;
        }
    }
    k <= damage * spare
}

// The camp fight turn by turn (CAMP_FIGHT_CLOSED_FORM off): each iteration advances to the next
// camp death, champion retreat threshold or ally arrival. Allies that retreat stop contributing.
fn simulate(ctx: &FightContext, mut camps: Vec<(Vec2, f32, f32, i32)>, helpers: &[Helper]) -> bool {
    camps.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut damage_rates = vec![0.0; camps.len()];
    let hardest_hit = camps.iter().map(|camp| camp.2).fold(0.0f32, f32::max);
    let mut combatants = Vec::with_capacity(helpers.len() + 1);
    combatants.push((ctx.hp, floor_against(ctx.floor, hardest_hit)));
    combatants.extend(
        helpers
            .iter()
            .map(|helper| (helper.3, floor_against(helper.4, hardest_hit))),
    );
    let mut active: Vec<bool> = combatants.iter().map(|&(hp, floor)| hp > floor).collect();
    // When each combatant is in the fight (we are from the start).
    let mut join_at = vec![0.0f32; combatants.len()];
    for (i, join) in join_times(&camps, helpers).into_iter().enumerate() {
        join_at[i + 1] = join.unwrap_or(f32::INFINITY);
    }
    let mut now = 0.0f32;
    let mut winnable = camps.iter().all(|&(_, health, _, _)| health <= 0.0);

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
            // at least ours hits us wherever we stand now.
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
    winnable
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
