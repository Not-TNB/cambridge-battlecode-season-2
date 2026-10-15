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
    