use crate::constants::{draft_class, meta_weight, ClassKind, CLASS_COUNT};
use cambc::{ChampionDraft, Result, Team};

// Healer (Willow) class weight. Over 3,432 of our own games, teams with Willow won 45%, without
// her ~53%. 1.0: unchanged.
pub const HEALER_BIAS: f64 = 1.0;
// Base weight of each class's chance, before the pick-by-pick biases below. Over ~7,000 of our own
// teams (5 picks), win rates by count: AD 0/1/2 = 45/53/62%, bruisers 1/2/3 = 46/57/69%, AP
// 0/1/2 = 56/52/43%, healer 0/1 = 52/47%. Empty: unchanged.
// 2026-10-21, 132 games each: 1.5/1.5/0.5/0.5 won 75-57 against unchanged (gold +8%, gems
// tributed +10%); 2.0/2.0/0.3/0.3 won 73-59 against that.
// Engine 3.0.4 balance patch (Vesper, Vendetta, Tavi, Prism buffed; Stellar, Heart of Ruin, Ren
// nerfed), 420 games with every champion equally likely (2026-10-21), 5-pick teams by count:
// AD 0/1/2 = 38/54/79%, AP 0/1/2 = 63/52/38%, bruisers ~50% whatever the count, healer 49/51%.
pub const CLASS_WEIGHTS: [(ClassKind, f64); 4] = [
    (ClassKind::DpsAd, 3.0),
    (ClassKind::Bruiser, 1.0),
    (ClassKind::DpsAp, 0.2),
    (ClassKind::Healer, 0.6),
];

fn calc_balance() -> Result<[f64; CLASS_COUNT]> {
    let theirs = cambc::draft::current_draft(Team::Enemy)?;
    let mine = cambc::draft::current_draft(Team::Ally)?;
    let mut bias = [1.0; CLASS_COUNT];
    bias[ClassKind::Healer as usize] *= HEALER_BIAS;
    for (class, weight) in CLASS_WEIGHTS {
        bias[class as usize] *= weight;
    }
    if theirs.is_empty() {
        bias[ClassKind::Bruiser as usize] *= 20.0;
    }

    for i in 0..theirs.len() {
        match draft_class(theirs[i]) {
            ClassKind::Tank => {
                bias[ClassKind::DpsAp as usize] *= 5.0;
                bias[ClassKind::DpsAd as usize] *= 5.0;
            }
            ClassKind::Assassin => {
                bias[ClassKind::Bruiser as usize] *= 3.0;
                bias[ClassKind::DpsAd as usize] *= 0.7;
                bias[ClassKind::DpsAp as usize] *= 0.5;
                bias[ClassKind::Tank as usize] *= 3.5;
            }
            ClassKind::DpsAd | ClassKind::DpsAp => {
                bias[ClassKind::Assassin as usize] *= 7.8;
                bias[ClassKind::DpsAp as usize] *= 2.4;
            }
            ClassKind::Bruiser => {
                bias[ClassKind::DpsAd as usize] *= 2.7;
                bias[ClassKind::DpsAp as usize] *= 3.0;
                bias[ClassKind::Assassin as usize] *= 0.8;
            }
            ClassKind::Healer => {
                bias[ClassKind::DpsAd as usize] *= 1.2;
                bias[ClassKind::Bruiser as usize] *= 1.5;
                bias[ClassKind::Assassin as usize] *= 1.2;
                bias[ClassKind::DpsAp as usize] *= 0.8;
            }
        }
    }

    for i in 0..mine.len() {
        match draft_class(mine[i]) {
            ClassKind::Tank => {
                bias[ClassKind::DpsAp as usize] *= 5.0;
                bias[ClassKind::DpsAd as usize] *= 5.0;
                bias[ClassKind::Tank as usize] *= 0.2;
            }
            ClassKind::Assassin => {
                bias[ClassKind::Assassin as usize] *= 0.1;
                bias[ClassKind::Healer as usize] *= 2.0;
            }
            ClassKind::DpsAd | ClassKind::DpsAp => {
                bias[ClassKind::DpsAd as usize] *= 0.2;
                bias[ClassKind::DpsAp as usize] *= 0.2;
                bias[ClassKind::Tank as usize] *= 5.0;
                bias[ClassKind::Healer as usize] *= 1.2;
            }
            ClassKind::Bruiser => {
                bias[ClassKind::Bruiser as usize] *= 0.8;
                bias[ClassKind::Healer as usize] *= 1.2;
            }
            ClassKind::Healer => {
                bias[ClassKind::Healer as usize] *= 0.0;
            }
        }
    }

    Ok(bias)
}

