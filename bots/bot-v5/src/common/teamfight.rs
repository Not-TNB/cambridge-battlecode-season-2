// Team fights. The duel model judges one enemy at a time and counts every visible enemy as hitting
// us but only allies already close as helping, so with a few of us against a few tanky enemies it
// said "lose" and everyone walked away (ladder, 2026-10-22: five games against KKB, a tanky
// Marcella/Stellar/Varg/Ryth/Ren team, with almost no deaths on either side while they farmed elk;
// the teams that beat KKB killed them, 6.8 deaths a game by round 600).
//
// With at least two of us and two of them within TEAMFIGHT_TILES of us, the whole fight is
// simulated instead: each side's damage a turn (Strength plus abilities over their cooldown) goes
// into the other side's lowest-health champion, for TEAMFIGHT_TURNS turns, counting who dies. We
// fight if we'd kill more than we'd lose (TEAMFIGHT_MARGIN; one less while we hold a capture),
// else we back off; if they chase, they arrive strung out, and next turn's simulation sees that.
// Everyone in the fight sees about the same champions and runs the same sum, so allies mostly agree
// without shared memory. Off: duels only.
// 2026-10-22, one game a map (40 games each): an even trade (margin 0) 16/40 against off (deaths
// 17.9 against 11.5), but 28/40 against our bot drafting KKB's tanky five (off: 24/40); a winning
// trade (margin 1) 18/40 against off (deaths 12.9 against 12.3) and 30/40 against the KKB draft
// (kills 15.8 against 6.9).
use super::attack::strike;
use super::me;
use super::world::{distance, sight};
use cambc::{Champion, Result, Vec2};

pub const TEAMFIGHT: bool = true;
const TEAMFIGHT_TILES: i32 = 7;
const TEAMFIGHT_TURNS: u32 = 6;
// Kills minus deaths we need to fight (0: an even trade is worth it).
const TEAMFIGHT_MARGIN: i32 = 1;
// Event simulation (the user's two models): the fight runs death by death (everyone hits the
// target until the first champion dies, then the sums are redone without it), once with both sides
// focusing the highest damage first and once the lowest health first; if either wins we fight,
// focusing the way that wins by more. Healing and shields a turn (the heal or shield over its 8-turn cooldown: Willow, Varg, Heart
// of Ruin) come off the other side's damage, and health counts for its Durability (mitigation.rs).
// Off: the turn-by-turn lowest-health simulation above.
// 2026-10-22, one game a map against the lowest-health simulation: 20/40 (gold +10%, tributes +9%,
// deaths 12.4 against 15.0); with mitigation.rs 21/40 (see there), 25/40 against the KKB draft.
pub const TEAMFIGHT_EVENT_SIM: bool = true;

// Damage a turn: Strength, plus each damaging ability (or attack reset) over its 8-turn cooldown.
fn turn_damage(c: &Champion) -> Result<f32> {
    super::dps::turn_damage(c, None)
}

// Healing plus shielding a turn: each heal or shield over its 8-turn cooldown.
fn turn_sustain(c: &Champion) -> Result<f32> {
    let mut sustain = 0.0;
    for ability in c.abilities() {
        if ability.info()?.is_none() {
            continue;
        }
        if ability.is_heal() {
            sustain += ability.heal_amount().unwrap_or(0.0) / 8.0;
        } else if ability.is_shield() {
            sustain += ability.shield_amount().unwrap_or(0.0) / 8.0;
        }
    }
    Ok(sustain)
}

#[derive(Clone, Copy)]
struct Fighter {
    hp: f32,
    damage: f32,
    sustain: f32,
}

fn fighter(c: &Champion) -> Result<Fighter> {
    Ok(Fighter {
        hp: c.health()? / super::mitigation::incoming(c),
        damage: turn_damage(c)?,
        sustain: turn_sustain(c)?,
    })
}

#[derive(Clone, Copy)]
enum Focus {
    HighestDamage,
    LowestHealth,
}

