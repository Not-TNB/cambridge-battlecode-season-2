// Turn skeleton shared by every champion. main.rs dispatches here after the draft; champion-specific
// decisions live in champions/<name>.rs and shared helpers in common/.
use crate::champions::{self, ChampionAi};
use crate::common::{self, collect, me, safety};
use cambc::{ItemType as Item, Result, Team};

// Fuel to keep in hand before starting a pathfinding step (bank_gems / farm).
const MIN_FUEL_FOR_PATHING: u64 = 40_000;
// v8 (overnight tests, 2026-10-24; the user's idea): go home for gold only once it buys shards
// adding at least this share to the stat our build buys next (or when hurt: the missing-health part
// of base_willingness, and safety's low-health recall). Champions went home a lot, and away from
// the fight the team takes fewer kills when ahead. 0.0: off (base_willingness: 150 gold is enough,
// plus missing health and rounds away).
// See rally::RALLY_COMMIT for the 2026-10-24 results. On: 0.15.
const HOME_STAT_GAIN: f32 = 0.15;
// v8 (2026-10-24): later on every shard is a smaller share of the stat, so the 15% bar was met less
// and less and champions ended games holding ~750 gold (older bots ~300). From HOME_LATE_ROUND the
// bar is HOME_STAT_GAIN_LATE. 0: off.
// 2026-10-24, with BUY_WHILE_DEAD, 50 games head to head against bot-v8 without both: 24/50,
// gold left at the end 500 against 692, deaths 12.0 against 12.7, gems submitted 769 against 803
// (neutral). On at the user's call (may do better against other teams).
const HOME_LATE_ROUND: u32 = 800;
const HOME_STAT_GAIN_LATE: f32 = 0.08;
// v8 (2026-10-24): the shop is open while dead (game rules), but a dead champion's turn ended before
// shopping. With this on it spends its gold while waiting to respawn. Off: only on spawn tiles.
// See HOME_LATE_ROUND for the test; on.
const BUY_WHILE_DEAD: bool = true;
// Fuel to keep in hand before end-of-turn extras (sightings, wards; up to ~20k).
const SPARE_FUEL: u64 = 30_000;
// Fuel to have left before working out the walking distances to altars (once; up to ~35k on the
// largest maps).
const ALTAR_FIELD_FUEL: u64 = 50_000;

// Why our recall channel was started: a go-home recall (to shop and heal) gives way to anything
// that ranks above going home (see cancel_recall_for); the low-health recall gives way to nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecallReason {
    LowHealth,
    GoHome,
}

// Cancel a go-home recall in progress when something that ranks above going home turns up: an
// enemy close enough to fight (fight_enemies decides whether to fight or back off), a reward to
// capture under us, or an ally's camp fight to help with. Off: recalls always run to the end.
const CANCEL_GO_HOME_RECALL: bool = true;
// Whether an enemy close enough to fight also cancels it. Off: with it (and no gem limit on helping)
// we lost 32% more gems on death for the same wins; a recall an enemy attacks is interrupted anyway.
const CANCEL_RECALL_FOR_ENEMIES: bool = false;
// Finish the camp fight we're in (collect::in_camp_fight) before going home to shop: leaving woke
// camps we'd already damaged hitting us as we walked off. The low-health recall still comes first.
// 2026-10-21, 132 games: 77-55 against off, gold +3%.
const FINISH_CAMP_BEFORE_HOME: bool = true;
// Greed (v4): with no enemy champion seen for GREED_QUIET_ROUNDS rounds and at least
// GREED_MIN_HEALTH of our health, the pull home must reach GREED_HOME_WILLINGNESS instead of 1
// (at full health: twice the gold) before a shopping trip. 67% of go-home recalls were at 70%+
// health and 91% with no enemy in sight: a camp's gold came in and we left with camps around
// (~11 trips a champion a game, ~25 rounds each). The low-health recall is unchanged. 1.0: off.
// 2026-10-21, 132 games against v3: 57 (any time it was quiet), 59 (only for a next camp within
// GREED_TARGET_TILES): the shopping trip pays for itself in items. Off.
const GREED_HOME_WILLINGNESS: f32 = 1.0;
const GREED_MIN_HEALTH: f32 = 0.6;
const GREED_QUIET_ROUNDS: u32 = 10;
// Gold close by (v4): with no enemy in sight and at least HOME_NEAR_GOLD_HEALTH of our health, the
// pull home must also cover the gold of the camp groups in sight within HOME_NEAR_GOLD_TILES that
// we'd clear (collect::nearby_camp_gold), in emblems' worth (150 gold a step), up to
// HOME_NEAR_GOLD_MAX in all: 150 gold isn't a hard limit when there's more to take next door.
// 0 tiles: off. 2026-10-21 (engine 3.0.4, 35 maps): 111/210 against off.
const HOME_NEAR_GOLD_TILES: i32 = 8;
const HOME_NEAR_GOLD_HEALTH: f32 = 0.5;
const HOME_NEAR_GOLD_MAX: f32 = 2.0;
// Only for a next camp this close: our farm target, alive and in sight.
const GREED_TARGET_TILES: i32 = 6;

