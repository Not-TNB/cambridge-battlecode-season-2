// What could damage us: visible camps and enemy champions, and the damage they deal on a tile.
use super::attack::is_damaging;
use super::duel::DUEL_ENEMIES_JOIN;
use super::me;
use super::world::{camp_groups, distance, group_at, sight};
use cambc::{Champion, ChampionType, Result, Vec2};

// One source of damage: hits any tile within Chebyshev `range` of `origin` for `damage` per turn.
// Enemy champions can walk `speed` tiles a turn towards us; camps never move (speed 0). A camp hits
// the closest champion in its range, so `shield` is how far the nearest other champion in its range
// is (ally or enemy): a tile further than that is safe from it (CAMPS_HIT_CLOSEST). None: nobody.
#[derive(Clone)]
pub struct Threat {
    pub origin: Vec2,
    pub range: i32,
    pub damage: f32,
    pub speed: i32,
    pub shield: Option<i32>,
}

// Awake camps hit the closest champion in their range (an elk deals 60 a round to one champion), so
// one only counts against a tile no other champion is closer to: standing further from an elk than
// the enemy we're fighting means it hits them, not us. Off: every awake camp in range hits us.
// 2026-10-21, 132 games: off lost 59 to 73 against on (fewer deaths to champions, more kills).
pub const CAMPS_HIT_CLOSEST: bool = true;

// The other champions in sight (allies and enemies, not us), for CAMPS_HIT_CLOSEST.
pub fn other_champions() -> Result<Vec<Vec2>> {
    let sight = sight()?;
    let mut bodies = Vec::with_capacity(sight.allies.len() + sight.enemies.len());
    for c in sight.allies.iter().chain(sight.enemies.iter()) {
        if c.is_alive().unwrap_or(false) {
            if let Ok(p) = c.position() {
                bodies.push(p);
            }
        }
    }
    Ok(bodies)
}

// How far the nearest of `bodies` within `range` of a camp at `origin` is (None: none in range).
pub fn camp_shield(origin: Vec2, range: i32, bodies: &[Vec2]) -> Option<i32> {
    if !CAMPS_HIT_CLOSEST {
        return None;
    }
    bodies
        .iter()
        .map(|&b| distance(origin, b))
        .filter(|&d| d <= range)
        .min()
}

// Everything visible that could damage us this turn. Scanning costs ~7k fuel, but checking a
// tile against the list afterwards is cheap, so scan once and reuse it for many tiles.
pub struct Threats(pub Vec<Threat>);

impl Threats {
    pub fn scan() -> Result<Self> {
        let mut threats = Vec::new();
        camp_threats(&mut threats)?;
        enemy_threats(&mut threats)?;
        Ok(Self(threats))
    }

    // Expected damage taken standing on `tile` for one turn.
    pub fn damage_at(&self, tile: Vec2) -> f32 {
        self.damage_at_turn(tile, 0)
    }

    // Expected damage on `tile` during the turn `turns` from now, if every enemy champion walks
    // straight at it meanwhile (ignoring walls, so this errs on the side of danger).
    pub fn damage_at_turn(&self, tile: Vec2, turns: u32) -> f32 {
        self.0
            .iter()
            .filter(|t| distance(t.origin, tile) - t.speed * turns as i32 <= t.range)
            .filter(|t| t.shield.is_none_or(|s| distance(t.origin, tile) <= s))
            .map(|t| t.damage)
            .sum()
    }
}

// Expected damage our champion would take standing on `tile` for one turn, from what we can see:
// awake camps within their attack range, plus enemy champions' attacks and averaged abilities.
// Every threat is assumed to target us, using raw Strength for attacks.
// Scans every call; to check several tiles, use `Threats::scan()` once and `damage_at` per tile.
#[allow(dead_code)] // kept as the simple single-tile entry point
pub fn expected_dmg(tile: Vec2) -> Result<f32> {
    Ok(Threats::scan()?.damage_at(tile))
}

// Off: the old guess only (see below).
pub const CAMP_AWAKE_QUERY: bool = true;

