// Turn skeleton shared by every champion. main.rs dispatches here after the draft; champion-specific
// decisions live in champions/<name>.rs and shared helpers in common.rs.
use crate::champions::{self, ChampionAi};
use crate::common::me;
use cambc::{ItemType as Item, Result, Team};

// Fuel to keep in hand before starting a pathfinding step (bank_gems / farm).
const MIN_FUEL_FOR_PATHING: u64 = 40_000;

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
            let item = [Item::PowerShardOfStrength, Item::PowerShardOfFocus, Item::PowerShardOfHealth][self.chosen_stat.unwrap()];
            let count = gold / item.cost()?;
            if count > 0 && me.can_buy(item, count)? { me.buy_count(item, count)?; }
            return Ok(());
        }
        let mut owned = [0u32; 42];
        for (item, count) in me.item_count()?.iter() { owned[item as usize] = count; }
        let mut wanted = [0u32; 42]; // emblems the build calls for up to the current entry
        for i in 0..64 {
            let shard = build[i.min(build.len() - 1)];
            let Some(emblem) = shard.result()? else { continue };
            wanted[emblem as usize] += 1;
            if owned[emblem as usize] >= wanted[emblem as usize] { continue; }
            let cost = shard.cost()?;
            let to_finish = shard.combine_count()?.saturating_sub(owned[shard as usize]);
            let count = to_finish.min(gold / cost);
            if count == 0 || !me.can_buy(shard, count)? { break; }
            me.buy_count(shard, count)?;
            gold -= count * cost;
            if count < to_finish { break; }
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
        result
    }

    fn turn(&mut self, ai: &mut dyn ChampionAi) -> Result<()> {
        if self.chosen_stat.is_none() {
            self.chosen_stat = Some(crate::random_number(3) as usize);
            println!("draft: {}", String::from_utf8_lossy(&cambc::draft_memory().bytes()[..11]));
        }
        // Read before writing: little-endian u32 at shared-memory byte offset zero.
        let mut bytes = [0; 4];
        cambc::shared_memory().read(0, &mut bytes)?;
        println!("shared round: {}", u32::from_le_bytes(bytes));
        cambc::shared_memory().write(0, &cambc::round_number()?.to_le_bytes())?;

        let me = me()?;

        // Priorities: shop and recall, fight enemies, bank gems, then farm.
        if !me.is_alive()? || me.is_recalling()? || me.is_submitting_gems()? || me.is_teleporting()?
            || (!me.can_move()? && !me.can_attack()?) { self.was_recalling = true; return Ok(()); }

        let at_spawn = cambc::map::spawn_region(Team::Ally)?.is_set(me.position()?)?;
        if at_spawn { self.last_at_spawn_round = cambc::round_number()?; }
        if at_spawn || ai.shops_anywhere() { self.buy_items(ai.build())?; }

        if self.was_recalling && !me.is_recalling()? {
            self.was_recalling = false;
            if at_spawn {
                return Ok(());
            }
        }

        if !at_spawn && me.health()? < me.max_health()? * 0.35 {
            return crate::common::retreat_and_recall();
        }

        if ai.fight()? { return Ok(()); }

        // Go home to spend gold and heal once the pull is strong enough (see base_willingness).
        let rounds_away = cambc::round_number()?.saturating_sub(self.last_at_spawn_round);
        if !at_spawn && crate::common::base_willingness(ai.shops_anywhere(), rounds_away)? >= 1.0 {
            return crate::common::retreat_and_recall();
        }

        // Banking and farming pathfind (up to ~37k fuel); skip them rather than run out mid-turn.
        if cambc::fuel_remaining()? < MIN_FUEL_FOR_PATHING { return Ok(()); }

        if ai.should_bank()? {
            ai.bank_gems()?;
            return Ok(());
        }

        ai.farm()?;

        cambc::self_champion()?.expect("gameplay champion").set_priority(1 + crate::random_number(100) as i32)
    }
}
