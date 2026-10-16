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
    ai: Option<Box<dyn ChampionAi>>,
}

impl Bot {
    fn buy_items(&self) -> Result<()> {
        // Spend only on power shards of the randomly chosen stat.
        let me = me()?;
        let item = [Item::PowerShardOfStrength, Item::PowerShardOfFocus, Item::PowerShardOfHealth][self.chosen_stat.unwrap()];
        let count = me.gold_count()? / item.cost()?;
        if count > 0 && me.can_buy(item, count)? { me.buy_count(item, count)?; }
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
        if at_spawn || ai.shops_anywhere() { self.buy_items()?; }

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

        // Banking and farming pathfind (up to ~37k fuel); skip them rather than run out mid-turn.
        if cambc::fuel_remaining()? < MIN_FUEL_FOR_PATHING { return Ok(()); }

        if me.gem_count()? > 0 {
            ai.bank_gems()?;
            return Ok(());
        }

        ai.farm()?;

        cambc::self_champion()?.expect("gameplay champion").set_priority(1 + crate::random_number(100) as i32)
    }
}
