#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ClassKind {
    Assassin,
    DpsAd,
    DpsAp,
    Healer,
    Tank,
    Bruiser,
    AssassinAndDpsAp
}

impl ClassKind {
    pub const ALL: [ClassKind; 7] = [
        ClassKind::Assassin, ClassKind::DpsAd, ClassKind::DpsAp, ClassKind::Healer,
        ClassKind::Tank, ClassKind::Bruiser, ClassKind::AssassinAndDpsAp,
    ];
}

pub const CLASS_COUNT: usize = 7;
pub const CHAMP_COUNT: usize = 11;

pub const CHAMP_CLASSES: [ClassKind; CHAMP_COUNT] = [
    ClassKind::Tank, // Ryth
    ClassKind::DpsAd, // VV
    ClassKind::Assassin, // Vendetta
    ClassKind::AssassinAndDpsAp, // C&I
    ClassKind::DpsAp, // Stellar
    ClassKind::DpsAp, // Prism
    ClassKind::Tank, // Marcella
    ClassKind::Healer, // Willow
    ClassKind::Bruiser, // Ren
    ClassKind::Bruiser, // Varg
    ClassKind::AssassinAndDpsAp // Tavi
];
    
// Class of a champion in play (ChampionType splits Cassi & Issac, which share one draft class).
pub fn class_of(champion: cambc::ChampionType) -> ClassKind {
    use cambc::ChampionType as C;
    let draft = match champion {
        C::Ryth => 0, C::VesperVale => 1, C::Vendetta => 2, C::Cassi | C::Issac => 3, C::Stellar => 4,
        C::Prism => 5, C::Marcella => 6, C::Willow => 7, C::Ren => 8, C::Varg => 9, C::Tavi => 10,
    };
    CHAMP_CLASSES[draft]
}

impl ClassKind {
    // Attack-damage carries: the champions camp rewards are funnelled to.
    pub fn is_ad_carry(self) -> bool {
        self == ClassKind::DpsAd
    }
}
