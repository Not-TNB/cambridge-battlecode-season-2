// Static map knowledge, built once per bot instance (draft and every champion build the same thing,
// so group ids agree across the team without any communication).
//
// Navigation uses whole-map distance fields from the engine's flood fill, exported once into a
// native Vec<u16> so every later read is a plain array index instead of a host call. A field is
// rooted at a goal (a camp group's tiles, the tiles next to an altar, our spawn), so walking to that
// goal from anywhere is "step to the neighbour with the smallest value": no parent map and no path
// to store, and the field never goes stale because terrain never changes. Units in the way are
// handled at step time (nav.rs). Group fields are built lazily from spare fuel and kept.
use crate::util::{cheb, DIRS};
use cambc::{map, Bitboard, CampType, DistanceField, Result, Team, Vec2};

pub const FAR: u16 = DistanceField::UNREACHABLE;

pub struct Field {
    pub d: Vec<u16>,
}

pub struct Group {
    pub tiles: Vec<Vec2>,
    pub center: Vec2,
    pub gems: u32,
    pub gold: u32,
    pub respawn: i32,
    pub hp: f32,
    // Summed strength of the monsters that hit back, and the longest reach among them.
    pub dps: f32,
    pub range: i32,
    // Monolith or golem: a late-game objective, never an early farm target.
    pub boss: bool,
    // Only clusters: nothing hits back.
    pub harmless: bool,
    // Walking steps from our spawn and from theirs (FAR when only a dash gets there).
    pub d_home: u16,
    pub d_enemy: u16,
}

pub struct MapInfo {
    pub w: i32,
    pub h: i32,
    // Packed bits (see bits()): set where we can't walk: walls, holes, altars, enemy spawn.
    pub blocked: Vec<u8>,
    pub holes: Vec<u8>,
    pub bush: Vec<u8>,
    pub blocked_bb: Bitboard,
    // Holes are passable here: the dashers' fields.
    pub dash_bb: Bitboard,
    pub groups: Vec<Group>,
    pub altars: Vec<Vec2>,
    pub ally_spawn: Vec<Vec2>,
    pub enemy_spawn: Vec<Vec2>,
    pub home_center: Vec2,
    pub enemy_center: Vec2,
    pub home: Field,
    pub enemy_home: Field,
    pub altar_field: Field,
    // Engine handles, same order as `groups` (for live group state queries).
    pub handles: Vec<cambc::CampGroup>,
    group_fields: Vec<Option<Field>>,
    dash_fields: Vec<Option<Field>>,
    // Steps between the two spawns (map scale for every distance threshold).
    pub spawn_gap: i32,
    // Fuel one whole-map flood (with export) cost here, measured on the first one.
    pub flood_cost: u64,
}

pub fn camp_stats(kind: CampType) -> (f32, f32, i32) {
    match kind {
        CampType::Wolf => (1350.0, 20.0, 3),
        CampType::Elk => (2400.0, 60.0, 4),
        CampType::GemCluster | CampType::GoldCluster => (500.0, 0.0, 3),
        CampType::Monolith => (20000.0, 200.0, 5),
        CampType::Golem => (5000.0, 120.0, 4),
    }
}

impl Field {
    #[inline]
    pub fn at(&self, info: &MapInfo, p: Vec2) -> u16 {
        if p.x < 0 || p.y < 0 || p.x >= info.w || p.y >= info.h {
            return FAR;
        }
        self.d[(p.y * info.w + p.x) as usize]
    }
}

// Bitboards export bit-packed: tile i = y * w + x is bit (i & 7) of byte i >> 3 (checked against
// is_set, 2026-10-29). Kept packed: reading a bit is as cheap as reading a byte, and unpacking a
// 10k-tile map would cost ~60k fuel.
pub fn bits(bb: &Bitboard, _n: usize) -> Result<Vec<u8>> {
    let mut v = vec![0u8; bb.linear_buffer_size()];
    bb.export_linear_buffer_into(&mut v)?;
    Ok(v)
}

