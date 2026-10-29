// The per-champion turn loop. Each turn: sense (visible units, camp states), then one decision in
// priority order, each step either acting and returning or falling through:
//   dead/channelling -> shop/heal at spawn -> altar lock -> fight or flee -> low-health retreat ->
//   bank gems -> shopping trip -> camp target (rally, clear, capture) -> explore
// The chosen target and action are kept between turns; a new target must beat the current one by
// the stickiness factor, so champions don't flip between two similar goals.
use crate::combat::{self, abils, threat, Abil, Unit};
use crate::draft::read_plan;
use crate::mapinfo::{MapInfo, FAR};
use crate::memory::{GState, Memory};
use crate::nav::Ctx;
use crate::profile::{params, Kind, Params};
use crate::shop;
use crate::team;
use crate::util::{cheb, round, fuel, DIRS};
use cambc::{
    vision, AbilityKind, Champion, ChampionType, ConsumableType, ItemFamily, Result, Team, Vec2,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Target {
    None,
    Camp(usize),
    Rally(usize),
    Enemy(Vec2),
    Altar,
    Home,
    Vantage(usize),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    Dead,
    Recalling,
    Banking,
    Collecting,
    Going,
    Attacking,
    Waiting,
    Retreating,
    Idle,
}

// A new camp target must score this much better than the current one.
const STICKINESS: f32 = 1.3;
// Fuel kept back for the end of turn (claims, recall); spare fuel above this builds fields.
const RESERVE_FUEL: u64 = 12_000;

const TRACE: bool = false;
// Enemies this close are fought even on a losing sim, if we'd last ENGAGE_SURVIVE turns (v9: 3, 4).
const ENGAGE_TILES: i32 = 1;
const ENGAGE_SURVIVE: f32 = 4.0;
const KITE_TILES: i32 = 2;
const GOLD_DETOUR: f32 = 3.0;
const STUCK_ROUNDS: i32 = 10;
// Extra steps a group costs per champion beyond the ones it needs (0: everyone piles on).
const SPREAD_COST: f32 = 6.0;
// A current target counts this many steps nearer (keeps champions from flipping as claims shift).
const STICK_STEPS: f32 = 8.0;
const BOSS_MARGIN: f32 = 1.5;
const FUNNEL: bool = true;
const GHOST_ROUNDS: i32 = 3;
const FLEE_LOCKOUT: i32 = 20;
// Allies within this many tiles answer a fight beacon.
const JOIN_TILES: i32 = 8;
// Off: answering beacons pulled champions off farming one at a time; they arrived alone and fled
// (4 maps vs v9, 2026-10-29: gold roughly halved, deaths 32 -> 34).
const ANSWER_BEACON: bool = true;
const FLEE_ALLY_PULL: f32 = 0.5;
const ENEMY_NEAR_COST: f32 = 6.0;
const CARRY_ORDER: [ChampionType; 13] = [
    ChampionType::VesperVale,
    ChampionType::Tavi,
    ChampionType::Vendetta,
    ChampionType::Stellar,
    ChampionType::Prism,
    ChampionType::Marcella,
    ChampionType::Varg,
    ChampionType::Ryth,
    ChampionType::HeartOfRuin,
    ChampionType::Ren,
    ChampionType::Cassi,
    ChampionType::Issac,
    ChampionType::Willow,
];

pub struct SeenCamp {
    pub camp: cambc::Camp,
    pub group: Option<usize>,
    pub pos: Vec2,
    pub kind: cambc::CampType,
    pub alive: bool,
    pub awake: bool,
    pub hp: f32,
}

pub struct Bot {
    info: MapInfo,
    mem: Memory,
    p: &'static Params,
    me: Champion,
    ctype: ChampionType,
    slot: usize,
    abil: [Option<Abil>; 2],
    dash: Option<Abil>,
    vantage: Vec<Vec2>,
    vantage_i: usize,
    target: Target,
    action: Action,
    target_since: i32,
    party: Vec<usize>,
    roster: Vec<Champion>,
    roster_kinds: Vec<ChampionType>,
    party_units: Vec<Champion>,
    lost_until: Vec<i32>,
    occ: Vec<Vec2>,
    // This turn's view.
    camps: Vec<SeenCamp>,
    my_camp_dps: f32,
    jungle: bool,
    pos: Vec2,
    hp: f32,
    max_hp: f32,
    allies: Vec<Unit>,
    enemies: Vec<Unit>,
    want_recall: bool,
    last_hp: f32,
    field_cursor: usize,
    hist: [u32; 9],
    // No-progress watchdog: (goal, best distance seen, round it was last improved).
    progress: (i32, i32, i32),
    bank_blocked_until: i32,
    healing: bool,
    recent: Vec<(Unit, i32)>,
}

impl Bot {
    pub fn new(me: Champion) -> Result<Bot> {
        let info = MapInfo::new()?;
        let (kind, vantage) = read_plan(&info);
        let ctype = me.champion_name()?;
        let slot = cambc::champions(Some(Team::Ally))?.iter().position(|c| *c == me).unwrap_or(0);
        let abil = abils(&me);
        let dash = abil.iter().flatten().find(|a| a.kind == AbilityKind::Dash).copied();
        let mem = Memory::new(&info);
        let roster = cambc::champions(Some(Team::Ally))?;
        let feats = crate::classify::features(&info);
        let size = crate::profile::party_size(kind, roster.len(), feats.harmless_share);
        let parties = (roster.len() / size).max(1);
        let party: Vec<usize> = (0..roster.len()).filter(|s| s % parties == slot % parties).collect();
        let party_units: Vec<Champion> = party.iter().filter(|s| **s != slot).map(|s| roster[*s].clone()).collect();
        let n = info.groups.len();
        println!("v10: {:?} {:?} slot {} groups {} party {:?}", kind, ctype, slot, n, party);

        Ok(Bot {
            info,
            mem,
            p: params(kind),
            me,
            ctype,
            slot,
            abil,
            dash,
            vantage,
            vantage_i: 0,
            target: Target::None,
            action: Action::Idle,
            target_since: 0,
            party,
            roster_kinds: roster.iter().map(|c| c.champion_name().unwrap_or(ChampionType::Varg)).collect(),
            roster: roster.clone(),
            party_units,
            lost_until: vec![0; n],
            occ: Vec::new(),
            camps: Vec::new(),
            my_camp_dps: 0.0,
            jungle: false,
            pos: Vec2::new(0, 0),
            hp: 0.0,
            max_hp: 1.0,
            allies: Vec::new(),
            enemies: Vec::new(),
            want_recall: false,
            last_hp: 0.0,
            field_cursor: 0,
            hist: [0; 9],
            progress: (-1, i32::MAX, 0),
            bank_blocked_until: 0,
            healing: false,
            recent: Vec::new(),
        })
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx { info: &self.info, me: &self.me, pos: self.pos, occ: &self.occ, dash: self.dash }
    }

    // Watchdog for walking goals: goal id (group, or -2 for the altar) and our distance to it.
    // Returns true when it hasn't improved for STUCK_ROUNDS (a gap only a longer dash crosses, a
    // tile held by a camp): the caller then gives that goal up for a while.
    fn stuck(&mut self, goal: i32, dist: i32) -> bool {
        let now = round();
        if self.progress.0 != goal || dist < self.progress.1 {
            self.progress = (goal, dist, now);
            return false;
        }
        now - self.progress.2 > STUCK_ROUNDS
    }

    fn set_target(&mut self, t: Target) {
        if t != self.target {
            self.target = t;
            self.target_since = round();
        }
    }

    pub fn turn(&mut self) -> Result<()> {
        self.want_recall = false;
        let me = self.me.clone();
        if !me.is_alive()? {
            self.hist[0] += 1;
            if TRACE && self.action != Action::Dead {
                println!("DEATH r{} {:?} at {},{} hp {:.0} {:?} {:?} enemies {} campthreat {:.0}", round(), self.ctype, self.pos.x, self.pos.y, self.last_hp, self.action, self.target, self.enemies.len(), self.camp_threat(self.pos));
            }
            self.action = Action::Dead;
            self.set_target(Target::None);
            let _ = shop::shop(&me, self.p, self.ctype, true);
            let _ = team::set_dead(self.slot);
            return Ok(());
        }
        if me.is_recalling()? {
            self.hist[1] += 1;
            self.action = Action::Recalling;
            return Ok(());
        }
        if me.is_submitting_gems()? || me.is_teleporting()? {
            self.hist[2] += 1;
            self.action = Action::Banking;
            return Ok(());
        }
        let f0 = fuel();
        self.sense()?;
        let f1 = fuel();
        self.decide()?;
        if TRACE && (round() == 300) {
            let f2 = fuel();
            println!("F r{} {:?} sense {} decide {} pos {},{}", round(), self.ctype, f0 - f1, f1 - f2, self.pos.x, self.pos.y);
            if let Target::Rally(g) | Target::Camp(g) = self.target {
                if let Some(f) = self.info.group_field(g) {
                    let p = self.pos;
                    let mut s = String::new();
                    for d in DIRS { let q = p + d; s += &format!(" {},{}:{}w{}o{}", q.x, q.y, f.at(&self.info, q), self.info.walkable(q) as u8, self.occ.contains(&q) as u8); }
                    println!("  here {} canmove {:?} {}", f.at(&self.info, p), self.me.can_move(), s);
                } else { println!("  no field"); }
            }
        }
        self.finish()
    }

    fn sense(&mut self) -> Result<()> {
        self.pos = self.me.position()?;
        self.hp = self.me.health()?;
        self.max_hp = self.me.max_health()?.max(1.0);
        let c0 = fuel();
        let mut occ = vision::champion_tiles(None)?;
        occ.union_assign(&vision::camp_tiles()?)?;
        self.occ = occ.positions()?;
        let pos = self.pos;
        self.occ.retain(|p| *p != pos);
        let c1 = fuel();
        self.camps.clear();
        for c in vision::camps()? {
            let st = c.state().unwrap_or(cambc::CampState::Dead);
            let alive = st.is_alive();
            let p = c.spawn_position();
            self.camps.push(SeenCamp {
                group: self.mem.group_at(&self.info, p),
                pos: p,
                kind: c.kind(),
                alive,
                awake: st.is_awake(),
                hp: if alive { c.health().unwrap_or(0.0) } else { 0.0 },
                camp: c,
            });
        }
        let c2 = fuel();
        self.jungle = self.has_emblem(ItemFamily::Jungle);
        self.my_camp_dps = self.camp_dps();
        let c3 = fuel();
        self.allies.clear();
        self.enemies.clear();
        let list = vision::champions(None)?;
        let c35 = fuel();
        for c in list {
            if c == self.me {
                continue;
            }
            if let Some(i) = self.roster.iter().position(|r| *r == c) {
                if let Some(u) = Unit::sense_light(&c, self.roster_kinds[i]) {
                    self.allies.push(u);
                }
                continue;
            }
            let Some(u) = Unit::sense(&c) else { continue };
            {
                self.mem.note_enemy(&self.info, &c, u.pos);
                self.enemies.push(u);
            }
        }
        // Enemies that just slipped out of sight (bush, vision edge) are still there: keep them
        // for GHOST_ROUNDS at their last position, or a fleeing champion walks straight back
        // into its chaser the turn it loses sight of it.
        let now = round();
        for e in &self.enemies {
            match self.recent.iter_mut().find(|r| r.0.champ == e.champ) {
                Some(r) => *r = (e.clone(), now),
                None => self.recent.push((e.clone(), now)),
            }
        }
        self.recent.retain(|r| now - r.1 <= GHOST_ROUNDS);
        for (u, r) in &self.recent {
            if *r < now && !self.enemies.iter().any(|e| e.champ == u.champ) {
                self.enemies.push(u.clone());
            }
        }
        let c4 = fuel();
        let seen: Vec<(cambc::Camp, usize, bool, f32)> = self
            .camps
            .iter()
            .filter_map(|c| c.group.map(|g| (c.camp.clone(), g, c.alive, c.hp)))
            .collect();
        let vis = crate::mapinfo::bits(&vision::visible_tiles()?, 0)?;
        let c5 = fuel();
        self.mem.update_groups(&self.info, &seen, &vis)?;
        if TRACE && round() == 60 { println!("S occ {} camps {} dps {} list {} champs {} vis {} groups {} ncamps {} flood {}", c0-c1, c1-c2, c2-c3, c3-c35, c35-c4, c4-c5, c5-fuel(), self.camps.len(), self.info.flood_cost); }
        Ok(())
    }

    fn me_unit(&self) -> Option<Unit> {
        Unit::sense(&self.me)
    }

    fn on_spawn(&self) -> bool {
        self.info.home.at(&self.info, self.pos) == 0
    }

    fn decide(&mut self) -> Result<()> {
        let me = self.me.clone();
        let at_spawn = self.on_spawn();
        if at_spawn || self.ctype == ChampionType::Tavi {
            let _ = shop::shop(&me, self.p, self.ctype, at_spawn);
        }
        self.altar_lock()?;
        if !self.enemies.is_empty() && self.combat()? {
            return Ok(());
        }
        // Heal: spawn recovery happens at the end of a turn spent on spawn.
        if at_spawn && self.hp < self.max_hp * 0.9 {
            self.action = Action::Idle;
            return Ok(());
        }
        if self.hp < self.max_hp * self.p.retreat_hp {
            return self.go_home(Action::Retreating);
        }
        // Broke off a camp to heal: keep going until healed (don't pick another camp on the way).
        if self.healing {
            if self.hp >= self.max_hp * 0.9 {
                self.healing = false;
            } else {
                return self.go_home(Action::Retreating);
            }
        }
        // Nothing pressing and half-dead: heal now rather than mid-fight later.
        if self.hp < self.max_hp * self.p.heal_hp && !matches!(self.action, Action::Attacking | Action::Collecting) {
            return self.go_home(Action::Retreating);
        }
        // Spend before anything else optional: unspent gold is strength we don't have (our carry
        // sat on 378 gold at round 500 while answering fights and banking).
        if self.ctype != ChampionType::Tavi && shop::worth_trip(&me, self.p)? && self.action != Action::Collecting {
            return self.go_home(Action::Going);
        }
        if self.answer_beacon()? {
            return Ok(());
        }
        if self.should_bank()? {
            return self.bank();
        }
        if let Some(g) = self.choose_camp()? {
            return self.work_camp(g);
        }
        self.explore()
    }

    // ---- fighting ----------------------------------------------------------------------------

    // Returns true when the turn was spent fighting, holding off or fleeing.
    //
    // The fight is simulated death by death (combat::simulate) with everyone in it: us, allies
    // within 6, enemies within 8. We fight when we'd kill at least `trade_margin` more than we'd
    // lose. Otherwise we stay just outside their reach (attack range + a step) instead of running
    // home: running at equal speed hands a chaser free hits, and holding keeps us on our camp.
    // Close enemies (ENGAGE_TILES) are fought even on a losing sim if we'd last ENGAGE_SURVIVE turns.
    fn combat(&mut self) -> Result<bool> {
        let Some(me_u) = self.me_unit() else { return Ok(false) };
        let pos = self.pos;
        let theirs: Vec<Unit> = self.enemies.iter().filter(|e| cheb(e.pos, pos) <= 8).cloned().collect();
        if theirs.is_empty() {
            return Ok(false);
        }
        let ours: Vec<&Unit> = self
            .allies
            .iter()
            .filter(|a| cheb(a.pos, pos) <= 6)
            .chain(core::iter::once(&me_u))
            .collect();
        let (kills, deaths) = combat::simulate(&ours, &theirs, 10.0);
        if let Some(e) = theirs.iter().min_by_key(|e| cheb(e.pos, pos)) {
            let _ = team::set_beacon(e.pos);
        }
        let me_dies = {
            // Do we personally survive ENGAGE_SURVIVE turns of what can reach us?
            let incoming: f32 = theirs.iter().filter(|e| cheb(e.pos, pos) <= e.range + 2).map(|e| e.dps + e.burst / 4.0).sum();
            incoming * ENGAGE_SURVIVE > self.hp
        };
        let low = self.hp < self.max_hp * self.p.retreat_hp;
        let nearest = theirs.iter().map(|e| cheb(e.pos, pos)).min().unwrap_or(99);
        let now = round();
        let win = kills >= 1 && kills as i32 - deaths as i32 >= self.p.trade_margin;
        if TRACE && round() < 2000 {
            let es: Vec<String> = theirs.iter().map(|e| format!("{:?}@{},{} hp{:.0} dps{:.0} b{:.0}", e.kind, e.pos.x, e.pos.y, e.hp, e.dps, e.burst)).collect();
            println!("C r{} {:?}@{},{} hp{:.0} allies{} sim {}-{} | {}", now, self.ctype, pos.x, pos.y, self.hp, ours.len() - 1, kills, deaths, es.join(" "));
        }
        let tag = |t: &str| if TRACE { println!("D r{} {:?} {}", round(), self.ctype, t); };
        if !low && win {
            let chase = self.p.chase_steps + me_u.range;
            let fresh = !matches!(self.target, Target::Enemy(_)) || now - self.target_since <= self.p.chase_steps + 2;
            let finishing = theirs.iter().any(|e| e.hp < e.max_hp * 0.3 && cheb(e.pos, pos) <= me_u.range + 2);
            // An enemy holding just outside our range (v9 does exactly that when it thinks it's
            // losing) is a stall, not a fight: ranged champions engage only what they can shoot
            // or what can hit them; melee ones only what's within a step, or a short fresh chase.
            let threatened = theirs.iter().any(|e| cheb(e.pos, pos) <= e.reach());
            let engage = if me_u.range > 1 {
                nearest <= me_u.range || threatened || (finishing && nearest <= me_u.range + 1)
            } else {
                nearest <= 2 || threatened || (nearest <= chase && (fresh || finishing))
            };
            if engage {
                tag("fight-win");
                self.fight(&me_u, &theirs)?;
                return Ok(true);
            }
            return Ok(false);
        }
        if !low && nearest <= ENGAGE_TILES && !me_dies {
            tag("fight-engage");
            self.fight(&me_u, &theirs)?;
            return Ok(true);
        }
        let reach = theirs.iter().any(|e| cheb(e.pos, pos) <= e.reach());
        // A melee chaser on us moves and hits every turn: running only feeds it free hits. Trade
        // back (we may still take it down, or buy allies time) unless a dash breaks contact.
        // Ranged chasers must stop to shoot, so running from them works.
        let melee_on_us = theirs.iter().any(|e| e.range <= 1 && !e.stunned && cheb(e.pos, pos) <= 2);
        let can_break = self.dash.map_or(false, |d| combat::is_ready(&self.me, &d));
        if melee_on_us && !can_break && (low || me_dies) {
            tag("trade-back");
            self.fight(&me_u, &theirs)?;
            return Ok(true);
        }
        // Losing fight in sight: leave while we're still out of reach. At equal speed a chaser
        // that starts outside its reach never closes the gap, and ranged chasers lose ground every
        // time they stop to shoot. Holding just outside reach got us caught on the next corner.
        let close = theirs.iter().any(|e| cheb(e.pos, pos) <= e.reach() + 3);
        if low || reach {
            tag(if low { "flee-low" } else { "flee-reach" });
            self.flee(&theirs)?;
            return Ok(true);
        }
        if close {
            // Not in reach yet: just get out of it and go farm elsewhere. Running all the way
            // home (as flee does) threw away 30 turns of walking each time.
            tag("back-off");
            self.back_off(&theirs)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn back_off(&mut self, theirs: &[Unit]) -> Result<()> {
        self.action = Action::Retreating;
        let allies: Vec<Vec2> = self.allies.iter().map(|a| a.pos).collect();
        if let Target::Camp(g) = self.target {
            self.lost_until[g] = round() + FLEE_LOCKOUT;
        }
        let info = &self.info;
        let cost = |t: Vec2| {
            let breach: i32 = theirs.iter().map(|e| (e.reach() + 4 - cheb(e.pos, t)).max(0)).sum();
            let h = info.home.at(info, t);
            let near_ally = allies.iter().map(|a| cheb(*a, t)).min().unwrap_or(99);
            let ally_pull = if near_ally <= 15 { near_ally as f32 * FLEE_ALLY_PULL } else { 0.0 };
            breach as f32 * 100.0 + if h == FAR { 50.0 } else { h as f32 * 0.2 } + ally_pull
        };
        self.pos = self.ctx().best_step(cost)?;
        Ok(())
    }

    // Stand just outside every visible enemy's reach (attack range + one step), as close to where
    // we are as possible; shoot whatever is in our own range if we didn't need to move.
    fn keep_distance(&mut self, me_u: &Unit, theirs: &[Unit]) -> Result<()> {
        self.action = Action::Waiting;
        let outside = |t: Vec2| theirs.iter().all(|e| e.stunned || cheb(e.pos, t) > e.reach());
        let here_ok = outside(self.pos);
        if !here_ok {
            let home = &self.info.home;
            let info = &self.info;
            let cost = |t: Vec2| {
                let breach: i32 = theirs.iter().map(|e| (e.reach() + 1 - cheb(e.pos, t)).max(0)).sum();
                let h = home.at(info, t);
                breach as f32 * 100.0 + if h == FAR { 50.0 } else { h as f32 * 0.5 }
            };
            let np = self.ctx().best_step(cost)?;
            if np != self.pos {
                self.pos = np;
                return Ok(());
            }
        }
        if let Some(t) = theirs.iter().filter(|e| cheb(e.pos, self.pos) <= me_u.range).min_by(|a, b| a.hp.partial_cmp(&b.hp).unwrap()) {
            let _ = combat::attack_champ(&self.me, &t.champ);
        }
        Ok(())
    }

    fn fight(&mut self, me_u: &Unit, theirs: &[Unit]) -> Result<()> {
        let me = self.me.clone();
        self.action = Action::Attacking;
        let my_hit = me_u.dps.max(me.strength().unwrap_or(40.0));
        // Something already in range beats a better target we'd have to walk to.
        let in_reach = combat::best_target(theirs, self.pos, me_u.range, my_hit).cloned();
        let Some(tgt) = in_reach.or_else(|| combat::best_target(theirs, self.pos, 12, my_hit).cloned()) else { return Ok(()) };
        self.set_target(Target::Enemy(tgt.pos));
        let range = me_u.range;
        let melee = range <= 1;
        let mut moved = false;
        let in_range = cheb(self.pos, tgt.pos) <= range;
        // Abilities that exclude walking (stuns, nukes, area hits; range 4, beyond most basic
        // attacks) go first: casting one is worth more than the step, so we stand and cast.
        let mut cast = self.cast_combat(theirs, &tgt, false, true)?;
        // Ranged: attack if we can, else close in. Melee: close in, then attack.
        if !in_range && !moved && !cast {
            // Gap-closing dash (Heart of Ruin's lands with damage and a stun).
            if let Some(d) = self.dash {
                if combat::is_ready(&me, &d) && cheb(self.pos, tgt.pos) <= range + 2 {
                    if let Some(land) = self.dash_land_near(tgt.pos, range) {
                        if combat::cast_at(&me, &d, land).unwrap_or(false) {
                            self.pos = land;
                            moved = true;
                        }
                    }
                }
            }
            // Ranged champions can't step and shoot in one turn: stepping to a target that steps
            // back is a chase that never fires. Close in only on a stunned target, or when we're
            // two or more out of range (then one step still leaves it to come to us).
            let d = cheb(self.pos, tgt.pos);
            let step = melee || tgt.stunned || d > range + 1;
            if d > range && step {
                let np = self.ctx().toward(tgt.pos)?;
                moved |= np != self.pos;
                self.pos = np;
            }
        }
        if !cast {
            cast = self.cast_combat(theirs, &tgt, moved, false)?;
        }
        let _ = cast;
        let mut hit = false;
        if cheb(self.pos, tgt.pos) <= range && (melee || !moved) {
            if combat::attack_champ(&me, &tgt.champ) {
                hit = true;
                self.attack_reset(|| combat::attack_champ(&me, &tgt.champ))?;
            }
        }
        if TRACE {
            println!("A r{} {:?}@{},{} hp{:.0} -> {:?}@{},{} hp{:.0} d{} moved {} hit {} canatk {:?}", round(), self.ctype, self.pos.x, self.pos.y, self.hp, tgt.kind, tgt.pos.x, tgt.pos.y, tgt.hp, cheb(self.pos, tgt.pos), moved, hit, me.can_attack());
        }
        Ok(())
    }

    // A free tile within dash range from which `t` is within `range`.
    fn dash_land_near(&self, t: Vec2, range: i32) -> Option<Vec2> {
        let mut best: Option<(i32, Vec2)> = None;
        for dx in -2..=2 {
            for dy in -2..=2 {
                let q = Vec2::new(self.pos.x + dx, self.pos.y + dy);
                if q == self.pos || !self.ctx().free(q) || cheb(q, t) > range {
                    continue;
                }
                // Heart of Ruin's landing damage would wake camps next to it.
                let camp_near = self.camps.iter().any(|c| c.alive && cheb(c.pos, q) <= 1);
                if camp_near {
                    continue;
                }
                let d = cheb(q, t);
                if best.map_or(true, |b| d < b.0) {
                    best = Some((d, q));
                }
            }
        }
        best.map(|b| b.1)
    }

    // One cast this turn. `only_excl`: just the abilities that exclude walking (tried before we
    // move, so standing still to cast them is the choice). Returns whether something was cast.
    fn cast_combat(&mut self, theirs: &[Unit], tgt: &Unit, moved: bool, only_excl: bool) -> Result<bool> {
        let me = self.me.clone();
        let any_stunned = theirs.iter().any(|e| e.stunned);
        for a in self.abil.iter().flatten().copied() {
            if !combat::is_ready(&me, &a) || (a.excl && moved) || (only_excl && !a.excl) {
                continue;
            }
            let done = match a.kind {
                AbilityKind::Shield => threat(self.pos, theirs) > 0.0 && combat::cast_self(&me, &a)?,
                AbilityKind::Heal => {
                    let hurt = self
                        .allies
                        .iter()
                        .filter(|u| cheb(u.pos, self.pos) <= a.range && u.max_hp - u.hp > 150.0)
                        .min_by(|x, y| (x.hp / x.max_hp).partial_cmp(&(y.hp / y.max_hp)).unwrap_or(core::cmp::Ordering::Equal));
                    match hurt {
                        Some(u) => combat::cast_on_champ(&me, &a, &u.champ)?,
                        None => false,
                    }
                }
                AbilityKind::Cc => {
                    // Stagger stuns: hold ours while someone is already stunned.
                    if any_stunned {
                        false
                    } else {
                        let pick = theirs
                            .iter()
                            .filter(|e| cheb(e.pos, self.pos) <= a.range)
                            .max_by(|x, y| x.dps.partial_cmp(&y.dps).unwrap_or(core::cmp::Ordering::Equal));
                        match pick {
                            Some(e) => combat::cast_on_champ(&me, &a, &e.champ)?,
                            None => false,
                        }
                    }
                }
                AbilityKind::Damage => {
                    cheb(tgt.pos, self.pos) <= a.range && combat::cast_on_champ(&me, &a, &tgt.champ)?
                }
                AbilityKind::AoeDamage => match combat::aoe_spot(self.pos, a.range, a.radius, theirs) {
                    Some((spot, n)) if n >= 1 && (!a.stuns || !any_stunned || n >= 2) => {
                        combat::cast_at(&me, &a, spot)?
                    }
                    _ => false,
                },
                _ => false,
            };
            if done {
                return Ok(true); // one cast per turn
            }
        }
        Ok(false)
    }

    // After a successful basic attack: reset it and swing again.
    fn attack_reset(&self, again: impl Fn() -> bool) -> Result<()> {
        let me = &self.me;
        if let Some(a) = self.abil.iter().flatten().find(|a| a.kind == AbilityKind::AttackReset) {
            if combat::is_ready(me, a) && combat::cast_self(me, a).unwrap_or(false) {
                again();
            }
        }
        Ok(())
    }

    fn flee(&mut self, theirs: &[Unit]) -> Result<()> {
        // Don't walk back into them for the camp we were on.
        if let Target::Camp(g) = self.target {
            self.lost_until[g] = round() + FLEE_LOCKOUT;
        }
        self.action = Action::Retreating;
        self.set_target(Target::Home);
        let info = &self.info;
        let allies: Vec<Vec2> = self.allies.iter().map(|a| a.pos).collect();
        let camps: Vec<(Vec2, f32, i32)> = self
            .camps
            .iter()
            .filter(|c| c.alive)
            .map(|c| {
                let (_, s, r) = crate::mapinfo::camp_stats(c.kind);
                (c.pos, s, r)
            })
            .filter(|c| c.1 > 0.0)
            .collect();
        let cost = |t: Vec2| {
            let h = info.home.at(info, t);
            let camp: f32 = camps.iter().filter(|c| cheb(c.0, t) <= c.2).map(|c| c.1).sum();
            // Run to friends when there are any nearby: the next fight sim then counts them.
            let near_ally = allies.iter().map(|a| cheb(*a, t)).min().unwrap_or(99);
            let ally_pull = if near_ally <= 15 { near_ally as f32 * FLEE_ALLY_PULL } else { 0.0 };
            threat(t, theirs) * 4.0 + camp * 6.0 + if h == FAR { 500.0 } else { h as f32 } + ally_pull
        };
        let ctx = self.ctx();
        let mut np = self.pos;
        let melee_on_us = theirs.iter().any(|e| e.range <= 1 && !e.stunned && cheb(e.pos, self.pos) <= 2);
        if melee_on_us || threat(self.pos, theirs) > self.hp * 0.3 {
            if let Some(q) = ctx.dash_escape(&cost)? {
                np = q;
            }
        }
        let ctx = Ctx { pos: np, ..self.ctx() };
        np = ctx.best_step(&cost)?;
        self.pos = np;
        // Still attack if something is in range and we're melee (moving doesn't block it).
        if self.me.attack_range().unwrap_or(1) <= 1 {
            if let Some(t) = theirs.iter().find(|e| cheb(e.pos, np) <= 1) {
                let _ = combat::attack_champ(&self.me, &t.champ);
            }
        }
        // Any damage cancels a recall: only start one with nobody able to reach us.
        let nearest = theirs.iter().map(|e| cheb(e.pos, np) - e.range).min().unwrap_or(99);
        if nearest > 4 && self.known_camp_threat(np) <= 0.0 && self.info.home.at(&self.info, np) > 6 {
            self.want_recall = true;
        }
        Ok(())
    }

    fn altar_lock(&mut self) -> Result<()> {
        let lock = &self.me.consumables()[ConsumableType::AltarLock];
        if lock.count().unwrap_or(0) == 0 {
            return Ok(());
        }
        for e in self.enemies.iter().filter(|e| e.submitting) {
            for a in &self.info.altars {
                if cheb(*a, e.pos) <= 1 && cheb(*a, self.pos) <= 2 {
                    if lock.use_at(*a).unwrap_or(false) {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    // Go help an ally's fight (team::beacon) when it's close and we're fit to fight.
    fn answer_beacon(&mut self) -> Result<bool> {
        let mem = team::read();
        if !ANSWER_BEACON {
            return Ok(false);
        }
        let Some(b) = team::beacon(&mem) else { return Ok(false) };
        let d = cheb(b, self.pos);
        if d <= 3 {
            // Here, and nobody to fight (combat() would have run): the fight is over.
            let _ = team::clear_beacon();
            return Ok(false);
        }
        if d > JOIN_TILES || self.hp < self.max_hp * 0.7 || matches!(self.action, Action::Collecting) {
            return Ok(false);
        }
        self.action = Action::Going;
        self.set_target(Target::Enemy(b));
        self.pos = self.ctx().toward(b)?;
        Ok(true)
    }

    // ---- going home, banking -------------------------------------------------------------------

    // Damage camps will deal to someone on `t` (awake monsters only, which is what hits).
    fn camp_threat(&self, t: Vec2) -> f32 {
        let mut d = 0.0;
        for c in self.camps.iter().filter(|c| c.alive && c.awake) {
            let (_, s, r) = crate::mapinfo::camp_stats(c.kind);
            if s > 0.0 && cheb(c.pos, t) <= r {
                d += s;
            }
        }
        if self.jungle {
            d *= 0.7;
        }
        d
    }

    // Damage from monsters of groups we believe alive, including ones we can't see (a camp in a
    // bush is invisible but still hits). Map knowledge: every camp's tile, kind and reach.
    fn known_camp_threat(&self, t: Vec2) -> f32 {
        let mut d = 0.0;
        for (g, grp) in self.info.groups.iter().enumerate() {
            if grp.harmless || self.mem.groups[g].state != GState::Alive {
                continue;
            }
            if grp.tiles.iter().all(|p| cheb(*p, t) > grp.range) {
                continue;
            }
            for (i, p) in grp.tiles.iter().enumerate() {
                let kind = self.info.handles[g].camps()[i].kind();
                let (_, s, r) = crate::mapinfo::camp_stats(kind);
                if s <= 0.0 || cheb(*p, t) > r {
                    continue;
                }
                // Camps only wake when damaged: a visible camp hits only if alive and awake. One we
                // can't see (in a bush) counts only if something just hurt us.
                match self.camps.iter().find(|c| c.pos == *p) {
                    Some(c) => {
                        if c.alive && c.awake {
                            d += s;
                        }
                    }
                    None => {
                        if self.hp < self.last_hp {
                            d += s;
                        }
                    }
                }
            }
        }
        if self.jungle {
            d *= 0.7;
        }
        d
    }

    fn go_home(&mut self, action: Action) -> Result<()> {
        self.action = action;
        self.set_target(Target::Home);
        let home = self.info.home.at(&self.info, self.pos);
        if home == 0 {
            return Ok(());
        }
        let enemy_near = self.enemies.iter().any(|e| cheb(e.pos, self.pos) <= e.range + 5);
        if self.stuck(-3, home as i32) && !enemy_near {
            self.want_recall = true;
            return Ok(());
        }
        if home <= 6 || self.known_camp_threat(self.pos) > 0.0 || enemy_near {
            let np = self.safe_step_home()?;
            self.pos = np;
            if self.info.home.at(&self.info, np) > 6 && self.known_camp_threat(np) <= 0.0 && !enemy_near {
                self.want_recall = true;
            }
            return Ok(());
        }
        // Recall as the last action of the turn: the starting turn counts as a tick.
        self.want_recall = true;
        Ok(())
    }

    // A step toward home that prices in camp and enemy reach (the field alone walks straight
    // through an awake elk's range).
    fn safe_step_home(&self) -> Result<Vec2> {
        let info = &self.info;
        let enemies = &self.enemies;
        let cost = |t: Vec2| {
            let h = info.home.at(info, t);
            let camp = self.known_camp_threat(t);
            // Out of camp range first (every turn inside costs a hit), then home.
            threat(t, enemies) * 2.0 + camp * 1000.0 + if h == FAR { 999.0 } else { h as f32 * 10.0 }
        };
        self.ctx().best_step(cost)
    }

    fn should_bank(&self) -> Result<bool> {
        let gems = self.me.gem_count()?;
        if gems == 0 || round() < self.bank_blocked_until {
            return Ok(false);
        }
        let ours = cambc::global_submitted_gems(Team::Ally).unwrap_or(0);
        if ours + gems >= cambc::constants().winning_gems {
            return Ok(true);
        }
        let mut limit = self.p.bank_gems;
        if self.ctype == ChampionType::Prism {
            limit += 10;
        }
        let altar = self.info.altar_field.at(&self.info, self.pos);
        Ok(gems >= limit
            || (altar <= 10 && gems >= 10)
            || (self.hp < self.max_hp * 0.5 && gems >= 15 && altar <= 12)
            || matches!(self.action, Action::Banking))
    }

    fn bank(&mut self) -> Result<()> {
        self.set_target(Target::Altar);
        self.action = Action::Banking;
        let d = self.info.altar_field.at(&self.info, self.pos);
        if self.stuck(-2, d as i32) {
            self.bank_blocked_until = round() + 40;
            self.progress = (-1, i32::MAX, round());
            self.action = Action::Idle;
            return Ok(());
        }
        if d == FAR {
            self.action = Action::Idle;
            return Ok(());
        }
        if d > 0 {
            self.pos = self.ctx().go(&self.info.altar_field, None, true)?;
            if TRACE && round() % 20 == 0 && self.ctype == ChampionType::VesperVale {
                let mut st = String::new();
                for dd in DIRS { let q = self.pos + dd; st += &format!(" {},{}:{}f{}", q.x, q.y, self.info.altar_field.at(&self.info, q), self.ctx().free(q) as u8); }
                println!("BANKWALK r{} {:?} d{} {}", round(), self.ctype, d, st);
            }
        }
        if self.info.altar_field.at(&self.info, self.pos) == 0 {
            // Prefer an altar without an enemy lock.
            let mut altars: Vec<Vec2> = self.info.altars.iter().copied().filter(|a| cheb(*a, self.pos) <= 1).collect();
            altars.sort_by_key(|a| vision::has_altar_lock(*a).unwrap_or(false) as i32);
            if let Some(a) = altars.first() {
                let r = self.me.submit_gems(*a);
                if TRACE && round() % 20 == 0 && self.ctype == ChampionType::VesperVale {
                    println!("BANK r{} {:?} at {},{} altar {},{} -> {:?} gems {:?}", round(), self.ctype, self.pos.x, self.pos.y, a.x, a.y, r, self.me.gem_count());
                }
                if r.is_ok() {
                    self.action = Action::Idle;
                }
            }
        }
        Ok(())
    }

    // ---- camps ---------------------------------------------------------------------------------

    // Our sustained damage per turn against camps.
    fn camp_dps(&self) -> f32 {
        let s = self.me.strength().unwrap_or(40.0);
        let f = self.me.focus().unwrap_or(100.0);
        let mut d = s;
        for a in self.abil.iter().flatten() {
            match a.kind {
                AbilityKind::Damage | AbilityKind::AoeDamage | AbilityKind::Cc => d += f * a.modifier / 8.0,
                AbilityKind::AttackReset => d += s / 8.0,
                _ => {}
            }
        }
        if self.ctype == ChampionType::Varg {
            d *= 1.3;
        }
        let monster = self.has_emblem(ItemFamily::Monster);
        if monster {
            d *= 1.1;
        }
        d
    }

    fn has_emblem(&self, f: ItemFamily) -> bool {
        let Ok(items) = self.me.item_count() else { return false };
        [cambc::ItemStat::Strength, cambc::ItemStat::Focus, cambc::ItemStat::Health]
            .iter()
            .any(|s| items[cambc::get_item_enum(f, *s, false)] > 0)
    }

    // Travel steps to group g (dashers may cross holes).
    fn travel(&self, g: usize) -> i32 {
        let d = self.info.group_dist(g, self.pos);
        if d < i32::MAX / 8 {
            return d;
        }
        if self.dash.is_some() {
            if let Some(df) = self.info.dash_field(g) {
                let v = df.at(&self.info, self.pos);
                if v != FAR {
                    return v as i32 + 4;
                }
            }
        }
        i32::MAX / 4
    }

    // (turns to clear, survivable) with `helpers` champions like us on it (the party), or the
    // allies actually there if more.
    fn clear_estimate(&self, g: usize, helpers: f32) -> (f32, bool) {
        let grp = &self.info.groups[g];
        let hp_left = self.mem.groups[g].hp.max(1.0);
        let near = 1.0 + self.allies.iter().filter(|a| cheb(a.pos, grp.center) <= 6).count() as f32;
        let helpers = helpers.max(near);
        let dps = self.my_camp_dps * helpers;
        let turns = hp_left / dps.max(1.0);
        if grp.harmless {
            return (turns, true);
        }
        let jungle = if self.jungle { 0.7 } else { 1.0 };
        // Camps hit the closest champion, and a boss (200 a hit at range 5) kills whoever tanks it
        // before the rest finish: bosses need a much bigger margin.
        let boss = if grp.boss { BOSS_MARGIN } else { 1.0 };
        let incoming = grp.dps * jungle * turns * 0.6 * boss / helpers;
        (turns, incoming < self.hp * 0.75)
    }

    // Party members alive (claim byte != DEAD), counting us.
    fn party_alive(&self, mem: &[u8; 8]) -> Vec<usize> {
        self.party.iter().copied().filter(|s| *s == self.slot || mem[*s] != team::DEAD).collect()
    }

    // Allies of our party within `r` of `p`.
    fn party_near(&self, p: Vec2, r: i32) -> usize {
        self.allies.iter().filter(|a| self.party_units.contains(&a.champ) && cheb(a.pos, p) <= r).count()
    }

    fn choose_camp(&mut self) -> Result<Option<usize>> {
        let now = round();
        let mem = team::read();
        let alive = self.party_alive(&mem);
        let leader = alive.iter().copied().min().unwrap_or(self.slot);
        // Members go where the leader goes.
        if leader != self.slot {
            let v = mem[leader];
            if v != 0 && v != team::DEAD && (v as usize) <= self.info.groups.len() {
                let g = v as usize - 1;
                if self.lost_until[g] <= now && self.travel(g) < i32::MAX / 8 {
                    self.set_target(Target::Camp(g));
                    return Ok(Some(g));
                }
            }
        }
        // Nearest available group, in steps, with small nudges. v9 found nearest-of-all farming
        // beats value-per-turn scoring: everyone heading for the nearest camp keeps the team
        // together, so elk get cleared by whoever is around instead of looking "too slow" solo.
        // Gold first (shards early compound), gems after the profile's gem round.
        let claimed: Vec<usize> = team::claims(&mem, self.slot).collect();
        let gem_phase = now >= self.p.gem_round;
        let helpers = 1 + self.allies.iter().filter(|a| cheb(a.pos, self.pos) <= 10).count();
        let current = match self.target {
            Target::Camp(g) | Target::Rally(g) => Some(g),
            _ => None,
        };
        let mut best: Option<(f32, usize)> = None;
        let mut best_any: Option<(f32, usize)> = None;
        for g in 0..self.info.groups.len() {
            let grp = &self.info.groups[g];
            if self.lost_until[g] > now {
                continue;
            }
            let travel = self.travel(g);
            if travel >= i32::MAX / 8 {
                continue;
            }
            let state = self.mem.groups[g].state;
            let mut cost = travel as f32;
            match state {
                GState::Reward => cost -= 10.0,
                GState::Respawn(r) => {
                    let wait = (r - now - travel).max(0) as f32;
                    if wait > 15.0 {
                        continue;
                    }
                    cost += wait;
                }
                GState::Alive => {
                    let (clear, ok) = self.clear_estimate(g, helpers as f32);
                    if !ok {
                        // Bosses (and elk alone) need more of us than are around.
                        if grp.boss {
                            continue;
                        }
                        cost += 15.0;
                    }
                    cost += clear * 0.15;
                }
            }
            if self.p.kind != Kind::Fight && grp.d_enemy != FAR && grp.d_home > grp.d_enemy {
                cost += (grp.d_home - grp.d_enemy) as f32 * 0.5;
            }
            // Enough of us on it already: extra hands mostly split the reward and walk.
            let claimers = claimed.iter().filter(|c| **c == g).count();
            // Only for groups that don't hit back: on a hostile camp every extra hand also cuts the
            // damage the camp deals in total (it dies sooner), so piling on pays.
            if claimers > 0 && state != GState::Reward && grp.harmless {
                let need = (1..=5).find(|n| self.clear_estimate(g, *n as f32).1).unwrap_or(5);
                if claimers >= need {
                    cost += SPREAD_COST * (claimers + 1 - need) as f32;
                }
            }
            if self.p.kind != Kind::Fight {
                let age = now - self.mem.groups[g].enemy_near;
                if age <= 10 {
                    cost += ENEMY_NEAR_COST;
                } else if age <= 25 {
                    cost += ENEMY_NEAR_COST * 0.5;
                }
            }
            if current == Some(g) {
                cost -= STICK_STEPS;
            }
            let preferred = if gem_phase { grp.gems > 0 } else { grp.gold > 0 && !grp.boss };
            if preferred && best.map_or(true, |b| cost < b.0) {
                best = Some((cost, g));
            }
            if best_any.map_or(true, |b| cost < b.0) {
                best_any = Some((cost, g));
            }
        }
        // A non-preferred group much nearer than the preferred one wins (a gold cluster on the
        // way in the gem phase, or anything when the preferred kind is far).
        let best = match (best, best_any) {
            (Some(p), Some(a)) if a.0 + GOLD_DETOUR < p.0 && a.1 != p.1 && !self.info.groups[a.1].boss => Some(a),
            (p, a) => p.or(a),
        };
        let Some((_, g)) = best else { return Ok(None) };
        if !self.info.has_field(g) && fuel() > self.info.flood_cost + RESERVE_FUEL {
            self.info.ensure_field(g, self.dash.is_some())?;
        }
        self.set_target(Target::Camp(g));
        Ok(Some(g))
    }

    fn work_camp(&mut self, g: usize) -> Result<()> {
        let me = self.me.clone();
        let now = round();
        if !self.info.has_field(g) {
            if fuel() > self.info.flood_cost + RESERVE_FUEL {
                self.info.ensure_field(g, self.dash.is_some())?;
            } else {
                // Not enough fuel for the field this turn: head roughly there.
                self.action = Action::Going;
                let c = self.info.groups[g].center;
                self.pos = self.ctx().toward(c)?;
                return Ok(());
            }
        }
        match self.mem.groups[g].state {
            GState::Reward => self.capture(g),
            GState::Respawn(_) => {
                self.action = Action::Waiting;
                // Never wait on a spawn tile: an occupied tile holds the whole group's respawn.
                if self.info.groups[g].tiles.contains(&self.pos) {
                    let tiles = self.info.groups[g].tiles.clone();
                    let info = &self.info;
                    let cost = |t: Vec2| if tiles.contains(&t) { 100.0 } else { info.group_dist(g, t) as f32 };
                    self.pos = self.ctx().best_step(cost)?;
                } else if self.info.group_dist(g, self.pos) > 3 {
                    self.walk_to_group(g)?;
                }
                Ok(())
            }
            GState::Alive => {
                let range = me.attack_range().unwrap_or(1);
                // Too hurt to keep trading with the camp: break off.
                // Leave only if what the camp deals before it dies (with whoever is hitting it) would
                // take us under the retreat line: a nearly dead elk isn't worth a 90-round round trip.
                let helpers = 1.0 + self.allies.iter().filter(|a| cheb(a.pos, self.info.groups[g].center) <= 6).count() as f32;
                let left = self.mem.groups[g].hp / (self.my_camp_dps * helpers).max(1.0);
                if self.camp_threat(self.pos) * left.min(3.0) > self.hp - self.max_hp * self.p.retreat_hp {
                    if TRACE {
                        println!("HURT r{} {:?} group {} hp {:.0}", now, self.ctype, g, self.hp);
                    }
                    self.lost_until[g] = now + 60;
                    self.healing = true;
                    return self.go_home(Action::Retreating);
                }
                let camps = self.group_camps(g);
                if camps.is_empty() {
                    // Out of sight still: walk there.
                    self.action = Action::Going;
                    return self.walk_to_group(g);
                }
                let in_reach = |p: Vec2, r: i32| camps.iter().any(|(c, _, _)| cheb(c.spawn_position(), p) <= r);
                let mut moved = false;
                if !in_reach(self.pos, range) {
                    self.action = Action::Going;
                    let before = self.pos;
                    self.walk_to_group(g)?;
                    moved = self.pos != before;
                    if !in_reach(self.pos, range) || (range > 1 && moved) {
                        return Ok(());
                    }
                }
                self.action = Action::Attacking;
                self.hit_camps(&camps, moved, range)
            }
        }
    }

    // Visible living camps of group g, hostile ones first, then lowest health.
    fn group_camps(&self, g: usize) -> Vec<(cambc::Camp, f32, bool)> {
        let mut out = Vec::new();
        for c in self.camps.iter().filter(|c| c.alive && c.group == Some(g)) {
            let hostile = crate::mapinfo::camp_stats(c.kind).1 > 0.0;
            out.push((c.camp.clone(), c.hp, hostile));
        }
        out.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal)));
        out
    }

    fn hit_camps(&mut self, camps: &[(cambc::Camp, f32, bool)], moved: bool, range: i32) -> Result<()> {
        let me = self.me.clone();
        let pos = self.pos;
        // Abilities on camps: damage the first target, area damage where it hits most.
        for a in self.abil.iter().flatten().copied() {
            if !combat::is_ready(&me, &a) || (a.excl && moved) {
                continue;
            }
            let done = match a.kind {
                AbilityKind::Damage | AbilityKind::Cc if a.modifier > 0.0 => {
                    match camps.iter().find(|(c, _, _)| cheb(c.spawn_position(), pos) <= a.range) {
                        Some((c, _, _)) => combat::cast_on_camp(&me, &a, c).unwrap_or(false),
                        None => false,
                    }
                }
                AbilityKind::AoeDamage => {
                    let mut best: Option<(usize, Vec2)> = None;
                    for (c, _, _) in camps {
                        let p = c.spawn_position();
                        if cheb(p, pos) > a.range {
                            continue;
                        }
                        let n = camps.iter().filter(|(o, _, _)| cheb(o.spawn_position(), p) <= a.radius).count();
                        if best.map_or(true, |b| n > b.0) {
                            best = Some((n, p));
                        }
                    }
                    match best {
                        Some((n, p)) if n >= 2 || camps.len() == 1 => combat::cast_at(&me, &a, p).unwrap_or(false),
                        _ => false,
                    }
                }
                AbilityKind::Shield => self.camp_threat(pos) > 0.0 && combat::cast_self(&me, &a).unwrap_or(false),
                _ => false,
            };
            if done {
                break;
            }
        }
        if let Some((c, _, _)) = camps.iter().find(|(c, _, _)| cheb(c.spawn_position(), pos) <= range) {
            if combat::attack_camp(&me, c) {
                let c2 = c.clone();
                self.attack_reset(|| combat::attack_camp(&me, &c2))?;
            }
        }
        Ok(())
    }

    // Party cohesion on the way: a member far from its leader closes up first; a leader with a
    // member trailing in sight waits for it (a pack wins the fights a string of singles loses).
    fn cohesion_hold(&mut self) -> Result<bool> {
        if self.party.len() <= 1 {
            return Ok(false);
        }
        let mem = team::read();
        let alive = self.party_alive(&mem);
        let leader = alive.iter().copied().min().unwrap_or(self.slot);
        let mates: Vec<Vec2> = self
            .allies
            .iter()
            .filter(|a| self.party_units.contains(&a.champ))
            .map(|a| a.pos)
            .collect();
        if leader == self.slot {
            let trailing = mates.iter().any(|m| cheb(*m, self.pos) >= 5);
            return Ok(trailing && !mates.iter().any(|m| cheb(*m, self.pos) <= 2));
        }
        let roster = cambc::champions(Some(Team::Ally))?;
        if let Some(l) = roster.get(leader) {
            if let Some(u) = self.allies.iter().find(|a| a.champ == *l) {
                if cheb(u.pos, self.pos) > 4 {
                    self.pos = self.ctx().toward(u.pos)?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn walk_to_group(&mut self, g: usize) -> Result<()> {
        let d = self.travel(g);
        if d > 3 && self.stuck(g as i32, d) {
            if TRACE {
                println!("STUCK r{} {:?} group {} d {} at {},{}", round(), self.ctype, g, d, self.pos.x, self.pos.y);
            }
            self.lost_until[g] = round() + 100;
            self.progress = (-1, i32::MAX, round());
            self.set_target(Target::None);
            return Ok(());
        }
        if self.enemies.is_empty() && self.info.group_dist(g, self.pos) > 6 && self.cohesion_hold()? {
            return Ok(());
        }
        let travel_dash = self.enemies.is_empty();
        let np = match self.info.group_field(g) {
            Some(f) => self.ctx().go(f, self.info.dash_field(g), travel_dash)?,
            None => self.ctx().toward(self.info.groups[g].center)?,
        };
        self.pos = np;
        Ok(())
    }

    // The team's carry: the first alive champion in CARRY_ORDER. Captures are split among everyone
    // standing on the group, so (as v9's funnel) only the carry captures when it's at hand; the
    // rest move on, saving the five capture turns and feeding one damage dealer.
    fn carry_slot(&self) -> Option<usize> {
        let mem = team::read();
        CARRY_ORDER.iter().find_map(|k| {
            (0..self.roster.len()).find(|s| self.roster_kinds[*s] == *k && mem.get(*s).map_or(true, |b| *b != team::DEAD))
        })
    }

    fn capture(&mut self, g: usize) -> Result<()> {
        if FUNNEL {
            if let Some(c) = self.carry_slot() {
                if c != self.slot {
                    let carry = &self.roster[c];
                    let near = self.allies.iter().any(|a| a.champ == *carry && cheb(a.pos, self.info.groups[g].center) <= 6);
                    if near {
                        self.lost_until[g] = round() + 8;
                        self.set_target(Target::None);
                        return match self.choose_camp()? {
                            Some(n) if n != g => self.work_camp(n),
                            _ => self.explore(),
                        };
                    }
                }
            }
        }
        let tiles = &self.info.groups[g].tiles;
        if tiles.contains(&self.pos) {
            self.action = Action::Collecting;
            // Both teams on the group's tiles: nobody captures. If we can't see who, they're in a
            // bush on another tile of the group: walk the capture line to find them.
            let contested = cambc::map::camp_groups()
                .ok()
                .and_then(|gs| {
                    gs.into_iter().find(|cg| cg.camps().first().map(|c| c.spawn_position()) == Some(tiles[0]))
                })
                .and_then(|cg| cg.capture_state().ok())
                .map_or(false, |s| s.state == cambc::CampGroupCaptureType::Contested);
            if contested && self.enemies.is_empty() {
                let next = tiles
                    .iter()
                    .filter(|t| **t != self.pos && self.info.is_bush(**t))
                    .min_by_key(|t| cheb(**t, self.pos))
                    .copied();
                if let Some(t) = next {
                    self.pos = self.ctx().toward(t)?;
                }
            }
            return Ok(());
        }
        self.action = Action::Going;
        // Walk onto a free tile of the group.
        let goal = tiles
            .iter()
            .filter(|t| self.ctx().free(**t) || **t == self.pos)
            .min_by_key(|t| cheb(**t, self.pos))
            .copied();
        match goal {
            Some(t) if cheb(t, self.pos) <= 2 => {
                self.pos = self.ctx().toward(t)?;
                Ok(())
            }
            _ => self.walk_to_group(g),
        }
    }

    // ---- exploring -----------------------------------------------------------------------------

    fn explore(&mut self) -> Result<()> {
        self.action = Action::Going;
        if self.vantage.is_empty() {
            self.action = Action::Idle;
            return Ok(());
        }
        let i = self.vantage_i % self.vantage.len();
        let v = self.vantage[i];
        if cheb(v, self.pos) <= 2 {
            self.vantage_i += 1;
        }
        self.set_target(Target::Vantage(i));
        if fuel() > 40_000 {
            let _ = self.me.move_towards(v);
            self.pos = self.me.position()?;
        } else {
            self.pos = self.ctx().toward(v)?;
        }
        Ok(())
    }

    // ---- end of turn ---------------------------------------------------------------------------

    fn finish(&mut self) -> Result<()> {
        let claim = match self.target {
            Target::Camp(g) | Target::Rally(g) => Some(g),
            _ => None,
        };
        let _ = team::set_claim(self.slot, claim);
        self.hist[self.action as usize] += 1;
        if TRACE && round() % 250 == 0 {
            let items: u32 = self.me.item_count().map(|c| c.iter().map(|(_, n)| n).sum()).unwrap_or(0);
            println!("I r{} {:?} items {} gold {} str {:.0} foc {:.0} maxhp {:.0}", round(), self.ctype, items, self.me.gold_count().unwrap_or(0), self.me.strength().unwrap_or(0.0), self.me.focus().unwrap_or(0.0), self.max_hp);
        }
        if TRACE && round() % 500 == 499 {
            println!("H r{} {:?} dead/recall/bank/collect/going/attack/wait/retreat/idle {:?}", round(), self.ctype, self.hist);
        }
        if matches!(self.action, Action::Attacking | Action::Collecting | Action::Waiting) && fuel() > RESERVE_FUEL {
            let _ = self.ward();
        }
        // Spare fuel: build the next nearest missing group field.
        while fuel() > self.info.flood_cost + RESERVE_FUEL {
            let n = self.info.groups.len();
            let mut next = None;
            let mut bestd = i32::MAX;
            for g in 0..n {
                if !self.info.has_field(g) && !self.info.groups[g].boss {
                    let d = cheb(self.info.groups[g].center, self.pos);
                    if d < bestd {
                        bestd = d;
                        next = Some(g);
                    }
                }
            }
            match next {
                Some(g) => self.info.ensure_field(g, self.dash.is_some())?,
                None => break,
            }
            self.field_cursor += 1;
        }
        self.last_hp = self.hp;
        if TRACE && round() % 500 == 0 {
            unsafe { println!("STATS {:?} {:?}", self.ctype, combat::STATS); }
        }
        if TRACE && round() % 20 == 0 && self.ctype == ChampionType::VesperVale {
            println!("T r{} {:?} pos {},{} hp {:.0}/{:.0} {:?} {:?} fuel {} gems {:?} gold {:?}", round(), self.ctype, self.pos.x, self.pos.y, self.hp, self.max_hp, self.action, self.target, fuel(), self.me.gem_count(), self.me.gold_count());
        }
        if self.want_recall && !self.on_spawn() {
            let _ = self.me.recall();
        }
        Ok(())
    }

    // Ward a bush next to where we're standing still, the one nearest the enemy's side.
    fn ward(&self) -> Result<()> {
        if self.me.ward_cooldown()? > 0 {
            return Ok(());
        }
        let mut best: Option<(u16, Vec2)> = None;
        for dx in -2..=2 {
            for dy in -2..=2 {
                let q = Vec2::new(self.pos.x + dx, self.pos.y + dy);
                if !self.info.is_bush(q) || vision::in_vision(q).unwrap_or(true) {
                    continue;
                }
                let d = self.info.enemy_home.at(&self.info, q);
                if best.map_or(true, |b| d < b.0) {
                    best = Some((d, q));
                }
            }
        }
        if let Some((_, q)) = best {
            let _ = self.me.place_ward(q);
        }
        Ok(())
    }
}