// Awake camps attack once per round; camps never move, so a live camp sits on its spawn tile. A group
// is awake if any visible member says so (Camp::is_awake). If that query fails, fall back to the old
// guess: awake once a visible member is damaged or missing (dead), which kept counting groups that
// had gone back to sleep after a quiet round (the rules say two; one is what we see). Only groups with a camp in sight can hurt us, so
// start from the visible camps.
fn camp_threats(threats: &mut Vec<Threat>) -> Result<()> {
    let sight = sight()?;
    if sight.camps.is_empty() {
        return remembered_threats(threats, &sight);
    }
    let groups = camp_groups()?;
    let bodies = other_champions()?;
    let mut done = Vec::new();
    for camp in &sight.camps {
        let Some(index) = group_at(camp.spawn_position())? else {
            continue;
        };
        if done.contains(&index) {
            continue;
        }
        done.push(index);
        let mut live = Vec::new();
        let mut awake = false;
        for member in groups[index].camps() {
            let spawn = member.spawn_position();
            match sight.camps.iter().find(|c| c.spawn_position() == spawn) {
                Some(c) => {
                    awake |= match c.is_awake() {
                        Ok(a) if CAMP_AWAKE_QUERY => a,
                        _ => c.health()? < c.max_health()?,
                    };
                    live.push(c);
                }
                // Seen dead: the live members say whether the group is awake (the old guess
                // counted a dead member as awake).
                None => awake |= !CAMP_AWAKE_QUERY && sight.visible.is_set(spawn)?,
            }
        }
        if !awake {
            continue;
        }
        for c in live {
            let range = c.attack_range()?;
            let damage = c.strength()?;
            if CAMP_THREAT_MEMORY {
                remember_awake(c.spawn_position(), range, damage);
            }
            threats.push(Threat {
                origin: c.spawn_position(),
                range,
                damage,
                speed: 0,
                shield: camp_shield(c.spawn_position(), range, &bodies),
            });
        }
    }
    remembered_threats(threats, &sight)
}

// v9 (2026-10-26, the user: champions walked out of a bush into awake camps they could no longer
// see, and died): awake camps we saw stay threats for CAMP_MEMORY_ROUNDS while out of sight (in a
// bush, behind us), for retreats, recall spots and the low-health check. One seen dead, or seen
// again (then it's in sight), drops out. Off: only camps in sight.
// 2026-10-26 (SDK 4.0, 70 maps), 140 games against bot-v8 with a same-seed control (72/140, gold
// 0.97x, deaths 12.0 a game, 1.2 of them to camps): 66/140 (+28/-34), gold at round 300
// 845 against 928, camp deaths unchanged (1.19). Off.
pub const CAMP_THREAT_MEMORY: bool = false;
const CAMP_MEMORY_ROUNDS: u32 = 6;

thread_local! {
    // (tile, attack range, strength, round last seen awake)
    static AWAKE_SEEN: std::cell::RefCell<Vec<(Vec2, i32, f32, u32)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn remember_awake(tile: Vec2, range: i32, damage: f32) {
    let round = cambc::round_number().unwrap_or(0);
    AWAKE_SEEN.with(|a| {
        let mut a = a.borrow_mut();
        match a.iter_mut().find(|e| e.0 == tile) {
            Some(e) => *e = (tile, range, damage, round),
            None => a.push((tile, range, damage, round)),
        }
    });
}

// Remembered awake camps out of sight now: (tile, attack range, strength). Camps on visible tiles
// are in sight (or dead) and drop out.
pub fn remembered_camps(sight: &super::world::Sight) -> Vec<(Vec2, i32, f32)> {
    if !CAMP_THREAT_MEMORY {
        return Vec::new();
    }
    let round = cambc::round_number().unwrap_or(0);
    AWAKE_SEEN.with(|a| {
        let mut a = a.borrow_mut();
        a.retain(|&(t, _, _, r)| {
            round < r + CAMP_MEMORY_ROUNDS && !sight.visible.is_set(t).unwrap_or(true)
        });
        a.iter().map(|&(t, range, d, _)| (t, range, d)).collect()
    })
}

fn remembered_threats(threats: &mut Vec<Threat>, sight: &super::world::Sight) -> Result<()> {
    for (origin, range, damage) in remembered_camps(sight) {
        threats.push(Threat {
            origin,
            range,
            damage,
            speed: 0,
            shield: None,
        });
    }
    Ok(())
}

// Enemies basic-attack anything within attack range. Abilities are averaged over their cooldown
// (1/8 of a hit per turn) rather than assuming a ready ability is cast now; an attack reset is
// worth 1/8 of an extra basic attack.
fn enemy_threats(threats: &mut Vec<Threat>) -> Result<()> {
    for (_, of_enemy) in enemy_threats_by_name()?.iter() {
        threats.extend(of_enemy.iter().cloned());
    }
    Ok(())
}

// Each visible enemy champion with its threats.
pub type ThreatsByName = Vec<(ChampionType, Vec<Threat>)>;

// Each visible enemy's threats, read once per round and position (each costs a dozen host calls)
// and shared by the threat scan and the sightings memory (safety::remember_enemies).
pub fn enemy_threats_by_name() -> Result<std::rc::Rc<ThreatsByName>> {
    thread_local! {
        static CACHE: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<ThreatsByName>)>> =
            const { std::cell::RefCell::new(None) };
    }
    let key = (cambc::round_number()?, me()?.position()?);
    if let Some(cached) = CACHE.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == key)
            .map(|(_, _, t)| t.clone())
    }) {
        return Ok(cached);
    }
    // An enemy's threats don't change during our turn (it moves on its own), so after we step each
    // one is read once a round: only enemies we couldn't see before the step cost anything. Reading
    // them all again after every step cost ~12k on the turns that ran out of fuel.
    thread_local! {
        static THIS_ROUND: std::cell::RefCell<(u32, ThreatsByName)> =
            const { std::cell::RefCell::new((0, Vec::new())) };
    }
    THIS_ROUND.with(|t| {
        let mut t = t.borrow_mut();
        if t.0 != key.0 {
            *t = (key.0, Vec::new());
        }
    });
    let mut all = Vec::new();
    for enemy in &sight()?.enemies {
        let Ok(name) = enemy.champion_name() else {
            continue;
        };
        if let Some(threats) = THIS_ROUND.with(|t| {
            t.borrow()
                .1
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, th)| th.clone())
        }) {
            all.push((name, threats));
            continue;
        }
        let mut threats = Vec::new();
        if enemy_threats_of(enemy, &mut threats).is_err() {
            continue;
        }
        THIS_ROUND.with(|t| t.borrow_mut().1.push((name, threats.clone())));
        all.push((name, threats));
    }
    let all = std::rc::Rc::new(all);
    CACHE.with(|c| *c.borrow_mut() = Some((key.0, key.1, all.clone())));
    Ok(all)
}