// Teams of SMALL_TEAM_MAX champions or fewer draft by their own weights instead (0: off). From 720
// three-champion teams in 360 games with uniformly random drafts on the six 3-champion maps
// (2026-10-22), team win rates: with Vesper 77%, Marcella 58%, Tavi 54%, Ryth 53%, Ren 52%,
// Vendetta 49%, Varg 49%, Heart of Ruin 48%, Prism 44%, Stellar 43%, Cassi & Issac 39%, Willow 35%;
// AD carries 0/1/2 = 38/65/68%; with a healer 35%; two AP 34%; AD + bruiser + tank 82%, AD + AP +
// tank 81%, AD + bruiser + bruiser 78%. Four-champion teams (960 teams on the eight 4-champion
// maps): Vesper 68%, Marcella 56%, Tavi 53% ... Vendetta 46%, Cassi & Issac 38%; AD 0/1/2 =
// 35/56/73%; two assassins 38%; Willow neutral (50%), no frontline no worse. On the ladder a
// 4-champion Ren/Willow/Cassi/Vendetta team (no AD) lost 00gemfight twice with 21-22 deaths.
// Five-champion teams (1,560 teams on the 26 five-champion maps, v5 play): Vesper 60%, Marcella
// 53%, Tavi 52%, Ryth 51%, Prism 51%, Willow 49%, Varg 49%, Heart of Ruin 48%, Vendetta 48%, Ren
// 48%, Cassi & Issac 47%, Stellar 45%; AD 0/1/2 = 39/54/60%; two AP or two assassins 46%; no
// frontline 41%. The old weights (draft_class/calc_balance) gave us one AD in ~35% of five-champion
// games against v4 (we won ~35% of those) and none in ~5% (~15%).
// 2026-10-22, 320 games against the same bot with SMALL_TEAM_MAX 4: 188 (five-champion maps
// 130/208; gold +12%, gems tributed +12%, deaths 13.4 against 17.9).
// Rerun with v5's builds (UF5v2, 2026-10-22, 1,560 teams): Vesper 60%, Marcella 55%, Vendetta 55%
// (was 48%), Tavi 54%, Ryth 52%, Ren 51%, Prism 49%, Willow 47%, Varg 47%, Stellar 45%, Cassi &
// Issac 44%, Heart of Ruin 43% (was 48%); the five-champion weights follow. 320 games against the
// old weights: 190-130, gems tributed +13%, deaths 12.2 against 16.0.
pub const SMALL_TEAM_MAX: usize = 5;

// Heart of Ruin over Varg (the user, 2026-10-22): Varg was drafted more often (five champions:
// Varg 0.25, Heart 0.1), while the ladder's opponents take Heart in most of their games. On: the two
// swap weights where Varg's was higher (three and five champions). Off: the tables as they are.
// 2026-10-22, one game a map against bot-v5 without it: 24/40 and 21/40 (gold a round +16% and
// +19%, deaths 12.7 and 11.4 against 16.4, tributes +7% and +13%).
pub const HEART_OVER_VARG: bool = true;