// The pull home needed with gold close by (HOME_NEAR_GOLD_*); 1.0 without.
fn near_gold_home_at() -> Result<f32> {
    let me = me()?;
    if HOME_NEAR_GOLD_TILES <= 0
        || me.health()? < HOME_NEAR_GOLD_HEALTH * me.max_health()?
        || !common::sight()?.enemies.is_empty()
    {
        return Ok(1.0);
    }
    let gold = collect::nearby_camp_gold(HOME_NEAR_GOLD_TILES)?;
    if gold > 0 {
        common::trace("home-near-gold");
    }
    Ok((1.0 + gold as f32 / 150.0).min(HOME_NEAR_GOLD_MAX))
}

fn next_camp_close() -> Result<bool> {
    let Some(target) = common::current_target() else {
        return Ok(false);
    };
    let here = me()?.position()?;
    Ok(common::distance(target, here) <= GREED_TARGET_TILES
        && common::sight()?
            .camps
            .iter()
            .any(|c| c.spawn_position() == target))
}

// Per-champion state survives between turns. Pick one stat and keep buying it.
#[derive(Default)]
pub struct Bot {
    recall_reason: Option<RecallReason>,
    chosen_stat: Option<usize>,
    was_recalling: bool,
    last_at_spawn_round: u32,
    ai: Option<Box<dyn ChampionAi>>,
    build: Option<&'static [Item]>,
    last_turn_hp: Option<f32>, // health at our last turn's start (safety::HIDDEN_ATTACKER)
}

// The Treasure shard of the same stat as `shard` (each family lists Strength, Focus, Health as
// shard/emblem pairs, so the stat is the item's index mod 6).
fn treasure_shard(shard: Item) -> Item {
    match shard as u32 % 6 {
        0 | 1 => Item::TreasureShardOfStrength,
        2 | 3 => Item::TreasureShardOfFocus,
        _ => Item::TreasureShardOfHealth,
    }
}

// Every champion's build gets a Treasure emblem (+20% gold) at this position when it has none:
// the top three ladder teams' builds (2026-10-22, 50 games) have one in the first two emblems on
// nearly every champion. usize::MAX: only our top carry's, first, on farming maps
// (opening::treasure_first). Never past the last entry, which repeats.
// 2026-10-22, 320 games each against off: first, before the Durability emblem 172-148 (gold +14%,
// most of it the Treasure itself, deaths 13.9 against 15.6); first, with it 173-147 (gems +6%,
// deaths 13.0 against 14.8); second 168-152 (gems -2%).
const TREASURE_EVERYONE_AT: usize = 0;

// v9 (2026-10-25): past the end of its build a champion repeated the last entry, so Vesper Vale
// (HEALTH_BUILDS, ~20-35 emblems a game) ended with 13.2 Bruiser Health emblems against 7.2
// Strength (top-30 Vespers: 73% Strength). On: a list ending in two different Bruiser emblems keeps
// alternating them. Off: the last entry repeats.
// 2026-10-26, 120 games against bot-v8 with a same-seed control (59/120): 54/120 (+4/-9: only
// games past ~20 emblems change), Vesper's gold at the end 3,783 against 3,612. Neutral; on for the
// build top teams use (mostly Strength late on Vesper).
pub const BUILD_ALTERNATE_TAIL: bool = true;