// The most damage we could take next round where we stand, if everything that can reach us hits at
// once: each enemy champion within its attack range plus a turn of walking lands a basic attack, a
// second one if its attack reset is ready, and its ready damaging abilities in range; every awake
// camp with us in range hits once. For "could we die this round" checks (Heart of Ruin's shield).
pub fn burst_damage_here() -> Result<f32> {
    let me = me()?;
    let here = me.position()?;
    let sight = sight()?;
    let mut burst = 0.0;
    for camp in &sight.camps {
        if camp.is_awake().unwrap_or(false)
            && distance(camp.spawn_position(), here) <= camp.attack_range()?
        {
            burst += camp.strength()?;
        }
    }
    for enemy in &sight.enemies {
        let gap = distance(enemy.position()?, here);
        let walk = enemy.base_stats()?.base_move_speed;
        let range = enemy.attack_range()?;
        if gap > range + walk {
            continue;
        }
        let strength = enemy.strength()?;
        burst += strength;
        for ability in enemy.abilities() {
            if !super::attack::ability_ready(ability, enemy).unwrap_or(true) {
                continue;
            }
            if ability.is_attack_reset() {
                burst += strength;
            } else if is_damaging(ability) {
                let reach = ability
                    .info()
                    .ok()
                    .flatten()
                    .and_then(|i| i.as_dyn().range().ok())
                    .map_or(range, |r| r as i32);
                if gap <= reach + walk {
                    burst += ability
                        .damage_amount_against(me)
                        .or_else(|_| ability.damage_amount())
                        .unwrap_or(0.0);
                }
            }
        }
    }
    Ok(burst)
}

pub fn enemy_threats_of(enemy: &Champion, threats: &mut Vec<Threat>) -> Result<()> {
    let me = me()?;
    let origin = enemy.position()?;
    let attack_range = enemy.attack_range()?;
    let strength = enemy.strength()?;
    let speed = if DUEL_ENEMIES_JOIN {
        enemy.base_stats()?.base_move_speed
    } else {
        0
    };
    // Averaged over 8 turns (dps.rs): fewer basic attacks for its spells, its resets and damage
    // passives folded into the basic-attack threat.
    let basic = if super::dps::AVERAGED {
        super::dps::parts(enemy, None)?.0
    } else {
        strength
    };
    threats.push(Threat {
        origin,
        range: attack_range,
        damage: basic,
        speed,
        shield: None,
    });
    for ability in enemy.abilities() {
        let Some(info) = ability.info()? else {
            continue;
        };
        let per_turn = 1.0 / info.as_dyn().max_cooldown().max(1) as f32;
        if ability.is_attack_reset() && super::dps::AVERAGED {
            continue;
        }
        if ability.is_attack_reset() {
            threats.push(Threat {
                origin,
                range: attack_range,
                damage: strength * per_turn,
                speed,
                shield: None,
            });
        } else if is_damaging(ability) {
            // Engine prediction includes our mitigation and shields; fall back to raw damage.
            let hit = ability
                .damage_amount_against(me)
                .or_else(|_| ability.damage_amount())?;
            threats.push(Threat {
                origin,
                range: info.as_dyn().range()? as i32,
                damage: hit * per_turn,
                speed,
                shield: None,
            });
        }
    }
    Ok(())
}