// Meta draft (v6): draft the ladder's top-20 core in a fixed order, cut short on smaller teams, as
// every top-10 team does (2026-10-22 ladder analysis: Heart of Ruin in 85% of top-20 drafts against
// 10% for ranks 301+; Heart of Ruin, Marcella, Vesper Vale and Vendetta the four best picks after
// controlling for team, +2.4 to +5.9pp; Stellar, Varg, Cassi & Issac and Ren the worst). The
// gem-carry picks still come first on gem-carry maps. Off: the weighted draw above.
// 2026-10-23, against bot-v5 (the weighted draw), 416 games head to head: 214/416 (p = 0.59; gold
// even, deaths 14.1 against 15.1). Neutral; on, to draft like the ladder's top teams.
pub const META_DRAFT: bool = true;
const META_ORDER: [ChampionDraft; 5] = [
    ChampionDraft::VesperVale,
    ChampionDraft::Marcella,
    ChampionDraft::HeartOfRuin,
    ChampionDraft::Vendetta,
    ChampionDraft::Ryth,
];

// One AD carry first (Vesper over Tavi; with four, a second one is welcome), at most one AP and one
// assassin, no Cassi & Issac; with three also a frontline (tank or bruiser) by the last pick and no
// Willow. Weights roughly follow the win rates above.
fn small_team_weight(p: ChampionDraft, mine: &[ChampionDraft], left: usize, size: usize) -> f64 {
    use ChampionDraft as D;
    let base = if size <= 3 {
        match p {
            D::VesperVale => 1.0,
            D::Marcella => 0.6,
            D::Ryth => 0.5,
            D::Tavi => 0.35,
            D::Ren => 0.3,
            D::Vendetta => 0.2,
            D::Varg | D::HeartOfRuin => 0.15,
            D::Prism | D::Stellar => 0.1,
            D::CassiAndIssac | D::Willow => 0.0,
        }
    } else if size >= 5 {
        match p {
            D::VesperVale => 1.0,
            D::Marcella => 0.6,
            D::Tavi | D::Vendetta => 0.55,
            D::Ryth => 0.5,
            D::Ren => 0.4,
            D::Prism => 0.35,
            D::Willow | D::Varg => 0.25,
            D::CassiAndIssac | D::Stellar | D::HeartOfRuin => 0.1,
        }
    } else {
        match p {
            D::VesperVale => 1.0,
            D::Marcella | D::Tavi => 0.5,
            D::Prism => 0.4,
            D::Willow => 0.3,
            D::HeartOfRuin | D::Ren | D::Ryth => 0.25,
            D::Stellar | D::Varg => 0.2,
            D::Vendetta => 0.15,
            D::CassiAndIssac => 0.0,
        }
    };
    let base = match p {
        D::HeartOfRuin if HEART_OVER_VARG && size <= 3 => 0.15,
        D::Varg if HEART_OVER_VARG && size <= 3 => 0.1,
        D::HeartOfRuin if HEART_OVER_VARG && size >= 5 => 0.25,
        D::Varg if HEART_OVER_VARG && size >= 5 => 0.1,
        _ => base,
    };
    let class = draft_class(p);
    let count = |k: ClassKind| mine.iter().filter(|&&m| draft_class(m) == k).count();
    let frontline = |k: ClassKind| matches!(k, ClassKind::Tank | ClassKind::Bruiser);
    let has_frontline = mine.iter().any(|&m| frontline(draft_class(m)));
    let mut w = base;
    match class {
        ClassKind::DpsAd => {
            w *= match count(ClassKind::DpsAd) {
                0 => 5.0,
                1 if size <= 3 => 0.3,
                1 if size == 4 => 1.5,
                1 => 3.0,
                _ => 0.2,
            }
        }
        ClassKind::DpsAp | ClassKind::Assassin if count(class) > 0 => w = 0.0,
        _ => {}
    }
    if left == 1 {
        // The last pick fills what's missing: with five a frontline first (41% without), then an AD
        // carry (teams without one won 35-39%; with five, the second one: 54% -> 60%); with three a
        // frontline after the AD (47% without).
        if size >= 5 && !has_frontline {
            if !frontline(class) {
                w = 0.0;
            }
        } else if count(ClassKind::DpsAd) == 0 || (size >= 5 && count(ClassKind::DpsAd) < 2) {
            if class != ClassKind::DpsAd {
                w = 0.0;
            }
        } else if size <= 3 && !has_frontline && !frontline(class) {
            w = 0.0;
        }
    }
    // With five, two picks left and no AD yet: an AD now, so the last can be the second.
    if left == 2 && size >= 5 && count(ClassKind::DpsAd) == 0 && class != ClassKind::DpsAd {
        w = 0.0;
    }
    w
}