// The champion's build with a Treasure emblem of its first entry's stat put in (see
// TREASURE_EVERYONE_AT and opening::treasure_first), unless the build already has one.
fn build_for(ai: &dyn ChampionAi) -> Result<&'static [Item]> {
    let build = ai.build();
    let has_treasure = build.iter().any(|&i| matches!(i as u32, 18..=23));
    let Some(&first) = build.first() else {
        return Ok(build);
    };
    let at = if crate::opening::treasure_first(me()?.champion_name()?) {
        0
    } else {
        TREASURE_EVERYONE_AT
    };
    if has_treasure || at >= build.len() {
        return Ok(build);
    }
    let mut opened = build.to_vec();
    opened.insert(at, treasure_shard(first));
    if common::TRACE {
        println!("build: Treasure at {at} ({:?})", opened[at]);
    }
    Ok(Vec::leak(opened))
}

impl Bot {
    // Follow the champion's build order: complete its emblems one at a time, buying only the
    // shards the current emblem still needs, and repeat the last entry once the list is done.
    // Champions without a build order buy power shards of one randomly chosen stat.
    // How much the shards our gold buys now (30 each) would add to the stat our build buys next, as
    // a share of that stat now (HOME_STAT_GAIN).
    fn upgrade_gain(&self) -> Result<f32> {
        let me = me()?;
        let shards = (me.gold_count()? / 30) as f32;
        if shards < 1.0 {
            return Ok(0.0);
        }
        // Shards come in families of six item ids: strength, its emblem, focus, its emblem, health,
        // its emblem.
        let mut stat = self.chosen_stat.unwrap_or(0) as u32 * 2;
        if let Some(build) = self.build.filter(|b| !b.is_empty()) {
            let mut owned = [0u32; 42];
            for (item, count) in me.item_count()?.iter() {
                owned[item as usize] = count;
            }
            let mut wanted = [0u32; 42];
            for i in 0..64 {
                let shard = build[i.min(build.len() - 1)];
                let Some(emblem) = shard.result()? else {
                    continue;
                };
                wanted[emblem as usize] += 1;
                if owned[emblem as usize] < wanted[emblem as usize] {
                    stat = shard as u32 % 6;
                    break;
                }
            }
        }
        let base = me.base_stats()?;
        let (now, growth) = match stat {
            0 => (me.strength()?, base.strength_growth),
            2 => (me.focus()?, base.focus_growth),
            _ => (me.max_health()?, base.health_growth),
        };
        Ok(shards * growth / now.max(1.0))
    }

    fn buy_items(&self, build: &[Item]) -> Result<()> {
        let me = me()?;
        let mut gold = me.gold_count()?;
        if build.is_empty() {
            let item = [
                Item::PowerShardOfStrength,
                Item::PowerShardOfFocus,
                Item::PowerShardOfHealth,
            ][self.chosen_stat.unwrap()];
            let count = gold / item.cost()?;
            if count > 0 && me.can_buy(item, count)? {
                me.buy_count(item, count)?;
            }
            return Ok(());
        }
        let mut owned = [0u32; 42];
        for (item, count) in me.item_count()?.iter() {
            owned[item as usize] = count;
        }
        let mut wanted = [0u32; 42]; // emblems the build calls for up to the current entry
                                     // BUILD_ALTERNATE_TAIL: a list ending in two different Bruiser emblems cycles them.
        let n = build.len();
        let cycle = BUILD_ALTERNATE_TAIL
            && n >= 2
            && build[n - 1] != build[n - 2]
            && [build[n - 1], build[n - 2]].iter().all(|&s| {
                matches!(
                    s,
                    Item::BruiserShardOfStrength
                        | Item::BruiserShardOfFocus
                        | Item::BruiserShardOfHealth
                )
            });
        for i in 0..64 {
            let shard = if i < n {
                build[i]
            } else if cycle {
                build[n - 2 + (i - n) % 2]
            } else {
                build[n - 1]
            };
            let Some(emblem) = shard.result()? else {
                continue;
            };
            wanted[emblem as usize] += 1;
            if owned[emblem as usize] >= wanted[emblem as usize] {
                continue;
            }
            let cost = shard.cost()?;
            let to_finish = shard.combine_count()?.saturating_sub(owned[shard as usize]);
            let count = to_finish.min(gold / cost);
            if count == 0 || !me.can_buy(shard, count)? {
                break;
            }
            me.buy_count(shard, count)?;
            gold -= count * cost;
            if count < to_finish {
                break;
            }
            owned[shard as usize] = 0;
            owned[emblem as usize] += 1;
        }
        Ok(())
    }