#[inline]
pub fn bit(v: &[u8], i: usize) -> bool {
    (v[i >> 3] >> (i & 7)) & 1 != 0
}

pub fn flood(blocked: &Bitboard, roots: &Bitboard, w: i32, h: i32) -> Result<Field> {
    let mut df = DistanceField::new()?;
    let mut visited = Bitboard::new()?;
    df.compute(blocked, roots, &mut visited, u16::MAX)?;
    let mut d = vec![0u16; (w * h) as usize];
    df.export_linear_buffer_into(&mut d)?;
    Ok(Field { d })
}

fn centre(tiles: &[Vec2]) -> Vec2 {
    let n = tiles.len().max(1) as i32;
    let (sx, sy) = tiles.iter().fold((0, 0), |(x, y), p| (x + p.x, y + p.y));
    let c = Vec2::new(sx / n, sy / n);
    // Snap to the member tile nearest the mean so the centre is a real tile.
    *tiles.iter().min_by_key(|p| cheb(**p, c)).unwrap_or(&c)
}

impl MapInfo {
    pub fn new() -> Result<Self> {
        let (w, h) = (map::width()?, map::height()?);
        let n = (w * h) as usize;
        let mut neutral = map::walls()?.union(map::holes()?)?;
        neutral.union_assign(map::altars()?)?;
        let mut blocked_bb = neutral.clone();
        blocked_bb.union_assign(map::spawn_region(Team::Enemy)?)?;
        let mut dash_bb = map::walls()?.union(map::altars()?)?;
        dash_bb.union_assign(map::spawn_region(Team::Enemy)?)?;
        let blocked = bits(&blocked_bb, n)?;
        let holes = bits(map::holes()?, n)?;
        let bush = bits(map::bushes()?, n)?;
        let ally_spawn = map::spawn_region(Team::Ally)?.positions()?;
        let enemy_spawn = map::spawn_region(Team::Enemy)?.positions()?;
        let f0 = cambc::fuel_remaining().unwrap_or(0);
        let home = flood(&blocked_bb, map::spawn_region(Team::Ally)?, w, h)?;
        let flood_cost = f0.saturating_sub(cambc::fuel_remaining().unwrap_or(0)).max(2_000);
        // Their field ignores their own spawn as an obstacle (it's ours that blocks them).
        let mut their_block = neutral.clone();
        their_block.union_assign(map::spawn_region(Team::Ally)?)?;
        let enemy_home = flood(&their_block, map::spawn_region(Team::Enemy)?, w, h)?;
        let altars = map::altars()?.positions()?;
        let mut altar_roots = Bitboard::new()?;
        for a in &altars {
            for d in DIRS {
                let p = *a + d;
                if p.x >= 0 && p.y >= 0 && p.x < w && p.y < h && !bit(&blocked, (p.y * w + p.x) as usize) {
                    altar_roots.set(p, true)?;
                }
            }
        }
        let altar_field = flood(&blocked_bb, &altar_roots, w, h)?;
        let mut info = MapInfo {
            w,
            h,
            blocked,
            holes,
            bush,
            blocked_bb,
            dash_bb,
            groups: Vec::new(),
            altars,
            home_center: centre(&ally_spawn),
            enemy_center: centre(&enemy_spawn),
            ally_spawn,
            enemy_spawn,
            home,
            enemy_home,
            altar_field,
            handles: Vec::new(),
            group_fields: Vec::new(),
            dash_fields: Vec::new(),
            spawn_gap: 0,
            flood_cost,
        };
        let gap = info.home.at(&info, info.enemy_center);
        info.spawn_gap = if gap == FAR { w.max(h) } else { gap as i32 }.max(8);
        let handles = map::camp_groups()?;
        for g in handles.iter() {
            let tiles: Vec<Vec2> = g.camps().iter().map(|c| c.spawn_position()).collect();
            let (mut hp, mut dps, mut range, mut boss, mut harmless) = (0.0, 0.0, 0, false, true);
            for c in g.camps() {
                let (bh, bs, br) = match c.base_stats() {
                    Ok(s) => (s.base_health, s.base_strength, s.base_attack_range),
                    Err(_) => camp_stats(c.kind()),
                };
                hp += bh;
                if bs > 0.0 {
                    dps += bs;
                    range = range.max(br);
                    harmless = false;
                }
                boss |= matches!(c.kind(), CampType::Monolith | CampType::Golem);
            }
            let d_home = tiles.iter().map(|p| info.home.at(&info, *p)).min().unwrap_or(FAR);
            let d_enemy = tiles.iter().map(|p| info.enemy_home.at(&info, *p)).min().unwrap_or(FAR);
            info.groups.push(Group {
                center: centre(&tiles),
                tiles,
                gems: g.gem_value(),
                gold: g.gold_value(),
                respawn: g.max_respawn_timer(),
                hp,
                dps,
                range,
                boss,
                harmless,
                d_home,
                d_enemy,
            });
        }
        info.handles = handles;
        info.group_fields = (0..info.groups.len()).map(|_| None).collect();
        info.dash_fields = (0..info.groups.len()).map(|_| None).collect();
        Ok(info)
    }

