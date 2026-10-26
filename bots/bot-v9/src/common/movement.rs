// Walking and dashing towards goals, and walking distances (WalkGrid).
use super::fight::fightable_enemies;
use super::me;
use super::trace::{checkpoint, trace};
use super::world::{blocked_tiles, distance, forget_sight, tiles_around};
use cambc::{Ability, Bitboard, DistanceField, Result, Vec2};

// Champions with a dash use it to travel when the goal is at least this many steps away and no
// enemy is in sight (fleeing always dashes). 0 turns travel dashing off.
pub const DASH_TRAVEL_MIN_STEPS: u16 = 4;

// Fuel fix 5: the field towards the goals (searched outwards from them until it reaches us: 8k on
// average, up to ~25k) is kept while the goals stay the same and it still covers our tile, for up
// to PATH_FIELD_KEEP_ROUNDS rounds, so walking to one target doesn't search again every step.
// Units that moved since only matter for the step itself, which avoids them (blocked_tiles). 0: off.
// Alone on 6 long-walk maps: 8% fewer fuel-exhausted turns (path field 8.9k -> 5.8k); the kept
// field's distance to the goal differed from a fresh one's on 3% of uses (units in the way).
pub const PATH_FIELD_KEEP_ROUNDS: u32 = 5;

thread_local! {
    static PATH_FIELD: std::cell::RefCell<Option<(u32, Bitboard, std::rc::Rc<DistanceField>)>> =
        const { std::cell::RefCell::new(None) };
}

fn compute_path_field(blocked: &Bitboard, goals: &Bitboard, origin: Vec2) -> Result<DistanceField> {
    let _m = super::trace::meter("fill-path");
    let mut field = DistanceField::new()?;
    field.compute_to(
        &blocked.intersect(&goals.complement()?)?,
        goals,
        &tiles_around(origin, 0)?,
        &mut Bitboard::new()?,
        DistanceField::UNREACHABLE,
    )?;
    Ok(field)
}

fn path_field(
    blocked: &Bitboard,
    goals: &Bitboard,
    origin: Vec2,
) -> Result<std::rc::Rc<DistanceField>> {
    let round = cambc::round_number()?;
    if PATH_FIELD_KEEP_ROUNDS > 0 {
        let kept = PATH_FIELD.with(|c| {
            c.borrow()
                .as_ref()
                .filter(|(built, kept_goals, field)| {
                    round < built + PATH_FIELD_KEEP_ROUNDS
                        && kept_goals == goals
                        && field
                            .get(origin)
                            .is_ok_and(|d| d != DistanceField::UNREACHABLE)
                })
                .map(|(_, _, field)| field.clone())
        });
        if let Some(field) = kept {
            super::trace::trace("path-field-kept");
            if super::trace::VERIFY_FUEL_CACHES
                && compute_path_field(blocked, goals, origin)?.get(origin)? != field.get(origin)?
            {
                super::trace::trace("path-cache-mismatch");
            }
            return Ok(field);
        }
    }
    let field = std::rc::Rc::new(compute_path_field(blocked, goals, origin)?);
    if PATH_FIELD_KEEP_ROUNDS > 0 {
        PATH_FIELD.with(|c| *c.borrow_mut() = Some((round, goals.clone(), field.clone())));
    }
    Ok(field)
}

// Our dash, if we have one and it's ready.
pub fn ready_dash() -> Result<Option<&'static Ability>> {
    for ability in me()?.abilities() {
        if ability.is_dash() && super::attack::ability_ready(ability, me()?)? {
            return Ok(Some(ability));
        }
    }
    Ok(None)
}

// A dash with landing effects (Heart of Ruin's: Focus x1.5 damage and a one-round stun to every
// enemy and camp within radius 1 of the landing tile): its radius, if we have one (ready or not).
pub fn impact_radius() -> Option<i32> {
    thread_local! {
        static RADIUS: std::cell::OnceCell<Option<i32>> = const { std::cell::OnceCell::new() };
    }
    RADIUS.with(|r| {
        *r.get_or_init(|| {
            let me = me().ok()?;
            me.abilities().iter().find_map(|a| match a.info() {
                Ok(Some(cambc::AbilityInfo::Dash(d))) if d.modifier > 0.0 => Some(d.radius as i32),
                _ => None,
            })
        })
    })
}

