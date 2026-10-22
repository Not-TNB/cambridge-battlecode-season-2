// Opening: how the team gathers gold before it's strong enough to clear camps alone.
//
// The draft bot looks at the camps on our side of the map and estimates the gold the team would
// have after OPENING_ROUNDS rounds three ways (simulate):
//   - pairs: what nearest-camp farming does anyway (champions drift around in twos);
//   - spread: every champion farms alone, so only camps one champion can beat count;
//   - grouped: the whole team moves as one, so it can take elk too, but walks everywhere together.
// From that it picks a mode and writes it into the draft memory, at OPENING_OFFSET:
//   - Group: grouping earns GROUP_GOLD_MARGIN times what pairs do (elk-heavy maps). Elk rallies are on whatever the map's elk share
//     (rally.rs), and the team farms one shared target group (team target) so it stays together.
//     Captures go to one damage dealer (FUNNEL_ORDER), who goes home to shop once it has
//     CARRY_RECALL_SHARDS shards' worth of gold (one on small maps, where enemies come early);
//     meanwhile the rest keep farming and funnel to the next damage dealer.
//   - Cluster: spreading earns more than pairs and most of it is gold clusters. Champions head for gold
//     clusters and leave one an ally is closer to (a cluster needs no help; a shared capture is
//     split).
//   - Normal: everything else (wolf maps). Nearest-camp farming, which keeps the team close anyway.
// Group and Cluster end once the team is fed (half of us could clear the hardest camp group on
// our side alone, at full health) or at OPENING_MAX_ROUND; then farming is as usual.
//
// Draft memory: b'O', version, mode, small map, the hardest group's first camp tile (x, y; 255 for
// none), then spread gold, grouped gold and spread gold from clusters (each / 10, capped at 255),
// then whether it's a farming map (see FARM_MAP_MEAN_STEPS).
// Shared memory (rally.rs has bytes 0-1): byte 2 = team target group + 1 (0: none), byte 3 = round
// it was set (low byte), byte 4 = fed champions (one bit per champion, in draft order).
use crate::common::{camp_groups, distance, me, sight, trace};
use cambc::{
    Bitboard, CampGroup, CampType, ChampionDraft, ChampionType, DistanceField, Result, Team, Vec2,
};
use std::cell::{Cell, OnceCell};

// Off: always Normal (no opening plan is written). Off in bot-v3 (2026-10-21, 264 games over 6
// seeds, with v3's other changes): 144-120 against on, gold +5%, deaths -12%. Group mode was
// worst on 00fragment (gold 1.8 against 5.8 a round) and best on ambush/00corners.
pub const OPENING: bool = false;
// The Group opening's parts, each switchable: the shared team target, funnelling captures to a
// damage dealer, that carry going home with CARRY_RECALL_SHARDS, and elk rallies on any map.
pub const GROUP_TEAM_TARGET: bool = true;
// Whether the plan may choose Group at all (Group lost 19/48 all game in v3). Off: a map that would
// be Group plays Normal; Cluster is still chosen where it fits. 2026-10-22, OPENING on with this
// off (Cluster on 01elk-arena, 00interlock, 00stampede, 01yellow-brick-road; the rest Normal):
// 8/16 against OPENING off on those four maps (elk-arena 2/4). OPENING stays off.
pub const GROUP_MODE: bool = false;
pub const GROUP_FUNNEL: bool = true;
pub const GROUP_CARRY_RECALL: bool = true;
pub const GROUP_ELK_RALLY: bool = true;
// Where the plan starts in the draft memory (the region table stops before it, regions.rs).
pub const OPENING_OFFSET: usize = 1000;
const MAGIC: u8 = b'O';
const VERSION: u8 = 2;
const NO_TILE: u8 = 255;
// Rounds of gold the draft compares the modes over.
const OPENING_ROUNDS: f32 = 150.0;
// Group needs more than this many times the pairs' gold; Cluster needs this share of the spread
// gold to come from gold clusters. Group on every map where it beat spreading out (11 of 21) lost
// 6/22 games there against 13/22 without it.
const GROUP_GOLD_MARGIN: f32 = 1.2;
const CLUSTER_SHARE: f32 = 0.5;
// No Cluster opening on maps whose camps are on average further than this from spawn: on trenches
// (96) champions spread out to far clusters and won 2/10 games, against 7/10 without the opening
// (scatter, 35: 6/10 against 1/10; orchard, 32: 4/10 against 5/10).
const CLUSTER_MAX_MEAN_STEPS: i32 = 60;
// A champion at the start, for the estimate (we don't know the picks yet): health, damage per turn
// against camps (strength plus an attack reset and a damage spell every 8 turns), and the share
// of health it fights down to (collect::CAMP_FIGHT_HP_FLOOR).
const BASE_HEALTH: f32 = 900.0;
const BASE_CAMP_DAMAGE: f32 = 60.0;
const FIGHT_FLOOR: f32 = 0.35;
// A map is small when the camps on our side are on average at most this many steps from spawn.
const SMALL_MAP_MEAN_STEPS: i32 = 12;
// A farming map: the early game is spent farming, not fighting, so the top carry opens with a
// Treasure emblem (treasure_first). That's the Cluster opening, or camps on our side at least this
// many steps from spawn on average (fights come late: trenches 96, ambush 56, expanse 37, lanes
// and scatter 35, orchard 32).
const FARM_MAP_MEAN_STEPS: i32 = 30;
// Who counts as the top carry for treasure_first: the first of these in our team.
const TREASURE_CARRIES: [ChampionType; 4] = [
    ChampionType::Prism,
    ChampionType::Tavi,
    ChampionType::VesperVale,
    ChampionType::Stellar,
];
const MAX_STEPS: u16 = 1000;
// Fuel needed to work the plan out (the first draft turn has ~1M; it takes far less).
const PLAN_MIN_FUEL: u64 = 300_000;

