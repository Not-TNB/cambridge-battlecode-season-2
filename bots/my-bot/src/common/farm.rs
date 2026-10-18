// Choosing where to farm: phase filter, respawn memory, avoiding enemies and the farm plan.
use super::attack::cast_abilities_on_camps;
use super::camps::{
    ally_on_group, attack_camp_in_range, carry_covers, FUNNEL_HEALERS, TANK_FUNNEL,
};
use super::duel::{cached_fight_view, fight_view, plan_duel};
use super::me;
use super::movement::{goals_were_unreachable, move_towards_closest};
use super::trace::FARM_TARGET_DIAGNOSTICS;
use super::world::{camp_groups, distance, group_at, sight};
use cambc::{Bitboard, Camp, Result, Vec2};
use std::cell::OnceCell;

// Round each camp group (indexed as in camp_groups()) is expected back after we last saw it
// respawning. Without this, a respawning group only counts as respawning while in sight, so a
// champion at the edge of vision steps away, loses sight, steps back, and oscillates.
thread_local! {
    static RESPAWN_READY: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Phase filter on top of nearest-camp farming: before FARM_GEM_PHASE_ROUND we skip gem clusters
// and monoliths (gems early don't help; gold buys items), after it we skip gold clusters (35 gold,
// no gems). If no camp of the preferred kinds is available we fall back to all camps. Scoring
// camps by value per turn instead tested worse (writeup entry 20). The filter applies to whole
// groups: skipping single camps abandoned a group whose elk was dead but whose gold cluster wasn't,
// so it could never be captured (and the champion stepped back and forth at its edge of vision).
pub const FARM_PHASE_FILTER: bool = true;
pub const FARM_GEM_PHASE_ROUND: u32 = 150;
// In the gem phase, gold-only groups (gold clusters) are goals too while they're at most this many
// tiles further than the nearest gem goal: gold still buys items, so take it when it's on the way
// or the closest thing around. They join the candidates rather than replace them, so the target
// memory keeps the current target while we walk. 0: skip gold.
pub const GEM_PHASE_GOLD_DETOUR: i32 = 3;
// Damaged camps within this many tiles are finished off first (see DAMAGED_CAMP_MAX_DETOUR).
const DAMAGED_CAMP_RADIUS: i32 = 8;

fn preferred_in_phase(kind: cambc::CampType, round: u32) -> bool {
    use cambc::CampType as C;
    if round < FARM_GEM_PHASE_ROUND {
        !matches!(kind, C::GemCluster | C::Monolith)
    } else {
        kind != C::GoldCluster
    }
}

// Don't farm next to enemies we aren't expected to beat: skip groups a visible enemy could reach
// within FARM_ENEMY_TURNS turns (its attack range plus that much walking).
pub const FARM_AVOID_ENEMIES: bool = true;
const FARM_ENEMY_TURNS: i32 = 2;
const FARM_ENEMY_DUEL_FUEL: u64 = 70_000;

// Where farming heads this turn (farm_camps).
pub struct FarmPlan {
    pub goals: Bitboard,
}

const FARM_TARGET_COOLDOWN_ROUNDS: u32 = 2;
// A farm target we find no walking path to is skipped for this many rounds: a pocket the region
// table doesn't know about, or a corridor units are standing in. Without it we kept the target
// and stood still.
const UNREACHABLE_TARGET_ROUNDS: u32 = 20;
const DAMAGED_CAMP_MAX_DETOUR: i32 = 3;

#[derive(Default)]
struct FarmTargetMemory {
    current: Option<Vec2>,
    abandoned: Vec<(Vec2, u32)>,
}

thread_local! {
    static FARM_TARGET_MEMORY: std::cell::RefCell<FarmTargetMemory> = std::cell::RefCell::new(FarmTargetMemory::default());
}

fn report_farm_target_change(
    previous: Option<Vec2>,
    target: Option<Vec2>,
    reason: &str,
    round: u32,
) {
    if !FARM_TARGET_DIAGNOSTICS || previous == target || previous.is_none() {
        return;
    }
    let point =
        |tile: Option<Vec2>| tile.map_or_else(|| "none".to_owned(), |p| format!("{},{}", p.x, p.y));
    println!(
        "farm-target-change r{} {} -> {} reason={}",
        round,
        point(previous),
        point(target),
        reason,
    );
}

fn avoid_abandoned_farm_targets(
    goals: &mut Bitboard,
    round: u32,
    here: Vec2,
    reach: i32,
    unavailable_reason: Option<&'static str>,
) -> Result<()> {
    let candidates = goals.positions()?;
    let Some(candidates_for_target) = (!candidates.is_empty()).then_some(candidates) else {
        let previous = FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().current.take());
        report_farm_target_change(
            previous,
            None,
            unavailable_reason.unwrap_or("no-eligible-farm-goals"),
            round,
        );
        return Ok(());
    };

    let previous = FARM_TARGET_MEMORY.with(|memory| memory.borrow().current);
    let (target, changed_reason) = FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        memory.abandoned.retain(|&(_, until)| round < until);
        let previous_is_abandoned =
            previous.is_some_and(|old| memory.abandoned.iter().any(|&(tile, _)| tile == old));
        let available: Vec<Vec2> = candidates_for_target
            .iter()
            .copied()
            .filter(|&tile| !memory.abandoned.iter().any(|&(old, _)| old == tile))
            .collect();
        let target = if available.is_empty() {
            memory.abandoned.clear();
            candidates_for_target
                .iter()
                .copied()
                .min_by_key(|&tile| distance(here, tile))
        } else {
            let current_is_reachable = memory
                .current
                .is_some_and(|tile| distance(here, tile) <= reach);
            if !current_is_reachable {
                if let Some(current) = memory.current.filter(|tile| available.contains(tile)) {
                    Some(current)
                } else {
                    available
                        .iter()
                        .copied()
                        .min_by_key(|&tile| distance(here, tile))
                }
            } else {
                available
                    .iter()
                    .copied()
                    .min_by_key(|&tile| distance(here, tile))
            }
        };
        let reason = previous.filter(|&old| Some(old) != target).map(|old| {
            if !candidates_for_target.contains(&old) {
                unavailable_reason.unwrap_or("previous-target-not-in-farm-goals")
            } else if previous_is_abandoned {
                "previous-target-cooldown"
            } else if distance(here, old) <= reach {
                "previous-target-within-attack-range"
            } else {
                "closer-available-target"
            }
        });
        (target, reason)
    });

    FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        if memory.current != target {
            if let Some(previous) = memory.current {
                if distance(here, previous) > reach {
                    memory
                        .abandoned
                        .push((previous, round + FARM_TARGET_COOLDOWN_ROUNDS));
                }
            }
            memory.current = target;
        }
    });

    for tile in candidates_for_target.iter().copied() {
        if Some(tile) == target {
            continue;
        }
        if goals.is_set(tile)? {
            goals.set(tile, false)?;
        }
    }
    if goals.is_empty()? {
        for tile in candidates_for_target {
            goals.set(tile, true)?;
        }
        FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().abandoned.clear());
    }
    if let Some(reason) = changed_reason {
        report_farm_target_change(previous, target, reason, round);
    }
    Ok(())
}

