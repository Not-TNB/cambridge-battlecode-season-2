// The three playstyles. Each is a parameter set plus a draft list and a build; the brain is shared.
// Thresholds are fractions or multiples of map quantities (spawn gap, max health), not fixed tiles.
use cambc::{ChampionDraft as D, ChampionType as C, ItemFamily as F};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Farm = 0,
    Fight = 1,
    Gem = 2,
}

impl Kind {
    pub fn from_u8(v: u8) -> Kind {
        match v {
            1 => Kind::Fight,
            2 => Kind::Gem,
            _ => Kind::Farm,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    Main,
    Health,
}

pub struct Params {
    pub kind: Kind,
    // Engage only when our side's fight strength is at least this multiple of theirs.
    pub fight_margin: f32,
    // Fight when the simulation has us killing at least this many more than we lose.
    pub trade_margin: i32,
    // Below this health fraction, disengage and recall.
    pub retreat_hp: f32,
    // Below this with nothing pressing, go heal.
    pub heal_hp: f32,
    // Target-score multiplier for a camp group an ally has claimed (<1 spreads us out).
    pub claimed_mult: f32,
    // Value of one gold in gems, for target scoring (early game; gems weigh more later).
    pub gold_weight: f32,
    // From this round, prefer camps with gems over gold-only ones.
    pub gem_round: i32,
    // Gold that justifies a trip home to shop (Tavi shops anywhere).
    pub trip_gold: u32,
    // Carry at most this many gems before banking.
    pub bank_gems: u32,
    // Chase visible enemies we beat, up to this many steps from our target.
    pub chase_steps: i32,
    // Value (gems-equivalent) a camp group needs before we call allies to it.
    pub rally_value: f32,
    // Rally answer radius, as a fraction of the spawn gap.
    pub rally_radius: f32,
    pub picks: &'static [D],
    pub build: &'static [(F, Stat)],
}

pub const FARM: Params = Params {
    kind: Kind::Farm,
    fight_margin: 1.5,
    trade_margin: 1,
    retreat_hp: 0.35,
    heal_hp: 0.3,
    claimed_mult: 0.45,
    gold_weight: 0.12,
    gem_round: 150,
    trip_gold: 300,
    bank_gems: 30,
    chase_steps: 3,
    rally_value: 25.0,
    rally_radius: 0.45,
    picks: &[D::VesperVale, D::Tavi, D::Varg, D::Willow, D::Vendetta, D::CassiAndIssac],
    build: &[
        (F::Monster, Stat::Main),
        (F::Treasure, Stat::Main),
        (F::Prospector, Stat::Main),
        (F::Jungle, Stat::Health),
        (F::Durability, Stat::Health),
        (F::Skirmish, Stat::Main),
        (F::Collectors, Stat::Main),
    ],
};

pub const FIGHT: Params = Params {
    kind: Kind::Fight,
    fight_margin: 1.05,
    trade_margin: 0,
    retreat_hp: 0.22,
    heal_hp: 0.45,
    claimed_mult: 1.15,
    gold_weight: 0.1,
    gem_round: 150,
    trip_gold: 300,
    bank_gems: 12,
    chase_steps: 8,
    rally_value: 12.0,
    rally_radius: 0.7,
    picks: &[D::Ryth, D::Stellar, D::HeartOfRuin, D::Ren, D::Vendetta, D::Varg, D::Willow],
    build: &[
        (F::Durability, Stat::Health),
        (F::Skirmish, Stat::Main),
        (F::Hunter, Stat::Main),
        (F::Home, Stat::Main),
        (F::Capture, Stat::Main),
        (F::Treasure, Stat::Main),
        (F::Collectors, Stat::Main),
    ],
};

pub const GEM: Params = Params {
    kind: Kind::Gem,
    fight_margin: 1.3,
    trade_margin: 1,
    retreat_hp: 0.33,
    heal_hp: 0.5,
    claimed_mult: 0.35,
    gold_weight: 0.08,
    gem_round: 0,
    trip_gold: 300,
    bank_gems: 25,
    chase_steps: 3,
    rally_value: 30.0,
    rally_radius: 0.4,
    picks: &[D::Prism, D::Marcella, D::Varg, D::VesperVale, D::Stellar, D::Tavi],
    build: &[
        (F::Monster, Stat::Main),
        (F::Prospector, Stat::Main),
        (F::Capture, Stat::Main),
        (F::Tribute, Stat::Main),
        (F::Skirmish, Stat::Main),
        (F::Durability, Stat::Health),
        (F::Collectors, Stat::Main),
    ],
};

pub fn params(kind: Kind) -> &'static Params {
    match kind {
        Kind::Farm => &FARM,
        Kind::Fight => &FIGHT,
        Kind::Gem => &GEM,
    }
}

// The stat a champion's damage scales with, per profile (fight tries AP Tavi/Marcella/Ren).
pub fn main_stat(c: C, kind: Kind) -> cambc::ItemStat {
    use cambc::ItemStat::*;
    match c {
        C::Stellar | C::Prism | C::Cassi | C::HeartOfRuin => Focus,
        C::Ren => {
            if kind == Kind::Fight {
                Focus
            } else {
                Health
            }
        }
        C::Ryth => Health,
        C::Tavi | C::Marcella if kind == Kind::Fight => Focus,
        _ => Strength,
    }
}

// Champions per party: the fight bot moves as one; the farm bot pairs up unless the map is mostly
// clusters (which need no help); the gem bot spreads out.
pub fn party_size(kind: Kind, team: usize, harmless_share: f32) -> usize {
    // Leader-following (a whole-team party) deadlocked the fight bot on concentrated-wealth and
    // cost the farm bot 40% of its gold: nearest-camp farming keeps everyone together anyway.
    let _ = team;
    let n = match kind {
        Kind::Fight => 1,
        Kind::Farm | Kind::Gem => {
            let _ = harmless_share;
            1
        }
    };
    n.clamp(1, team.max(1))
}
