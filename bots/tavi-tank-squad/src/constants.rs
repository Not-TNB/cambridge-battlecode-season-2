use cambc::{ChampionDraft, ChampionType};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ClassKind {
    Assassin,
    DpsAd,
    DpsAp,
    Healer,
    Tank,
    Bruiser,
    AssassinAndDpsAp,
}

impl ClassKind {
    // Attack-damage carries: the champions camp rewards are funnelled to.
    pub fn is_ad_carry(self) -> bool {
        self == ClassKind::DpsAd
    }
}

// Class of a draft pick.
pub fn draft_class(champion: ChampionDraft) -> ClassKind {
    use ChampionDraft as D;
    match champion {
        D::Ryth | D::Marcella => ClassKind::Tank,
        D::VesperVale => ClassKind::DpsAd,
        D::Vendetta => ClassKind::Assassin,
        D::CassiAndIssac | D::Tavi => ClassKind::AssassinAndDpsAp,
        D::Stellar | D::Prism => ClassKind::DpsAp,
        D::Willow => ClassKind::Healer,
        D::Ren | D::Varg | D::HeartOfRuin => ClassKind::Bruiser,
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
        C::HeartOfRuin => ChampionDraft::HeartOfRuin,
        C::Tavi => ChampionDraft::Tavi,
    }
}