thread_local! {
    static FARM_PLAN: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<FarmPlan>)>> = const { std::cell::RefCell::new(None) };
}

// What farm_plan needs about each group that never changes, computed once. Pure Rust work is
// metered too (looping over every camp of every group several times a turn cost ~30k fuel), so a
// turn only looks at the camps of groups close enough to be in sight.
struct GroupInfo {
    centre: Vec2,
    radius: i32,          // furthest camp from the centre
    preferred: [bool; 2], // has a camp preferred before / from FARM_GEM_PHASE_ROUND
    reachable: bool,      // we can get there: on foot, or with our dash (see regions.rs)
}

struct FarmStatics {
    groups: Vec<GroupInfo>,
    all: Bitboard,            // every camp's tile
    preferred: [Bitboard; 2], // camps of preferred groups, per phase
}

fn farm_statics() -> Result<&'static FarmStatics> {
    thread_local! {
        static STATICS: OnceCell<&'static FarmStatics> = const { OnceCell::new() };
    }
    if let Some(statics) = STATICS.with(|c| c.get().copied()) {
        return Ok(statics);
    }
    let mut statics = FarmStatics {
        groups: Vec::new(),
        all: Bitboard::new()?,
        preferred: [Bitboard::new()?, Bitboard::new()?],
    };
    for (index, group) in camp_groups()?.iter().enumerate() {
        let n = group.camps().len().max(1) as i32;
        let (x, y) = group.camps().iter().fold((0, 0), |(x, y), c| {
            (x + c.spawn_position().x, y + c.spawn_position().y)
        });
        let centre = Vec2::new(x / n, y / n);
        let radius = group
            .camps()
            .iter()
            .map(|c| distance(c.spawn_position(), centre))
            .max()
            .unwrap_or(0);
        let mut preferred = [false; 2];
        for (phase, round) in [0, FARM_GEM_PHASE_ROUND].into_iter().enumerate() {
            preferred[phase] = group
                .camps()
                .iter()
                .any(|c| preferred_in_phase(c.kind(), round));
        }
        // Groups we can't get to are never goals: on ambush, champions stood still for the rest of
        // the game with a camp in a walled-off room as their target.
        let reachable = crate::regions::reachable_for_me(index)?;
        for camp in group.camps().iter().filter(|_| reachable) {
            statics.all.set(camp.spawn_position(), true)?;
            for phase in 0..2 {
                if preferred[phase] {
                    statics.preferred[phase].set(camp.spawn_position(), true)?;
                }
            }
        }
        statics.groups.push(GroupInfo {
            centre,
            radius,
            preferred,
            reachable,
        });
    }
    let statics: &'static FarmStatics = Box::leak(Box::new(statics));
    STATICS.with(|c| {
        let _ = c.set(statics);
    });
    Ok(statics)
}