// Dash to the tile in dash range that `score` rates lowest, if it beats staying put. Dashes may
// cross holes, but we only consider tiles reachable on foot. Returns whether we dashed.
// v8 (2026-10-24): a dash may cross holes (game rules), but dashes here only landed on tiles we
// could also walk to within its range, and paths treat holes as walls, so a strip of holes between
// us and the goal was walked round (00lost-world's middle). With this on, a dash may also land on a
// free tile in range across holes, if the dash path is legal (regions::dash_path_ok); landings are
// still scored by walking distance to the goal, so it crosses only when that's shorter. Off: walk.
// 2026-10-24, 2 seeds x 25 maps against the same games without it (24/50): 28/50 (+12/-8), gold
// +10.6%, kills 11.4 a game against 8.8, gems submitted 766 against 705, deaths 12.5 against 12.1
// (00lost-world flipped to a win). On. Rally answers (rally::answer) still estimate the trip on foot.
pub const DASH_OVER_HOLES: bool = true;

// Where a dash of `range` from `origin` may land: free tiles we could walk to within the range, plus
// (DASH_OVER_HOLES) free tiles across holes with a legal dash path.
fn dash_landings(origin: Vec2, range: i32, blocked: &Bitboard) -> Result<Vec<Vec2>> {
    let walk = blocked.reachable_tiles(origin, range as u16)?;
    let mut tiles = walk.positions()?;
    if DASH_OVER_HOLES {
        let mut extra = tiles_around(origin, range)?;
        extra.intersect_assign(&blocked.complement()?)?;
        extra.intersect_assign(&walk.complement()?)?;
        for tile in extra.positions()? {
            if crate::regions::dash_path_ok(origin, tile)? {
                tiles.push(tile);
            }
        }
    }
    Ok(tiles)
}

// v8 (2026-10-24, the user's map 01starvation): paths treat holes as walls, and a dash only crossed a
// strip of holes (DASH_OVER_HOLES) if we happened to stand beside it with the dash ready, so a
// one-tile strip between us and the goal (starvation's row of holes at y=58, its hole columns) was
// walked round. With this on, a champion with a dash compares walking round with walking to a gap
// (rally::gap_crossings, within GAP_ROUTE_TILES), dashing over (waiting there for the cooldown if
// needed) and walking on, and takes the gap if it saves GAP_ROUTE_MARGIN steps or more. The full
// scan runs at most every GAP_ROUTE_RESCAN rounds. Off: walk round.
pub const GAP_ROUTE: bool = false;
const GAP_ROUTE_TILES: i32 = 12;
const GAP_ROUTE_MARGIN: i32 = 3;
const GAP_ROUTE_MIN_STEPS: i32 = 4;
const GAP_ROUTE_RESCAN: u32 = 3;

thread_local! {
    // (round scanned, chosen crossing): kept GAP_ROUTE_RESCAN rounds while it still pays.
    static GAP_PLAN: std::cell::Cell<Option<(u32, Option<(Vec2, Vec2)>)>> = const { std::cell::Cell::new(None) };
}

// The gap crossing (launch, landing) to take towards the goals of `field`, if one beats walking.
fn gap_route(field: &DistanceField, origin: Vec2) -> Result<Option<(Vec2, Vec2)>> {
    let me = me()?;
    let Some(dash) = me.abilities().iter().find(|a| a.is_dash()) else {
        return Ok(None);
    };
    let here = field.get(origin)?;
    let walk = if here == DistanceField::UNREACHABLE {
        i32::MAX
    } else {
        i32::from(here)
    };
    if walk < GAP_ROUTE_MIN_STEPS {
        return Ok(None);
    }
    let cooldown = dash.cooldown()?.max(0);
    let steps = super::walk::StepsFrom::new(origin);
    let total = |a: Vec2, b: Vec2| -> Result<Option<i32>> {
        let after = field.get(b)?;
        if after == DistanceField::UNREACHABLE {
            return Ok(None);
        }
        Ok(Some(steps.steps(a).max(cooldown) + 1 + i32::from(after)))
    };
    let round = cambc::round_number()?;
    // A recent choice, while it still beats walking.
    if let Some((at, choice)) = GAP_PLAN.with(|g| g.get()) {
        if round < at + GAP_ROUTE_RESCAN {
            return Ok(match choice {
                Some((a, b)) => total(a, b)?
                    .filter(|&t| t.saturating_add(GAP_ROUTE_MARGIN) <= walk)
                    .map(|_| (a, b)),
                None => None,
            });
        }
    }
    let mut best: Option<(i32, Vec2, Vec2)> = None;
    for &(a, b) in super::rally::gap_crossings()? {
        let near = super::world::distance(origin, a);
        if near > GAP_ROUTE_TILES {
            continue;
        }
        // Straight-line lower bound before any lookup.
        if (near.max(cooldown) + 1).saturating_add(GAP_ROUTE_MARGIN) >= walk {
            continue;
        }
        if let Some(t) = total(a, b)? {
            if t.saturating_add(GAP_ROUTE_MARGIN) <= walk && best.is_none_or(|(bt, ..)| t < bt) {
                best = Some((t, a, b));
            }
        }
    }
    let choice = best.map(|(_, a, b)| (a, b));
    GAP_PLAN.with(|g| g.set(Some((round, choice))));
    Ok(choice)
}

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
    for tile in dash_landings(origin, info.as_dyn().range()? as i32, blocked)? {
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
    let dashed = match dash.cast_at(best.0) {
        Ok(true) => true,
        Ok(false) => {
            super::world::remember_hidden_block(best.0);
            false
        }
        Err(_) => false,
    };
    forget_sight();
    Ok(dashed)
}

