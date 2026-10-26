// Walking distances over fixed terrain (walls, holes, altars, the enemy spawn; units ignored).
// Straight-line distance misleads where a line of holes or walls splits the map: on spine an elk
// 3 tiles away across the hole line is ~47 steps round its end, and 42% of new farm targets were
// such camps (walked to, then often given up on).
use super::world::{distance, terrain_tiles};
use cambc::{Bitboard, DistanceField, Result, Vec2};
use std::cell::OnceCell;

// Off: farm targets are ranked by straight-line distance again.
pub const WALK_TARGETS: bool = true;
// The field from us stops here (a capped field is cheaper: ~4-6k fuel against 5-11k); tiles
// further away rank after every tile within it, by straight-line distance.
const FIELD_STEPS: u16 = 60;

// Steps from `origin` to any tile, the field built on the first lookup (~4-6k fuel; each lookup is
// a host call, ~40). Any failure falls back to straight-line distance.
pub struct StepsFrom {
    origin: Vec2,
    field: OnceCell<Option<std::rc::Rc<DistanceField>>>,
    // How far the field searches, set by the first nearest_with before it's built (FIELD_STEPS
    // otherwise). Tiles beyond it rank by straight-line distance after every tile within it.
    cap: std::cell::Cell<u16>,
}

// The field only needs to reach the nearest candidate (in a straight line) plus this many steps of
// detour and penalty: it costs by the area it covers, ~14k at 60 steps on a 100x100 map (01big-world)
// against ~7k at 20. 0: always FIELD_STEPS.
pub const NEAREST_FIELD_SLACK: i32 = 20;

// v8 group fields: each camp group's walking-distance field over the fixed terrain (from all its camp
// tiles), built once per champion with fuel left over at the end of a turn (GROUP_FIELD_FUEL), one
// group a turn, nearest first. Terrain never changes, so a field is good all game: the steps from
// any tile to a group's nearest camp are then one lookup, and ranking farm targets no longer builds
// a field from where we stand (~8k a plan, 2026-10-23 profile). Off: the field from us only.
// 2026-10-23, 50 games (2 seeds x 25 maps) against bot-v8 without it, read game by game against a
// control run of the same games with no change (21/50): 28/50, flips +15/-8, gold +8%,
// fuel-exhausted turns 163 a 1,000 rounds against 172; in the combination with CHIP_SAFE (see there)
// not confirmed on fresh seeds. Off.
pub const GROUP_FIELDS: bool = false;
const GROUP_FIELD_FUEL: u64 = 45_000;
const GROUP_FIELD_STEPS: u16 = 200;

