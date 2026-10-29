// Movement along distance fields: step to the free neighbour with the smallest field value. Dashes
// cross holes when a path needs one, and shortcut long walks when no hole crossing is coming up
// within a cooldown's worth of steps (so the dash is ready again when it is needed).
use crate::combat::{cast_at, is_ready, Abil};
use crate::mapinfo::{Field, MapInfo, FAR};
use crate::util::{cheb, DIRS};
use cambc::{Champion, Result, Vec2};

pub struct Ctx<'a> {
    pub info: &'a MapInfo,
    pub me: &'a Champion,
    pub pos: Vec2,
    // Tiles where a unit we can see stands (champions and living camps), this turn. A short list:
    // scanning it beats any per-tile map pass (a native loop over 10k tiles costs ~60k fuel).
    pub occ: &'a [Vec2],
    pub dash: Option<Abil>,
}

const DASH_COOLDOWN: u16 = 8;

impl<'a> Ctx<'a> {
    pub fn free(&self, p: Vec2) -> bool {
        self.info.walkable(p) && !self.occ.contains(&p)
    }

    fn dash_ready(&self) -> bool {
        self.dash.map_or(false, |d| is_ready(self.me, &d))
    }

    fn try_dash(&self, to: Vec2) -> Result<bool> {
        match self.dash {
            Some(d) if is_ready(self.me, &d) && self.free(to) => {
                Ok(cast_at(self.me, &d, to).unwrap_or(false))
            }
            _ => Ok(false),
        }
    }

    // One walking step down `f`. Returns the tile we're on afterwards.
    fn walk(&self, f: &Field, from: Vec2) -> Result<Option<Vec2>> {
        if !self.me.can_move().unwrap_or(false) {
            return Ok(None);
        }
        let here = f.at(self.info, from);
        let mut opts: [(u16, Vec2); 8] = [(FAR, from); 8];
        let mut n = 0;
        for d in DIRS {
            let q = from + d;
            if !self.free(q) {
                continue;
            }
            let v = f.at(self.info, q);
            // Downhill, or sideways to get round a unit in the way.
            if v < here || (v == here && here != FAR && here > 0) {
                opts[n] = (v, q);
                n += 1;
            }
        }
        let opts = &mut opts[..n];
        opts.sort_by_key(|o| o.0);
        for (v, q) in opts.iter() {
            if *v > here {
                break;
            }
            // false: a hidden unit (bush) stands there; it's now revealed, try the next.
            if self.me.move_absolute(*q).unwrap_or(false) {
                return Ok(Some(*q));
            }
        }
        Ok(None)
    }

    // Would following `df` from `from` need a hole crossing within `steps` steps?
    fn crossing_ahead(&self, df: &Field, from: Vec2, steps: u16) -> bool {
        let mut p = from;
        for _ in 0..steps {
            let here = df.at(self.info, p);
            if here == 0 || here == FAR {
                return false;
            }
            let mut next = None;
            for d in DIRS {
                let q = p + d;
                if df.at(self.info, q) < here {
                    next = Some(q);
                    break;
                }
            }
            match next {
                Some(q) if self.info.is_hole(q) => return true,
                Some(q) => p = q,
                None => return false,
            }
        }
        false
    }

    // Best dash landing: a free tile within 2 that gains the most along the field.
    fn dash_landing(&self, f: &Field, from: Vec2, min_gain: u16) -> Option<Vec2> {
        let here = f.at(self.info, from);
        if here == FAR {
            return None;
        }
        let mut best: Option<(u16, Vec2)> = None;
        for dx in -2..=2 {
            for dy in -2..=2 {
                let q = Vec2::new(from.x + dx, from.y + dy);
                if q == from || !self.free(q) {
                    continue;
                }
                let v = f.at(self.info, q);
                if v == FAR || v + min_gain > here {
                    continue;
                }
                if best.map_or(true, |b| v < b.0) {
                    best = Some((v, q));
                }
            }
        }
        best.map(|b| b.1)
    }

