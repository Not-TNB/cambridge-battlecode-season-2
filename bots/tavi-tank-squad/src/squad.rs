use crate::champions::ChampionAi;
use crate::common::{self, me};
use cambc::{ChampionType, Result, Vec2};
use std::cell::Cell;

const MEMORY_MAGIC: u8 = 0x53;
const COMMAND_MAX_AGE: u32 = 6;
const TAVI_BANK_GEMS_THRESHOLD: u32 = 200;
const IDLE_FOLLOW_RADIUS: i32 = 5;
const RECALL_FINISH_MAX_ROUNDS: u32 = 10;
pub const TAVI_LATE_BUILD_ROUND: u32 = 800;

// Followers play on their own, with bot-v2's turn (v2/, play_turn in bot.rs), instead of following
// Tavi's commands: while Tavi is dead, and for the rest of the game from SOLO_ROUND, when Tavi can
// hold its own (it was past 3x its base Strength by round 500 in 95% of games, so a Strength test
// added nothing). Every champion knows the round, so it needs no signal. u32::MAX: never.
// 2026-10-20, 126 games each, wins against bot-v2 / against the squad without it: from 1500 65 / 81,
// from 1000 61 / 73 (most gems: 70.9k against bot-v2's 66.2k), never 58.
pub const SOLO_ROUND: u32 = 1500;
// Followers play on their own while Tavi is dead. Off: they wait at spawn (as they used to).
// Neutral (58 / 60 of 126, as above), with fewer Tavi deaths against bot-v2 (336 against 378).
pub const FREE_WHILE_TAVI_DEAD: bool = true;

// From SOLO_ROUND Tavi keeps its commands (its own recall's state) to itself and hands the shared
// memory over to the followers' bot-v2 code (rallies and so on): it clears it on its first turn of
// SOLO_ROUND, alive or dead, and the followers start using it HANDOVER_ROUNDS later.
const HANDOVER_ROUNDS: u32 = 2;

thread_local! {
    static LOCAL_COMMAND: Cell<[u8; 8]> = const { Cell::new([0; 8]) };
    static HANDED_OVER: Cell<bool> = const { Cell::new(false) };
}

fn solo() -> Result<bool> {
    Ok(cambc::round_number()? >= SOLO_ROUND)
}

// Whether the followers' bot-v2 code may use the shared memory (see HANDOVER_ROUNDS).
pub fn v2_owns_shared_memory() -> Result<bool> {
    Ok(SOLO_ROUND < u32::MAX - HANDOVER_ROUNDS
        && cambc::round_number()? >= SOLO_ROUND + HANDOVER_ROUNDS)
}

// Tavi, every turn (bot.rs): on the first one from SOLO_ROUND, clear the shared memory for the
// followers (see HANDOVER_ROUNDS).
pub fn hand_over_shared_memory() -> Result<()> {
    if is_leader()? && solo()? && !HANDED_OVER.with(Cell::get) {
        cambc::shared_memory().write(0, &[0; 8])?;
        HANDED_OVER.with(|h| h.set(true));
    }
    Ok(())
}

// The command bytes: the shared memory, or once solo, Tavi's own copy.
fn read_command_bytes() -> Result<[u8; 8]> {
    if solo()? {
        return Ok(LOCAL_COMMAND.with(Cell::get));
    }
    let mut bytes = [0; 8];
    cambc::shared_memory().read(0, &mut bytes)?;
    Ok(bytes)
}

fn write_command_bytes(bytes: [u8; 8]) -> Result<()> {
    if solo()? {
        LOCAL_COMMAND.with(|c| c.set(bytes));
        return Ok(());
    }
    cambc::shared_memory().write(0, &bytes)
}

#[derive(Clone, Copy)]
struct HealthSample {
    champion: ChampionType,
    health: f32,
}

