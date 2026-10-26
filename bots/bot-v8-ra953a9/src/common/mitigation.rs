// Damage reduction from emblem passives, for the fight maths: a Durability emblem takes 10% off all
// incoming damage, a Jungle emblem 30% off damage from camps. Every build of ours has a Durability
// emblem and the frontliners open with Jungle (so does the ladder's KKB, on every champion), but
// the duel, team-fight and camp-fight checks took every hit at full value. Ability damage read
// with `damage_amount_against` is already reduced by the engine; basic attacks and camp hits are
// scaled here. Read once a round per champion (`item_count` is a host call). Off: no reduction.
use cambc::{Champion, ChampionType, ItemType};
use std::cell::RefCell;

// 2026-10-22, one game a map against off: 22/40; with TEAMFIGHT_EVENT_SIM 21/40 (gold 5.75 a round
// against 4.84, tributes +12%, deaths 11.7 against 16.0).
pub const MITIGATION: bool = true;
const DURABILITY_FACTOR: f32 = 0.9;
const JUNGLE_FACTOR: f32 = 0.7;

thread_local! {
    // (champion, round, durability, jungle)
    static SEEN: RefCell<Vec<(ChampionType, u32, bool, bool)>> = const { RefCell::new(Vec::new()) };
}

fn passives(c: &Champion) -> (bool, bool) {
    let Ok(name) = c.champion_name() else {
        return (false, false);
    };
    let round = cambc::round_number().unwrap_or(0);
    if let Some(hit) = SEEN.with(|s| {
        s.borrow()
            .iter()
            .find(|&&(n, r, _, _)| n == name && r == round)
            .map(|&(_, _, d, j)| (d, j))
    }) {
        return hit;
    }
    let (d, j) = match c.item_count() {
        Ok(items) => {
            let has = |list: [ItemType; 3]| list.iter().any(|&i| items[i] > 0);
            (
                has([
                    ItemType::DurabilityEmblemOfStrength,
                    ItemType::DurabilityEmblemOfFocus,
                    ItemType::DurabilityEmblemOfHealth,
                ]),
                has([
                    ItemType::JungleEmblemOfStrength,
                    ItemType::JungleEmblemOfFocus,
                    ItemType::JungleEmblemOfHealth,
                ]),
            )
        }
        Err(_) => (false, false),
    };
    SEEN.with(|s| {
        let mut s = s.borrow_mut();
        s.retain(|&(n, _, _, _)| n != name);
        s.push((name, round, d, j));
    });
    (d, j)
}

// Share of champion damage `c` takes (basic attacks).
pub fn incoming(c: &Champion) -> f32 {
    if !MITIGATION {
        return 1.0;
    }
    if passives(c).0 {
        DURABILITY_FACTOR
    } else {
        1.0
    }
}

// Share of camp damage `c` takes.
pub fn from_camps(c: &Champion) -> f32 {
    if !MITIGATION {
        return 1.0;
    }
    let (d, j) = passives(c);
    (if d { DURABILITY_FACTOR } else { 1.0 }) * (if j { JUNGLE_FACTOR } else { 1.0 })
}
