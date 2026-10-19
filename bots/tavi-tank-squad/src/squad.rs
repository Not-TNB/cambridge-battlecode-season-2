use crate::champions::ChampionAi;
use crate::common::{self, me};
use cambc::{Bitboard, ChampionType, Result, Vec2};
use std::cell::Cell;

const MEMORY_MAGIC: u8 = 0x53;
const COMMAND_MAX_AGE: u32 = 6;
const LOW_HEALTH_RECALL_FRACTION: f32 = 0.35;
const TAVI_BANK_GEMS_THRESHOLD: u32 = 200;
const IDLE_FOLLOW_RADIUS: i32 = 5;
const RECALL_FINISH_MAX_ROUNDS: u32 = 10;
pub const TAVI_LATE_BUILD_ROUND: u32 = 800;

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
    let mut bytes = [0; 8];
    cambc::shared_memory().read(0, &mut bytes)?;
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
    cambc::shared_memory().write(0, &bytes)
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
    cambc::shared_memory().write(0, &[0; 8])
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
            | ChampionType::Varg
    ))
}

fn rank() -> Result<usize> {
    Ok(match me()?.champion_name()? {
        ChampionType::Tavi => 0,
        ChampionType::Willow => 1,
        ChampionType::Varg => 2,
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
    move_to_recall_spot(command)
}

fn move_to_recall_spot(previous: Command) -> Result<()> {
    let me = me()?;
    let here = me.position()?;
    let mut safe_tiles = crate::safety::recall_safe_tiles()?;
    let blocked = common::blocked_tiles()?;
    safe_tiles.intersect_assign(&blocked.complement()?)?;
    let safe_positions = safe_tiles.positions()?;
    if safe_positions.is_empty() {
        publish(Mode::RecallMove, here)?;
        return Ok(());
    }
    let target = if safe_tiles.is_set(here)? {
        here
    } else if previous.mode == Mode::RecallMove && safe_tiles.is_set(previous.target)? {
        previous.target
    } else {
        safe_positions
            .into_iter()
            .min_by_key(|&position| common::distance(here, position))
            .expect("safe positions checked above")
    };
    publish(Mode::RecallMove, target)?;
    if here != target {
        let mut goal = Bitboard::new()?;
        goal.set(target, true)?;
        common::move_towards_closest(goal)?;
    }
    if !crate::safety::recall_safe()? || me.position()? != target {
        return Ok(());
    }
    if !allies_gathered(target)? {
        return Ok(());
    }
    publish(Mode::RecallNow, target)?;
    me.recall()
}

fn allies_gathered(target: Vec2) -> Result<bool> {
    for ally in &common::sight()?.allies {
        if ally.is_alive()? && common::distance(ally.position()?, target) > 2 {
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
        clear_command()?;
    }

    let command = read_command()?;
    if let Some(command) = command.filter(|command| {
        matches!(
            command.mode,
            Mode::RecallFinishFarm | Mode::RecallFinishEnemy | Mode::RecallMove | Mode::RecallNow
        )
    }) {
        return advance_recall(ai, command);
    }

    if champion.gem_count()? >= TAVI_BANK_GEMS_THRESHOLD {
        publish(Mode::Follow, champion.position()?)?;
        ai.bank_gems()?;
        return publish(Mode::Follow, me()?.position()?);
    }

    if let Some(target) = common::fight_squad_enemy_in_attack_range()? {
        return publish(Mode::Enemy, target);
    }

    let retaliatory_target = retaliatory_target()?;
    let hp_recall_needed =
        champion.health()? / champion.max_health()?.max(1.0) <= LOW_HEALTH_RECALL_FRACTION;
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
            return publish(Mode::Farm, target);
        }
    }

    if common::aggressive_gem_phase(cambc::round_number()?)? {
        if let Some(target) = common::fight_squad_enemy(0)? {
            return publish(Mode::Enemy, target);
        }
    }
    let plan = common::farm_plan()?;
    let goals = plan.goals.positions()?;
    if goals.is_empty() {
        if let Some(target) = common::fight_squad_enemy(0)? {
            return publish(Mode::Enemy, target);
        }
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
    let champion = me()?;
    if !at_spawn
        && champion.health()? / champion.max_health()?.max(1.0) <= LOW_HEALTH_RECALL_FRACTION
    {
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
