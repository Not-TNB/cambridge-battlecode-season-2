// Willow (ranged healer).
// Slot 0: ally heal, up to 3 x Focus, range 4: cast on the most hurt ally in range (support()).
// Slot 1: attack reset.
use super::ChampionAi;
use crate::common::{distance, me};
use cambc::{ItemType as I, Result};

// Heal an ally in range whenever they are missing a meaningful amount of health.
pub const HEAL_BELOW: f32 = 0.75;

#[derive(Default)]
pub struct Willow;

impl ChampionAi for Willow {
    // Build: AP max; Power -> Bruiser.
    fn build(&self) -> &'static [I] {
        &[
            I::PowerShardOfFocus,
            I::BruiserShardOfFocus,
            I::PowerShardOfFocus,
        ]
    }

    // Heal the ally in range with the lowest health fraction, if it's below HEAL_BELOW.
    fn support(&mut self) -> Result<()> {
        let me = me()?;
        let Some(heal) = me.abilities().iter().find(|a| a.is_heal()) else {
            return Ok(());
        };
        let Some(info) = heal.info()? else {
            return Ok(());
        };
        if !heal.is_ready()? {
            return Ok(());
        }
        let range = info.as_dyn().range()? as i32;
        let mut best = None;
        let sight = crate::common::sight()?;
        for ally in &sight.allies {
            if !ally.is_alive()? {
                continue;
            }
            if distance(ally.position()?, me.position()?) > range {
                continue;
            }
            let fraction = ally.health()? / ally.max_health()?;
            if fraction < HEAL_BELOW && best.as_ref().is_none_or(|(_, f)| fraction < *f) {
                best = Some((ally, fraction));
            }
        }
        if let Some((ally, _)) = best {
            heal.cast_on(ally)?;
        }
        Ok(())
    }
}
