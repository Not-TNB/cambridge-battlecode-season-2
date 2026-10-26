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

// v9 (2026-10-25, teammate's ladder report: 485 scrims against the top 100): Vesper Vale and
// Vendetta take Health emblems as the top-30 teams do (Strength 73/61%, Health 27/39% for them,
// against 95/91% Strength for us; 2+ Health emblems in the first four win 56-57% against 53% in
// 3,642 top-100 ranked games), and alternate Strength and Health instead of repeating Bruiser
// Strength. Our Vendetta died 3.5 times a game against 1.9 for theirs. Off: Strength builds.
// 2026-10-25, 120 games against bot-v8 with a same-seed control (55/120): 59/120 (+27/-23), team
// deaths 11.9 against 13.2 (Vesper 1.4 against 1.9, Vendetta 2.9 against 3.7), gold -6% (Vesper's
// clears are slower). With FUNNEL_MELEE off as well: 59/120, gold -10%. On.
pub const HEALTH_BUILDS: bool = true;

// Decision points bot.rs calls each turn, in priority order: shop, fight, bank gems, farm.
pub trait ChampionAi {
    // Emblems to complete in order, each given by the shard that builds it (five combine into
    // one emblem); the last entry repeats once the list is done. Empty = random single stat.
    // Frontliners (tanks and bruisers, who take the camps' hits) open with a Jungle emblem, 30%
    // less damage from camps: a tank tanking an elk alone went home after ~30 rounds of it. Against
    // the same builds without it (2026-10-21, 132 games): 77-55, gems tributed +9%.
    // Stats come in Bruiser emblems (5 shards of the stat plus one of each other: 7 shards' worth,
    // where Power is 6), as the top ladder teams build. After the first one, a Durability Health
    // emblem (10% less damage from everything; a second emblem's passive adds nothing). Against the
    // same builds without it (2026-10-22, 320 games): 188-132, gems +8%, deaths 13.5 against 15.5.
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
        crate::common::fight_enemies()
    }

    // Whether to head for an altar now rather than keep farming with the gems we carry.
    fn should_bank(&self) -> Result<bool> {
        crate::common::should_bank()
    }

    // Carry gems to an altar and tribute them.
    fn bank_gems(&mut self) -> Result<()> {
        crate::common::bank_gems()
    }

    // Clear camps and capture their rewards.
    fn farm(&mut self) -> Result<()> {
        crate::common::farm_camps()
    }
}

// Build the per-champion AI for the champion this program instance controls.
pub fn for_champion(champion: ChampionType) -> Box<dyn ChampionAi> {
    match champion {
        ChampionType::Cassi => Box::new(cassi::Cassi),
        ChampionType::Issac => Box::new(issac::Issac),
        ChampionType::Marcella => Box::new(marcella::Marcella),
        ChampionType::Prism => Box::new(prism::Prism),
        ChampionType::Ren => Box::new(ren::Ren),
        ChampionType::Ryth => Box::new(ryth::Ryth),
        ChampionType::Stellar => Box::new(stellar::Stellar),
        ChampionType::Tavi => Box::new(tavi::Tavi),
        ChampionType::Varg => Box::new(varg::Varg::default()),
        ChampionType::Vendetta => Box::new(vendetta::Vendetta),
        ChampionType::VesperVale => Box::new(vesper_vale::VesperVale),
        ChampionType::Willow => Box::new(willow::Willow),
        ChampionType::HeartOfRuin => Box::new(heart_of_ruin::HeartOfRuin::default()),
    }
}