// The opening ends by this round even if the team never gets fed.
pub const OPENING_MAX_ROUND: u32 = 400;
// Fed: this share of the team could clear the hardest group alone. Checked every few rounds.
const FED_SHARE: f32 = 0.5;
const FED_CHECK_ROUNDS: u32 = 10;
// A team target nobody has replaced is dropped after this many rounds.
const TEAM_TARGET_ROUNDS: u32 = 40;
// Who captures in Group mode: the first of these in reach of the group (FUNNEL_RADIUS). Willow,
// tanks and Cassi & Issac aren't on it: early damage matters most. The first FUNNEL_CARRIES of
// them in our team go home to shop with what they're given.
const FUNNEL_ORDER: [ChampionType; 9] = [
    ChampionType::Prism,
    ChampionType::Tavi,
    ChampionType::VesperVale,
    ChampionType::Stellar,
    ChampionType::Vendetta,
    ChampionType::Marcella,
    ChampionType::HeartOfRuin,
    ChampionType::Ren,
    ChampionType::Varg,
];
// Only while the carry is this close: walking off a capture for a carry 8 tiles away lost 5/22
// games where every other part of the Group opening alone did better.
const FUNNEL_RADIUS: i32 = 3;
const FUNNEL_CARRIES: usize = 2;
const CARRY_RECALL_SHARDS: u32 = 4;
const CARRY_RECALL_SHARDS_SMALL: u32 = 1;
const SHARED_OFFSET: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Normal = 0,
    Group = 1,
    Cluster = 2,
}

// ---- Draft ----

// A camp group as the estimate sees it.
struct Site {
    key: Vec2,              // first camp's tile
    centre: Vec2,           // where we stand to fight it
    steps: f32,             // walking steps from our spawn
    gold: f32,              // reward
    camps: Vec<(f32, f32)>, // (health, strength), weakest first
    respawn: f32,
    gold_only: bool, // only gold clusters
}

impl Site {
    fn health(&self) -> f32 {
        self.camps.iter().map(|c| c.0).sum()
    }

    // Whether `n` champions together beat it before falling to the fight floor: they focus the
    // weakest camp, and every camp still alive hits one of them.
    fn beaten_by(&self, n: f32) -> bool {
        let damage = BASE_CAMP_DAMAGE * n;
        let mut alive: f32 = self.camps.iter().map(|c| c.1).sum();
        let mut taken = 0.0;
        for &(health, strength) in &self.camps {
            taken += alive * health / damage;
            alive -= strength;
        }
        taken / n <= (1.0 - FIGHT_FLOOR) * BASE_HEALTH
    }
}

