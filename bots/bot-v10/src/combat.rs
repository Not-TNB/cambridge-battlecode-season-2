// Combat model: cheap per-champion damage/health numbers, a Lanchester-style fight estimate, and
// how we spend attacks and abilities.
//
// Cost control: every number comes from a handful of host queries per visible champion, done once a
// turn (Unit::sense); the fight estimate and tile threat are plain arithmetic over those ≤10 units.
// Nothing is evaluated per map tile except the ≤9 tiles we could step to.
use crate::util::cheb;
use cambc::{AbilityInfo, AbilityKind, Camp, Champion, ChampionType, Result, Vec2};

#[derive(Clone, Copy)]
pub struct Abil {
    pub slot: usize,
    pub kind: AbilityKind,
    pub range: i32,
    pub modifier: f32,
    pub radius: i32,
    // "Excludes walking": can't share a turn with a step.
    pub excl: bool,
    pub stuns: bool,
}

// Ability metadata never changes per champion type: read it once per type, then copy.
thread_local! {
    static ABIL_CACHE: core::cell::RefCell<Vec<(ChampionType, [Option<Abil>; 2], i32)>> = const { core::cell::RefCell::new(Vec::new()) };
}

// (abilities, basic attack range) for this champion's type.
pub fn kit(c: &Champion) -> Option<([Option<Abil>; 2], i32)> {
    let t = c.champion_name().ok()?;
    kit_of(c, t)
}

pub fn kit_of(c: &Champion, t: ChampionType) -> Option<([Option<Abil>; 2], i32)> {
    if let Some(k) = ABIL_CACHE.with(|v| v.borrow().iter().find(|e| e.0 == t).map(|e| (e.1, e.2))) {
        return Some(k);
    }
    let a = abils(c);
    let r = c.attack_range().unwrap_or(1);
    ABIL_CACHE.with(|v| v.borrow_mut().push((t, a, r)));
    Some((a, r))
}

pub fn abils(c: &Champion) -> [Option<Abil>; 2] {
    let mut out = [None, None];
    for (slot, a) in c.abilities().iter().enumerate() {
        let Ok(Some(info)) = a.info() else { continue };
        let excl = info.as_dyn().is_exclusive_with_movement();
        let (kind, range, modifier, radius, stuns) = match info {
            AbilityInfo::AttackReset(_) => (AbilityKind::AttackReset, 0.0, 0.0, 0.0, false),
            AbilityInfo::Damage(d) => (AbilityKind::Damage, d.range, d.modifier, 0.0, false),
            AbilityInfo::AoeDamage(d) => (AbilityKind::AoeDamage, d.range, d.modifier, d.radius, d.duration > 0),
            AbilityInfo::Cc(d) => (AbilityKind::Cc, d.range, d.modifier, 0.0, true),
            AbilityInfo::Heal(d) => (AbilityKind::Heal, d.range, d.modifier, 0.0, false),
            AbilityInfo::Dash(d) => (AbilityKind::Dash, d.range, d.modifier, d.radius, d.duration > 0),
            AbilityInfo::Shield(d) => (AbilityKind::Shield, d.range, d.modifier, 0.0, false),
        };
        out[slot] = Some(Abil {
            slot,
            kind,
            range: range as i32,
            modifier,
            radius: radius as i32,
            excl,
            stuns,
        });
    }
    out
}

// A visible champion, summarised for the fight estimate.
#[derive(Clone)]
pub struct Unit {
    pub champ: Champion,
    pub kind: ChampionType,
    pub pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    // Sustained damage per turn against champions.
    pub dps: f32,
    // Ability damage available right now (front-loaded into the first exchanges).
    pub burst: f32,
    pub range: i32,
    pub stunned: bool,
    pub submitting: bool,
    // Has a dash in its kit: reaches two tiles further when it's up.
    pub dasher: bool,
}

impl Unit {
    // Tiles from which this unit can hit us next turn (walk or dash, then attack).
    pub fn reach(&self) -> i32 {
        self.range + 1 + if self.dasher { 2 } else { 0 }
    }
}

impl Unit {
    // Allies: position, health and an average damage figure are all we use (5 queries, not ~20).
    pub fn sense_light(c: &Champion, kind: ChampionType) -> Option<Unit> {
        let pos = c.position().ok()?;
        let hp = c.health().ok()?;
        if hp <= 0.0 {
            return None;
        }
        let max_hp = c.max_health().unwrap_or(hp).max(hp);
        let strength = c.strength().unwrap_or(40.0);
        let focus = c.focus().unwrap_or(100.0);
        let (kit, range) = kit_of(c, kind)?;
        let mut dps = strength;
        let mut burst = 0.0;
        for a in kit.iter().flatten() {
            match a.kind {
                AbilityKind::Damage | AbilityKind::AoeDamage | AbilityKind::Cc | AbilityKind::Dash => {
                    dps += focus * a.modifier / 8.0;
                    burst += focus * a.modifier * 0.5;
                }
                AbilityKind::AttackReset => dps += strength / 8.0,
                _ => {}
            }
        }
        if kind == ChampionType::Varg {
            dps *= 1.25;
        }
        let dasher = kit.iter().flatten().any(|a| a.kind == AbilityKind::Dash);
        Some(Unit { champ: c.clone(), kind, pos, hp, max_hp, dps, burst, range, stunned: false, submitting: false, dasher })
    }

