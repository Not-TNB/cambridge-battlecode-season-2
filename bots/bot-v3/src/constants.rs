use cambc::{ChampionDraft, ChampionType};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ClassKind {
    Assassin,
    DpsAd,
    DpsAp,
    Healer,
    Tank,
    Bruiser,
}

impl ClassKind {
    pub const ALL: [ClassKind; 6] = [
        ClassKind::Assassin,
        ClassKind::DpsAd,
        ClassKind::DpsAp,
        ClassKind::Healer,
        ClassKind::Tank,
        ClassKind::Bruiser,
    ];

    // The carries camp rewards are funnelled to (camps.rs TANK_FUNNEL): AD carries, and AP carries
    // too with FUNNEL_AP_CARRIES (Prism got 1 capture to Vesper Vale's 15 in one hedgerows game).
    pub fn is_funnel_carry(self) -> bool {
        self == ClassKind::DpsAd || (FUNNEL_AP_CARRIES && self == ClassKind::DpsAp)
    }
}

pub const CLASS_COUNT: usize = ClassKind::ALL.len();
pub const FUNNEL_AP_CARRIES: bool = true;

// Class of a draft pick.
pub fn draft_class(champion: ChampionDraft) -> ClassKind {
    use ChampionDraft as D;
    match champion {
        D::Ryth => ClassKind::Tank,
        D::VesperVale | D::Tavi => ClassKind::DpsAd,
        D::Vendetta | D::CassiAndIssac => ClassKind::Assassin,
        D::Stellar | D::Prism => ClassKind::DpsAp,
        D::Willow => ClassKind::Healer,
        D::Marcella | D::Ren | D::Varg | D::HeartOfRuin => ClassKind::Bruiser,
    }
}

// How strong a pick is within its class after the balance patch: the draft splits a class's chance
// between its available champions by these weights, roughly 80/20 between meta and off-meta picks
// (a third champion gets 80/20 of the remainder).
pub fn meta_weight(champion: ChampionDraft) -> f64 {
    use ChampionDraft as D;
    match champion {
        // Bruisers: Marcella = Heart of Ruin (new in 3.0) -> Ren -> Varg.
        D::Marcella | D::HeartOfRuin => 0.40,
        D::Ren => 0.16,
        // Varg (was 0.04): teams with him won 6-39% of our own games, and the bruiser class weight
        // (draft::CLASS_WEIGHTS) drafts bruisers more. Not 0: a class's weights are divided by
        // their sum, which would be 0 with only Varg left.
        D::Varg => 0.001,
        D::Prism => 0.80,
        D::Stellar => 0.20,
        // Vesper over Tavi (was 0.50 each): over 3,432 of our own games, teams with Vesper won 65%,
        // with Tavi 44% (Vesper's passive: +20% gold). 2026-10-21, 132 games: 77-55 against 0.50
        // each, gold +7.5%, gems tributed +10%.
        D::VesperVale => 0.95,
        D::Tavi => 0.05,
        D::Vendetta => 0.80,
        D::CassiAndIssac => 0.20,
        D::Ryth | D::Willow => 1.0,
    }
}

// Class of a champion in play (ChampionType splits Cassi & Issac, which share one draft class).
pub fn class_of(champion: ChampionType) -> ClassKind {
    draft_class(draft_of(champion))
}

// The draft pick a champion in play came from (Cassi and Issac share one).
pub fn draft_of(champion: ChampionType) -> ChampionDraft {
    use ChampionType as C;
    match champion {
        C::Ryth => ChampionDraft::Ryth,
        C::VesperVale => ChampionDraft::VesperVale,
        C::Vendetta => ChampionDraft::Vendetta,
        C::Cassi | C::Issac => ChampionDraft::CassiAndIssac,
        C::Stellar => ChampionDraft::Stellar,
        C::Prism => ChampionDraft::Prism,
        C::Marcella => ChampionDraft::Marcella,
        C::Willow => ChampionDraft::Willow,
        C::Ren => ChampionDraft::Ren,
        C::Varg => ChampionDraft::Varg,
        C::Tavi => ChampionDraft::Tavi,
        C::HeartOfRuin => ChampionDraft::HeartOfRuin,
    }
}