    #[inline]
    pub fn idx(&self, p: Vec2) -> usize {
        (p.y * self.w + p.x) as usize
    }
    #[inline]
    pub fn inside(&self, p: Vec2) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.w && p.y < self.h
    }
    #[inline]
    pub fn walkable(&self, p: Vec2) -> bool {
        self.inside(p) && !bit(&self.blocked, self.idx(p))
    }
    #[inline]
    pub fn is_bush(&self, p: Vec2) -> bool {
        self.inside(p) && bit(&self.bush, self.idx(p))
    }
    #[inline]
    pub fn is_hole(&self, p: Vec2) -> bool {
        self.inside(p) && bit(&self.holes, self.idx(p))
    }

    pub fn has_field(&self, g: usize) -> bool {
        self.group_fields[g].is_some()
    }

    // Build group g's walking field (and, for dashers, its hole-crossing field) if missing.
    pub fn ensure_field(&mut self, g: usize, dasher: bool) -> Result<()> {
        let mut roots = None;
        if self.group_fields[g].is_none() {
            let mut r = Bitboard::new()?;
            for p in &self.groups[g].tiles {
                r.set(*p, true)?;
            }
            self.group_fields[g] = Some(flood(&self.blocked_bb, &r, self.w, self.h)?);
            roots = Some(r);
        }
        if dasher && self.dash_fields[g].is_none() {
            let r = match roots {
                Some(r) => r,
                None => {
                    let mut r = Bitboard::new()?;
                    for p in &self.groups[g].tiles {
                        r.set(*p, true)?;
                    }
                    r
                }
            };
            self.dash_fields[g] = Some(flood(&self.dash_bb, &r, self.w, self.h)?);
        }
        Ok(())
    }

    pub fn group_field(&self, g: usize) -> Option<&Field> {
        self.group_fields[g].as_ref()
    }
    pub fn dash_field(&self, g: usize) -> Option<&Field> {
        self.dash_fields[g].as_ref()
    }

    // Walking steps from p to group g; an estimate when its field isn't built yet.
    pub fn group_dist(&self, g: usize, p: Vec2) -> i32 {
        match &self.group_fields[g] {
            Some(f) => {
                let d = f.at(self, p);
                if d == FAR {
                    i32::MAX / 4
                } else {
                    d as i32
                }
            }
            None => {
                let c = self.groups[g].tiles.iter().map(|t| cheb(*t, p)).min().unwrap_or(0);
                c * 5 / 4 + 1
            }
        }
    }
}