// Gold after OPENING_ROUNDS for `agents` parties of `size` champions each, every party greedily
// taking the site it would finish soonest (walking, waiting for a respawn, clearing, capturing).
// Returns (all gold, gold from cluster-only sites).
fn simulate(sites: &[Site], agents: usize, size: f32, capture: f32) -> (f32, f32) {
    let usable: Vec<&Site> = sites.iter().filter(|s| s.beaten_by(size)).collect();
    let mut ready = vec![0.0f32; usable.len()];
    let mut parties: Vec<(f32, Option<usize>)> = vec![(0.0, None); agents];
    let (mut gold, mut clusters) = (0.0, 0.0);
    while let Some(p) = (0..parties.len())
        .filter(|&p| parties[p].0 < OPENING_ROUNDS)
        .min_by(|&a, &b| parties[a].0.total_cmp(&parties[b].0))
    {
        let (free, at) = parties[p];
        let best = usable
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let walk = match at {
                    None => s.steps,
                    Some(j) => distance(usable[j].centre, s.centre) as f32,
                };
                let start = (free + walk).max(ready[i]);
                (i, start + s.health() / (BASE_CAMP_DAMAGE * size) + capture)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((i, finish)) if finish <= OPENING_ROUNDS => {
                gold += usable[i].gold;
                if usable[i].gold_only {
                    clusters += usable[i].gold;
                }
                ready[i] = finish + usable[i].respawn;
                parties[p] = (finish, Some(i));
            }
            _ => parties[p].0 = f32::INFINITY,
        }
    }
    (gold, clusters)
}

fn distances_from(sources: &Bitboard, blocked: &Bitboard) -> Result<(Vec<u16>, i32)> {
    let mut field = DistanceField::new()?;
    field.compute(blocked, sources, &mut Bitboard::new()?, MAX_STEPS)?;
    Ok((field.export_linear_buffer()?, field.width()))
}

// Walking steps from our spawn and from theirs, per tile (each side blocked by the other's spawn).
fn spawn_distances() -> Result<(Vec<u16>, Vec<u16>, i32)> {
    let blocked = crate::regions::walk_blocked()?;
    let (ours, w) = distances_from(cambc::map::spawn_region(Team::Ally)?, &blocked)?;
    let mut enemy_blocked = cambc::map::walls()?.clone();
    enemy_blocked.union_assign(cambc::map::holes()?)?;
    enemy_blocked.union_assign(cambc::map::altars()?)?;
    enemy_blocked.union_assign(cambc::map::spawn_region(Team::Ally)?)?;
    let (theirs, _) = distances_from(cambc::map::spawn_region(Team::Enemy)?, &enemy_blocked)?;
    Ok((ours, theirs, w))
}

// The groups on our side of the map we can walk to, without monoliths (nobody takes one early).
fn our_sites() -> Result<Vec<Site>> {
    let (ours, theirs, w) = spawn_distances()?;
    let at = |p: Vec2| (p.y * w + p.x) as usize;
    let mut sites = Vec::new();
    for group in &cambc::map::camp_groups()? {
        let camps = group.camps();
        let Some(first) = camps.first() else {
            continue;
        };
        if group.gold_value() == 0 || camps.iter().any(|c| c.kind() == CampType::Monolith) {
            continue;
        }
        let steps = camps
            .iter()
            .map(|c| ours[at(c.spawn_position())])
            .min()
            .unwrap_or(u16::MAX);
        let enemy = camps
            .iter()
            .map(|c| theirs[at(c.spawn_position())])
            .min()
            .unwrap_or(u16::MAX);
        if steps == DistanceField::UNREACHABLE || enemy < steps {
            continue;
        }
        let mut fight = Vec::with_capacity(camps.len());
        for camp in camps {
            let base = camp.base_stats()?;
            fight.push((base.base_health, base.base_strength));
        }
        fight.sort_by(|a, b| a.0.total_cmp(&b.0));
        let n = camps.len() as i32;
        let (x, y) = camps.iter().fold((0, 0), |(x, y), c| {
            (x + c.spawn_position().x, y + c.spawn_position().y)
        });
        sites.push(Site {
            key: first.spawn_position(),
            centre: Vec2::new(x / n, y / n),
            steps: steps as f32,
            gold: group.gold_value() as f32,
            camps: fight,
            respawn: group.max_respawn_timer() as f32,
            gold_only: camps.iter().all(|c| c.kind() == CampType::GoldCluster),
        });
    }
    Ok(sites)
}

