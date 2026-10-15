use cambc::{ChampionDraft, Result, Team};
use crate::constants::{CHAMP_CLASSES, CLASS_COUNT, ClassKind};


fn calc_balance() -> Result<[f64; CLASS_COUNT]> {
    let theirs = cambc::draft::current_draft(Team::Enemy)?;
    let mine   = cambc::draft::current_draft(Team::Ally)?;
    let mut bias = [1.0; CLASS_COUNT];
    if theirs.len() == 0 {
        bias[ClassKind::Bruiser as usize] *= 20.0;
    }
    
    for i in 0..theirs.len() {
        match CHAMP_CLASSES[theirs[i] as usize] {
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
        match CHAMP_CLASSES[mine[i] as usize] {
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
    let pick_bias = calc_balance()?;

    let picked_class = weighted_pick(&pick_bias);
    
    /* Debug print to check if the pick bias works correctly or needs tuning */
    println!("{:?}", pick_bias);        // [1.2, 3.3, 0.2]
    println!("{:.2?}", pick_bias);      // 2 decimal places: [1.20, 3.30, 0.20]

    println!("Picked {}", picked_class);

    let mut filtered_champs: Vec<ChampionDraft> = Vec::new();
    let ally_draft: Vec<ChampionDraft> = cambc::draft::current_draft(Team::Ally)?;
    for i in 0..11 {
        let pick = ChampionDraft::try_from(i)?;
        let target = ClassKind::ALL[picked_class];   // picked_class is the index from weighted_pick

        if CHAMP_CLASSES[i as usize] == target && !ally_draft.contains(&pick) {
            filtered_champs.push(pick);
        }
    }

    cambc::draft::pick(filtered_champs[crate::random_number(filtered_champs.len() as u32) as usize])
}