// Death by death until TEAMFIGHT_TURNS turns have passed or a side is down. Returns (their
// deaths, our deaths).
fn event_sim(mut ours: Vec<Fighter>, mut theirs: Vec<Fighter>, focus: Focus) -> (i32, i32) {
    let alive = |f: &Fighter| f.hp > 0.0;
    let target = |side: &[Fighter]| -> Option<usize> {
        let it = side.iter().enumerate().filter(|(_, f)| alive(f));
        match focus {
            Focus::HighestDamage => it
                .max_by(|a, b| a.1.damage.total_cmp(&b.1.damage))
                .map(|(i, _)| i),
            Focus::LowestHealth => it.min_by(|a, b| a.1.hp.total_cmp(&b.1.hp)).map(|(i, _)| i),
        }
    };
    let out = |from: &[Fighter], against: &[Fighter]| -> f32 {
        let damage: f32 = from.iter().filter(|f| alive(f)).map(|f| f.damage).sum();
        let sustain: f32 = against.iter().filter(|f| alive(f)).map(|f| f.sustain).sum();
        (damage - sustain).max(0.0)
    };
    let horizon = TEAMFIGHT_TURNS as f32;
    let mut t = 0.0;
    while t < horizon {
        let (Some(a), Some(b)) = (target(&theirs), target(&ours)) else {
            break;
        };
        let (ours_out, theirs_out) = (out(&ours, &theirs), out(&theirs, &ours));
        let time = |hp: f32, dps: f32| if dps > 0.0 { hp / dps } else { f32::INFINITY };
        let dt = time(theirs[a].hp, ours_out)
            .min(time(ours[b].hp, theirs_out))
            .min(horizon - t);
        if !dt.is_finite() || dt <= 0.0 {
            break;
        }
        theirs[a].hp -= ours_out * dt;
        ours[b].hp -= theirs_out * dt;
        // A target finished within rounding counts as dead.
        for f in theirs.iter_mut().chain(ours.iter_mut()) {
            if f.hp < 0.01 {
                f.hp = 0.0;
            }
        }
        t += dt;
    }
    let dead = |side: &[Fighter]| side.iter().filter(|f| !alive(f)).count() as i32;
    (dead(&theirs), dead(&ours))
}

// Each side: (health, damage a turn); returns (their deaths, our deaths).
fn simulate(mut ours: Vec<(f32, f32)>, mut theirs: Vec<(f32, f32)>) -> (i32, i32) {
    let hit = |side: &mut Vec<(f32, f32)>, damage: f32| {
        if let Some(target) = side
            .iter_mut()
            .filter(|(hp, _)| *hp > 0.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))
        {
            target.0 -= damage;
        }
    };
    for _ in 0..TEAMFIGHT_TURNS {
        let ours_out: f32 = ours
            .iter()
            .filter(|(hp, _)| *hp > 0.0)
            .map(|(_, d)| d)
            .sum();
        let theirs_out: f32 = theirs
            .iter()
            .filter(|(hp, _)| *hp > 0.0)
            .map(|(_, d)| d)
            .sum();
        hit(&mut theirs, ours_out);
        hit(&mut ours, theirs_out);
        if ours.iter().all(|(hp, _)| *hp <= 0.0) || theirs.iter().all(|(hp, _)| *hp <= 0.0) {
            break;
        }
    }
    let dead = |side: &Vec<(f32, f32)>| side.iter().filter(|(hp, _)| *hp <= 0.0).count() as i32;
    (dead(&theirs), dead(&ours))
}

pub enum Verdict {
    Fight(Vec2), // the enemy to focus (lowest health)
    Retreat,
}