fn plan() -> Result<Vec<u8>> {
    let sites = our_sites()?;
    let team = cambc::draft::team_size().unwrap_or(5).max(1);
    let capture = cambc::constants().capture_time as f32;
    let (pairs, _) = simulate(&sites, team.div_ceil(2), 2.0, capture);
    let (spread, from_clusters) = simulate(&sites, team, 1.0, capture);
    let (grouped, _) = simulate(&sites, 1, team as f32, capture);
    let mean_steps = if sites.is_empty() {
        i32::MAX
    } else {
        (sites.iter().map(|s| s.steps).sum::<f32>() / sites.len() as f32) as i32
    };
    let mode = if GROUP_MODE && grouped > pairs * GROUP_GOLD_MARGIN {
        Mode::Group
    } else if spread > pairs
        && from_clusters >= spread * CLUSTER_SHARE
        && mean_steps <= CLUSTER_MAX_MEAN_STEPS
    {
        Mode::Cluster
    } else {
        Mode::Normal
    };
    let small = mean_steps <= SMALL_MAP_MEAN_STEPS;
    let farming = mode == Mode::Cluster || (mean_steps >= FARM_MAP_MEAN_STEPS && !sites.is_empty());
    // The group the team must beat alone to be fed: the toughest one the whole team beats.
    let hardest = sites
        .iter()
        .filter(|s| s.beaten_by(team as f32))
        .max_by(|a, b| {
            let threat = |s: &Site| s.health() * s.camps.iter().map(|c| c.1).sum::<f32>();
            threat(a).total_cmp(&threat(b))
        })
        .map(|s| s.key);
    // Integers only: formatting floats costs ~9k fuel each.
    println!(
        "opening mode={:?} pairs={} spread={} grouped={} clusters={} sites={} mean_steps={} small={} farming={}",
        mode,
        pairs as i32,
        spread as i32,
        grouped as i32,
        from_clusters as i32,
        sites.len(),
        mean_steps,
        small,
        farming
    );
    let byte = |gold: f32| (gold / 10.0).min(255.0) as u8;
    let (hx, hy) = hardest.map_or((NO_TILE, NO_TILE), |p| (p.x as u8, p.y as u8));
    Ok(vec![
        MAGIC,
        VERSION,
        mode as u8,
        small as u8,
        hx,
        hy,
        byte(spread),
        byte(grouped),
        byte(from_clusters),
        farming as u8,
    ])
}

// Draft: the plan's bytes, worked out once, on the first draft turn (later ones lack the fuel; a
// failure means no plan, so Normal).
pub fn plan_bytes() -> Result<Vec<u8>> {
    thread_local! {
        static PLAN: OnceCell<Vec<u8>> = const { OnceCell::new() };
    }
    if !OPENING {
        return Ok(Vec::new());
    }
    if let Some(bytes) = PLAN.with(|p| p.get().cloned()) {
        return Ok(bytes);
    }
    let bytes = if cambc::fuel_remaining()? >= PLAN_MIN_FUEL {
        plan().unwrap_or_default()
    } else {
        Vec::new()
    };
    PLAN.with(|p| {
        let _ = p.set(bytes.clone());
    });
    Ok(bytes)
}

// ---- Play ----

struct Plan {
    mode: Mode,
    small: bool,
    farming: bool,
    hardest: Option<usize>, // camp group index
}

fn read_plan() -> &'static Plan {
    thread_local! {
        static PLAN: OnceCell<&'static Plan> = const { OnceCell::new() };
    }
    if let Some(plan) = PLAN.with(|p| p.get().copied()) {
        return plan;
    }
    let mut plan = Plan {
        mode: Mode::Normal,
        small: false,
        farming: false,
        hardest: None,
    };
    let bytes = cambc::draft_memory().bytes();
    if let Some(b) = bytes.get(OPENING_OFFSET..OPENING_OFFSET + 10) {
        if OPENING && b[0] == MAGIC && b[1] == VERSION {
            plan.mode = match b[2] {
                1 => Mode::Group,
                2 => Mode::Cluster,
                _ => Mode::Normal,
            };
            plan.small = b[3] != 0;
            plan.farming = b[9] != 0;
            if b[4] != NO_TILE {
                let key = Vec2::new(b[4] as i32, b[5] as i32);
                plan.hardest = camp_groups().ok().and_then(|groups| {
                    groups
                        .iter()
                        .position(|g| g.camps().first().is_some_and(|c| c.spawn_position() == key))
                });
            }
        }
    }
    let plan: &'static Plan = Box::leak(Box::new(plan));
    PLAN.with(|p| {
        let _ = p.set(plan);
    });
    plan
}