    pub fn play_turn(&mut self) -> Result<()> {
        cambc::debug_block! {
            common::activity::start_turn();
        }
        // Allies' respawn messages (respawns.rs); a failed read just means not hearing one.
        let _ = common::hear_respawns();
        if self.ai.is_none() {
            self.ai = Some(champions::for_champion(me()?.champion_name()?));
        }
        // Take the AI out for the turn so it and the shared state can both be borrowed mutably.
        let mut ai = self.ai.take().expect("set above");
        let result = self.turn(ai.as_mut());
        self.ai = Some(ai);
        common::checkpoint("turn-decisions");
        // With fuel to spare, remember where enemies are (for next turn's safety checks) and ward a
        // hidden tile nearby. Both are optional, so they wait until the turn's real decisions are
        // made, and a failed query (e.g. a unit just left vision) is ignored.
        if cambc::fuel_remaining()? >= SPARE_FUEL && me()?.is_alive()? {
            let _ = safety::remember_enemies();
            common::checkpoint("turn-sightings");
            if cambc::fuel_remaining()? >= SPARE_FUEL {
                let _ = common::ward_hidden_spots();
            }
            common::checkpoint("turn-ward");
            let _ = crate::opening::update_fed(SPARE_FUEL);
            common::checkpoint("turn-opening-fed");
            if cambc::fuel_remaining()? >= ALTAR_FIELD_FUEL {
                common::prepare_walk_fields();
            }
            common::checkpoint("turn-altar-steps");
            let _ = common::walk::build_next_group_field();
            // The camp graph, a few rows a turn from the spare fuel (common::graph).
            if let Err(error) = common::graph::build_step() {
                println!("bot error (graph): {error}");
            }
        }
        // Profiling output: free in local debug builds, skipped on the platform (debug_block).
        cambc::debug_block! {
            self.print_trace()?;
        }
        let _ = common::claims::sync(common::current_target());
        cambc::debug_block! {
            let _ = common::activity::end_turn(None);
        }
        result
    }

    fn print_trace(&self) -> Result<()> {
        if common::TRACE && common::trace_wanted() {
            let me = me()?;
            // Integers only: formatting a float costs ~9k fuel.
            // Situation counts for the cut-off analysis. After a step, sight is listed again (~8k
            // fuel), so in a debug block (free in local builds).
            let (mut en, mut al, mut cp) = (0, 0, 0);
            cambc::debug_block! {
                if let Ok(s) = common::sight() {
                    (en, al, cp) = (s.enemies.len(), s.allies.len(), s.camps.len());
                }
            }
            println!(
                "T {:?} r{} {},{} hp={}/{} gems={} fuel={} en={} al={} cp={} |{}",
                me.champion_name()?,
                cambc::round_number()?,
                me.position()?.x,
                me.position()?.y,
                me.health()? as i32,
                me.max_health()? as i32,
                me.gem_count()?,
                cambc::fuel_remaining()?,
                en,
                al,
                cp,
                common::take_trace()
            );
        }
        Ok(())
    }