    pub fn sense(c: &Champion) -> Option<Unit> {
        let pos = c.position().ok()?;
        let hp = c.health().ok()?;
        if hp <= 0.0 {
            return None;
        }
        let max_hp = c.max_health().unwrap_or(hp).max(hp);
        let strength = c.strength().unwrap_or(40.0);
        let focus = c.focus().unwrap_or(100.0);
        let kind = c.champion_name().ok()?;
        let (kit, range) = kit(c)?;
        let mut dps = strength;
        let mut burst = 0.0;
        let mut hp_eff = hp;
        for a in kit.iter().flatten() {
            let ready = c.abilities()[a.slot].is_ready().unwrap_or(true);
            match a.kind {
                AbilityKind::Damage | AbilityKind::AoeDamage | AbilityKind::Cc | AbilityKind::Dash => {
                    let dmg = focus * a.modifier;
                    dps += dmg / 8.0;
                    if ready {
                        burst += dmg;
                    }
                    // A stun takes an enemy's turn away: worth roughly a turn of their damage.
                    if a.stuns && ready {
                        burst += 50.0;
                    }
                }
                AbilityKind::AttackReset => {
                    dps += strength / 8.0;
                    if ready {
                        burst += strength;
                    }
                }
                AbilityKind::Shield if ready => hp_eff += focus * a.modifier,
                AbilityKind::Heal if ready => hp_eff += focus * a.modifier * 0.5,
                _ => {}
            }
        }
        match kind {
            ChampionType::Varg => dps *= 1.25,
            ChampionType::Ren => dps += focus * 0.2,
            ChampionType::HeartOfRuin => dps *= 1.0 + 0.5 * (1.0 - hp / max_hp),
            _ => {}
        }
        let buffs = c.buffs().ok();
        if let Some(b) = &buffs {
            if let Some(cambc::BuffPayload::Shield { value, .. }) = b.get(cambc::BuffKind::Shield) {
                hp_eff += value;
            }
        }
        Some(Unit {
            champ: c.clone(),
            kind,
            pos,
            hp: hp_eff,
            max_hp,
            dps,
            burst,
            range,
            stunned: buffs.map(|b| b.has_stun()).unwrap_or(false),
            submitting: c.is_submitting_gems().unwrap_or(false),
            dasher: kit.iter().flatten().any(|a| a.kind == AbilityKind::Dash),
        })
    }
}

// Lanchester square law: a side's strength is damage output times health pool. Burst counts as a
// few turns' worth of extra damage up front.
pub fn power<'a>(units: impl Iterator<Item = &'a Unit>) -> f32 {
    let (mut dmg, mut hp) = (0.0, 0.0);
    for u in units {
        dmg += u.dps + u.burst * 0.25;
        hp += u.hp;
    }
    dmg * hp
}

// Damage enemies could deal to someone standing on `tile` next turn (one step then attack, or an
// ability at range 4).
pub fn threat(tile: Vec2, enemies: &[Unit]) -> f32 {
    let mut t = 0.0;
    for e in enemies {
        if e.stunned {
            continue;
        }
        let d = cheb(tile, e.pos);
        if d <= e.range + 1 {
            t += e.dps;
        }
        if d <= 5 {
            t += e.burst * 0.5;
        }
    }
    t
}

// Pick whom to hit among enemies in `reach` of `from`: a kill this turn first, then the most
// damage removed per point of health we must chew through.
pub fn best_target<'a>(enemies: &'a [Unit], from: Vec2, reach: i32, my_hit: f32) -> Option<&'a Unit> {
    enemies
        .iter()
        .filter(|e| cheb(e.pos, from) <= reach)
        .max_by(|a, b| {
            let sa = if a.hp <= my_hit { 1e6 } else { 0.0 } + a.dps / a.hp.max(1.0) * 1000.0 + if a.submitting { 50.0 } else { 0.0 };
            let sb = if b.hp <= my_hit { 1e6 } else { 0.0 } + b.dps / b.hp.max(1.0) * 1000.0 + if b.submitting { 50.0 } else { 0.0 };
            sa.partial_cmp(&sb).unwrap_or(core::cmp::Ordering::Equal)
        })
}

// Try a basic attack; false when it isn't allowed now.
pub static mut STATS: [u32; 6] = [0; 6];
pub fn attack_champ(me: &Champion, t: &Champion) -> bool {
    let r = me.attack(t);
    unsafe {
        STATS[0] += 1;
        if r.is_ok() {
            STATS[1] += 1;
        } else if STATS[2] < 5 {
            STATS[2] += 1;
            println!("atk err {:?} me {:?} t {:?} d {}", r, me.position(), t.position(), crate::util::cheb(me.position().unwrap_or_default(), t.position().unwrap_or_default()));
        }
    }
    r.is_ok()
}