pub fn mode() -> Mode {
    read_plan().mode
}

// Our champions in draft order (Cassi & Issac are one pick, two champions).
pub(crate) fn team() -> &'static [ChampionType] {
    thread_local! {
        static TEAM: OnceCell<&'static [ChampionType]> = const { OnceCell::new() };
    }
    TEAM.with(|t| {
        *t.get_or_init(|| {
            let mut team = Vec::new();
            for pick in cambc::draft::current_draft(Team::Ally).unwrap_or_default() {
                use ChampionDraft as D;
                use ChampionType as C;
                team.extend_from_slice(match pick {
                    D::CassiAndIssac => &[C::Cassi, C::Issac][..],
                    D::Ryth => &[C::Ryth],
                    D::VesperVale => &[C::VesperVale],
                    D::Vendetta => &[C::Vendetta],
                    D::Stellar => &[C::Stellar],
                    D::Prism => &[C::Prism],
                    D::Marcella => &[C::Marcella],
                    D::Willow => &[C::Willow],
                    D::Ren => &[C::Ren],
                    D::Varg => &[C::Varg],
                    D::Tavi => &[C::Tavi],
                    D::HeartOfRuin => &[C::HeartOfRuin],
                });
            }
            Vec::leak(team)
        })
    })
}

fn read_shared() -> Result<[u8; 3]> {
    let mut bytes = [0u8; 3];
    cambc::shared_memory().read(SHARED_OFFSET, &mut bytes)?;
    Ok(bytes)
}

thread_local! {
    // Once the opening is over it stays over.
    static OVER: Cell<bool> = const { Cell::new(false) };
    // This round's reading of the shared bytes: (round, bytes).
    static SHARED: Cell<Option<(u32, [u8; 3])>> = const { Cell::new(None) };
    static NEXT_FED_CHECK: Cell<u32> = const { Cell::new(0) };
}

fn shared() -> Result<[u8; 3]> {
    let round = cambc::round_number()?;
    if let Some((r, bytes)) = SHARED.with(|s| s.get()) {
        if r == round {
            return Ok(bytes);
        }
    }
    let bytes = read_shared()?;
    SHARED.with(|s| s.set(Some((round, bytes))));
    Ok(bytes)
}

fn write_shared(offset: usize, bytes: &[u8]) -> Result<()> {
    cambc::shared_memory().write(SHARED_OFFSET + offset, bytes)?;
    SHARED.with(|s| s.set(None));
    Ok(())
}

// Whether the Group or Cluster opening is still on.
pub fn active() -> Result<bool> {
    if mode() == Mode::Normal || OVER.with(|o| o.get()) {
        return Ok(false);
    }
    let fed = shared()?[2].count_ones() as f32;
    let needed = (FED_SHARE * team().len().max(1) as f32).ceil();
    if cambc::round_number()? >= OPENING_MAX_ROUND || fed >= needed {
        trace("opening-over");
        OVER.with(|o| o.set(true));
        return Ok(false);
    }
    Ok(true)
}

pub fn group_active() -> Result<bool> {
    Ok(mode() == Mode::Group && active()?)
}

// Group opening: elk rallies whatever the map's elk share (rally.rs).
pub fn elk_rallies() -> Result<bool> {
    Ok(GROUP_ELK_RALLY && group_active()?)
}

pub fn cluster_active() -> Result<bool> {
    Ok(mode() == Mode::Cluster && active()?)
}

// Whether we open with a Treasure emblem: on a farming map (FARM_MAP_MEAN_STEPS), as the team's
// top carry (TREASURE_CARRIES). Not tied to the opening being on: the extra gold lasts all game.
pub const TREASURE_FIRST: bool = true;

pub fn treasure_first(champion: ChampionType) -> bool {
    TREASURE_FIRST
        && read_plan().farming
        && team()
            .iter()
            .find(|c| TREASURE_CARRIES.contains(c))
            .is_some_and(|&top| top == champion)
}