// A farm plan is reused for up to PLAN_REUSE_ROUNDS rounds while what it depends on in sight is
// unchanged: our tile and health, the gold/gem phase, the visible camps and their health, and
// where the visible enemies and allies are. Building one costs ~30k fuel, and we stand still on
// many turns (waiting on a capture or a respawn, or holding back). The age limit covers what isn't
// in the key: respawn timers, judged-lost camps and target cooldowns.
const PLAN_REUSE_ROUNDS: u32 = 5;

#[derive(PartialEq)]
struct PlanKey {
    rally: Option<usize>,
    here: Vec2,
    hp: i32,
    phase: bool,
    camps: Vec<(Vec2, i32)>,
    enemies: Vec<Vec2>,
    allies: Vec<Vec2>,
}

thread_local! {
    static PLAN_REUSE: std::cell::RefCell<Option<(u32, PlanKey, std::rc::Rc<FarmPlan>)>> =
        const { std::cell::RefCell::new(None) };
}

fn plan_key(here: Vec2, round: u32) -> Result<PlanKey> {
    let sight = sight()?;
    let mut camps = Vec::with_capacity(sight.camps.len());
    for camp in &sight.camps {
        camps.push((camp.spawn_position(), camp.health()? as i32));
    }
    let mut enemies = Vec::with_capacity(sight.enemies.len());
    for enemy in &sight.enemies {
        enemies.push(enemy.position()?);
    }
    let mut allies = Vec::with_capacity(sight.allies.len());
    for ally in &sight.allies {
        allies.push(ally.position()?);
    }
    Ok(PlanKey {
        rally: super::rally::active()?,
        here,
        hp: me()?.health()? as i32,
        phase: round >= FARM_GEM_PHASE_ROUND,
        camps,
        enemies,
        allies,
    })
}

