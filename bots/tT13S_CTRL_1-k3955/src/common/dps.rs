// Damage a champion deals a turn, averaged over 8 turns (the ability cooldown). A turn allows one
// ability cast and one basic attack, but a damaging spell's turn has no basic attack (the user,
// 2026-10-22: "ability and auto can't be used in one turn"), while an attack reset gives a second
// basic attack the turn it's cast. So over 8 turns: (8 - damaging casts + resets) basic attacks plus
// each spell once, over 8. The old sums added both every turn.
//
// Passives that change damage are counted: Heart of Ruin hits up to 50% harder as its health drops;
// Ren retaliates 0.2 x Focus on every hit he takes (one a turn in a fight); Varg's basic attacks
// splash their Strength on a neighbour (half the time). Ryth's Strength from health and Prism's
// Focus from carried gems are in their live stats already.
use cambc::{Champion, ChampionType, Result};

// Off: the old sum (a basic attack every turn plus every spell and reset over its cooldown).
// 2026-10-22, one game a map against off: 17/40 (gems lost on death 206 against 145); with
// attack::STUN_HIGHEST_DAMAGE 17/40 against it alone. Off.
pub const AVERAGED: bool = false;
const TURNS: f32 = 8.0;
const HEART_MAX_BONUS: f32 = 0.5;
const REN_RETALIATION: f32 = 0.2;
const VARG_SPLASH_SHARE: f32 = 0.5;

// `c`'s damage a turn; against `target` when given (its Durability on basic attacks, and the
// engine's damage-after-mitigation for abilities).
pub fn turn_damage(c: &Champion, target: Option<&Champion>) -> Result<f32> {
    let (basic, spells) = parts(c, target)?;
    Ok(basic + spells)
}

// (basic attacks a turn, spells a turn), passives included.
pub fn parts(c: &Champion, target: Option<&Champion>) -> Result<(f32, f32)> {
    let strength = c.strength()? * target.map_or(1.0, super::mitigation::incoming);
    let mut autos = TURNS;
    let mut spells = 0.0;
    for ability in c.abilities() {
        let Some(info) = ability.info()? else {
            continue;
        };
        let uses = TURNS / info.as_dyn().max_cooldown().max(1) as f32;
        if ability.is_attack_reset() {
            autos += uses;
        } else if super::attack::is_damaging(ability) {
            let hit = match target {
                Some(t) => ability
                    .damage_amount_against(t)
                    .or_else(|_| ability.damage_amount())?,
                None => ability.damage_amount()?,
            };
            if AVERAGED {
                autos -= uses;
            }
            spells += hit * uses;
        }
    }
    if !AVERAGED {
        return Ok((strength * (autos / TURNS), spells / TURNS));
    }
    let autos = autos.max(0.0);
    let mut basic = autos * strength / TURNS;
    let mut spells = spells / TURNS;
    match c.champion_name()? {
        ChampionType::HeartOfRuin => {
            let missing = 1.0 - c.health()? / c.max_health()?.max(1.0);
            let bonus = 1.0 + HEART_MAX_BONUS * missing.clamp(0.0, 1.0);
            basic *= bonus;
            spells *= bonus;
        }
        ChampionType::Ren => basic += REN_RETALIATION * c.focus()?,
        ChampionType::Varg => basic += VARG_SPLASH_SHARE * strength * autos / TURNS,
        _ => {}
    }
    Ok((basic, spells))
}