// Take one ordinary step. Our occupancy view can lag a unit that just appeared (e.g. right after a
// dash), and stepping onto a known occupied tile is an error that changes nothing; treat it as not
// moving instead of ending the turn. A step that fails because an unseen unit stands there is
// remembered, so we walk round it (world::HIDDEN_BLOCK_ROUNDS).
pub(super) fn step_to(tile: Vec2) -> bool {
    match me().and_then(|me| me.move_absolute(tile)) {
        Ok(true) => true,
        Ok(false) => {
            super::world::remember_hidden_block(tile);
            false
        }
        Err(_) => false,
    }
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
    // A landing-damage dash (attack::IMPACT_DASH) is kept for camps and fights, not travel.
    let may_dash = DASH_TRAVEL_MIN_STEPS > 0
        && !(super::attack::IMPACT_DASH && impact_radius().is_some())
        && fightable_enemies()?.is_empty();
    move_towards_closest_with_pull(goals, |_| 0.0, may_dash)
}

// Like move_towards_closest, but each candidate step also pays `pull(tile)` (in steps), so among
// steps that get similarly close to the goals we prefer the one `pull` likes best. With `may_dash`,
// a ready dash first jumps along the way when the goals are DASH_TRAVEL_MIN_STEPS or more away.
// Ties between equally short steps go towards the middle of the map. Moving in 8 directions there
// are usually several, and the first one listed won, the same way every time, so paths ran along
// walls and the map's edge. The bias is at most CENTRE_TIE_BREAK of a step (straight-line distance
// to the centre, over the map's diagonal), so it never makes a path longer. 0.0: off.
// 2026-10-21, 132 games: neutral (off 65 against on 67).
pub const CENTRE_TIE_BREAK: f32 = 0.2;