thread_local! {
    static GROUP_FIELD: std::cell::RefCell<Vec<Option<std::rc::Rc<DistanceField>>>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static LAST_FIELD_COST: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

// Steps from `origin` to camp tile `tile` from its group's field, if built: at least the
// straight-line distance (tiles of one group differ by that). None: no field yet.
fn group_field_steps(origin: Vec2, tile: Vec2) -> Option<i32> {
    let group = super::world::group_at(tile).ok()??;
    let field = GROUP_FIELD.with(|g| g.borrow().get(group).cloned().flatten())?;
    let d = field.get(origin).ok()?;
    let straight = distance(origin, tile);
    Some(if d == DistanceField::UNREACHABLE {
        GROUP_FIELD_STEPS as i32 + straight
    } else {
        (d as i32).max(straight)
    })
}

// Build the nearest reachable group's field not yet built, if there's fuel for it (end of turn).
pub fn build_next_group_field() -> Result<()> {
    // The last build's cost plus a margin must fit too (01big-world: ~30k a field).
    let need = GROUP_FIELD_FUEL.max(LAST_FIELD_COST.with(|c| c.get()) * 3 / 2 + 10_000);
    if !GROUP_FIELDS || cambc::fuel_remaining()? < need {
        return Ok(());
    }
    let groups = super::world::camp_groups()?;
    let here = super::me()?.position()?;
    let built = GROUP_FIELD.with(|g| {
        let mut g = g.borrow_mut();
        if g.len() < groups.len() {
            g.resize(groups.len(), None);
        }
        g.iter().map(|f| f.is_some()).collect::<Vec<_>>()
    });
    let mut next: Option<(i32, usize)> = None;
    for (index, group) in groups.iter().enumerate() {
        if built[index] || !crate::regions::reachable_for_me(index)? {
            continue;
        }
        let gap = group
            .camps()
            .iter()
            .map(|c| distance(c.spawn_position(), here))
            .min()
            .unwrap_or(i32::MAX);
        if next.is_none_or(|(g, _)| gap < g) {
            next = Some((gap, index));
        }
    }
    let Some((_, index)) = next else {
        return Ok(());
    };
    let _m = super::trace::meter("group-field");
    let before = cambc::fuel_remaining()?;
    let mut roots = Bitboard::new()?;
    for camp in groups[index].camps() {
        roots.set(camp.spawn_position(), true)?;
    }
    let mut field = DistanceField::new()?;
    field.compute(
        terrain_tiles()?,
        &roots,
        &mut Bitboard::new()?,
        GROUP_FIELD_STEPS,
    )?;
    GROUP_FIELD.with(|g| g.borrow_mut()[index] = Some(std::rc::Rc::new(field)));
    let after = cambc::fuel_remaining()?;
    if after <= before {
        LAST_FIELD_COST.with(|c| c.set(before - after));
    }
    Ok(())
}

impl StepsFrom {
    pub fn new(origin: Vec2) -> Self {
        Self {
            origin,
            field: OnceCell::new(),
            cap: std::cell::Cell::new(FIELD_STEPS),
        }
    }

    // The nearest of `tiles` on foot. One tile needs no field. Steps are never fewer than the
    // straight-line distance, so tiles are tried nearest in a straight line first and the search
    // stops once none left could beat the best (a lookup is a host call; goals can be 100 tiles).
    pub fn nearest(&self, tiles: &[Vec2]) -> Option<Vec2> {
        self.nearest_with(tiles, |_| 0)
    }

    // Like nearest, with `extra` steps added per tile (a penalty). One pass for the straight-line
    // lower bounds (steps are never fewer), then only tiles whose bound beats the best found so far
    // are looked up: sorting all of them cost up to ~50k with a few hundred camp tiles as goals.
    pub fn nearest_with(&self, tiles: &[Vec2], extra: impl Fn(Vec2) -> i32) -> Option<Vec2> {
        if tiles.len() <= 1 {
            return tiles.first().copied();
        }
        let mut bounds = Vec::with_capacity(tiles.len());
        let mut extras = Vec::with_capacity(tiles.len());
        let mut first = 0;
        // The most any tile is pulled closer (a negative extra, farm::JACKPOT_PULL_STEPS): the field
        // has to reach that much further, or the pulled tile's steps come out as unreachable.
        let mut pull = 0;
        for (i, &tile) in tiles.iter().enumerate() {
            let e = extra(tile);
            pull = pull.max(-e);
            let bound = distance(self.origin, tile) + e;
            if bound < bounds.get(first).copied().unwrap_or(i32::MAX) {
                first = i;
            }
            bounds.push(bound);
            extras.push(e);
        }
        if NEAREST_FIELD_SLACK > 0 && self.field.get().is_none() {
            let reach = (bounds[first] + pull + NEAREST_FIELD_SLACK).clamp(1, FIELD_STEPS as i32);
            self.cap.set(reach as u16);
        }
        super::trace::checkpoint("n-bounds");
        let mut best = (self.steps(tiles[first]) + extras[first], first);
        super::trace::checkpoint("n-first");
        for (i, &bound) in bounds.iter().enumerate() {
            if i != first && bound < best.0 {
                let steps = self.steps(tiles[i]) + extras[i];
                if steps < best.0 {
                    best = (steps, i);
                }
            }
        }
        super::trace::checkpoint("n-rest");
        Some(tiles[best.1])
    }

    pub fn steps(&self, tile: Vec2) -> i32 {
        let straight = distance(self.origin, tile);
        // At a camp group (or our spawn) the camp graph knows (common::graph::GRAPH_STEPS).
        if let Ok(Some(steps)) = super::graph::steps_from(self.origin, tile) {
            return steps.max(straight);
        }
        if let Ok(Some(steps)) = super::flood::steps_from(self.origin, tile) {
            return steps.max(straight);
        }
        if !WALK_TARGETS {
            return straight;
        }
        if GROUP_FIELDS {
            if let Some(steps) = group_field_steps(self.origin, tile) {
                return steps;
            }
        }
        // farm::PLAN_FUEL_RESERVE: no fuel to spare for a field, straight-line distance.
        if self.field.get().is_none()
            && super::farm::short_of(super::farm::RANK_FIELD_FUEL, "rank-field-skipped")
        {
            return straight;
        }
        let field = self
            .field
            .get_or_init(|| reused_or_built(self.origin, self.cap.get()));
        match field.as_ref().map(|f| f.get(tile)) {
            Some(Ok(d)) if d != DistanceField::UNREACHABLE => d as i32,
            Some(Ok(_)) => self.cap.get() as i32 + straight,
            _ => straight,
        }
    }
}

// The last field built, kept for FIELD_REUSE_ROUNDS rounds while we stay within FIELD_REUSE_TILES
// of where it was built from: a step changes every distance by at most one, which doesn't change
// which camp is nearest. Building it was ~16k of the farm plan on turns that ran out of fuel. 0: off.
pub const FIELD_REUSE_ROUNDS: u32 = 4;
const FIELD_REUSE_TILES: i32 = 2;

thread_local! {
    static LAST_FIELD: std::cell::RefCell<Option<(Vec2, u32, u16, std::rc::Rc<DistanceField>)>> =
        const { std::cell::RefCell::new(None) };
}

fn reused_or_built(origin: Vec2, cap: u16) -> Option<std::rc::Rc<DistanceField>> {
    let round = cambc::round_number().ok()?;
    if FIELD_REUSE_ROUNDS > 0 {
        let kept = LAST_FIELD.with(|l| {
            l.borrow()
                .as_ref()
                .filter(|(o, r, c, _)| {
                    distance(*o, origin) <= FIELD_REUSE_TILES
                        && round < r + FIELD_REUSE_ROUNDS
                        && *c >= cap
                })
                .map(|(_, _, _, f)| f.clone())
        });
        if kept.is_some() {
            super::trace::trace("walk-field-reused");
            return kept;
        }
    }
    let field = std::rc::Rc::new(build(origin, cap).ok()?);
    LAST_FIELD.with(|l| *l.borrow_mut() = Some((origin, round, cap, field.clone())));
    Some(field)
}

fn build(origin: Vec2, cap: u16) -> Result<DistanceField> {
    let _m = super::trace::meter("walk-field");
    let mut source = Bitboard::new()?;
    source.set(origin, true)?;
    let mut field = DistanceField::new()?;
    field.compute(terrain_tiles()?, &source, &mut Bitboard::new()?, cap)?;
    Ok(field)
}

thread_local! {
    static ALTARS: OnceCell<Option<(Vec<u16>, i32)>> = const { OnceCell::new() };
}

thread_local! {
    static SPAWN: OnceCell<Option<(Vec<u16>, i32)>> = const { OnceCell::new() };
}

// Work out altar_steps and spawn_steps once per champion: one field from every altar and one from
// our spawn, copied into Rust memory (~10-30k fuel each, so done at the end of a turn with fuel to
// spare, bot.rs; one per turn).
pub fn prepare_walk_fields() {
    if ALTARS.with(|a| a.get().is_none()) {
        ALTARS.with(|a| {
            a.get_or_init(|| altar_field().ok());
        });
        return;
    }
    SPAWN.with(|s| {
        s.get_or_init(|| spawn_field().ok());
    });
}

// Steps from `tile` to our spawn region. None until prepare_walk_fields has run, or if it failed.
pub fn spawn_steps(tile: Vec2) -> Option<i32> {
    SPAWN.with(|s| {
        let (steps, width) = s.get()?.as_ref()?;
        let d = *steps.get((tile.y * width + tile.x) as usize)?;
        (d != DistanceField::UNREACHABLE).then_some(d as i32)
    })
}

fn spawn_field() -> Result<(Vec<u16>, i32)> {
    let spawn = cambc::map::spawn_region(cambc::Team::Ally)?;
    let mut field = DistanceField::new()?;
    field.compute(
        terrain_tiles()?,
        spawn,
        &mut Bitboard::new()?,
        DistanceField::UNREACHABLE,
    )?;
    Ok((field.export_linear_buffer()?, field.width()))
}

// Steps from `tile` to the nearest altar (the altar itself is the source; tribute range is 1).
// None until prepare_altar_steps has run, or if it failed.
pub fn altar_steps(tile: Vec2) -> Option<i32> {
    ALTARS.with(|a| {
        let (steps, width) = a.get()?.as_ref()?;
        let d = *steps.get((tile.y * width + tile.x) as usize)?;
        (d != DistanceField::UNREACHABLE).then_some(d as i32)
    })
}

fn altar_field() -> Result<(Vec<u16>, i32)> {
    let altars = cambc::map::altars()?;
    let blocked = terrain_tiles()?.intersect(&altars.complement()?)?;
    let mut field = DistanceField::new()?;
    field.compute(
        &blocked,
        altars,
        &mut Bitboard::new()?,
        DistanceField::UNREACHABLE,
    )?;
    Ok((field.export_linear_buffer()?, field.width()))
}
