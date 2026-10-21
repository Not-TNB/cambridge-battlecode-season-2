// Choosing where to farm: phase filter, respawn memory, avoiding enemies and the farm plan.
use super::attack::cast_abilities_on_camps;
use super::camps::{
    ally_on_group, attack_camp_in_range, carry_covers, FUNNEL_HEALERS, TANK_FUNNEL,
};
use super::duel::{cached_fight_view, fight_view, plan_duel};
use super::me;
use super::movement::{goals_were_unreachable, move_towards_closest};
use super::trace::{checkpoint, FARM_TARGET_DIAGNOSTICS};
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

// Narrow `goals` to one target: keep the current one while it's still a goal, else the nearest on
// foot (`steps`) that isn't on cooldown.
// Fix 5: ranged champions (fragile carries) count camp groups about as close to the enemy's spawn
// as to ours (within CONTESTED_MARGIN tiles) as this many steps further when picking a target:
// both teams meet there (hedgerows: 41% of Prism's targets were the two middle cluster groups, and
// most of its deaths were on the way there or back). 0: off.
pub const CONTESTED_FARM_PENALTY: i32 = 8;
const CONTESTED_MARGIN: i32 = 2;

fn contested_penalty() -> Result<impl Fn(Vec2) -> i32> {
    let statics = farm_statics()?;
    let ranged = me()?.attack_range()? > 1;
    Ok(move |tile: Vec2| {
        if CONTESTED_FARM_PENALTY <= 0 || !ranged {
            return 0;
        }
        match group_at(tile) {
            Ok(Some(index)) if statics.groups[index].contested => CONTESTED_FARM_PENALTY,
            _ => 0,
        }
    })
}

