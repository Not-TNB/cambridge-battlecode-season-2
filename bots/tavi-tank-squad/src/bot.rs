// Turn skeleton shared by every champion. main.rs dispatches here after the draft; champion-specific
// decisions live in champions/<name>.rs and shared helpers in common.rs.
use crate::champions::{self, ChampionAi};
use crate::common::me;
use crate::{collect, common, safety};
use cambc::{ItemType as Item, Result, Team};

// Fuel to keep in hand before starting a pathfinding step (bank_gems / farm).
const MIN_FUEL_FOR_PATHING: u64 = 40_000;
const MIN_FUEL_FOR_LOW_HP_CAPTURE_CHECK: u64 = MIN_FUEL_FOR_PATHING + 10_000;
// Fuel to keep in hand before end-of-turn extras (sightings, wards; up to ~20k).
const SPARE_FUEL: u64 = 30_000;

// Per-champion state survives between turns. Pick one stat and keep buying it.
#[derive(Default)]
pub struct Bot {
    chosen_stat: Option<usize>,
    was_recalling: bool,
    last_at_spawn_round: u32,
    ai: Option<Box<dyn ChampionAi>>,
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
        }
        if common::TRACE {
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
        result
    }

    fn turn(&mut self, ai: &mut dyn ChampionAi) -> Result<()> {
        if self.chosen_stat.is_none() {
            self.chosen_stat = Some(crate::random_number(3) as usize);
            println!("draft memory: {} bytes", cambc::draft_memory().len());
        }
        let me = me()?;
        common::checkpoint("turn-start");

        // Priorities: shop and recall, fight enemies, bank gems, then farm.
        if !me.is_alive()? {
            common::trace("skip-dead");
            self.was_recalling = true;
            if me.champion_name()? == cambc::ChampionType::Tavi {
                crate::squad::signal_tavi_dead()?;
            }
            return Ok(());
        }
        if crate::squad::is_squad_bot()? {
            let priority = if me.champion_name()? == cambc::ChampionType::Tavi {
                1
            } else {
                100
            };
            me.set_priority(priority)?;
        }
        if me.is_recalling()? {
            common::trace("skip-recalling");
            self.was_recalling = true;
            return Ok(());
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
            self.last_at_spawn_round = cambc::round_number()?;
        }
        if at_spawn || ai.shops_anywhere() {
            let build = if me.champion_name()? == cambc::ChampionType::Tavi
                && cambc::round_number()? >= crate::squad::TAVI_LATE_BUILD_ROUND
            {
                crate::champions::tavi_late_build()
            } else {
                ai.build()
            };
            self.buy_items(build)?;
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
        common::checkpoint("turn-support");

        if crate::squad::is_squad_bot()? {
            return crate::squad::take_turn(ai);
        }

        let mut low_hp_stay_for_capture = false;
        if !at_spawn && me.health()? < me.max_health()? * 0.35 {
            low_hp_stay_for_capture = cambc::fuel_remaining()? >= MIN_FUEL_FOR_LOW_HP_CAPTURE_CHECK
                && !safety::imminent_danger()?
                && common::nearby_capture_reward()?
                && cambc::fuel_remaining()? >= MIN_FUEL_FOR_PATHING;
            if !low_hp_stay_for_capture {
                common::trace("low-hp");
                return safety::retreat_and_recall();
            }
            common::trace("low-hp-capture");
        }
        // Tanks and Willow go home with the damage dealers they escort.
        if !at_spawn && collect::escort_recalls()? {
            common::trace("escort-recall");
            return safety::retreat_and_recall();
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

        // Go home to spend gold and heal once the pull is strong enough (see base_willingness).
        let rounds_away = cambc::round_number()?.saturating_sub(self.last_at_spawn_round);
        let wants_home = !at_spawn
            && !capturing
            && !low_hp_stay_for_capture
            && crate::common::base_willingness(ai.shops_anywhere(), rounds_away)? >= 1.0;
        common::checkpoint("turn-base-check");
        if wants_home && (!safety::GO_HOME_NEEDS_ENEMY || !common::sight()?.enemies.is_empty()) {
            if crate::common::BANK_BEFORE_RECALL
                && me.gem_count()? > 0
                && crate::common::unlocked_altar_within(crate::common::BANK_BEFORE_RECALL_TILES)?
                && cambc::fuel_remaining()? >= MIN_FUEL_FOR_PATHING
            {
                common::trace("bank-first");
                return ai.bank_gems();
            }
            common::trace("go-home");
            return safety::retreat_and_recall();
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
            common::trace("bank");
            ai.bank_gems()?;
            return Ok(());
        }
        common::checkpoint("turn-farm-check");

        if !capturing {
            if let Some(goal) = collect::escort_goal()? {
                common::trace("escort");
                common::move_towards_closest(goal)?;
                return Ok(());
            }
        }

        ai.farm()?;

        Ok(())
    }
}
