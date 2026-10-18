// What could damage us: visible camps and enemy champions, and the damage they deal on a tile.
use super::attack::is_damaging;
use super::duel::DUEL_ENEMIES_JOIN;
use super::me;
use super::world::{camp_groups, distance, group_at, sight};
use cambc::{Champion, ChampionType, Result, Vec2};

// One source of damage: hits any tile within Chebyshev `range` of `origin` for `damage` per turn.
// Enemy champions can walk `speed` tiles a turn towards us; camps never move (speed 0).
#[derive(Clone)]
pub struct Threat {
    pub origin: Vec2,
    pub range: i32,
    pub damage: f32,
    pub speed: i32,
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

// Awake camps attack once per round; camps never move, so a live camp sits on its spawn tile.
// There is no "awake" query: a group counts as awake once a visible member is damaged or missing
// (dead). Groups that went back to sleep after two quiet rounds are still counted, overestimating.
// Only groups with a camp in sight can hurt us, so start from the visible camps.
fn camp_threats(threats: &mut Vec<Threat>) -> Result<()> {
    let sight = sight()?;
    if sight.camps.is_empty() {
        return Ok(());
    }
    let visible = &sight.visible;
    let groups = camp_groups()?;
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
                    if c.health()? < c.max_health()? {
                        awake = true;
                    }
                    live.push(c);
                }
                None => {
                    if visible.is_set(spawn)? {
                        awake = true;
                    }
                } // seen dead
            }
        }
        if !awake {
            continue;
        }
        for c in live {
            threats.push(Threat {
                origin: c.spawn_position(),
                range: c.attack_range()?,
                damage: c.strength()?,
                speed: 0,
            });
        }
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

// Each visible enemy's threats, read once per round and position (each costs a dozen host calls)
// and shared by the threat scan and the sightings memory (safety::remember_enemies).
pub fn enemy_threats_by_name() -> Result<std::rc::Rc<Vec<(ChampionType, Vec<Threat>)>>> {
    thread_local! {
        static CACHE: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<Vec<(ChampionType, Vec<Threat>)>>)>> = const { std::cell::RefCell::new(None) };
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
    let mut all = Vec::new();
    for enemy in &sight()?.enemies {
        let mut threats = Vec::new();
        let Ok(name) = enemy.champion_name() else {
            continue;
        };
        if enemy_threats_of(enemy, &mut threats).is_err() {
            continue;
        }
        all.push((name, threats));
    }
    let all = std::rc::Rc::new(all);
    CACHE.with(|c| *c.borrow_mut() = Some((key.0, key.1, all.clone())));
    Ok(all)
}

pub fn enemy_threats_of(enemy: &Champion, threats: &mut Vec<Threat>) -> Result<()> {
    let me = me()?;
    let origin = enemy.position()?;
    let attack_range = enemy.attack_range()?;
    let strength = enemy.strength()?;
    let speed = if DUEL_ENEMIES_JOIN {
        enemy.base_stats()?.base_move_speed as i32
    } else {
        0
    };
    threats.push(Threat {
        origin,
        range: attack_range,
        damage: strength,
        speed,
    });
    for ability in enemy.abilities() {
        let Some(info) = ability.info()? else {
            continue;
        };
        let per_turn = 1.0 / info.as_dyn().max_cooldown().max(1) as f32;
        if ability.is_attack_reset() {
            threats.push(Threat {
                origin,
                range: attack_range,
                damage: strength * per_turn,
                speed,
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
            });
        }
    }
    Ok(())
}
