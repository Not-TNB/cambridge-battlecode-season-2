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
pub const CLASS_WEIGHTS: [(ClassKind, f64); 4] = [
    (ClassKind::DpsAd, 2.0),
    (ClassKind::Bruiser, 2.0),
    (ClassKind::DpsAp, 0.3),
    (ClassKind::Healer, 0.3),
];

fn calc_balance() -> Result<[f64; CLASS_COUNT]> {
    let theirs = cambc::draft::current_draft(Team::Enemy)?;
    let mine = cambc::draft::current_draft(Team::Ally)?;
    let mut bias = [1.0; CLASS_COUNT];
    bias[ClassKind::Healer as usize] *= HEALER_BIAS;
    for (class, weight) in CLASS_WEIGHTS {
        bias[class as usize] *= weight;
    }
    if theirs.len() == 0 {
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

    // No debug print of the weights here: formatting floats costs ~30k fuel, and draft turns after
    // the first get only ~100k (running out means the game picks for us at random).
    let choice = picks[weighted_pick(&weights)];
    println!("Picked {:?}", choice);

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