    fn turn(&mut self, ai: &mut dyn ChampionAi) -> Result<()> {
        if self.chosen_stat.is_none() {
            self.chosen_stat = Some(crate::random_number(3) as usize);
            if common::TRACE {
                println!("draft memory: {} bytes", cambc::draft_memory().len());
            }
        }
        let me = me()?;
        common::checkpoint("turn-start");

        // Damage dealers tell follow-only escorts where they are (collect::ESCORT_FOLLOW_ONLY).
        if let Err(error) = collect::publish_lead() {
            println!("bot error (lead): {error}");
        }

        // Priorities: shop and recall, fight enemies, bank gems, then farm.
        // Damage nothing we see explains: someone in a bush (safety::HIDDEN_ATTACKER).
        if common::safety::HIDDEN_ATTACKER {
            let hp = if me.is_alive()? {
                Some(me.health()?)
            } else {
                None
            };
            if let (Some(before), Some(now)) = (self.last_turn_hp, hp) {
                if before - now >= common::safety::HIDDEN_MIN_DAMAGE {
                    if let Err(error) = common::safety::note_hidden_attacker(before - now) {
                        println!("bot error (hidden): {error}");
                    }
                }
            }
            self.last_turn_hp = hp;
        }
        if !me.is_alive()? {
            let _ = common::rally::set_unavailable(true);
            // The shop is open while dead (game rules): spend now, not after the walk home.
            if BUY_WHILE_DEAD && !ai.shops_anywhere() {
                let build = match self.build {
                    Some(build) => build,
                    None => *self.build.insert(build_for(ai)?),
                };
                if let Err(error) = self.buy_items(build) {
                    println!("bot error (dead shop): {error}");
                }
            }
            common::trace("skip-dead");
            self.was_recalling = true;
            return Ok(());
        }
        if me.is_recalling()? {
            let cancel =
                if CANCEL_GO_HOME_RECALL && self.recall_reason == Some(RecallReason::GoHome) {
                    cancel_recall_for()?
                } else {
                    None
                };
            match cancel {
                Some(why) if me.cancel_recall().is_ok() => {
                    common::trace(why);
                    self.recall_reason = None;
                }
                _ => {
                    common::trace("skip-recalling");
                    self.was_recalling = true;
                    return Ok(());
                }
            }
        }
        if !me.is_recalling()? {
            // A recall we started that neither took us home nor was cancelled by us was broken
            // (damage, a stun): remember where (safety::RECALL_INTERRUPT_MEMORY).
            if self.recall_reason.is_some()
                && !cambc::map::spawn_region(Team::Ally)?.is_set(me.position()?)?
            {
                common::trace("recall-interrupted");
                if common::TRACE {
                    cambc::debug_block! {
                        println!("RECALL-INTERRUPTED r{} {},{}", cambc::round_number()?, me.position()?.x, me.position()?.y);
                    }
                }
                safety::recall_interrupted(me.position()?)?;
            }
            self.recall_reason = None;
        }
        if me.is_submitting_gems()? {
            common::trace("skip-tribute");
            self.was_recalling = true;
            return Ok(());
        }
        if me.is_teleporting()? {
            common::trace("skip-teleport");
            self.was_recalling = true;
            return Ok(());
        }
        if !me.can_move()? && !me.can_attack()? {
            common::trace("skip-no-actions");
            self.was_recalling = true;
            return Ok(());
        }

        let at_spawn = cambc::map::spawn_region(Team::Ally)?.is_set(me.position()?)?;
        if at_spawn {
            // Arriving (not every turn on it: a champion waiting there judged every group again
            // each turn): shopping and healing change which fights we'd win.
            let round = cambc::round_number()?;
            if self.last_at_spawn_round + 1 < round {
                collect::forget_lost_camps();
                common::forget_far_cache();
                common::forget_presumed();
                safety::forget_fled();
            }
            self.last_at_spawn_round = round;
        }
        if at_spawn || ai.shops_anywhere() {
            let build = match self.build {
                Some(build) => build,
                None => *self.build.insert(build_for(ai)?),
            };
            self.buy_items(build)?;
            if let Err(error) = common::locks::buy_lock() {
                println!("bot error (lock): {error}");
            }
        }
        common::checkpoint("turn-shop");

        if self.was_recalling && !me.is_recalling()? {
            self.was_recalling = false;
            if at_spawn {
                common::trace("wait-spawn");
                return Ok(());
            }
        }

        ai.support()?;
        // Cancel an enemy's tribute in reach with an altar lock (common::locks).
        let _ = common::locks::lock_enemy_tribute();
        common::checkpoint("turn-support");
        // Answer a new rally straight away (rally::RALLY_REPLIES).
        if let Err(error) = common::rally::answer() {
            println!("bot error (rally): {error}");
        }

        let low_hp = !at_spawn && safety::low_hp_recall()?;
        common::checkpoint("turn-low-hp-check");
        if low_hp {
            let _ = common::rally::drop_out();
            common::trace("low-hp");
            let result = safety::retreat_and_recall();
            if me.is_recalling().unwrap_or(false) {
                self.recall_reason = Some(RecallReason::LowHealth);
            }
            return result;
        }

        // A unit can drop out of vision mid-turn ("Unit is not in vision"); that shouldn't cost
        // the whole turn, so log it and carry on to banking and farming.
        match ai.fight() {
            Ok(true) => {
                let _ = common::rally::set_unavailable(true);
                return Ok(());
            }
            Ok(false) => {}
            Err(error) => println!("bot error (fight): {error}"),
        }
        common::checkpoint("turn-fight");
        let _ = common::rally::set_unavailable(false);
        // Standing on a cleared group's capture: stay until it pays out (collect::CAPTURE_HOLD).
        let capturing = collect::capturing()?;
        common::checkpoint("turn-capture-check");

        // Go home to spend gold and heal once the pull is strong enough (see base_willingness),
        // or, as the Group opening's carry, once we have enough gold (opening.rs).
        let rounds_away = cambc::round_number()?.saturating_sub(self.last_at_spawn_round);
        let greedy = GREED_HOME_WILLINGNESS > 1.0
            && me.health()? >= GREED_MIN_HEALTH * me.max_health()?
            && next_camp_close()?
            && safety::quiet_for(GREED_QUIET_ROUNDS)?;
        let home_at = if greedy { GREED_HOME_WILLINGNESS } else { 1.0 };
        let willingness = if HOME_STAT_GAIN > 0.0 && !ai.shops_anywhere() {
            // Missing health only (base_willingness without gold or rounds away), plus 1 once the
            // gold buys a real upgrade (HOME_STAT_GAIN).
            let hp = common::base_willingness(true, 0)?;
            let bar = if HOME_LATE_ROUND > 0 && cambc::round_number()? >= HOME_LATE_ROUND {
                HOME_STAT_GAIN_LATE
            } else {
                HOME_STAT_GAIN
            };
            if self.upgrade_gain()? >= bar {
                1.0 + hp
            } else {
                hp
            }
        } else {
            common::base_willingness(ai.shops_anywhere(), rounds_away)?
        };
        let wants_home = !at_spawn
            && !capturing
            && !(FINISH_CAMP_BEFORE_HOME && collect::in_camp_fight()?)
            && !common::end_game_bank()?
            && ((willingness >= home_at && willingness >= near_gold_home_at()?)
                || crate::opening::carry_wants_home(ai.shops_anywhere())?
                || common::rally::wants_recall()?);
        common::checkpoint("turn-base-check");
        if wants_home {
            if collect::grab_reward_before_recall()? {
                return Ok(());
            }
            if let Some(camps) = collect::ally_camp_fight_to_join()? {
                common::trace("help-ally-camp");
                return common::join_camp_fight(&camps);
            }
            if common::BANK_BEFORE_RECALL
                && me.gem_count()? > 0
                && common::unlocked_altar_within(common::BANK_BEFORE_RECALL_TILES)?
                && cambc::fuel_remaining()? >= MIN_FUEL_FOR_PATHING
            {
                common::trace("bank-first");
                return ai.bank_gems();
            }
            common::trace("go-home");
            let result = safety::retreat_and_recall();
            if me.is_recalling().unwrap_or(false) {
                self.recall_reason = Some(RecallReason::GoHome);
            }
            return result;
        }

        // Banking and farming pathfind (up to ~37k fuel); skip them rather than run out mid-turn.
        if cambc::fuel_remaining()? < MIN_FUEL_FOR_PATHING {
            common::trace("no-fuel");
            common::checkpoint("turn-pathing-gate");
            return Ok(());
        }

        let should_bank = !capturing && !collect::reward_waiting_nearby()? && ai.should_bank()?;
        common::checkpoint("turn-bank-check");
        if should_bank {
            if let Some(camps) = collect::ally_camp_fight_to_join_while_banking()? {
                common::trace("help-ally-before-bank");
                return common::join_camp_fight(&camps);
            }
            common::trace("bank");
            ai.bank_gems()?;
            return Ok(());
        }
        common::checkpoint("turn-farm-check");

        ai.farm()?;

        cambc::self_champion()?
            .expect("gameplay champion")
            // After calling a rally, act last next round to see every answer (rally::RALLY_AVAIL).
            .set_priority(if common::rally::awaiting_replies() {
                0
            } else {
                1 + crate::random_number(100) as i32
            })
    }
}

// What, if anything, outranks the go-home recall we're channelling (see CANCEL_GO_HOME_RECALL).
fn cancel_recall_for() -> Result<Option<&'static str>> {
    if CANCEL_RECALL_FOR_ENEMIES && !common::fightable_enemies()?.is_empty() {
        return Ok(Some("recall-cancel-enemy"));
    }
    if collect::capturing()? {
        return Ok(Some("recall-cancel-capture"));
    }
    if collect::ally_camp_fight_to_join()?.is_some() {
        return Ok(Some("recall-cancel-help"));
    }
    Ok(None)
}