fn avoid_abandoned_farm_targets(
    goals: &mut Bitboard,
    round: u32,
    here: Vec2,
    reach: i32,
    unavailable_reason: Option<&'static str>,
    steps: &super::walk::StepsFrom,
) -> Result<()> {
    let penalty = contested_penalty()?;
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
            steps.nearest_with(&candidates_for_target, &penalty)
        } else {
            let current_is_reachable = memory
                .current
                .is_some_and(|tile| distance(here, tile) <= reach);
            if !current_is_reachable {
                if let Some(current) = memory.current.filter(|tile| available.contains(tile)) {
                    Some(current)
                } else {
                    steps.nearest_with(&available, &penalty)
                }
            } else {
                steps.nearest_with(&available, &penalty)
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

    // The goals become just the target (clearing every other candidate a tile at a time cost two
    // host calls each); with no target they stay as they are and the cooldowns are dropped.
    match target {
        Some(tile) => {
            *goals = Bitboard::new()?;
            goals.set(tile, true)?;
        }
        None => FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().abandoned.clear()),
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
    gold_only: bool,      // only gold clusters (the Cluster opening, opening.rs)
    contested: bool,      // about as close to the enemy's spawn as to ours (CONTESTED_FARM_PENALTY)
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
            gold_only: group
                .camps()
                .iter()
                .all(|c| c.kind() == cambc::CampType::GoldCluster),
            contested: distance(centre, super::base::enemy_spawn_centre()?)
                <= distance(centre, super::base::ally_spawn_centre()?) + CONTESTED_MARGIN,
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

#[derive(PartialEq, Clone)]
struct PlanKey {
    rally: Option<usize>,
    opening: (bool, Option<usize>, bool), // Group on, its team target, Cluster on
    here: Vec2,
    hp: i32,
    phase: bool,
    camps: Vec<(Vec2, i32)>,
    enemies: Vec<Vec2>,
    allies: Vec<Vec2>,
}

// Fuel fix 1: a rally's join-or-skip decision (the steps to it plus RALLY_WAIT_STEPS against the
// nearest other camp) is kept this many rounds instead of worked out every plan (~22k fuel each
// time a rally was active). 0: every plan. Alone on 8 rally maps: 12% fewer fuel-exhausted turns;
// the kept decision differed from a fresh one on 1.7% of uses.
pub const RALLY_DECISION_ROUNDS: u32 = 5;

thread_local! {
    // (rally group, round the decision expires, whether we join)
    static RALLY_DECISION: std::cell::Cell<Option<(usize, u32, bool)>> = const { std::cell::Cell::new(None) };
}

thread_local! {
    static PLAN_REUSE: std::cell::RefCell<Option<(u32, PlanKey, std::rc::Rc<FarmPlan>)>> =
        const { std::cell::RefCell::new(None) };
}

// Walking reuse: on the way to goals out of sight, the last plan is kept while nothing it looks at
// in sight has changed (the camp tiles we see and the live camps' health, the rally, the team
// target, our health, the phase) and no enemy is in sight, for up to WALK_REUSE_ROUNDS rounds. The
// key above includes our tile, so a plan (~36k fuel on average, ~60k at the 90th percentile, of a
// 100k turn) was rebuilt on 93% of farming turns, mostly while walking.
pub const WALK_REUSE: bool = true;
const WALK_REUSE_ROUNDS: u32 = 10;

struct WalkSnapshot {
    built: u32,
    key: PlanKey,
    seen: Bitboard, // camp tiles in sight, live or not (a cleared group coming into view)
    plan: std::rc::Rc<FarmPlan>,
}

thread_local! {
    static WALK_PLAN: std::cell::RefCell<Option<WalkSnapshot>> = const { std::cell::RefCell::new(None) };
}

fn seen_camp_tiles() -> Result<Bitboard> {
    sight()?.visible.intersect(&farm_statics()?.all)
}

// Whether any of `goals` is in sight.
fn goals_in_sight(goals: &Bitboard) -> Result<bool> {
    Ok(!goals.intersect(&sight()?.visible)?.is_empty()?)
}

fn walking_reuse(round: u32, key: &PlanKey) -> Result<Option<std::rc::Rc<FarmPlan>>> {
    if !WALK_REUSE || !key.enemies.is_empty() {
        return Ok(None);
    }
    let candidate = WALK_PLAN.with(|c| {
        c.borrow().as_ref().and_then(|s| {
            let k = &s.key;
            (round < s.built + WALK_REUSE_ROUNDS
                && k.enemies.is_empty()
                && (k.rally, k.opening, k.hp, k.phase) == (key.rally, key.opening, key.hp, key.phase)
                && k.camps == key.camps)
                .then(|| (s.seen.clone(), s.plan.clone()))
        })
    });
    let Some((seen, plan)) = candidate else {
        return Ok(None);
    };
    // Goals in sight need a fresh plan (fight checks, captures); so do new camp tiles in sight.
    if goals_in_sight(&plan.goals)? || seen != seen_camp_tiles()? {
        return Ok(None);
    }
    Ok(Some(plan))
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
    let group = crate::opening::group_active()?;
    Ok(PlanKey {
        rally: super::rally::active()?,
        opening: (
            group,
            if group { crate::opening::team_target()? } else { None },
            crate::opening::cluster_active()?,
        ),
        here,
        hp: me()?.health()? as i32,
        phase: round >= FARM_GEM_PHASE_ROUND,
        camps,
        enemies,
        allies,
    })
}

// Far-group cache (see the group loop in farm_plan): the bookkeeping for groups well out of sight
// is kept for up to FAR_GROUP_CACHE_ROUNDS rounds while we've moved at most FAR_CACHE_MOVE tiles
// and no enemy is in reach, and dropped as soon as any of those groups' respawn timer or loss
// memory runs out (those timers are all that can change a group we can't see). Even the fast path
// cost ~250 fuel a group, 12-15k a plan on maps with 50-60 groups. 0: off.
pub const FAR_GROUP_CACHE_ROUNDS: u32 = 5;
const FAR_CACHE_MOVE: i32 = 3;

#[derive(Clone)]
struct FarCache {
    origin: Vec2,
    phase: usize,
    cluster_mode: bool,
    valid_until: u32,
    far: Vec<bool>,
    dropped: Vec<usize>,
    available: Vec<usize>,
    clusters: Vec<(usize, i32)>,
    preferred: u32,
    off_phase: Vec<(usize, i32)>,
}

thread_local! {
    static FAR_CACHE: std::cell::RefCell<Option<FarCache>> = const { std::cell::RefCell::new(None) };
}

// Drop the far-group cache (the loss memory it relied on was just cleared, collect::forget_lost_camps).
pub fn forget_far_cache() {
    FAR_CACHE.with(|c| *c.borrow_mut() = None);
}

// Fuel fix 4 (see the group loop in farm_plan). Off: every group takes the full pass. Alone on the
// 6 maps with the most groups: 14% fewer fuel-exhausted turns (group loop 23k -> 20k there).
pub const FAR_GROUP_FAST_PATH: bool = true;

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
    if let Some(key) = &key {
        if let Some(plan) = walking_reuse(round, key)? {
            FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
            super::trace::trace("farm-plan-walking");
            return Ok(plan);
        }
    }
    checkpoint("plan-key");
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
    checkpoint("plan-enemies");
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
    // The opening (opening.rs): in Group mode the team farms one shared target group and funnels
    // captures to a damage dealer; in Cluster mode champions head for gold clusters.
    let opening_group = crate::opening::group_active()?;
    let opening_cluster = crate::opening::cluster_active()?;
    // The rally and team target as they stand, re-read only when this loop changes them (reading
    // them per group cost a host call each).
    let mut rally_now = rally;
    let mut team_now = if opening_group {
        crate::opening::team_target()?
    } else {
        None
    };
    let mut team_tiles = Vec::new(); // live camps of the team target group
    let mut clusters = Vec::new(); // available gold-cluster groups: (index, nearest camp)
    // The far-group cache (FAR_GROUP_CACHE_ROUNDS): use it if still good, else build a fresh one.
    let cache_usable = FAR_GROUP_CACHE_ROUNDS > 0 && FAR_GROUP_FAST_PATH && enemy_reach.is_empty();
    let far_cache = if cache_usable {
        FAR_CACHE.with(|c| c.borrow().clone()).filter(|c| {
            round < c.valid_until
                && distance(c.origin, here) <= FAR_CACHE_MOVE
                && c.phase == phase
                && c.cluster_mode == opening_cluster
                && rally_now.is_none_or(|g| !c.far.get(g).copied().unwrap_or(false))
                && team_now.is_none_or(|g| !c.far.get(g).copied().unwrap_or(false))
        })
    } else {
        None
    };
    let mut fresh = (cache_usable && far_cache.is_none()).then(|| FarCache {
        origin: here,
        phase,
        cluster_mode: opening_cluster,
        valid_until: round + FAR_GROUP_CACHE_ROUNDS,
        far: vec![false; groups.len()],
        dropped: Vec::new(),
        available: Vec::new(),
        clusters: Vec::new(),
        preferred: 0,
        off_phase: Vec::new(),
    });
    for (index, group) in groups.iter().enumerate() {
        let info = &statics.groups[index];
        if !info.reachable {
            continue;
        }
        if far_cache.as_ref().is_some_and(|c| c.far[index]) {
            continue;
        }
        // Nearest any of its camps can be.
        let gap = distance(info.centre, here) - info.radius;
        // Fuel fix 4: a group out of sight, not the rally or team target and out of any enemy's
        // reach only needs its respawn and loss memory checked and its goal bookkeeping done; the
        // full pass below costs ~500 fuel a group, 15-25k a plan on maps with 40-50 groups.
        if FAR_GROUP_FAST_PATH
            && gap > vision
            && gap > DAMAGED_CAMP_RADIUS
            && (enemy_reach.is_empty() || gap > max_reach + vision)
            && rally != Some(index)
            && rally_now != Some(index)
            && team_now != Some(index)
        {
            let respawning = round < ready[index];
            // Record it in a fresh far-group cache if it'll stay far while we move FAR_CACHE_MOVE.
            let cached = fresh.as_mut().filter(|_| {
                gap > vision + FAR_CACHE_MOVE
                    && gap > DAMAGED_CAMP_RADIUS + FAR_CACHE_MOVE
                    && rally != Some(index)
            });
            if respawning || super::collect::recently_lost(index, round) {
                if previous_group == Some(index) {
                    previous_target_reason = Some(if respawning {
                        "group-respawning"
                    } else {
                        "camp-fight-unwinnable"
                    });
                }
                dropped.push(index);
                if let Some(c) = cached {
                    c.far[index] = true;
                    c.dropped.push(index);
                    let until = if respawning { ready[index] } else { super::collect::lost_until(index) };
                    c.valid_until = c.valid_until.min(until.max(round + 1));
                }
                continue;
            }
            available[index] = true;
            if opening_cluster && info.gold_only {
                clusters.push((index, gap));
            }
            if info.preferred[phase] {
                preferred_left += 1;
            } else {
                off_phase.push((index, gap));
            }
            if let Some(c) = cached {
                c.far[index] = true;
                c.available.push(index);
                if opening_cluster && info.gold_only {
                    c.clusters.push((index, gap));
                }
                if info.preferred[phase] {
                    c.preferred += 1;
                } else {
                    c.off_phase.push((index, gap));
                }
            }
            continue;
        }
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
            rally_now = super::rally::active()?;
        }
        // So does the team target; whoever plans next sets a new one.
        if team_now == Some(index) && ((seen && !any_live) || round < ready[index]) {
            crate::opening::end_team_target(index)?;
            team_now = crate::opening::team_target()?;
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
                cambc::CampGroupState::RewardAvailable
                    if opening_group && crate::opening::funnel_elsewhere(group)? =>
                {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("opening-funnel");
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
        // Some members dead but the group not cleared: the rest are alive out of our sight (a bush).
        let hidden_alive = super::collect::PARTIAL_GROUPS && seen && !any_live && !reward;
        let partly = super::collect::PARTIAL_GROUPS && (!seen_dead.is_empty() || damaged_camps.iter().any(|&(g, _)| g == index));
        let mut unwinnable = !reward
            && ((super::collect::recently_lost(index, round) && !partly)
                || (any_live && super::collect::camp_fight_winnable(index)? == Some(false)));
        // An elk or monolith group we'd beat with a few more allies: call them (rally.rs). Follow-only
        // escorts don't call rallies, only answer them (collect::ESCORT_FOLLOW_ONLY).
        let rally_helpers = if unwinnable
            && any_live
            && rally.is_none()
            && !super::collect::follow_only()?
        {
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
                rally_now = super::rally::active()?;
            } else {
                super::rally::checked(index, round);
            }
        }
        // A rally group stays a goal while we wait for allies.
        let rallying = rally_now == Some(index);
        // The team target too, while the team gathers (it attacks once the fight is winnable).
        let teaming = !rallying && team_now == Some(index);
        if rallying || teaming {
            unwinnable = false;
        }
        if unwinnable && previous_group == Some(index) {
            previous_target_reason = Some("camp-fight-unwinnable");
        }
        // While part of the group is alive, only its live (or unseen) camps are goals. A dead camp's
        // tile only becomes a goal once the whole group is cleared (standing there captures it):
        // otherwise a melee champion standing on one counts as having arrived and never walks over
        // to the live camp out of reach (seen stuck for 65 turns).
        // With its live members out of sight, its goals are the tiles we can't see, to find them.
        let is_goal = |c: &Camp| {
            !((any_live || hidden_alive) && seen_dead.contains(&c.spawn_position()))
        };
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
        if any_live || hidden_alive {
            not_goals.extend(seen_dead.iter().copied());
        }
        if hidden_alive {
            super::trace::trace("group-hidden-alive");
        }
        available[index] = true;
        if rallying {
            for camp in group.camps().iter().filter(|c| is_goal(c)) {
                rally_tiles.push(camp.spawn_position());
            }
        }
        if teaming {
            for camp in group.camps().iter().filter(|c| is_goal(c)) {
                team_tiles.push(camp.spawn_position());
            }
        }
        if opening_cluster && info.gold_only && !reward {
            clusters.push((index, gap));
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
    // Far groups from the cache (checked against a fresh pass with VERIFY_FUEL_CACHES).
    if let Some(cache) = far_cache {
        super::trace::trace("far-cache-used");
        if super::trace::VERIFY_FUEL_CACHES {
            let mut wrong = false;
            for &index in &cache.dropped {
                wrong |= !(round < ready[index] || super::collect::recently_lost(index, round));
            }
            for &index in &cache.available {
                wrong |= round < ready[index] || super::collect::recently_lost(index, round);
            }
            if wrong {
                super::trace::trace("far-cache-mismatch");
            }
        }
        dropped.extend(cache.dropped.iter().copied());
        for &index in &cache.available {
            available[index] = true;
        }
        clusters.extend(cache.clusters.iter().copied());
        preferred_left += cache.preferred;
        off_phase.extend(cache.off_phase.iter().copied());
        FAR_CACHE.with(|c| *c.borrow_mut() = Some(cache));
    } else if let Some(fresh) = fresh {
        FAR_CACHE.with(|c| *c.borrow_mut() = Some(fresh));
    }
    RESPAWN_READY.with(|cell| cell.replace(ready));
    checkpoint("plan-groups");

    // Ranking goals by walking distance (walk.rs): built on the first lookup, if any.
    let steps = super::walk::StepsFrom::new(here);
    // Every camp (of the phase's preferred groups, if any is available), minus what's dropped.
    let regular_goals = || -> Result<(Bitboard, &'static str)> {
        let reason = if FARM_PHASE_FILTER && preferred_left > 0 {
            "preferred-phase-goals"
        } else {
            "ordinary-farm-goals"
        };
        let mut goals = if FARM_PHASE_FILTER && preferred_left > 0 {
            statics.preferred[phase].clone()
        } else {
            statics.all.clone()
        };
        checkpoint("goals-base");
        for &index in &dropped {
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
        checkpoint("goals-dropped");
        for &tile in &not_goals {
            goals.set(tile, false)?;
        }
        checkpoint("goals-not-goals");
        // Only in the gold phase: past it, clusters pulled champions off gem camps (trenches went
        // from 7/10 to 1/10).
        if !clusters.is_empty() && phase == 0 {
            prefer_clusters(&mut goals, &clusters, here)?;
        }
        Ok((goals, reason))
    };
    // A rally only wins if it's no further, counting RALLY_WAIT_STEPS for the wait, than the
    // nearest camp we'd farm without it (the rally group itself left out). Decided once per rally
    // and kept for RALLY_DECISION_ROUNDS rounds (fuel fix 1).
    let mut rally_regular = None;
    let mut rally_skipped = false;
    // Follow-only escorts (collect::ESCORT_FOLLOW_ONLY) always answer a rally: they have no camps
    // of their own to compare it with.
    let follow_only = super::collect::follow_only()?;
    let rally_wanted = !rally_tiles.is_empty()
        && me()?.health()? >= super::rally::RALLY_MIN_HEALTH * me()?.max_health()?
        && (follow_only || {
            let cached = if RALLY_DECISION_ROUNDS > 0 {
                RALLY_DECISION
                    .with(|c| c.get())
                    .filter(|&(group, until, _)| Some(group) == rally_now && round < until)
                    .map(|(_, _, wanted)| wanted)
            } else {
                None
            };
            if cached.is_some() {
                super::trace::trace("rally-cached");
            }
            let wanted = match cached {
                Some(wanted) if !super::trace::VERIFY_FUEL_CACHES => wanted,
                _ => {
                    let (mut regular, reason) = regular_goals()?;
                    for &tile in &rally_tiles {
                        regular.set(tile, false)?;
                    }
                    let rally_steps =
                        steps.nearest(&rally_tiles).map_or(i32::MAX, |t| steps.steps(t));
                    let regular_steps =
                        steps.nearest(&regular.positions()?).map(|t| steps.steps(t));
                    let better = super::rally::RALLY_WAIT_STEPS > 0
                        && regular_steps.is_some_and(|r| {
                            r < rally_steps.saturating_add(super::rally::RALLY_WAIT_STEPS)
                        });
                    if better {
                        rally_regular = Some((regular, reason));
                    }
                    match cached {
                        Some(wanted) => {
                            if wanted == better {
                                super::trace::trace("rally-cache-mismatch");
                            }
                            wanted
                        }
                        None => {
                            if let Some(group) = rally_now {
                                RALLY_DECISION.with(|c| {
                                    c.set(Some((group, round + RALLY_DECISION_ROUNDS, !better)))
                                });
                            }
                            !better
                        }
                    }
                }
            };
            if !wanted {
                super::trace::trace("rally-skip-closer-camp");
                rally_skipped = true;
            }
            wanted
        });
    checkpoint("plan-rally-decision");
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
    } else if rally_wanted {
        selection_reason = "rally";
        super::rally::join()?;
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
    } else if !team_tiles.is_empty()
        && me()?.health()? >= super::rally::RALLY_MIN_HEALTH * me()?.max_health()?
    {
        selection_reason = "team-target";
        goals = Bitboard::new()?;
        for &tile in &team_tiles {
            goals.set(tile, true)?;
        }
    } else if follow_only {
        selection_reason = "escort-follow";
        goals = super::collect::escort_follow_goals()?;
    } else {
        let (mut regular, reason) = match rally_regular {
            Some(chosen) => chosen,
            None => regular_goals()?,
        };
        if rally_skipped {
            for &tile in &rally_tiles {
                regular.set(tile, false)?;
            }
        }
        goals = regular;
        selection_reason = reason;
    }
    checkpoint("plan-goals");
    let previous_was_regular_goal = match previous_target {
        Some(tile) => goals.is_set(tile)?,
        None => false,
    };
    if !holding
        && rewards.is_empty()
        && !matches!(selection_reason, "rally" | "team-target")
        && !damaged.is_empty()
    {
        let nearest_regular = steps
            .nearest(&goals.positions()?)
            .map(|tile| steps.steps(tile));
        let nearest_damaged = steps.nearest(&damaged);
        let damaged_distance = nearest_damaged.map_or(i32::MAX, |tile| steps.steps(tile));
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
    checkpoint("plan-damaged");
    if selection_reason == "escort-follow" {
        // Following an ally isn't a farm target: no target memory or cooldowns.
        FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().current = None);
    } else {
        avoid_abandoned_farm_targets(
            &mut goals,
            round,
            here,
            me()?.attack_range()?,
            previous_target_reason,
            &steps,
        )?;
    }
    // Group opening with no team target: ours becomes the team's.
    if opening_group && !holding && rewards.is_empty() && selection_reason != "rally" {
        let target = FARM_TARGET_MEMORY.with(|memory| memory.borrow().current);
        if let Some(index) = match target {
            Some(tile) => group_at(tile)?,
            None => None,
        } {
            crate::opening::set_team_target(index)?;
        }
    }
    checkpoint("plan-target");
    let plan = std::rc::Rc::new(FarmPlan { goals });
    FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
    // Only a plan heading out of sight can be kept while walking (and only without enemies).
    if WALK_REUSE {
        let snapshot = match &key {
            // A follow plan chases a moving ally: never kept while walking.
            Some(key)
                if key.enemies.is_empty()
                    && selection_reason != "escort-follow"
                    && !goals_in_sight(&plan.goals)? =>
            {
                Some(WalkSnapshot {
                    built: round,
                    key: key.clone(),
                    seen: seen_camp_tiles()?,
                    plan: plan.clone(),
                })
            }
            _ => None,
        };
        WALK_PLAN.with(|c| *c.borrow_mut() = snapshot);
    }
    PLAN_REUSE.with(|c| *c.borrow_mut() = key.map(|key| (round, key, plan.clone())));
    Ok(plan)
}

// While we wait to fight a camp group (not winnable yet: allies still coming), don't stand where its
// awake camps can hit us: goal tiles within an awake camp's range become the nearest free tiles just
// outside it, and if we're inside one already we step out. Camps don't chase, so waiting there costs
// nothing; standing next to one we weren't attacking just took its hits. Once the fight is
// winnable, attack_camp_in_range engages as before. Off: wait where the plan says.
pub const WAIT_OUTSIDE_AWAKE_CAMPS: bool = true;
// Fuel fix 6. 0: always check. Small: the check went from 2.6k to 1.9k on average.
pub const WAIT_PRECHECK_TILES: i32 = 7;

fn wait_outside_awake_camps(goals: Bitboard) -> Result<Bitboard> {
    if !WAIT_OUTSIDE_AWAKE_CAMPS {
        return Ok(goals);
    }
    let sight = sight()?;
    // Fuel fix 6: no camp within WAIT_PRECHECK_TILES (beyond the longest camp range plus a step)
    // means nothing to wait outside of yet; checked with plain distances before any host call.
    let here = me()?.position()?;
    if WAIT_PRECHECK_TILES > 0
        && !sight
            .camps
            .iter()
            .any(|c| distance(c.spawn_position(), here) <= WAIT_PRECHECK_TILES)
    {
        return Ok(goals);
    }
    let mut danger = Bitboard::new()?;
    let mut reach = 0;
    for camp in &sight.camps {
        if !camp.is_awake().unwrap_or(false) {
            continue;
        }
        let Some(index) = group_at(camp.spawn_position())? else {
            continue;
        };
        if super::collect::camp_fight_winnable(index)? == Some(true) {
            continue;
        }
        let range = camp.attack_range()?;
        danger.draw_chebyshev(camp.spawn_position(), range as f32)?;
        reach = reach.max(range);
    }
    if danger.is_empty()? {
        return Ok(goals);
    }
    let goals_in_danger = goals.intersect(&danger)?;
    if goals_in_danger.is_empty()? && !danger.is_set(here)? {
        return Ok(goals);
    }
    let free = super::world::blocked_tiles()?.union(&danger)?.complement()?;
    let mut outside = Bitboard::new()?;
    for tile in goals_in_danger.positions()? {
        outside.draw_chebyshev(tile, (reach + 1) as f32)?;
    }
    outside.intersect_assign(&free)?;
    if outside.is_empty()? {
        // No goal near the danger: just get out of it.
        outside = super::world::tiles_around(here, 2)?.intersect(&free)?;
    }
    if outside.is_empty()? {
        return Ok(goals);
    }
    super::trace::trace("wait-outside-camp");
    Ok(outside)
}

// Cluster opening: gold clusters within CLUSTER_DETOUR tiles of the nearest goal become the only
// goals, except ones an ally in sight is closer to (a cluster needs no help, and a shared capture
// is split). Nothing changes when no cluster qualifies.
const CLUSTER_DETOUR: i32 = 8;

fn prefer_clusters(goals: &mut Bitboard, clusters: &[(usize, i32)], here: Vec2) -> Result<()> {
    let statics = farm_statics()?;
    let groups = camp_groups()?;
    let nearest = goals
        .positions()?
        .iter()
        .map(|&tile| distance(here, tile))
        .min()
        .unwrap_or(i32::MAX);
    let allies = &sight()?.allies;
    let mut chosen = Bitboard::new()?;
    for &(index, gap) in clusters {
        if gap > nearest.saturating_add(CLUSTER_DETOUR) {
            continue;
        }
        let centre = statics.groups[index].centre;
        let mut taken = false;
        for ally in allies {
            if distance(ally.position()?, centre) < distance(here, centre) {
                taken = true;
                break;
            }
        }
        if taken {
            continue;
        }
        for camp in groups[index].camps() {
            if goals.is_set(camp.spawn_position())? {
                chosen.set(camp.spawn_position(), true)?;
            }
        }
    }
    if !chosen.is_empty()? {
        super::trace::trace("opening-clusters");
        *goals = chosen;
    }
    Ok(())
}

// Join an ally's camp fight (collect::ally_camp_fight_to_join): attack a camp in range, otherwise
// walk to the group's live camps and attack after the step if one is in range.
pub fn join_camp_fight(camps: &[Vec2]) -> Result<()> {
    if attack_camp_in_range()? {
        cast_abilities_on_camps(false)?;
        return Ok(());
    }
    let mut goals = Bitboard::new()?;
    for &tile in camps {
        goals.set(tile, true)?;
    }
    if move_towards_closest(goals)? {
        let me = me()?;
        let (here, range) = (me.position()?, me.attack_range()?);
        if camps.iter().any(|&c| distance(c, here) <= range) && attack_camp_in_range()? {
            cast_abilities_on_camps(true)?;
        }
    }
    Ok(())
}

fn skip_unreachable_target(round: u32) {
    // A kept plan would send us back to it.
    WALK_PLAN.with(|c| *c.borrow_mut() = None);
    PLAN_REUSE.with(|c| *c.borrow_mut() = None);
    FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        if let Some(target) = memory.current.take() {
            memory
                .abandoned
                .push((target, round + UNREACHABLE_TARGET_ROUNDS));
        }
    });
}

// On the old maps it kept the last plan on only 0.9% of farming turns (2% fewer fuel-exhausted
// turns); on the 3.0 maps turns run out far more (7-28%), mostly before the move. It only ever
// reuses the last plan's own goals, so a low-fuel turn carries on with what the previous full plan
// chose and can't flip the target back and forth. 0: off.
pub const FARM_PLAN_MIN_FUEL: u64 = 60_000;
// Below this, skip the wait-outside check (a few bitboards and fight checks) and just move.
const WAIT_CHECK_MIN_FUEL: u64 = 30_000;
const STALE_PLAN_ROUNDS: u32 = 10;

// Clear camps and capture their rewards, following farm_plan.
pub fn farm_camps() -> Result<()> {
    checkpoint("farm-start");
    let attacked = attack_camp_in_range()?;
    checkpoint("farm-attack-check");
    if attacked {
        cast_abilities_on_camps(false)?;
        checkpoint("farm-cast");
        return Ok(());
    }
    // Fuel fix 2: with less than FARM_PLAN_MIN_FUEL left, building a plan (~36k on average) left
    // too little for the move itself, the most common place a turn ran out of fuel (the champion
    // then stands still). Keep walking to the last plan's goals instead, if it's recent.
    let round = cambc::round_number()?;
    let stale = if FARM_PLAN_MIN_FUEL > 0 && cambc::fuel_remaining()? < FARM_PLAN_MIN_FUEL {
        FARM_PLAN.with(|c| {
            c.borrow()
                .as_ref()
                .filter(|(built, _, _)| round < built + STALE_PLAN_ROUNDS)
                .map(|(_, _, plan)| plan.clone())
        })
    } else {
        None
    };
    let plan = match stale {
        Some(plan) => {
            super::trace::trace("plan-low-fuel");
            plan
        }
        None => farm_plan()?,
    };
    checkpoint("farm-plan");
    // Camps don't move: the ones in sight before the step are the ones we might reach after it.
    let camps_before: Vec<Vec2> = sight()?.camps.iter().map(|c| c.spawn_position()).collect();
    let goals = if cambc::fuel_remaining()? >= WAIT_CHECK_MIN_FUEL {
        wait_outside_awake_camps(plan.goals.clone())?
    } else {
        plan.goals.clone()
    };
    checkpoint("farm-wait-outside");
    let moved = move_towards_closest(goals)?;
    if !moved && goals_were_unreachable() {
        super::trace::trace("farm-unreachable");
        skip_unreachable_target(cambc::round_number()?);
    }
    // Attack after the step only with a camp in range of where we now stand: the check rebuilds
    // sight and the camp-fight context for the new tile (~16k fuel), and this was where turns most
    // often ran out of fuel.
    if moved {
        let me = me()?;
        let (here, range) = (me.position()?, me.attack_range()?);
        if camps_before.iter().any(|&c| distance(c, here) <= range) && attack_camp_in_range()? {
            cast_abilities_on_camps(true)?;
        }
        checkpoint("farm-post-step");
    }
    Ok(())
}
