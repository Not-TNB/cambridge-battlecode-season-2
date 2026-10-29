// Draft: classify the map, pick for that playstyle, and hand the plan to the champions through
// draft memory: [MAGIC, kind, vantage count, (x, y) * count].
use crate::classify;
use crate::mapinfo::{flood, MapInfo, FAR};
use crate::profile::{params, Kind};
use crate::util::cheb;
use cambc::{Bitboard, ChampionDraft, Result, Team, Vec2};
use std::cell::RefCell;

pub const MAGIC: u8 = 0xA7;
const MAX_VANTAGE: usize = 8;

// Force a playstyle for testing (bench A/B per profile); None = classify.
pub const FORCE_KIND: Option<Kind> = None;

struct Plan {
    kind: Kind,
    memory: Vec<u8>,
}

thread_local! {
    static PLAN: RefCell<Option<Plan>> = const { RefCell::new(None) };
}

fn make_plan() -> Result<Plan> {
    let info = MapInfo::new()?;
    let f = classify::features(&info);
    let kind = FORCE_KIND.unwrap_or_else(|| classify::classify(&f));
    println!(
        "draft: kind={:?} gem={:.2} contested={:.2} ours={:.2} top={:.2}{} gap={} locked={:.2} altars={}",
        kind,
        f.gem_share,
        f.contested,
        f.ours,
        f.top_share,
        if f.top_contested { "c" } else { "" },
        f.gap,
        f.dash_locked,
        f.altars
    );
    let vantage = vantage_points(&info)?;
    let mut memory = vec![MAGIC, kind as u8, vantage.len() as u8];
    for p in vantage {
        memory.push(p.x as u8);
        memory.push(p.y as u8);
    }
    Ok(Plan { kind, memory })
}

// Tiles furthest (walking) from every resource, spawn and altar: places nothing pulls us to, where
// unseen enemies and unknown camp states hide. Champions with nothing better to do scout them.
fn vantage_points(info: &MapInfo) -> Result<Vec<Vec2>> {
    let mut roots = Bitboard::new()?;
    for g in &info.groups {
        for p in &g.tiles {
            roots.set(*p, true)?;
        }
    }
    for p in info.ally_spawn.iter().chain(&info.enemy_spawn) {
        if info.walkable(*p) {
            roots.set(*p, true)?;
        }
    }
    let field = flood(&info.blocked_bb, &roots, info.w, info.h)?;
    let mut cands: Vec<(u16, Vec2)> = Vec::new();
    for y in 0..info.h {
        for x in 0..info.w {
            let p = Vec2::new(x, y);
            let d = field.at(info, p);
            // Only tiles we can walk to from home.
            if d != FAR && d >= 4 && info.home.at(info, p) != FAR {
                cands.push((d, p));
            }
        }
    }
    cands.sort_by(|a, b| b.0.cmp(&a.0));
    let spacing = (info.spawn_gap / 4).max(6);
    let mut out: Vec<Vec2> = Vec::new();
    for (_, p) in cands {
        if out.len() >= MAX_VANTAGE {
            break;
        }
        if out.iter().all(|q| cheb(*q, p) >= spacing) {
            out.push(p);
        }
    }
    Ok(out)
}

pub fn play() -> Result<()> {
    let have = PLAN.with(|p| p.borrow().is_some());
    if !have {
        let plan = make_plan()?;
        PLAN.with(|p| *p.borrow_mut() = Some(plan));
    }
    let (kind, memory) = PLAN.with(|p| {
        let p = p.borrow();
        let p = p.as_ref().unwrap();
        (p.kind, p.memory.clone())
    });
    let _ = cambc::draft_memory().submit(&memory);
    let mine = cambc::draft::current_draft(Team::Ally)?;
    let pick = choose(kind, &mine);
    cambc::draft::pick(pick)
}

fn choose(kind: Kind, mine: &[ChampionDraft]) -> ChampionDraft {
    let all = [
        ChampionDraft::VesperVale,
        ChampionDraft::Varg,
        ChampionDraft::Tavi,
        ChampionDraft::Ryth,
        ChampionDraft::Marcella,
        ChampionDraft::Vendetta,
        ChampionDraft::Stellar,
        ChampionDraft::Ren,
        ChampionDraft::Prism,
        ChampionDraft::HeartOfRuin,
        ChampionDraft::Willow,
        ChampionDraft::CassiAndIssac,
    ];
    params(kind)
        .picks
        .iter()
        .chain(all.iter())
        .copied()
        .find(|p| !mine.contains(p))
        .unwrap_or(ChampionDraft::Varg)
}

// Champions: read the plan, or classify again if the memory is missing (draft bot ran out of fuel).
pub fn read_plan(info: &MapInfo) -> (Kind, Vec<Vec2>) {
    let mem = cambc::draft_memory().bytes();
    if mem.len() >= 3 && mem[0] == MAGIC {
        let n = (mem[2] as usize).min((mem.len() - 3) / 2);
        let pts = (0..n)
            .map(|i| Vec2::new(mem[3 + 2 * i] as i32, mem[4 + 2 * i] as i32))
            .collect();
        return (Kind::from_u8(mem[1]), pts);
    }
    let kind = FORCE_KIND.unwrap_or_else(|| classify::classify(&classify::features(info)));
    (kind, Vec::new())
}
