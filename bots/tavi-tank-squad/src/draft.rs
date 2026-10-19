use cambc::{ChampionDraft, Result, Team};

const PRESET: [ChampionDraft; 5] = [
    ChampionDraft::Willow,
    ChampionDraft::Varg,
    ChampionDraft::Ryth,
    ChampionDraft::Marcella,
    ChampionDraft::Tavi,
];

pub fn play() -> Result<()> {
    let picks = cambc::draft::current_draft(Team::Ally)?;
    let team_size = cambc::draft::team_size()?;
    if !(3..=PRESET.len()).contains(&team_size) {
        return Err(cambc::Error::Engine {
            raw_code: 0,
            code: None,
            message: format!("unsupported Tavi squad team size: {team_size}"),
        });
    }
    let lineup = PRESET
        .iter()
        .copied()
        .filter(|pick| *pick != ChampionDraft::Tavi)
        .take(team_size - 1)
        .chain(std::iter::once(ChampionDraft::Tavi))
        .collect::<Vec<_>>();
    if picks.iter().any(|pick| !lineup.contains(pick)) {
        return Err(cambc::Error::Engine {
            raw_code: 0,
            code: None,
            message: "unexpected pick in the Tavi squad draft".to_owned(),
        });
    }
    let Some(next) = lineup.iter().find(|pick| !picks.contains(pick)).copied() else {
        return Err(cambc::Error::Engine {
            raw_code: 0,
            code: None,
            message: "the Tavi squad preset is already fully drafted".to_owned(),
        });
    };
    let mut plan = picks;
    plan.push(next);
    if let Err(error) = crate::hotspots::submit_plan(&plan) {
        println!("squad draft plan failed: {error}");
    }
    cambc::draft::pick(next)
}
