use cambc::{ChampionDraft, Result, Team};
use crate::constants::{CLASS_COUNT, ClassKind, draft_class};


fn calc_balance() -> Result<[f64; CLASS_COUNT]> {
    let theirs = cambc::draft::current_draft(Team::Enemy)?;
    let mine   = cambc::draft::current_draft(Team::Ally)?;
    let mut bias = [1.0; CLASS_COUNT];
    if theirs.len() == 0 {
        bias[ClassKind::Bruiser as usize] *= 20.0;
    }
    
    for i in 0..theirs.len() {
        match draft_class(theirs[i]) {
            ClassKind::Tank => {
                bias[ClassKind::DpsAp as usize] *= 5.0;
                bias[ClassKind::DpsAd as usize] *= 5.0;
            }
            ClassKind::Assassin | ClassKind::AssassinAndDpsAp => {
                bias[ClassKind::Bruiser as usize] *= 3.0;
                bias[ClassKind::DpsAd as usize]   *= 0.7;
                bias[ClassKind::DpsAp as usize]   *= 0.5;
                bias[ClassKind::Tank as usize]    *= 3.5;
            }
            ClassKind::DpsAd | ClassKind::DpsAp => {
                bias[ClassKind::Assassin as usize] *= 7.8;
                bias[ClassKind::AssassinAndDpsAp as usize] *= 6.3;
                bias[ClassKind::DpsAp as usize] *= 2.4;
            }
            ClassKind::Bruiser => {
                bias[ClassKind::DpsAd as usize] *= 2.7;
                bias[ClassKind::DpsAp as usize] *= 3.0;
                bias[ClassKind::Assassin as usize] *= 0.8;
            }
            ClassKind::Healer => {
                bias[ClassKind::DpsAd as usize]    *= 1.2;
                bias[ClassKind::Bruiser as usize]  *= 1.5;
                bias[ClassKind::Assassin as usize] *= 1.2;
                bias[ClassKind::DpsAp as usize]    *= 0.8;
                bias[ClassKind::AssassinAndDpsAp as usize] *= 1.2;
            }
        }
    }

    for i in 0..mine.len() {
        match draft_class(mine[i]) {
            ClassKind::Tank => {
                bias[ClassKind::DpsAp as usize] *= 5.0;
                bias[ClassKind::DpsAd as usize] *= 5.0;
                bias[ClassKind::AssassinAndDpsAp as usize] *= 1.3;
                bias[ClassKind::Tank as usize]  *= 0.2;
            }
            ClassKind::Assassin | ClassKind::AssassinAndDpsAp => {
                bias[ClassKind::Assassin as usize] *= 0.1;
                bias[ClassKind::AssassinAndDpsAp as usize] *= 0.3;
                bias[ClassKind::Healer as usize] *= 2.0;
            }
            ClassKind::DpsAd | ClassKind::DpsAp => {
                bias[ClassKind::DpsAd as usize] *= 0.2;
                bias[ClassKind::DpsAp as usize] *= 0.2;
                bias[ClassKind::Tank as usize]  *= 5.0;
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
    biases.len() - 1   // fallback for floating-point rounding at the very end
}

pub fn play() -> Result<()> {
    let mut pick_bias = calc_balance()?;
    let ally_draft: Vec<ChampionDraft> = cambc::draft::current_draft(Team::Ally)?;
    // Never roll a class we have no champions left in (e.g. a second assassin: Vendetta is the only
    // one): the empty pick list used to panic, and the game then picked for us at random.
    for (i, bias) in pick_bias.iter_mut().enumerate() {
        let class = ClassKind::ALL[i];
        if !ChampionDraft::ALL.iter().any(|&p| draft_class(p) == class && !ally_draft.contains(&p)) { *bias = 0.0; }
    }

    let picked_class = weighted_pick(&pick_bias);
    
    // No debug print of pick_bias here: formatting floats costs ~30k fuel, and draft turns after
    // the first get only ~100k (running out means the game picks for us at random).

    println!("Picked {}", picked_class);

    let mut filtered_champs: Vec<ChampionDraft> = Vec::new();
    let target = ClassKind::ALL[picked_class];   // picked_class is the index from weighted_pick
    for &pick in ChampionDraft::ALL {
        if draft_class(pick) == target && !ally_draft.contains(&pick) {
            filtered_champs.push(pick);
        }
    }

    let choice = filtered_champs[crate::random_number(filtered_champs.len() as u32) as usize];
    // Hot-spot plan for our picks including this one. It must be submitted before picking (the
    // pick ends our draft turn: submitting after it is engine error 18), and a failure here must
    // not cost us the pick.
    let mut picks = ally_draft;
    picks.push(choice);
    if let Err(error) = crate::hotspots::submit_plan(&picks) { println!("hotspot plan failed: {error}"); }
    cambc::draft::pick(choice)
}
