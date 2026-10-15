use cambc::{ChampionDraft, Result, Team};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn play() -> Result<()> {
    // Choose randomly from the champions our team has not picked.
    let chosen = match COUNTER.fetch_add(1, Ordering::Relaxed) {
        0 => ChampionDraft::CassiAndIssac,
        1 => ChampionDraft::Varg,
        2 => ChampionDraft::Marcella,
        3 => ChampionDraft::Tavi,
        4 => ChampionDraft::Vendetta,
        _ => ChampionDraft::Ryth
    };
    // Draft memory starts at byte zero and is copied to each champion.
    cambc::draft_memory().submit(b"hello world")?;
    cambc::draft::pick(chosen)
}