thread_local! {
    static ALLY_HEALTH: std::cell::RefCell<Vec<HealthSample>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Mode {
    Follow = 1,
    Farm = 2,
    Enemy = 3,
    RecallFinishFarm = 4,
    RecallFinishEnemy = 5,
    RecallMove = 6,
    RecallNow = 7,
    TaviDead = 8,
}

impl Mode {
    fn from_byte(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Follow),
            2 => Some(Self::Farm),
            3 => Some(Self::Enemy),
            4 => Some(Self::RecallFinishFarm),
            5 => Some(Self::RecallFinishEnemy),
            6 => Some(Self::RecallMove),
            7 => Some(Self::RecallNow),
            8 => Some(Self::TaviDead),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Command {
    mode: Mode,
    target: Vec2,
    leader: Vec2,
}

thread_local! {
    static RECALL_STARTED: Cell<Option<u32>> = const { Cell::new(None) };
}

fn read_command() -> Result<Option<Command>> {
    let bytes = read_command_bytes()?;
    if bytes[0] != MEMORY_MAGIC {
        return Ok(None);
    }
    let Some(mode) = Mode::from_byte(bytes[1]) else {
        return Ok(None);
    };
    let round = u16::from_le_bytes([bytes[4], bytes[5]]) as u32;
    let age = cambc::round_number()?.wrapping_sub(round) & 0xffff;
    if age > COMMAND_MAX_AGE {
        return Ok(None);
    }
    Ok(Some(Command {
        mode,
        target: Vec2::new(bytes[2] as i32, bytes[3] as i32),
        leader: Vec2::new(bytes[6] as i32, bytes[7] as i32),
    }))
}

// Whether this follower plays on its own (bot-v2's turn, bot.rs) rather than following Tavi: from
// SOLO_ROUND, or while Tavi is dead (FREE_WHILE_TAVI_DEAD).
pub fn follower_free() -> Result<bool> {
    if is_leader()? {
        return Ok(false);
    }
    if solo()? {
        return Ok(true);
    }
    Ok(FREE_WHILE_TAVI_DEAD && read_command()?.is_some_and(|c| c.mode == Mode::TaviDead))
}

fn publish_at(mode: Mode, target: Vec2, leader: Vec2) -> Result<()> {
    let x = u8::try_from(target.x).map_err(|_| cambc::Error::Engine {
        raw_code: 0,
        code: None,
        message: format!("squad target x coordinate out of range: {}", target.x),
    })?;
    let y = u8::try_from(target.y).map_err(|_| cambc::Error::Engine {
        raw_code: 0,
        code: None,
        message: format!("squad target y coordinate out of range: {}", target.y),
    })?;
    let leader_x = u8::try_from(leader.x).map_err(|_| cambc::Error::Engine {
        raw_code: 0,
        code: None,
        message: format!("squad leader x coordinate out of range: {}", leader.x),
    })?;
    let leader_y = u8::try_from(leader.y).map_err(|_| cambc::Error::Engine {
        raw_code: 0,
        code: None,
        message: format!("squad leader y coordinate out of range: {}", leader.y),
    })?;
    let round = cambc::round_number()? as u16;
    let mut bytes = [0; 8];
    bytes[0] = MEMORY_MAGIC;
    bytes[1] = mode as u8;
    bytes[2] = x;
    bytes[3] = y;
    bytes[4..6].copy_from_slice(&round.to_le_bytes());
    bytes[6] = leader_x;
    bytes[7] = leader_y;
    write_command_bytes(bytes)
}

fn ally_spawn_position() -> Result<Vec2> {
    let positions = cambc::map::spawn_region(cambc::Team::Ally)?.positions()?;
    positions.first().copied().ok_or(cambc::Error::Engine {
        raw_code: 0,
        code: None,
        message: "ally spawn region has no valid tiles".to_owned(),
    })
}

fn publish(mode: Mode, target: Vec2) -> Result<()> {
    let leader = me()?.position()?;
    if u8::try_from(leader.x).is_err() || u8::try_from(leader.y).is_err() {
        let spawn = ally_spawn_position()?;
        return publish_at(Mode::TaviDead, spawn, spawn);
    }
    publish_at(mode, target, leader)
}

fn clear_command() -> Result<()> {
    write_command_bytes([0; 8])
}

pub fn signal_tavi_dead() -> Result<()> {
    let spawn = ally_spawn_position()?;
    publish_at(Mode::TaviDead, spawn, spawn)
}

fn is_leader() -> Result<bool> {
    Ok(me()?.champion_name()? == ChampionType::Tavi)
}

pub fn is_squad_bot() -> Result<bool> {
    Ok(matches!(
        me()?.champion_name()?,
        ChampionType::Tavi
            | ChampionType::Willow
            | ChampionType::Ryth
            | ChampionType::Marcella
            | ChampionType::HeartOfRuin
    ))
}

fn rank() -> Result<usize> {
    Ok(match me()?.champion_name()? {
        ChampionType::Tavi => 0,
        ChampionType::Willow => 1,
        ChampionType::HeartOfRuin => 2,
        ChampionType::Ryth => 3,
        ChampionType::Marcella => 4,
        _ => 0,
    })
}

fn retaliatory_target() -> Result<Option<Vec2>> {
    let me = me()?;
    let mut current = vec![HealthSample {
        champion: me.champion_name()?,
        health: me.health()?,
    }];
    for ally in &common::sight()?.allies {
        if ally.is_alive()? {
            current.push(HealthSample {
                champion: ally.champion_name()?,
                health: ally.health()?,
            });
        }
    }

    let damaged: Vec<_> = ALLY_HEALTH.with(|previous| {
        let previous = previous.borrow();
        current
            .iter()
            .filter(|sample| {
                previous
                    .iter()
                    .find(|old| old.champion == sample.champion)
                    .is_some_and(|old| sample.health < old.health)
            })
            .map(|sample| sample.champion)
            .collect()
    });
    ALLY_HEALTH.with(|previous| {
        let mut previous = previous.borrow_mut();
        for sample in &current {
            if let Some(old) = previous
                .iter_mut()
                .find(|old| old.champion == sample.champion)
            {
                old.health = sample.health;
            } else {
                previous.push(*sample);
            }
        }
    });
    if damaged.is_empty() {
        return Ok(None);
    }

    let sight = common::sight()?;
    let my_name = me.champion_name()?;
    let mut candidates = Vec::new();
    for enemy in &sight.enemies {
        let position = enemy.position()?;
        if !common::enemy_position_safe_to_target(position)? {
            continue;
        }
        let mut can_reach_damaged_ally = false;
        for champion in &damaged {
            let ally_position = if *champion == my_name {
                me.position()?
            } else {
                let mut position = None;
                for ally in &sight.allies {
                    if ally.champion_name()? == *champion {
                        position = Some(ally.position()?);
                        break;
                    }
                }
                let Some(position) = position else {
                    continue;
                };
                position
            };
            let mut reach = enemy.attack_range()?;
            for ability in enemy.abilities() {
                if common::is_damaging(ability) {
                    if let Some(info) = ability.info()? {
                        reach = reach.max(info.as_dyn().range()? as i32);
                    }
                }
            }
            if common::distance(position, ally_position) <= reach {
                can_reach_damaged_ally = true;
                break;
            }
        }
        if can_reach_damaged_ally {
            candidates.push((enemy.health()?, position));
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(candidates.first().map(|&(_, position)| position))
}

fn begin_recall(command: Option<Command>) -> Result<()> {
    let round = cambc::round_number()?;
    RECALL_STARTED.with(|started| started.set(Some(round)));
    if let Some(command) = command {
        match command.mode {
            Mode::Farm => return publish(Mode::RecallFinishFarm, command.target),
            Mode::Enemy => return publish(Mode::RecallFinishEnemy, command.target),
            _ => {}
        }
    }
    publish(Mode::RecallMove, me()?.position()?)
}

fn recall_started() -> Option<u32> {
    RECALL_STARTED.with(Cell::get)
}

fn advance_recall(ai: &mut dyn ChampionAi, command: Command) -> Result<()> {
    let Some(started) = recall_started() else {
        begin_recall(Some(command))?;
        return Ok(());
    };
    match command.mode {
        Mode::RecallFinishEnemy => {
            let age = cambc::round_number()?.saturating_sub(started);
            if age < RECALL_FINISH_MAX_ROUNDS
                && common::fight_squad_enemy_at_position(command.target)?
            {
                publish(Mode::RecallFinishEnemy, command.target)?;
                return Ok(());
            }
        }
        Mode::RecallFinishFarm => {
            let age = cambc::round_number()?.saturating_sub(started);
            if age < RECALL_FINISH_MAX_ROUNDS && common::claim_camp_reward(command.target)? {
                publish(Mode::RecallFinishFarm, command.target)?;
                return Ok(());
            }
            let active = common::farm_target_active(command.target)?;
            let can_win = match common::group_at(command.target)? {
                Some(group) => crate::collect::camp_fight_winnable(group)? != Some(false),
                None => false,
            };
            if age < RECALL_FINISH_MAX_ROUNDS && active != Some(false) && can_win {
                ai.farm()?;
                publish(
                    Mode::RecallFinishFarm,
                    common::current_farm_target().unwrap_or(command.target),
                )?;
                return Ok(());
            }
        }
        _ => {}
    }
    move_to_recall_spot()
}

// Walk to where the squad can recall, and recall there once everyone's gathered. Close to our spawn
// we walk home instead (safety::WALK_HOME_STEPS; standing on it heals). Otherwise we head for the
// tiles safe for the whole channel, leaning towards home (common::RETREAT_HOME_PULL): the nearest
// safe tile could lie across the middle of the map. The followers walk to wherever we are.
fn move_to_recall_spot() -> Result<()> {
    let me = me()?;
    if crate::safety::walk_home_instead()? && crate::safety::walk_home()? {
        return publish(Mode::RecallMove, me.position()?);
    }
    let Some(safe_tiles) = crate::safety::recall_spot()? else {
        let here = me.position()?;
        publish(Mode::RecallMove, here)?;
        if !allies_gathered(here)? {
            common::trace("recall-wait-allies");
            return Ok(());
        }
        publish(Mode::RecallNow, here)?;
        return me.recall();
    };
    common::trace("recall-walk-safe");
    GATHER_SINCE.with(|g| g.set(None));
    if !common::move_towards_closest_with_pull(safe_tiles, crate::safety::toward_home, false)? {
        // No way out towards safety: don't stand in a camp's or enemy's reach.
        crate::safety::dodge_if_in_danger()?;
    }
    publish(Mode::RecallMove, me.position()?)
}

// Before recalling, Tavi waits for the allies it can see to gather within 2 tiles, so the squad
// recalls together. Allies already on our spawn, or recalling, aren't waited for: a follower on
// our spawn waits there while Tavi's command is a recall (follower_turn), so each waited for the
// other (spiral: Tavi stood still at 408/1200 health for 184 rounds, the others on our spawn 11
// tiles away). And after GATHER_MAX_ROUNDS rounds of waiting Tavi recalls anyway, whoever's
// missing. u32::MAX: wait as long as it takes.
const GATHER_MAX_ROUNDS: u32 = 10;

thread_local! {
    // The round Tavi started waiting at its current recall spot (with its tile), if it is.
    static GATHER_SINCE: Cell<Option<(u32, Vec2)>> = const { Cell::new(None) };
}

fn allies_gathered(target: Vec2) -> Result<bool> {
    // Solo, the followers play on their own: nobody's coming.
    if solo()? {
        return Ok(true);
    }
    let round = cambc::round_number()?;
    let since = match GATHER_SINCE.with(Cell::get) {
        Some((since, tile)) if tile == target => since,
        _ => {
            GATHER_SINCE.with(|g| g.set(Some((round, target))));
            round
        }
    };
    if round.saturating_sub(since) >= GATHER_MAX_ROUNDS {
        common::trace("recall-gather-timeout");
        return Ok(true);
    }
    let spawn = cambc::map::spawn_region(cambc::Team::Ally)?;
    for ally in &common::sight()?.allies {
        if !ally.is_alive()? || ally.is_recalling()? {
            continue;
        }
        let position = ally.position()?;
        if common::distance(position, target) > 2 && !spawn.is_set(position)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn visible_tavi_is_recalling() -> Result<bool> {
    for ally in &common::sight()?.allies {
        if ally.champion_name()? == ChampionType::Tavi && ally.is_recalling()? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn visible_tavi_position() -> Result<Option<Vec2>> {
    for ally in &common::sight()?.allies {
        if ally.champion_name()? == ChampionType::Tavi {
            return Ok(Some(ally.position()?));
        }
    }
    Ok(None)
}

fn follow_or_idle_near_tavi(target: Vec2) -> Result<()> {
    if common::distance(me()?.position()?, target) <= IDLE_FOLLOW_RADIUS
        && common::wander_adjacent_near(target, IDLE_FOLLOW_RADIUS)?
    {
        return Ok(());
    }
    common::follow_squad_target(target)
}

fn leader_turn(ai: &mut dyn ChampionAi) -> Result<()> {
    let champion = me()?;
    let at_spawn = cambc::map::spawn_region(cambc::Team::Ally)?.is_set(champion.position()?)?;
    if at_spawn {
        RECALL_STARTED.with(|started| started.set(None));
        GATHER_SINCE.with(|g| g.set(None));
        clear_command()?;
    }

    // A melee enemy right next to us: stop running (to a recall spot, to an altar) and hit it. The
    // squad is called in to fight it; once it's gone a low-health recall starts over.
    if let Some(target) = common::fight_adjacent_melee()? {
        common::trace("melee-stand");
        return publish(Mode::Enemy, target);
    }

    let command = read_command()?;
    if let Some(command) = command.filter(|command| {
        matches!(
            command.mode,
            Mode::RecallFinishFarm | Mode::RecallFinishEnemy | Mode::RecallMove | Mode::RecallNow
        )
    }) {
        common::trace("recall-advance");
        return advance_recall(ai, command);
    }

    if champion.gem_count()? >= TAVI_BANK_GEMS_THRESHOLD {
        common::trace("bank-200");
        publish(Mode::Follow, champion.position()?)?;
        ai.bank_gems()?;
        return publish(Mode::Follow, me()?.position()?);
    }

    if let Some(target) = common::fight_squad_enemy_in_attack_range()? {
        common::trace("enemy-in-range");
        return publish(Mode::Enemy, target);
    }

    let retaliatory_target = retaliatory_target()?;
    let hp_recall_needed = crate::safety::low_hp_recall()?;
    if !at_spawn && hp_recall_needed && retaliatory_target.is_none() {
        if let Some(command) = command.filter(|command| command.mode == Mode::Enemy) {
            begin_recall(Some(command))?;
            return Ok(());
        }
        if let Some(target) = common::current_farm_target() {
            begin_recall(Some(Command {
                mode: Mode::Farm,
                target,
                leader: me()?.position()?,
            }))?;
            return Ok(());
        }
        begin_recall(None)?;
        return Ok(());
    }

    if let Some(target) = retaliatory_target {
        common::fight_squad_enemy_at_position(target)?;
        return publish(Mode::Enemy, target);
    }

    if let Some(target) = common::current_farm_target() {
        if common::claim_camp_reward(target)? {
            common::trace("claim-reward");
            return publish(Mode::Farm, target);
        }
    }

    if common::aggressive_gem_phase(cambc::round_number()?)? {
        if let Some(target) = common::fight_squad_enemy(0)? {
            return publish(Mode::Enemy, target);
        }
    }
    let plan = common::farm_plan_or_stale()?;
    let goals = plan.goals.positions()?;
    if goals.is_empty() {
        if let Some(target) = common::fight_squad_enemy(0)? {
            return publish(Mode::Enemy, target);
        }
        common::trace("no-farm-goals");
        return publish(Mode::Follow, me()?.position()?);
    }
    let here = me()?.position()?;
    ai.farm()?;
    let current = common::current_farm_target();
    let target = match current {
        Some(target) => Some(target),
        None => goals
            .into_iter()
            .min_by_key(|&tile| common::distance(here, tile)),
    };
    match target {
        Some(target) => publish(Mode::Farm, target),
        None => publish(Mode::Follow, me()?.position()?),
    }
}

fn follower_turn() -> Result<()> {
    let command = read_command()?;
    let tavi_position = visible_tavi_position()?;
    let at_spawn = cambc::map::spawn_region(cambc::Team::Ally)?.is_set(me()?.position()?)?;
    if command.is_some_and(|command| command.mode == Mode::TaviDead) {
        if at_spawn {
            return Ok(());
        }
        return crate::safety::retreat_and_recall();
    }
    if at_spawn
        && (visible_tavi_is_recalling()?
            || command.is_some_and(|command| {
                matches!(
                    command.mode,
                    Mode::RecallFinishFarm
                        | Mode::RecallFinishEnemy
                        | Mode::RecallMove
                        | Mode::RecallNow
                )
            }))
    {
        return Ok(());
    }
    if !at_spawn && crate::safety::low_hp_recall()? {
        return crate::safety::retreat_and_recall();
    }
    if visible_tavi_is_recalling()? {
        if crate::safety::recall_safe()? {
            return me()?.recall();
        }
        if let Some(command) = command {
            common::follow_squad_target(command.target)?;
        } else if let Some(position) = tavi_position {
            common::follow_squad_target(position)?;
        }
        return Ok(());
    }
    let Some(command) = command else {
        if let Some(position) = tavi_position {
            follow_or_idle_near_tavi(position)?;
        }
        return Ok(());
    };
    let tavi_position = tavi_position.or(Some(command.leader));
    match command.mode {
        Mode::Farm | Mode::RecallFinishFarm => {
            common::farm_squad_target(command.target, tavi_position)
        }
        Mode::Enemy | Mode::RecallFinishEnemy => {
            if common::fight_squad_enemy(rank()?)?.is_some() {
                return Ok(());
            }
            if command.mode == Mode::Enemy {
                follow_or_idle_near_tavi(tavi_position.unwrap_or(command.leader))
            } else {
                common::follow_squad_target(tavi_position.unwrap_or(command.leader))
            }
        }
        Mode::Follow => follow_or_idle_near_tavi(tavi_position.unwrap_or(command.target)),
        Mode::RecallMove => common::follow_squad_target(command.target),
        Mode::RecallNow => {
            if crate::safety::recall_safe()?
                && common::distance(me()?.position()?, command.target) <= 2
            {
                me()?.recall()
            } else {
                common::follow_squad_target(command.target)
            }
        }
        Mode::TaviDead => crate::safety::retreat_and_recall(),
    }
}

pub fn take_turn(ai: &mut dyn ChampionAi) -> Result<()> {
    if is_leader()? {
        leader_turn(ai)
    } else {
        follower_turn()
    }
}
