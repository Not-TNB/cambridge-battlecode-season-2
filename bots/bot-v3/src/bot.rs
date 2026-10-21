// Turn skeleton shared by every champion. main.rs dispatches here after the draft; champion-specific
// decisions live in champions/<name>.rs and shared helpers in common/.
use crate::champions::{self, ChampionAi};
use crate::common::{self, collect, me, safety};
use cambc::{ItemType as Item, Result, Team};

// Fuel to keep in hand before starting a pathfinding step (bank_gems / farm).
const MIN_FUEL_FOR_PATHING: u64 = 40_000;
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

// Per-champion state survives between turns. Pick one stat and keep buying it.
#[derive(Default)]
pub struct Bot {
    recall_reason: Option<RecallReason>,
    chosen_stat: Option<usize>,
    was_recalling: bool,
    last_at_spawn_round: u32,
    ai: Option<Box<dyn ChampionAi>>,
    build: Option<&'static [Item]>,
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

// The champion's build, opened with a Treasure emblem of its main stat on farming maps when it's
// our top carry (opening::treasure_first), unless the build already has one.
fn build_for(ai: &dyn ChampionAi) -> Result<&'static [Item]> {
    let build = ai.build();
    let has_treasure = build
        .iter()
        .any(|&i| matches!(i as u32, 18..=23));
    let Some(&first) = build.first() else {
        return Ok(build);
    };
    if has_treasure || !crate::opening::treasure_first(me()?.champion_name()?) {
        return Ok(build);
    }
    let mut opened = vec![treasure_shard(first)];
    opened.extend_from_slice(build);
    println!("build: Treasure first ({:?})", opened[0]);
    Ok(Vec::leak(opened))
}

impl Bot {
    // Follow the champion's build order: complete its emblems one at a time, buying only the
    // shards the current emblem still needs, and repeat the last entry once the list is done.
    // Champions without a build order buy power shards of one randomly chosen stat.
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
        for i in 0..64 {
            let shard = build[i.min(build.len() - 1)];
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
            println!(
                "T {:?} r{} {},{} hp={}/{} gems={} fuel={} |{}",
                me.champion_name()?,
                cambc::round_number()?,
                me.position()?.x,
                me.position()?.y,
                me.health()? as i32,
                me.max_health()? as i32,
                me.gem_count()?,
                cambc::fuel_remaining()?,
                common::take_trace()
            );
        }
        Ok(())
    }

    fn turn(&mut self, ai: &mut dyn ChampionAi) -> Result<()> {
        if self.chosen_stat.is_none() {
            self.chosen_stat = Some(crate::random_number(3) as usize);
            println!("draft memory: {} bytes", cambc::draft_memory().len());
        }
        let me = me()?;
        common::checkpoint("turn-start");

        // Damage dealers tell follow-only escorts where they are (collect::ESCORT_FOLLOW_ONLY).
        if let Err(error) = collect::publish_lead() {
            println!("bot error (lead): {error}");
        }

        // Priorities: shop and recall, fight enemies, bank gems, then farm.
        if !me.is_alive()? {
            common::trace("skip-dead");
            self.was_recalling = true;
            return Ok(());
        }
        if me.is_recalling()? {
            let cancel = if CANCEL_GO_HOME_RECALL && self.recall_reason == Some(RecallReason::GoHome) {
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

        let low_hp = !at_spawn && safety::low_hp_recall()?;
        common::checkpoint("turn-low-hp-check");
        if low_hp {
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
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => println!("bot error (fight): {error}"),
        }
        common::checkpoint("turn-fight");
        // Standing on a cleared group's capture: stay until it pays out (collect::CAPTURE_HOLD).
        let capturing = collect::capturing()?;
        common::checkpoint("turn-capture-check");

        // Go home to spend gold and heal once the pull is strong enough (see base_willingness),
        // or, as the Group opening's carry, once we have enough gold (opening.rs).
        let rounds_away = cambc::round_number()?.saturating_sub(self.last_at_spawn_round);
        let wants_home = !at_spawn
            && !capturing
            && !(FINISH_CAMP_BEFORE_HOME && collect::in_camp_fight()?)
            && !crate::common::end_game_bank()?
            && (crate::common::base_willingness(ai.shops_anywhere(), rounds_away)? >= 1.0
                || crate::opening::carry_wants_home(ai.shops_anywhere())?);
        common::checkpoint("turn-base-check");
        if wants_home {
            if collect::grab_reward_before_recall()? {
                return Ok(());
            }
            if let Some(camps) = collect::ally_camp_fight_to_join()? {
                common::trace("help-ally-camp");
                return common::join_camp_fight(&camps);
            }
            if crate::common::BANK_BEFORE_RECALL
                && me.gem_count()? > 0
                && crate::common::unlocked_altar_within(crate::common::BANK_BEFORE_RECALL_TILES)?
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

        let should_bank = !capturing && ai.should_bank()?;
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
            .set_priority(1 + crate::random_number(100) as i32)
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