// Plan farming: the nearest camps that aren't respawning (including ones we saw respawning
// earlier), skipping groups we'd lose to (collect::CAMP_FIGHT) or that enemies are next to.
// Rewards in sight and elk rallies come first, and a capture we're standing on is held. Tanks and
// healers still fight camps but leave the capture of a cleared group to a covering AD carry (see
// TANK_FUNNEL). Cached per round and position.
pub fn farm_plan() -> Result<std::rc::Rc<FarmPlan>> {
    let here = me()?.position()?;
    let round = cambc::round_number()?;
    if let Some(plan) = FARM_PLAN.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == (round, here))
            .map(|(_, _, p)| p.clone())
    }) {
        return Ok(plan);
    }
    // A plan from the last few rounds, if nothing it looks at has changed since. A failed read
    // (a unit can drop out of vision mid-turn) just means building a new plan.
    let key = plan_key(here, round).ok();
    if let Some(plan) = key.as_ref().and_then(|key| {
        PLAN_REUSE.with(|c| {
            c.borrow()
                .as_ref()
                .filter(|(built, k, _)| round < built + PLAN_REUSE_ROUNDS && k == key)
                .map(|(_, _, p)| p.clone())
        })
    }) {
        FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
        super::trace::trace("farm-plan-reused");
        return Ok(plan);
    }
    let statics = farm_statics()?;
    let class = crate::constants::class_of(me()?.champion_name()?);
    let is_tank = TANK_FUNNEL
        && (class == crate::constants::ClassKind::Tank
            || (FUNNEL_HEALERS && class == crate::constants::ClassKind::Healer));
    let sight = sight()?;
    let groups = camp_groups()?;
    // Enemy reach: (position, attack range + FARM_ENEMY_TURNS turns of walking).
    let mut potential_enemy_reach = Vec::new();
    if FARM_AVOID_ENEMIES {
        for enemy in &sight.enemies {
            potential_enemy_reach.push((
                enemy.clone(),
                enemy.position()?,
                enemy.attack_range()?
                    + enemy.base_stats()?.base_move_speed as i32 * FARM_ENEMY_TURNS,
            ));
        }
    }
    let potentially_threatens_farm = potential_enemy_reach.iter().any(|(_, position, reach)| {
        groups.iter().any(|group| {
            group
                .camps()
                .iter()
                .any(|camp| distance(*position, camp.spawn_position()) <= *reach)
        })
    });
    let duel_view = if potentially_threatens_farm {
        match cached_fight_view()? {
            Some(view) => Some(view),
            None if cambc::fuel_remaining()? >= FARM_ENEMY_DUEL_FUEL => Some(fight_view()?),
            None => None,
        }
    } else {
        None
    };
    let mut enemy_reach = Vec::new();
    for (enemy, position, reach) in potential_enemy_reach {
        let enemy_is_winnable = if let Some(view) = &duel_view {
            plan_duel(&enemy, &view.threats, &view.allies, &view.from_me)?
                .is_some_and(|duel| duel.winnable)
        } else {
            false
        };
        if !enemy_is_winnable {
            enemy_reach.push((position, reach));
        }
    }
    let max_reach = enemy_reach.iter().map(|&(_, r)| r).max().unwrap_or(0);
    let vision = me()?.base_stats()?.vision_radius as i32;
    let phase = usize::from(round >= FARM_GEM_PHASE_ROUND);
    let previous_target = FARM_TARGET_MEMORY.with(|memory| memory.borrow().current);
    let previous_group = match previous_target {
        Some(tile) => group_at(tile)?,
        None => None,
    };
    let mut previous_target_reason = None;
    let mut damaged_camps = Vec::new();
    for camp in &sight.camps {
        if camp.health()? < camp.max_health()? {
            if let Some(index) = group_at(camp.spawn_position())? {
                damaged_camps.push((index, camp.spawn_position()));
            }
        }
    }
    let mut ready = RESPAWN_READY.with(|cell| cell.take());
    ready.resize(groups.len(), 0);
    let mut available = vec![false; groups.len()];
    let mut dropped = Vec::new(); // groups that aren't goals this turn
    let mut not_goals = Vec::new(); // dead camps of a group that's still partly alive
    let mut damaged = Vec::new();
    let mut rewards = Vec::new(); // capture tiles of cleared groups in sight, reward waiting
    let rally = super::rally::active()?;
    let mut rally_tiles = Vec::new(); // live camps of the rally group (rally.rs)
    let mut off_phase = Vec::new(); // available groups the phase skips: (index, nearest camp)
    let mut preferred_left = 0;
    let mut holding = false;
    for (index, group) in groups.iter().enumerate() {
        let info = &statics.groups[index];
        if !info.reachable {
            continue;
        }
        // Nearest any of its camps can be.
        let gap = distance(info.centre, here) - info.radius;
        let mut seen = false;
        let mut any_live = false;
        let mut seen_dead = Vec::new(); // camps we can see are gone
        if gap <= vision {
            for camp in group.camps() {
                let spawn = camp.spawn_position();
                if !sight.visible.is_set(spawn)? {
                    continue;
                }
                seen = true;
                if sight.camps.iter().any(|c| c.spawn_position() == spawn) {
                    any_live = true;
                } else {
                    seen_dead.push(spawn);
                }
            }
        }
        if any_live {
            ready[index] = 0;
        }
        // A rally ends once its group is cleared (reward waiting) or respawning.
        if rally == Some(index) && ((seen && !any_live) || round < ready[index]) {
            super::rally::end(index)?;
        }
        if round < ready[index] {
            if previous_group == Some(index) {
                previous_target_reason = Some("group-respawning");
            }
            dropped.push(index);
            continue;
        }
        // Only a seen group with no live camps can be respawning or waiting for capture.
        let mut reward = false;
        if seen && !any_live {
            match group.state()? {
                cambc::CampGroupState::Respawning(timer) => {
                    ready[index] = round + timer.max(0) as u32;
                    if previous_group == Some(index) {
                        previous_target_reason = Some("group-respawning");
                    }
                    dropped.push(index);
                    continue;
                }
                cambc::CampGroupState::RewardAvailable if is_tank && carry_covers(group)? => {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("ally-carry-captures-reward");
                    }
                    dropped.push(index);
                    continue;
                }
                // An ally already on the capture region is collecting this reward; don't path to
                // the occupied capture tile or compete for the split.
                cambc::CampGroupState::RewardAvailable if ally_on_group(group)? => {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("ally-captures-reward");
                    }
                    dropped.push(index);
                    continue;
                }
                cambc::CampGroupState::RewardAvailable => reward = true,
                _ => {}
            }
        }
        // A cleared group has nothing left to fight: a loss judged while it was alive doesn't count.
        let mut unwinnable = !reward
            && (super::collect::recently_lost(index, round)
                || (any_live && super::collect::camp_fight_winnable(index)? == Some(false)));
        // An elk or monolith group we'd beat with a few more allies: call them (rally.rs).
        let rally_helpers = if unwinnable && any_live && rally.is_none() {
            super::rally::max_helpers(group, round)?
        } else {
            None
        };
        if let Some(max) = rally_helpers.filter(|_| super::rally::worth_checking(index, round)) {
            let mut winnable = false;
            for extra in 1..=max {
                if super::collect::winnable_with_standins(index, extra)? {
                    winnable = true;
                    break;
                }
            }
            if winnable {
                super::rally::call(index)?;
            } else {
                super::rally::checked(index, round);
            }
        }
        // A rally group stays a goal while we wait for allies.
        let rallying = super::rally::active()? == Some(index);
        if rallying {
            unwinnable = false;
        }
        if unwinnable && previous_group == Some(index) {
            previous_target_reason = Some("camp-fight-unwinnable");
        }
        // While part of the group is alive, only its live (or unseen) camps are goals. A dead camp's
        // tile only becomes a goal once the whole group is cleared (standing there captures it):
        // otherwise a melee champion standing on one counts as having arrived and never walks over
        // to the live camp out of reach (seen stuck for 65 turns).
        let is_goal = |c: &Camp| !(any_live && seen_dead.contains(&c.spawn_position()));
        if gap <= max_reach + vision
            && !enemy_reach.is_empty()
            && group.camps().iter().filter(|c| is_goal(c)).any(|c| {
                enemy_reach
                    .iter()
                    .any(|&(p, r)| distance(p, c.spawn_position()) <= r)
            })
        {
            if previous_group == Some(index) {
                previous_target_reason = Some("enemy-can-reach-group");
            }
            dropped.push(index);
            continue;
        }
        if unwinnable {
            dropped.push(index);
            continue;
        }
        if let Some(tile) = previous_target.filter(|tile| {
            group
                .camps()
                .iter()
                .any(|camp| camp.spawn_position() == *tile)
        }) {
            if any_live && seen_dead.contains(&tile) {
                previous_target_reason = Some("camp-dead-while-group-still-active");
            }
        }
        if reward && group.camps().iter().any(|c| c.spawn_position() == here) {
            holding = super::collect::CAPTURE_HOLD;
        }
        if any_live {
            not_goals.extend(seen_dead.iter().copied());
        }
        available[index] = true;
        if rallying {
            for camp in group.camps().iter().filter(|c| is_goal(c)) {
                rally_tiles.push(camp.spawn_position());
            }
        }
        if !info.preferred[phase] {
            off_phase.push((index, gap));
        }
        if reward {
            for camp in group.camps() {
                rewards.push(camp.spawn_position());
            }
        }
        if info.preferred[phase] {
            preferred_left += 1;
        }
        if gap <= DAMAGED_CAMP_RADIUS {
            damaged.extend(
                damaged_camps
                    .iter()
                    .filter(|&&(group_index, tile)| {
                        group_index == index && distance(tile, here) <= DAMAGED_CAMP_RADIUS
                    })
                    .map(|&(_, tile)| tile),
            );
        }
    }
    RESPAWN_READY.with(|cell| cell.replace(ready));

    let mut goals;
    let selection_reason;
    if holding {
        selection_reason = "capture-hold";
        goals = Bitboard::new()?;
        goals.set(here, true)?;
    } else if !rewards.is_empty() {
        // A reward we can see is taken, whatever the phase: otherwise a cleared gold cluster in
        // the gem phase was no goal, nobody captured it, and it never respawned (ambush: both
        // middle clusters, for 1,000+ rounds). Enemies that could reach it drop it above, unless
        // we'd win the 1v1 (plan_duel).
        selection_reason = "reward-in-sight";
        goals = Bitboard::new()?;
        for &tile in &rewards {
            goals.set(tile, true)?;
        }
    } else if !rally_tiles.is_empty()
        && me()?.health()? >= super::rally::RALLY_MIN_HEALTH * me()?.max_health()?
    {
        selection_reason = "rally";
        goals = Bitboard::new()?;
        for &tile in &rally_tiles {
            goals.set(tile, true)?;
        }
        // Ranged champions wait at the edge of their attack range from the nearest elk rather
        // than next to it: in range to shoot when the fight starts, but as far back as that allows.
        let range = me()?.attack_range()?;
        if range > 1 {
            let (mut within, mut closer) = (Bitboard::new()?, Bitboard::new()?);
            for &tile in &rally_tiles {
                within.draw_chebyshev(tile, range as f32)?;
                closer.draw_chebyshev(tile, (range - 1) as f32)?;
            }
            let mut ring = within.intersect(&closer.complement()?)?;
            ring.intersect_assign(&super::world::blocked_tiles()?.complement()?)?;
            if !ring.is_empty()? {
                goals = ring;
            }
        }
    } else {
        selection_reason = if FARM_PHASE_FILTER && preferred_left > 0 {
            "preferred-phase-goals"
        } else {
            "ordinary-farm-goals"
        };
        // Every camp (of the phase's preferred groups, if any is available), minus what's dropped.
        goals = if FARM_PHASE_FILTER && preferred_left > 0 {
            statics.preferred[phase].clone()
        } else {
            statics.all.clone()
        };
        for index in dropped {
            for camp in groups[index].camps() {
                goals.set(camp.spawn_position(), false)?;
            }
        }
        if FARM_PHASE_FILTER && preferred_left > 0 && phase == 1 && GEM_PHASE_GOLD_DETOUR > 0 {
            let nearest = goals
                .positions()?
                .iter()
                .map(|&tile| distance(here, tile))
                .min()
                .unwrap_or(i32::MAX);
            for &(index, gap) in &off_phase {
                if gap <= nearest.saturating_add(GEM_PHASE_GOLD_DETOUR) {
                    for camp in groups[index].camps() {
                        goals.set(camp.spawn_position(), true)?;
                    }
                }
            }
        }
        for tile in not_goals {
            goals.set(tile, false)?;
        }
    }
    let previous_was_regular_goal = match previous_target {
        Some(tile) => goals.is_set(tile)?,
        None => false,
    };
    if !holding && rewards.is_empty() && selection_reason != "rally" && !damaged.is_empty() {
        let nearest_regular = goals
            .positions()?
            .iter()
            .map(|&tile| distance(here, tile))
            .min();
        let nearest_damaged = damaged
            .iter()
            .copied()
            .min_by_key(|&tile| distance(here, tile));
        let damaged_distance = nearest_damaged.map_or(i32::MAX, |tile| distance(here, tile));
        if nearest_regular
            .is_none_or(|regular| damaged_distance <= regular + DAMAGED_CAMP_MAX_DETOUR)
        {
            goals = Bitboard::new()?;
            if let Some(tile) = nearest_damaged {
                goals.set(tile, true)?;
            }
            if previous_was_regular_goal {
                if let Some(old) = previous_target.filter(|old| Some(*old) != nearest_damaged) {
                    previous_target_reason = Some(if damaged.contains(&old) {
                        "closer-damaged-camp"
                    } else {
                        "damaged-camp-priority"
                    });
                }
            }
        } else if previous_was_regular_goal
            && previous_target.is_some_and(|old| damaged.contains(&old))
        {
            previous_target_reason = Some("damaged-camp-too-far-from-regular-target");
        }
    }
    if let Some(old) = previous_target {
        if !goals.is_set(old)? && previous_target_reason.is_none() {
            previous_target_reason = Some(selection_reason);
        }
    }
    avoid_abandoned_farm_targets(
        &mut goals,
        round,
        here,
        me()?.attack_range()?,
        previous_target_reason,
    )?;
    let plan = std::rc::Rc::new(FarmPlan { goals });
    FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
    PLAN_REUSE.with(|c| *c.borrow_mut() = key.map(|key| (round, key, plan.clone())));
    Ok(plan)
}

fn skip_unreachable_target(round: u32) {
    FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        if let Some(target) = memory.current.take() {
            memory
                .abandoned
                .push((target, round + UNREACHABLE_TARGET_ROUNDS));
        }
    });
}

// Clear camps and capture their rewards, following farm_plan.
pub fn farm_camps() -> Result<()> {
    let attacked = attack_camp_in_range()?;
    if attacked {
        cast_abilities_on_camps(false)?;
        return Ok(());
    }
    let plan = farm_plan()?;
    let moved = move_towards_closest(plan.goals.clone())?;
    if !moved && goals_were_unreachable() {
        super::trace::trace("farm-unreachable");
        skip_unreachable_target(cambc::round_number()?);
    }
    if moved {
        if attack_camp_in_range()? {
            cast_abilities_on_camps(true)?;
        }
    }
    Ok(())
}