// The team-fight verdict for `enemies` (fightable, in sight), if this is a team fight.
pub fn verdict(enemies: &[Champion]) -> Result<Option<Verdict>> {
    if !TEAMFIGHT {
        return Ok(None);
    }
    let me = me()?;
    let here = me.position()?;
    let mut theirs = Vec::new();
    let mut theirs_ev = Vec::new();
    let mut places = Vec::new(); // theirs_ev's tiles
    let mut focus: Option<(f32, Vec2)> = None;
    for enemy in enemies {
        let p = enemy.position()?;
        if distance(p, here) > TEAMFIGHT_TILES {
            continue;
        }
        let hp = enemy.health()?;
        theirs.push((hp, turn_damage(enemy)?));
        if TEAMFIGHT_EVENT_SIM {
            theirs_ev.push(fighter(enemy)?);
            places.push(p);
        }
        if focus.is_none_or(|(h, _)| hp < h) {
            focus = Some((hp, p));
        }
    }
    let mut ours = vec![(me.health()?, turn_damage(me)?)];
    let mut ours_ev = if TEAMFIGHT_EVENT_SIM {
        vec![fighter(me)?]
    } else {
        Vec::new()
    };
    for ally in &sight()?.allies {
        if !ally.is_alive()?
            || ally.is_recalling()?
            || distance(ally.position()?, here) > TEAMFIGHT_TILES
        {
            continue;
        }
        ours.push((ally.health()?, turn_damage(ally)?));
        if TEAMFIGHT_EVENT_SIM {
            ours_ev.push(fighter(ally)?);
        }
    }
    if ours.len() < 2 || theirs.len() < 2 {
        return Ok(None);
    }
    let margin = TEAMFIGHT_MARGIN - i32::from(super::collect::capturing()?);
    let wins = |(kills, deaths): (i32, i32)| kills - deaths >= margin && kills > 0;
    if TEAMFIGHT_EVENT_SIM {
        // Either way of focusing that wins is a fight; we focus the way that wins by more (lowest
        // health first on a tie).
        let mut best: Option<(i32, Focus)> = None;
        for focus in [Focus::LowestHealth, Focus::HighestDamage] {
            let result = event_sim(ours_ev.clone(), theirs_ev.clone(), focus);
            if wins(result) && best.is_none_or(|(b, _)| result.0 - result.1 > b) {
                best = Some((result.0 - result.1, focus));
            }
        }
        let Some((_, focus)) = best else {
            super::trace::trace("teamfight-retreat");
            return Ok(Some(Verdict::Retreat));
        };
        let pick = match focus {
            Focus::LowestHealth => {
                (0..theirs_ev.len()).min_by(|&a, &b| theirs_ev[a].hp.total_cmp(&theirs_ev[b].hp))
            }
            Focus::HighestDamage => (0..theirs_ev.len())
                .max_by(|&a, &b| theirs_ev[a].damage.total_cmp(&theirs_ev[b].damage)),
        };
        super::trace::trace("teamfight-fight");
        return Ok(pick.map(|i| Verdict::Fight(places[i])));
    }
    if wins(simulate(ours, theirs)) {
        super::trace::trace("teamfight-fight");
        Ok(focus.map(|(_, p)| Verdict::Fight(p)))
    } else {
        super::trace::trace("teamfight-retreat");
        Ok(Some(Verdict::Retreat))
    }
}

// Act on a verdict: hit what we can, then close on the focus target (fight) or back off (retreat).
pub fn act(verdict: Verdict, enemies: &[Champion]) -> Result<bool> {
    match verdict {
        Verdict::Fight(target) => {
            if strike(enemies, false)? {
                return Ok(true);
            }
            let range = me()?.attack_range()?;
            let mut goals = cambc::Bitboard::new()?;
            goals.draw_chebyshev(target, range as f32)?;
            goals.intersect_assign(&super::world::blocked_tiles()?.complement()?)?;
            if super::movement::move_towards_closest(goals)? {
                strike(enemies, true)?;
            }
            Ok(true)
        }
        Verdict::Retreat => {
            if super::safety::retreat_from_fight()? {
                strike(enemies, true)?;
                return Ok(true);
            }
            Ok(false)
        }
    }
}