    // Move along a group/altar/home field (walking), using the dash field when only a dash gets
    // there. `travel_dash`: allow dashing purely for speed.
    pub fn go(&self, f: &Field, df: Option<&Field>, travel_dash: bool) -> Result<Vec2> {
        let mut pos = self.pos;
        let walking = f.at(self.info, pos);
        if walking == FAR {
            // Only reachable across holes.
            let Some(df) = df else { return Ok(pos) };
            if let Some(q) = self.dash_landing(df, pos, 2) {
                let crosses = DIRS.iter().any(|d| self.info.is_hole(pos + *d) && cheb(pos + *d, q) <= 1);
                if crosses && self.dash_ready() && self.try_dash(q)? {
                    pos = q;
                }
            }
            if let Some(q) = self.walk_dash_field(df, pos)? {
                pos = q;
            }
            return Ok(pos);
        }
        if travel_dash && walking >= 6 && self.dash_ready() {
            let crossing = df.map_or(false, |d| self.crossing_ahead(d, pos, DASH_COOLDOWN + 2));
            if !crossing {
                if let Some(q) = self.dash_landing(f, pos, 2) {
                    if self.try_dash(q)? {
                        pos = q;
                    }
                }
            }
        }
        if f.at(self.info, pos) > 0 {
            if let Some(q) = self.walk(f, pos)? {
                pos = q;
            }
        }
        Ok(pos)
    }

    // Walk along the hole-crossing field, never onto a hole.
    fn walk_dash_field(&self, df: &Field, from: Vec2) -> Result<Option<Vec2>> {
        if !self.me.can_move().unwrap_or(false) {
            return Ok(None);
        }
        let here = df.at(self.info, from);
        let mut best: Option<(u16, Vec2)> = None;
        for d in DIRS {
            let q = from + d;
            if !self.free(q) {
                continue;
            }
            let v = df.at(self.info, q);
            if v < here && best.map_or(true, |b| v < b.0) {
                best = Some((v, q));
            }
        }
        if let Some((_, q)) = best {
            if self.me.move_absolute(q).unwrap_or(false) {
                return Ok(Some(q));
            }
        }
        Ok(None)
    }

    // Greedy step toward a point (no field): the free neighbour closest to it. For short hops
    // (chasing a visible enemy, closing to attack range).
    pub fn toward(&self, target: Vec2) -> Result<Vec2> {
        if !self.me.can_move().unwrap_or(false) {
            return Ok(self.pos);
        }
        let here = cheb(self.pos, target);
        let mut opts: Vec<(i32, i32, Vec2)> = Vec::with_capacity(8);
        for d in DIRS {
            let q = self.pos + d;
            if self.free(q) {
                let c = cheb(q, target);
                let manhattan = (q.x - target.x).abs() + (q.y - target.y).abs();
                if c <= here {
                    opts.push((c, manhattan, q));
                }
            }
        }
        opts.sort();
        for (c, _, q) in opts {
            if c >= here && here <= 1 {
                break;
            }
            if self.me.move_absolute(q).unwrap_or(false) {
                return Ok(q);
            }
        }
        Ok(self.pos)
    }

    // Step to the free neighbour with the lowest cost (staying put allowed when it's cheapest).
    pub fn best_step(&self, cost: impl Fn(Vec2) -> f32) -> Result<Vec2> {
        if !self.me.can_move().unwrap_or(false) {
            return Ok(self.pos);
        }
        let mut opts: Vec<(f32, Vec2)> = Vec::with_capacity(9);
        let stay = cost(self.pos);
        for d in DIRS {
            let q = self.pos + d;
            if self.free(q) {
                let c = cost(q);
                if c < stay {
                    opts.push((c, q));
                }
            }
        }
        opts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));
        for (_, q) in opts {
            if self.me.move_absolute(q).unwrap_or(false) {
                return Ok(q);
            }
        }
        Ok(self.pos)
    }

    // Escape dash: the free tile within 2 with the lowest cost, if it beats every walking option.
    pub fn dash_escape(&self, cost: impl Fn(Vec2) -> f32) -> Result<Option<Vec2>> {
        if !self.dash_ready() {
            return Ok(None);
        }
        let mut best: Option<(f32, Vec2)> = None;
        for dx in -2i32..=2 {
            for dy in -2i32..=2 {
                if dx.abs().max(dy.abs()) < 2 {
                    continue;
                }
                let q = Vec2::new(self.pos.x + dx, self.pos.y + dy);
                if !self.free(q) {
                    continue;
                }
                let c = cost(q);
                if best.map_or(true, |b| c < b.0) {
                    best = Some((c, q));
                }
            }
        }
        if let Some((c, q)) = best {
            let walk_best = DIRS
                .iter()
                .map(|d| self.pos + *d)
                .filter(|q| self.free(*q))
                .map(&cost)
                .fold(cost(self.pos), f32::min);
            if c < walk_best && self.try_dash(q)? {
                return Ok(Some(q));
            }
        }
        Ok(None)
    }
}
