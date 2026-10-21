use cambc::{ChampionDraft, Result, Team};

const PRESET: [ChampionDraft; 5] = [
    ChampionDraft::Willow,
    ChampionDraft::HeartOfRuin,
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
    if let Err(error) = submit_v2_regions() {
        println!("squad draft regions failed: {error}");
    }
    cambc::draft::pick(next)
}

// Fuel to keep for the pick itself; below this the region table isn't submitted.
const MEMORY_MIN_FUEL: u64 = 20_000;

// bot-v2's region table (which camp groups can be walked or dashed to), for the followers' bot-v2
// turn (v2/): without it, it takes every group as reachable, pockets behind holes included. Worked
// out on the first draft turn, the only one with the fuel (~1M). Draft memory is otherwise unused
// (hotspots are off; with them on, this would overwrite their plan).
fn submit_v2_regions() -> Result<()> {
    if crate::hotspots::HOTSPOTS {
        return Ok(());
    }
    let bytes = crate::v2::regions::table_bytes()?;
    if bytes.is_empty() || cambc::fuel_remaining()? < MEMORY_MIN_FUEL {
        return Ok(());
    }
    cambc::draft_memory().submit(&bytes)
}
