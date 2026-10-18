// Walking and dashing towards goals, and walking distances (WalkGrid).
use super::fight::fightable_enemies;
use super::me;
use super::trace::checkpoint;
use super::world::{blocked_tiles, forget_sight, tiles_around};
use cambc::{Ability, Bitboard, DistanceField, Result, Vec2};

// Champions with a dash use it to travel when the goal is at least this many steps away and no
// enemy is in sight (fleeing always dashes). 0 turns travel dashing off.
pub const DASH_TRAVEL_MIN_STEPS: u16 = 4;

// Our dash, if we have one and it's ready.
pub fn ready_dash() -> Result<Option<&'static Ability>> {
    for ability in me()?.abilities() {
        if ability.is_dash() && ability.is_ready()? {
            return Ok(Some(ability));
        }
    }
    Ok(None)
}

// Dash to the tile in dash range that `score` rates lowest, if it beats staying put. Dashes may
// cross holes, but we only consider tiles reachable on foot. Returns whether we dashed.
pub(super) fn dash_to_best(
    dash: &Ability,
    blocked: &Bitboard,
    score: impl Fn(Vec2) -> Result<f32>,
) -> Result<bool> {
    let Some(info) = dash.info()? else {
        return Ok(false);
    };
    let origin = me()?.position()?;
    let mut best = (origin, score(origin)?);
    for tile in blocked
        .reachable_tiles(origin, info.as_dyn().range()? as u16)?
        .positions()?
    {
        let s = score(tile)?;
        if s < best.1 {
            best = (tile, s);
        }
    }
    // false: an unseen unit blocks the landing tile; just walk instead. A unit our occupancy view
    // hasn't caught up with makes it an error instead, which changes nothing: treat it the same.
    // Either way the attempt may have revealed a unit, so what's blocked must be looked up again.
    if best.0 == origin {
        return Ok(false);
    }
    let dashed = dash.cast_at(best.0).unwrap_or(false);
    forget_sight();
    Ok(dashed)
}

// Take one ordinary step. Our occupancy view can lag a unit that just appeared (e.g. right after a
// dash), and stepping onto a known occupied tile is an error that changes nothing; treat it as not
// moving instead of ending the turn.
pub(super) fn step_to(tile: Vec2) -> bool {
    me().and_then(|me| me.move_absolute(tile)).unwrap_or(false)
}

thread_local! {
    static GOALS_UNREACHABLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// Whether the last move_towards_closest(_with_pull) found no walking path to any of its goals.
pub fn goals_were_unreachable() -> bool {
    GOALS_UNREACHABLE.with(|u| u.get())
}

pub fn move_towards_closest(goals: Bitboard) -> Result<bool> {
    // Goals across a strip of holes are reached with a dash (regions.rs), for farming and banking
    // alike: a champion that dashed into a pocket to farm otherwise couldn't walk out to bank.
    if let Some(moved) = crate::regions::cross_towards(&goals)? {
        GOALS_UNREACHABLE.with(|u| u.set(false));
        return Ok(moved);
    }
    let may_dash = DASH_TRAVEL_MIN_STEPS > 0 && fightable_enemies()?.is_empty();
    move_towards_closest_with_pull(goals, |_| 0.0, may_dash)
}

// Like move_towards_closest, but each candidate step also pays `pull(tile)` (in steps), so among
// steps that get similarly close to the goals we prefer the one `pull` likes best. With `may_dash`,
// a ready dash first jumps along the way when the goals are DASH_TRAVEL_MIN_STEPS or more away.
pub fn move_towards_closest_with_pull(
    mut goals: Bitboard,
    pull: impl Fn(Vec2) -> f32,
    may_dash: bool,
) -> Result<bool> {
    let me = me()?;
    let origin = me.position()?;
    GOALS_UNREACHABLE.with(|u| u.set(false));
    if !me.can_move()? || goals.is_set(origin)? {
        return Ok(false);
    }

    let mut blocked = blocked_tiles()?;
    checkpoint("path-blocked");
    goals.intersect_assign(&blocked.complement()?.union(&cambc::vision::camp_tiles()?)?)?;
    if goals.is_empty()? {
        return Ok(false);
    }

    let mut field = DistanceField::new()?;
    field.compute_to(
        &blocked.intersect(&goals.complement()?)?,
        &goals,
        &tiles_around(origin, 0)?,
        &mut Bitboard::new()?,
        DistanceField::UNREACHABLE,
    )?;
    checkpoint("path-field");
    GOALS_UNREACHABLE.with(|u| u.set(field.get(origin) == Ok(DistanceField::UNREACHABLE)));

    let score = |tile: Vec2| -> Result<f32> {
        let d = field.get(tile)?;
        Ok(if d == DistanceField::UNREACHABLE {
            f32::MAX
        } else {
            d as f32 + pull(tile)
        })
    };
    let mut dashed = false;
    if may_dash && DASH_TRAVEL_MIN_STEPS > 0 && field.get(origin)? >= DASH_TRAVEL_MIN_STEPS {
        if let Some(dash) = ready_dash()? {
            dashed = dash_to_best(dash, &blocked, score)?;
            // The dash (even a failed one, onto a hidden unit) may have revealed units: walking
            // onto a known occupied tile is an error, so recompute what's blocked.
            blocked = blocked_tiles()?;
        }
    }
    checkpoint("path-dash");
    let origin = me.position()?;

    let mut step = origin;
    let mut best = field.get(origin)? as f32 + pull(origin);
    for tile in blocked
        .reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        let d = field.get(tile)?;
        if d == DistanceField::UNREACHABLE {
            continue;
        }
        let score = d as f32 + pull(tile);
        if score < best {
            step = tile;
            best = score;
        }
    }
    checkpoint("path-step");
    if step == origin {
        Ok(dashed)
    } else {
        Ok(step_to(step) || dashed)
    }
}

// Walking distances from one tile, exported into Rust memory so lookups cost no host calls
// (each DistanceField::get is a host call). Blocked tiles, and tiles more than `max_steps`
// away, are UNREACHABLE.
pub struct WalkGrid {
    width: i32,
    height: i32,
    dist: Vec<u16>,
}

impl WalkGrid {
    pub fn from(origin: Vec2, blocked: &Bitboard, max_steps: u16) -> Result<Self> {
        let mut field = DistanceField::new()?;
        field.compute(
            blocked,
            &tiles_around(origin, 0)?,
            &mut Bitboard::new()?,
            max_steps,
        )?;
        Ok(Self {
            width: field.width(),
            height: field.height(),
            dist: field.export_linear_buffer()?,
        })
    }

    // Steps from the origin to `tile`, or UNREACHABLE (also for tiles off the map).
    pub fn get(&self, tile: Vec2) -> u16 {
        if tile.x < 0 || tile.y < 0 || tile.x >= self.width || tile.y >= self.height {
            return DistanceField::UNREACHABLE;
        }
        self.dist[(tile.y * self.width + tile.x) as usize]
    }

    // Shortest path from the origin to `end`, excluding the origin, traced back along the grid.
    pub fn path_to(&self, end: Vec2) -> Option<Vec<Vec2>> {
        let mut path = vec![end];
        let mut pos = end;
        while self.get(pos) > 1 {
            let d = self.get(pos);
            pos = (-1..=1)
                .flat_map(|dx| (-1..=1).map(move |dy| Vec2::new(pos.x + dx, pos.y + dy)))
                .find(|&n| self.get(n) == d - 1)?;
            path.push(pos);
        }
        path.reverse();
        Some(path)
    }
}