pub fn attack_camp(me: &Champion, t: &Camp) -> bool {
    me.attack(t).is_ok()
}

// The tile within `range` of `from` that hits the most enemies in an area of `radius`.
pub fn aoe_spot(from: Vec2, range: i32, radius: i32, enemies: &[Unit]) -> Option<(Vec2, usize)> {
    let mut best: Option<(Vec2, usize)> = None;
    for e in enemies {
        if cheb(from, e.pos) > range + radius {
            continue;
        }
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let c = Vec2::new(e.pos.x + dx, e.pos.y + dy);
                if cheb(from, c) > range {
                    continue;
                }
                let n = enemies.iter().filter(|o| cheb(o.pos, c) <= radius).count();
                if best.map_or(true, |(_, b)| n > b) {
                    best = Some((c, n));
                }
            }
        }
    }
    best
}

pub fn is_ready(me: &Champion, a: &Abil) -> bool {
    me.abilities()[a.slot].is_ready().unwrap_or(false)
}

pub fn cast_on_champ(me: &Champion, a: &Abil, t: &Champion) -> Result<bool> {
    let r = me.abilities()[a.slot].cast_on(t);
    unsafe {
        STATS[3] += 1;
        if matches!(r, Ok(true)) {
            STATS[4] += 1;
        } else if STATS[5] < 5 {
            STATS[5] += 1;
            println!("cast err {:?} {:?}", r, a.kind);
        }
    }
    r
}

pub fn cast_on_camp(me: &Champion, a: &Abil, t: &Camp) -> Result<bool> {
    me.abilities()[a.slot].cast_on(t)
}

pub fn cast_at(me: &Champion, a: &Abil, p: Vec2) -> Result<bool> {
    me.abilities()[a.slot].cast_at(p)
}

pub fn cast_self(me: &Champion, a: &Abil) -> Result<bool> {
    me.abilities()[a.slot].cast_untargeted()
}

// Death-by-death team fight simulation (adapted from v9's event model). Each side pours its
// damage into one target of the other side; whichever target dies first is removed, the time is
// advanced, and the sums are redone. Our side tries two focus orders (lowest health, then highest
// damage) and keeps the better; the enemy is assumed to focus our lowest health. Returns
// (enemies killed, ours lost) within `horizon` turns. Burst (ready abilities) lands up front.
pub const ENEMY_DAMAGE_WEIGHT: f32 = 1.3;

#[derive(Clone, Copy)]
struct F {
    hp: f32,
    dmg: f32,
}

fn run(mut a: Vec<F>, mut b: Vec<F>, a_focus_dmg: bool, horizon: f32) -> (u32, u32) {
    let (mut kills, mut deaths) = (0, 0);
    let mut t = 0.0;
    while !a.is_empty() && !b.is_empty() && t < horizon {
        let da: f32 = a.iter().map(|f| f.dmg).sum();
        let db: f32 = b.iter().map(|f| f.dmg).sum();
        let ib = if a_focus_dmg {
            (0..b.len()).max_by(|x, y| b[*x].dmg.partial_cmp(&b[*y].dmg).unwrap()).unwrap()
        } else {
            (0..b.len()).min_by(|x, y| b[*x].hp.partial_cmp(&b[*y].hp).unwrap()).unwrap()
        };
        let ia = (0..a.len()).min_by(|x, y| a[*x].hp.partial_cmp(&a[*y].hp).unwrap()).unwrap();
        let tb = b[ib].hp / da.max(0.1);
        let ta = a[ia].hp / db.max(0.1);
        // Ties go to us: we act on the turn we decide.
        let dt = tb.min(ta).min(horizon - t);
        t += dt;
        b[ib].hp -= da * dt;
        a[ia].hp -= db * dt;
        if b[ib].hp <= 0.5 {
            b.swap_remove(ib);
            kills += 1;
        }
        if !a.is_empty() && a[ia.min(a.len() - 1)].hp <= 0.5 && tb > ta {
            let i = ia.min(a.len() - 1);
            a.swap_remove(i);
            deaths += 1;
        }
    }
    (kills, deaths)
}

pub fn simulate(ours: &[&Unit], theirs: &[Unit], horizon: f32) -> (u32, u32) {
    let conv = |u: &Unit| F { hp: u.hp + 0.0, dmg: u.dps + u.burst / horizon.max(1.0) };
    let a: Vec<F> = ours.iter().map(|u| conv(u)).collect();
    // Enemy items and passives we can't read (Durability, enhanced attacks) make them hit harder
    // than Strength + Focus suggests: weigh their damage up.
    let b: Vec<F> = theirs.iter().map(|u| { let f = conv(u); F { hp: f.hp, dmg: f.dmg * ENEMY_DAMAGE_WEIGHT } }).collect();
    let low = run(a.clone(), b.clone(), false, horizon);
    let high = run(a, b, true, horizon);
    let score = |r: (u32, u32)| r.0 as i32 - r.1 as i32;
    if score(high) > score(low) {
        high
    } else {
        low
    }
}
