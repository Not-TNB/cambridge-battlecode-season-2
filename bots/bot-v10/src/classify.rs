// Map classification: which playstyle this map rewards. Scale-free features only (shares of the
// map's resources, distances relative to the spawn gap), so it generalises to unseen maps instead
// of memorising the ladder pool.
//
// Labels it was checked against (2026-10-29, from the team): farm = ambush, anvil, breadcrumbs,
// collision, boss-battle, corridors, famine, fields; fight = concentrated-wealth, treasure-box.
use crate::mapinfo::{MapInfo, FAR};
use crate::profile::Kind;
use crate::util::{cheb, DIRS};

pub struct Features {
    pub gem_share: f32,
    // Share of value no closer to either spawn (|d_home - d_enemy| small).
    pub contested: f32,
    // Share of value clearly on our half.
    pub ours: f32,
    // Biggest single group's share, and whether it sits in the contested middle.
    pub top_share: f32,
    pub top_contested: bool,
    // Share of value only a dash reaches.
    pub dash_locked: f32,
    pub gap: i32,
    pub altars: usize,
    // Some altar much closer to us than to them.
    pub safe_altar: bool,
    // Share of value in groups that never hit back (clusters only).
    pub harmless_share: f32,
}

pub fn value(gems: u32, gold: u32) -> f32 {
    gems as f32 + gold as f32 / 10.0
}

pub fn features(info: &MapInfo) -> Features {
    let mut gap = i32::MAX;
    for p in &info.ally_spawn {
        for d in DIRS {
            let v = info.enemy_home.at(info, *p + d);
            if v != FAR {
                gap = gap.min(v as i32 + 1);
            }
        }
    }
    if gap == i32::MAX {
        gap = cheb(info.home_center, info.enemy_center);
    }
    let (mut total, mut gems, mut gold10, mut contested, mut ours, mut locked) = (0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut top, mut top_contested) = (0.0f32, false);
    let mut harmless = 0.0;
    for g in info.groups.iter().filter(|g| !g.boss) {
        let v = value(g.gems, g.gold);
        if g.d_home == FAR && g.d_enemy == FAR {
            locked += v;
            continue;
        }
        total += v;
        if g.harmless {
            harmless += v;
        }
        gems += g.gems as f32;
        gold10 += g.gold as f32 / 10.0;
        let (a, b) = (g.d_home as i32, g.d_enemy as i32);
        let near = (a - b).abs() <= 3.max((a + b) * 15 / 100);
        if near {
            contested += v;
        } else if a < b {
            ours += v;
        }
        if v > top {
            top = v;
            top_contested = near;
        }
    }
    let total = total.max(1.0);
    let safe_altar = info.altars.iter().any(|a| {
        let (mut da, mut db) = (FAR, FAR);
        for d in DIRS {
            da = da.min(info.home.at(info, *a + d));
            db = db.min(info.enemy_home.at(info, *a + d));
        }
        (da as i32) * 3 < (db as i32) * 2
    });
    Features {
        gem_share: gems / (gems + gold10).max(1.0),
        contested: contested / total,
        ours: ours / total,
        top_share: top / total,
        top_contested,
        dash_locked: locked / (total + locked),
        gap,
        altars: info.altars.len(),
        safe_altar,
        harmless_share: harmless / total,
    }
}

pub fn classify(f: &Features) -> Kind {
    // Spawns nearly touching: every resource is a fight.
    if f.gap <= 12 {
        return Kind::Fight;
    }
    // One pile in the middle worth a quarter of the map: everyone converges on it.
    if f.top_contested && f.top_share >= 0.25 {
        return Kind::Fight;
    }
    if f.gem_share >= 0.85 {
        return Kind::Gem;
    }
    Kind::Farm
}