// Every FED_CHECK_ROUNDS rounds while the opening is on: if we alone, at full health, would clear
// the hardest group on our side, mark us fed in the shared memory.
pub fn update_fed(min_fuel: u64) -> Result<()> {
    if mode() == Mode::Normal || cambc::fuel_remaining()? < min_fuel {
        return Ok(());
    }
    let round = cambc::round_number()?;
    if round < NEXT_FED_CHECK.with(|n| n.get()) || !active()? {
        return Ok(());
    }
    NEXT_FED_CHECK.with(|n| n.set(round + FED_CHECK_ROUNDS));
    let Some(hardest) = read_plan().hardest else {
        return Ok(());
    };
    let name = me()?.champion_name()?;
    let Some(slot) = team().iter().position(|&c| c == name).filter(|&s| s < 8) else {
        return Ok(());
    };
    let fed = shared()?[2];
    if fed & (1 << slot) == 0 && crate::common::collect::could_solo(hardest)? {
        trace("opening-fed");
        write_shared(2, &[fed | (1 << slot)])?;
    }
    Ok(())
}

// The team target group, if one is set and not stale.
// Like everything here, it costs no host calls outside the Group and Cluster modes: extra fuel
// alone changed how Normal-mode games went (a run of turns differs from round ~140 on).
pub fn team_target() -> Result<Option<usize>> {
    // Past the opening its bytes hold target claims (common/claims.rs).
    if !GROUP_TEAM_TARGET || mode() != Mode::Group || crate::common::claims::active() {
        return Ok(None);
    }
    let [group, set, _] = shared()?;
    if group == 0 {
        return Ok(None);
    }
    let round = cambc::round_number()?;
    if (round as u8).wrapping_sub(set) as u32 >= TEAM_TARGET_ROUNDS {
        write_shared(0, &[0, 0])?;
        return Ok(None);
    }
    Ok(Some(group as usize - 1))
}

pub fn set_team_target(index: usize) -> Result<()> {
    if !GROUP_TEAM_TARGET || index >= u8::MAX as usize || team_target()?.is_some() {
        return Ok(());
    }
    trace("team-target-set");
    write_shared(0, &[index as u8 + 1, cambc::round_number()? as u8])
}

// End the team target at group `index` (cleared, or respawning).
pub fn end_team_target(index: usize) -> Result<()> {
    if team_target()? == Some(index) {
        trace("team-target-end");
        write_shared(0, &[0, 0])?;
    }
    Ok(())
}

fn funnel_rank(champion: ChampionType) -> Option<usize> {
    FUNNEL_ORDER.iter().position(|&c| c == champion)
}

// Group mode: whether a cleared group's capture should go to an ally rather than us, because an
// ally higher on FUNNEL_ORDER is in reach of it (rewards are split between everyone capturing).
pub fn funnel_elsewhere(group: &CampGroup) -> Result<bool> {
    if !GROUP_FUNNEL {
        return Ok(false);
    }
    let me = me()?;
    let mine = funnel_rank(me.champion_name()?);
    let here = me.position()?;
    for ally in &sight()?.allies {
        let position = ally.position()?;
        if position == here || !ally.is_alive()? || ally.is_recalling().unwrap_or(false) {
            continue;
        }
        let Some(rank) = funnel_rank(ally.champion_name()?) else {
            continue;
        };
        if mine.is_some_and(|m| m <= rank) {
            continue;
        }
        if group
            .camps()
            .iter()
            .any(|c| distance(c.spawn_position(), position) <= FUNNEL_RADIUS)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

// Group mode: a damage dealer we funnel to goes home to shop once it has CARRY_RECALL_SHARDS
// shards' worth of gold (one on small maps). Champions that shop anywhere (Tavi) don't need to.
pub fn carry_wants_home(shops_anywhere: bool) -> Result<bool> {
    if !GROUP_CARRY_RECALL || shops_anywhere || !group_active()? {
        return Ok(false);
    }
    let me = me()?;
    let name = me.champion_name()?;
    let mut carries: Vec<ChampionType> = team()
        .iter()
        .copied()
        .filter(|&c| funnel_rank(c).is_some())
        .collect();
    carries.sort_by_key(|&c| funnel_rank(c));
    if !carries.iter().take(FUNNEL_CARRIES).any(|&c| c == name) {
        return Ok(false);
    }
    let shards = if read_plan().small {
        CARRY_RECALL_SHARDS_SMALL
    } else {
        CARRY_RECALL_SHARDS
    };
    let shard = cambc::ItemType::PowerShardOfStrength.cost()?;
    Ok(me.gold_count()? >= shards * shard)
}