fn weighted_pick(biases: &[f64]) -> usize {
    let total: f64 = biases.iter().sum();

    // random float in [0, total), built from the starter's integer random_number
    const RES: u32 = 1_000_000;
    let mut r = crate::random_number(RES) as f64 / RES as f64 * total;

    for (i, &b) in biases.iter().enumerate() {
        if r < b {
            return i;
        }
        r -= b;
    }
    biases.len() - 1 // fallback for floating-point rounding at the very end
}

pub fn play() -> Result<()> {
    let class_bias = calc_balance()?;
    let ally_draft: Vec<ChampionDraft> = cambc::draft::current_draft(Team::Ally)?;
    let open = |p: &ChampionDraft| !ally_draft.contains(p);

    // Each class's chance is split between its champions we haven't picked by meta_weight, so the
    // class odds are unchanged by how many champions it has left (and a class with none left gets
    // nothing: an empty class used to panic, and the game then picked for us at random).
    let mut class_meta = [0.0; CLASS_COUNT];
    for p in ChampionDraft::ALL.iter().filter(|p| open(p)) {
        class_meta[draft_class(*p) as usize] += meta_weight(*p);
    }
    let picks: Vec<ChampionDraft> = ChampionDraft::ALL.iter().copied().filter(open).collect();
    let weights: Vec<f64> = picks
        .iter()
        .map(|&p| {
            let class = draft_class(p) as usize;
            class_bias[class] * meta_weight(p) / class_meta[class]
        })
        .collect();

    let mut weights = weights;
    let size = cambc::draft::team_size()?;
    if size <= SMALL_TEAM_MAX {
        let left = size.saturating_sub(ally_draft.len());
        let small: Vec<f64> = picks
            .iter()
            .map(|&p| small_team_weight(p, &ally_draft, left, size))
            .collect();
        if small.iter().sum::<f64>() > 0.0 {
            weights = small;
        }
    }

    // No debug print of the weights here: formatting floats costs ~30k fuel, and draft turns after
    // the first get only ~100k (running out means the game picks for us at random).
    crate::mix_draft_into_random();
    // Gem-carry maps: Marcella and Prism first (common::gemcarry).
    let meta = META_ORDER
        .into_iter()
        .find(|p| META_DRAFT && picks.contains(p));
    let choice = match (
        crate::common::gemcarry::draft_pick(&picks, &ally_draft),
        meta,
    ) {
        (Some(pick), _) | (None, Some(pick)) => pick,
        (None, None) => picks[weighted_pick(&weights)],
    };
    if crate::common::TRACE {
        println!("Picked {:?}", choice);
    }

    // Draft memory. It must be submitted before picking (the pick ends our draft turn:
    // submitting after it is engine error 18), and a failure here must not cost us the pick.
    if let Err(error) = submit_memory() {
        println!("draft memory failed: {error}");
    }
    cambc::draft::pick(choice)
}

// Fuel to keep for the pick itself; below this the memory isn't submitted (missing a pick means
// the game picks for us at random).
const MEMORY_MIN_FUEL: u64 = 20_000;

// The opening plan (opening.rs, worked out first: it's cheap) and the region table (regions.rs).
// Submitting replaces the whole memory, so both are written every time; the table stops searching
// when fuel runs low, so the pick is still made.
fn submit_memory() -> Result<()> {
    let opening = crate::opening::plan_bytes()?;
    let mut bytes = crate::regions::table_bytes()?;
    if !opening.is_empty() {
        bytes.resize(crate::opening::OPENING_OFFSET, 0);
        bytes.extend_from_slice(&opening);
    }
    if bytes.is_empty() || cambc::fuel_remaining()? < MEMORY_MIN_FUEL {
        return Ok(());
    }
    cambc::draft_memory().submit(&bytes)
}