pub fn centre_bias(tile: Vec2) -> f32 {
    thread_local! {
        static MAP: std::cell::OnceCell<(f32, f32, f32)> = const { std::cell::OnceCell::new() };
    }
    if CENTRE_TIE_BREAK <= 0.0 {
        return 0.0;
    }
    let (cx, cy, diagonal) = MAP.with(|m| {
        *m.get_or_init(|| {
            let walls = cambc::map::walls().ok();
            let (w, h) = walls.map_or((1, 1), |b| (b.width(), b.height()));
            let (w, h) = (w as f32, h as f32);
            (
                (w - 1.0) / 2.0,
                (h - 1.0) / 2.0,
                (w * w + h * h).sqrt().max(1.0),
            )
        })
    });
    let (dx, dy) = (tile.x as f32 - cx, tile.y as f32 - cy);
    CENTRE_TIE_BREAK * (dx * dx + dy * dy).sqrt() / diagonal
}

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
        // Every goal is a tile we can't stand on: as unreachable as a sealed one (the farm target
        // is then skipped). Not flagging it left a champion standing still with its goals all
        // blocked (Vendetta, 117 turns on 01concentrated-wealth).
        trace("goals-all-blocked");
        GOALS_UNREACHABLE.with(|u| u.set(true));
        return Ok(false);
    }

    let field = path_field(&blocked, &goals, origin)?;
    checkpoint("path-field");
    GOALS_UNREACHABLE.with(|u| u.set(field.get(origin) == Ok(DistanceField::UNREACHABLE)));

    // GAP_ROUTE: through a gap a dash crosses, when that beats walking round.
    if GAP_ROUTE && may_dash {
        if let Some((launch, landing)) = gap_route(&field, origin)? {
            if origin == launch {
                if let Some(dash) = ready_dash()? {
                    if dash.cast_at(landing).unwrap_or(false) {
                        trace("gap-dash");
                        forget_sight();
                        return Ok(true);
                    }
                    // Landing taken (an unseen unit): walk round this turn.
                } else {
                    trace("gap-wait");
                    return Ok(false); // the dash is back soon (counted in the route)
                }
            } else {
                trace("gap-route");
                let mut to_launch = Bitboard::new()?;
                to_launch.set(launch, true)?;
                return move_towards_closest_with_pull(to_launch, pull, false);
            }
        }
    }

    let score = |tile: Vec2| -> Result<f32> {
        let d = field.get(tile)?;
        Ok(if d == DistanceField::UNREACHABLE {
            f32::MAX
        } else {
            d as f32 + pull(tile) + centre_bias(tile)
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
    let mut best = field.get(origin)? as f32 + pull(origin) + centre_bias(origin);
    for tile in blocked
        .reachable_tiles(origin, me.base_stats()?.base_move_speed as u16)?
        .positions()?
    {
        let d = field.get(tile)?;
        if d == DistanceField::UNREACHABLE {
            continue;
        }
        let score = d as f32 + pull(tile) + centre_bias(tile);
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
    dist: Vec<u16>, // the whole field, copied out (small maps)
    // LAZY_WALKGRID_TILES (big maps): the field itself, asked tile by tile, answers remembered in a
    // window of max_steps around the origin (nothing further is reachable).
    field: Option<DistanceField>,
    origin: Vec2,
    reach: i32,
    asked: std::cell::RefCell<Vec<u16>>,
}
const NOT_ASKED: u16 = DistanceField::UNREACHABLE - 1;

// v9 (2026-10-25): the fight view's walking grid was copied out whole (~2.4 fuel a tile: 23k on a
// 100x100 map, 3x what computing it cost) though it only reaches MAX_CHASE_STEPS from us and a duel
// reads ~130-170 tiles of it. Asking the field one tile at a time costs ~79 fuel, so on maps of
// LAZY_WALKGRID_TILES tiles or more the field is kept and asked tile by tile (answers remembered);
// smaller maps still copy it out. 0: always copy.
pub const LAZY_WALKGRID_TILES: i32 = 5000;

impl WalkGrid {
    pub fn from(origin: Vec2, blocked: &Bitboard, max_steps: u16) -> Result<Self> {
        let m = super::trace::meter("fill-walkgrid");
        let mut field = DistanceField::new()?;
        field.compute(
            blocked,
            &tiles_around(origin, 0)?,
            &mut Bitboard::new()?,
            max_steps,
        )?;
        drop(m);
        let (width, height) = (field.width(), field.height());
        if LAZY_WALKGRID_TILES > 0 && width * height >= LAZY_WALKGRID_TILES {
            let reach = i32::from(max_steps).min(width.max(height));
            let side = (2 * reach + 1) as usize;
            return Ok(Self {
                width,
                height,
                dist: Vec::new(),
                field: Some(field),
                origin,
                reach,
                asked: std::cell::RefCell::new(vec![NOT_ASKED; side * side]),
            });
        }
        let _e = super::trace::meter("fill-walkgrid-export");
        Ok(Self {
            width,
            height,
            dist: field.export_linear_buffer()?,
            field: None,
            origin,
            reach: 0,
            asked: std::cell::RefCell::new(Vec::new()),
        })
    }

    // Steps from the origin to `tile`, or UNREACHABLE (also for tiles off the map).
    pub fn get(&self, tile: Vec2) -> u16 {
        if tile.x < 0 || tile.y < 0 || tile.x >= self.width || tile.y >= self.height {
            return DistanceField::UNREACHABLE;
        }
        let Some(field) = &self.field else {
            return self.dist[(tile.y * self.width + tile.x) as usize];
        };
        let (dx, dy) = (tile.x - self.origin.x, tile.y - self.origin.y);
        if dx.abs() > self.reach || dy.abs() > self.reach {
            return DistanceField::UNREACHABLE;
        }
        let i = ((dy + self.reach) * (2 * self.reach + 1) + dx + self.reach) as usize;
        let known = self.asked.borrow()[i];
        if known != NOT_ASKED {
            return known;
        }
        let d = field.get(tile).unwrap_or(DistanceField::UNREACHABLE);
        self.asked.borrow_mut()[i] = d;
        d
    }

    // Shortest path from the origin to `end`, excluding the origin, traced back along the grid.
    pub fn path_to(&self, end: Vec2) -> Option<Vec<Vec2>> {
        let mut path = vec![end];
        let mut pos = end;
        while self.get(pos) > 1 {
            let d = self.get(pos);
            // Of the neighbours a step closer, the one nearest the map's centre (centre_bias): the
            // first one found went the same way every time, along walls and edges.
            pos = (-1..=1)
                .flat_map(|dx| (-1..=1).map(move |dy| Vec2::new(pos.x + dx, pos.y + dy)))
                // (a tile d - 1 steps away is at most d - 1 tiles away: skip the rest unasked)
                .filter(|&n| distance(n, self.origin) < i32::from(d) && self.get(n) == d - 1)
                .min_by(|&a, &b| centre_bias(a).total_cmp(&centre_bias(b)))?;
            path.push(pos);
        }
        path.reverse();
        Some(path)
    }
}
