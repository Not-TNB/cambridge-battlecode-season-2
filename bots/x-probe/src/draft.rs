use cambc::{ChampionDraft, Result};
use std::cell::Cell;
thread_local! { static DONE: Cell<bool> = const { Cell::new(false) }; }
pub fn play() -> Result<()> {
    if !DONE.with(|d| d.replace(true)) {
        let mut out = String::from("MAP\n");
        for t in [cambc::Team::Ally, cambc::Team::Enemy] {
            let p = cambc::map::spawn_region(t)?.positions()?;
            out += &format!("spawn {:?} first {:?} last {:?}\n", t, p.first(), p.last());
        }
        for (i, g) in cambc::map::camp_groups()?.iter().enumerate() {
            let kinds: Vec<String> = g.camps().iter().map(|c| format!("{:?}@{},{}", c.kind(), c.spawn_position().x, c.spawn_position().y)).collect();
            out += &format!("group {} {}\n", i, kinds.join(" "));
        }
        for a in cambc::map::altars()?.positions()? { out += &format!("altar {},{}\n", a.x, a.y); }
        println!("{out}");
    }
    let picks = cambc::draft::current_draft(cambc::Team::Ally)?;
    let next = [ChampionDraft::Willow, ChampionDraft::Varg, ChampionDraft::Ryth, ChampionDraft::Marcella, ChampionDraft::Prism].into_iter().find(|p| !picks.contains(p)).unwrap();
    cambc::draft::pick(next)
}
