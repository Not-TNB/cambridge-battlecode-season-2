// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.
// One file per champion. Each implements `ChampionAi`, overriding only the decisions where
// its kit should play differently; everything else falls back to the shared logic in common/.
use cambc::{ChampionType, ItemType, Result};

mod cassi;
mod heart_of_ruin;
mod issac;
mod marcella;
mod prism;
mod ren;
mod ryth;
mod stellar;
mod tavi;
mod varg;
mod vendetta;
mod vesper_vale;
mod willow;

// Decision points bot.rs calls each turn, in priority order: shop, fight, bank gems, farm.
pub trait ChampionAi {
    // Emblems to complete in order, each given by the shard that builds it (five combine into
    // one emblem); the last entry repeats once the list is done. Empty = random single stat.
    fn build(&self) -> &'static [ItemType] {
        &[]
    }

    // Whether this champion may buy items away from its spawn.
    fn shops_anywhere(&self) -> bool {
        false
    }

    // Free actions each turn that don't use up the turn (heals, shields). Runs before recalls and
    // fighting, so it also happens while retreating.
    fn support(&mut self) -> Result<()> {
        Ok(())
    }

    // Act against visible enemy champions; return true if that used up the turn.
    fn fight(&mut self) -> Result<bool> {
        crate::v2::common::fight_enemies()
    }

    // Whether to head for an altar now rather than keep farming with the gems we carry.
    fn should_bank(&self) -> Result<bool> {
        crate::v2::common::should_bank()
    }

    // Carry gems to an altar and tribute them.
    fn bank_gems(&mut self) -> Result<()> {
        crate::v2::common::bank_gems()
    }

    // Clear camps and capture their rewards.
    fn farm(&mut self) -> Result<()> {
        crate::v2::common::farm_camps()
    }
}

// Build the per-champion AI for the champion this program instance controls.
pub fn for_champion(champion: ChampionType) -> Box<dyn ChampionAi> {
    match champion {
        ChampionType::Cassi => Box::new(cassi::Cassi::default()),
        ChampionType::Issac => Box::new(issac::Issac::default()),
        ChampionType::Marcella => Box::new(marcella::Marcella::default()),
        ChampionType::Prism => Box::new(prism::Prism::default()),
        ChampionType::Ren => Box::new(ren::Ren::default()),
        ChampionType::Ryth => Box::new(ryth::Ryth::default()),
        ChampionType::Stellar => Box::new(stellar::Stellar::default()),
        ChampionType::Tavi => Box::new(tavi::Tavi::default()),
        ChampionType::Varg => Box::new(varg::Varg::default()),
        ChampionType::Vendetta => Box::new(vendetta::Vendetta::default()),
        ChampionType::VesperVale => Box::new(vesper_vale::VesperVale::default()),
        ChampionType::Willow => Box::new(willow::Willow::default()),
        ChampionType::HeartOfRuin => Box::new(heart_of_ruin::HeartOfRuin::default()),
    }
}
