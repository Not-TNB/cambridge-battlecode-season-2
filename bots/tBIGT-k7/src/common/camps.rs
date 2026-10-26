// Fighting one camp group and capturing its reward.
use super::attack::attack_if_in_range;
use super::me;
use super::world::{distance, group_at, sight};
use crate::constants::{class_of, ClassKind};
use cambc::{Camp, CampGroup, ChampionType, Result, Vec2};

// A tank or healer leaves a cleared camp group's capture to an AD carry ally whose attack range
// covers it: capture rewards are split between every ally on the region, so the carry gets all of it.
pub(super) const TANK_FUNNEL: bool = true;
pub const FUNNEL_HEALERS: bool = true;

// Which camps we hit. Basic attacks and spells went to the weakest camp in range of any group, so
// a champion clearing one group fired into the next one's sleeping wolf (1,350 health against an
// elk's 2,400; on 00fragment the two big groups are five tiles apart, inside Marcella's range-4
// spell), waking a second group to hit us. With CAMP_FOCUS: camps of a group already awake (being
// fought) come first, our farm target's (or the rally's) before others; while a group is awake, a
// sleeping one is only woken if it's that target. Splash (Varg's passive, area spells, radius one)
// mustn't reach a sleeping camp we wouldn't wake either. Off: the weakest camp in range.
// 2026-10-21, 140 games: 74 against off (deaths 15.7 against 18.0 a game; hits waking a second
// group -86% in traced games), but clears -5% and fuel-exhausted turns +15%: it also kept us off
// sleeping groups we passed on the way to the target with nothing awake (those are free clears
// now), and it asked every camp in sight whether it was awake on every attack (now only camps
// within FOCUS_TILES, once a turn).
pub const CAMP_FOCUS: bool = true;
const SPLASH_RADIUS: i32 = 1;
// Reach of our longest attack or spell (range 5) plus a splash: only awake groups this close matter.
const FOCUS_TILES: i32 = 6;

pub(super) struct CampFocus {
    awake: Vec<usize>,
    target: Option<usize>,
    rally: Option<usize>,
}

thread_local! {
    // This turn's awake groups near us: (round, tile, groups).
    static AWAKE_NEAR: std::cell::RefCell<Option<(u32, Vec2, Vec<usize>)>> = const { std::cell::RefCell::new(None) };
}

impl CampFocus {
    pub(super) fn new() -> Result<Self> {
        let mut awake = Vec::new();
        if CAMP_FOCUS {
            let (round, here) = (cambc::round_number()?, me()?.position()?);
            let cached = AWAKE_NEAR.with(|a| {
                a.borrow()
                    .as_ref()
                    .filter(|(r, p, _)| (*r, *p) == (round, here))
                    .map(|(_, _, groups)| groups.clone())
            });
            match cached {
                Some(groups) => awake = groups,
                None => {
                    for camp in &sight()?.camps {
                        if distance(camp.spawn_position(), here) <= FOCUS_TILES
                            && camp.is_awake().unwrap_or(false)
                        {
                            if let Some(index) = group_at(camp.spawn_position())? {
                                if !awake.contains(&index) {
                                    awake.push(index);
                                }
                            }
                        }
                    }
                    AWAKE_NEAR.with(|a| *a.borrow_mut() = Some((round, here, awake.clone())));
                }
            }
        }
        let target = match super::farm::current_target() {
            Some(tile) if CAMP_FOCUS => group_at(tile)?,
            _ => None,
        };
        let rally = if CAMP_FOCUS {
            super::rally::active()?
        } else {
            None
        };
        Ok(Self {
            awake,
            target,
            rally,
        })
    }

    // Order to hit a camp of group `index` in (lower first), or None to leave it alone.
    pub(super) fn rank(&self, index: usize) -> Option<u8> {
        // Never a group we're fleeing (safety::FLEE_COMMIT_ROUNDS).
        if super::safety::fled(index) {
            return None;
        }
        if !CAMP_FOCUS {
            return Some(0);
        }
        let awake = self.awake.contains(&index);
        let target = self.target == Some(index) || self.rally == Some(index);
        match (awake, target) {
            (true, true) => Some(0),
            (true, false) => Some(1),
            (false, true) => Some(2),
            (false, false) if self.awake.is_empty() => Some(3),
            (false, false) => None,
        }
    }

    // Whether a hit on `tile` that splashes would also reach a camp we'd leave alone.
    pub(super) fn splash_wakes(&self, tile: Vec2) -> Result<bool> {
        if !CAMP_FOCUS {
            return Ok(false);
        }
        for camp in &sight()?.camps {
            let at = camp.spawn_position();
            if at == tile || distance(at, tile) > SPLASH_RADIUS {
                continue;
            }
            if group_at(at)?.is_some_and(|index| self.rank(index).is_none()) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

// Camps in sight within `tiles` of us that a landing dash mustn't touch (attack::IMPACT_DASH): those
// CampFocus would leave alone, or with `fight`, any sleeping camp (a camp woken in the middle of a
// fight with enemy champions hits whoever's closest). Their positions and health, and the targets:
// the camps we may hit.
pub(super) fn impact_camps(tiles: i32, fight: bool) -> Result<(Vec<(Vec2, f32)>, Vec<Vec2>)> {
    let here = me()?.position()?;
    let focus = CampFocus::new()?;
    let (mut targets, mut avoid) = (Vec::new(), Vec::new());
    for camp in &sight()?.camps {
        let at = camp.spawn_position();
        if distance(at, here) > tiles {
            continue;
        }
        let awake = camp.is_awake().unwrap_or(false);
        let index = group_at(at)?;
        let allowed = index.is_some_and(|i| {
            focus.rank(i).is_some()
                && super::collect::may_hit_camp(camp, i).unwrap_or(false)
                && super::collect::camp_fight_winnable(i).ok() != Some(Some(false))
        });
        if (fight && !awake) || !allowed {
            avoid.push(at);
        } else if !fight {
            targets.push((at, camp.health()?));
        }
    }
    Ok((targets, avoid))
}

// Attack the camp in range CampFocus puts first (the weakest among equals; camps never move, so
// their spawn tile is their position), skipping groups we'd lose to (collect::CAMP_FIGHT): leave
// those instead.
pub(super) fn attack_camp_in_range() -> Result<bool> {
    let me = me()?;
    if !me.can_attack()? {
        return Ok(false);
    }
    let (position, range) = (me.position()?, me.attack_range()?);
    let sight = sight()?;
    let focus = CampFocus::new()?;
    let splashes = CAMP_FOCUS && me.passive()? == cambc::PassiveType::SplashAttack;
    let mut best: Option<((u8, f32), &Camp)> = None;
    // On the way to a rally we said yes to (rally::RALLY_REPLIES): only its camps, or awake ones.
    let rally_only = if super::rally::RALLY_REPLIES && super::rally::going()? {
        super::rally::active()?
    } else {
        None
    };
    for camp in &sight.camps {
        if distance(camp.spawn_position(), position) > range {
            continue;
        }
        let Some(index) = group_at(camp.spawn_position())? else {
            continue;
        };
        if rally_only.is_some_and(|g| g != index) && !camp.is_awake().unwrap_or(false) {
            continue;
        }
        let Some(rank) = focus.rank(index) else {
            continue;
        };
        if !super::collect::may_hit_camp(camp, index)?
            || super::collect::camp_fight_winnable(index)? == Some(false)
            || (splashes && focus.splash_wakes(camp.spawn_position())?)
        {
            continue;
        }
        let key = (rank, camp.health()?);
        if best.is_none_or(|(k, _)| key < k) {
            best = Some((key, camp));
        }
    }
    match best {
        Some((_, camp)) => {
            trace_wake(camp);
            let hit = attack_if_in_range(camp, super::world::STALE_CAMPS)?;
            if hit {
                if let Some(index) = group_at(camp.spawn_position())? {
                    super::collect::note_camp_hit(index);
                    super::rally::call_for_help(index)?;
                }
            }
            Ok(hit)
        }
        None => Ok(false),
    }
}

// Debug tag for a hit on a sleeping camp while another group is awake or our target is another
// group (what CAMP_FOCUS prevents; measured with it on or off).
pub(super) fn trace_wake(camp: &Camp) {
    cambc::debug_block! {
        if super::trace::TRACE && !camp.is_awake().unwrap_or(true) {
            let index = group_at(camp.spawn_position()).ok().flatten();
            let target = super::farm::current_target().and_then(|t| group_at(t).ok().flatten());
            let other_awake = sight().is_ok_and(|s| {
                s.camps.iter().any(|c| {
                    c.is_awake().unwrap_or(false)
                        && group_at(c.spawn_position()).ok().flatten() != index
                })
            });
            if other_awake || (target.is_some() && target != index) {
                super::trace::trace("wake-other-group");
            }
        }
    }
}

// Bruisers and assassins (Marcella, Vendetta, Ren, ...) leave captures to a covering Vesper too
// (the gold-bonus carry: +33% gold, +53% with Treasure, so her capturing adds gold). They're melee,
// so they stand on the capture tiles while clearing, and she (ranged, shooting from three tiles)
// found an ally on every cleared group and walked on: in a Marcella/Vendetta/Vesper game on
// 00fragment she captured almost nothing. She no longer gives way to them (they leave, or share
// the split). Off: only tanks and healers funnel.
// 2026-10-21, 140 games: funnelling to every carry (Tavi, Prism, Stellar too) 66 against off
// (Vesper's gold 3.6k -> 5.2k, clears -2%): to the others it only moves gold around, and the
// walk to the capture tile costs clears.
// 2026-10-25 (teammate's ladder report: our melee earn 41-65% of the same champions' gold on the
// other side), 120 games against bot-v8 with a same-seed control (55/120): off 52/120, team gold@300
// 986 against 1,128. Marcella, Heart of Ruin and Vendetta gained ~70-100 each by round 300, but
// Vesper fell from 585 to 208 (she shoots from range, so melee on the capture tiles take it all)
// and died 3.1 times a game against 1.9. On.
pub const FUNNEL_MELEE: bool = true;

// Whether a champion of `class` leaves cleared groups' captures to a covering carry; melee classes
// (FUNNEL_MELEE) only to a gold-bonus one.
pub(super) fn funnels(class: ClassKind) -> bool {
    TANK_FUNNEL && (class == ClassKind::Tank || (FUNNEL_HEALERS && class == ClassKind::Healer))
}

pub(super) fn funnels_to_gold_carry(class: ClassKind) -> bool {
    TANK_FUNNEL
        && FUNNEL_MELEE
        && matches!(class, ClassKind::Bruiser | ClassKind::Assassin)
        && !funnel_capped()
}

// v9 (2026-10-25): the melee funnel (FUNNEL_MELEE) ends once the gold carry has completed
// FUNNEL_CAP_EMBLEMS emblems, or from round FUNNEL_CAP_ROUND: early gold on Vesper pays most, while
// with the funnel off our melee earned ~70-100 more each by round 300 (see FUNNEL_MELEE). Both sides
// agree: the carry counts her own emblems, a melee ally the carry it sees (none in sight: no cap, as
// the funnel needs her covering the group anyway). 0: no cap.
// 2026-10-25, 120 games each against bot-v8 with a same-seed control (57/120, team gold 1.01x
// theirs): after 2 emblems 54/120 (0.90x), after 3 59/120 (0.93x), from round 300 55/120 (0.94x).
// The melee gained ~250-300 gold each by round 1000 and Vesper lost ~750: no cap.
pub const FUNNEL_CAP_EMBLEMS: u32 = 0;
pub const FUNNEL_CAP_ROUND: u32 = 0;

fn funnel_capped() -> bool {
    thread_local! {
        static CAPPED: std::cell::Cell<Option<(u32, bool)>> = const { std::cell::Cell::new(None) };
    }
    let Ok(round) = cambc::round_number() else {
        return false;
    };
    if FUNNEL_CAP_ROUND > 0 && round >= FUNNEL_CAP_ROUND {
        return true;
    }
    if FUNNEL_CAP_EMBLEMS == 0 {
        return false;
    }
    if let Some((r, capped)) = CAPPED.with(|c| c.get()) {
        if r == round {
            return capped;
        }
    }
    let emblems = |champion: &cambc::Champion| -> u32 {
        champion.item_count().map_or(0, |items| {
            items
                .iter()
                // Shards and emblems alternate in the item ids: emblems are the odd ones.
                .filter(|(item, _)| *item as u32 % 2 == 1)
                .map(|(_, count)| count)
                .sum()
        })
    };
    let capped = (|| -> Result<bool> {
        if gold_carry()? {
            return Ok(emblems(me()?) >= FUNNEL_CAP_EMBLEMS);
        }
        for ally in &sight()?.allies {
            if ally.passive()? == cambc::PassiveType::BonusGold {
                return Ok(emblems(ally) >= FUNNEL_CAP_EMBLEMS);
            }
        }
        Ok(false)
    })()
    .unwrap_or(false);
    CAPPED.with(|c| c.set(Some((round, capped))));
    capped
}

// Whether we're a gold-bonus carry (Vesper Vale's passive).
pub(super) fn gold_carry() -> Result<bool> {
    Ok(me()?.passive()? == cambc::PassiveType::BonusGold)
}

// Whether an ally (not us) stands on one of `group`'s camp tiles. A gold-bonus carry
// (`gold_carry`) ignores the melee allies that funnel captures to it.
pub(super) fn ally_on_group(group: &CampGroup, gold_carry: bool) -> Result<bool> {
    for ally in &sight()?.allies {
        let p = ally.position()?;
        if !group.camps().iter().any(|c| c.spawn_position() == p) {
            continue;
        }
        if gold_carry && funnels_to_gold_carry(class_of(ally.champion_name()?)) {
            continue;
        }
        return Ok(true);
    }
    Ok(false)
}

// One capturer per cleared group. The funnel rules above each looked at a reward from one side
// (a tank leaves it to a covering carry, a melee to Vesper, anyone to an ally already on it), so
// two allies could both leave it, or both come back, a few steps each way. With CAPTURE_CLAIM
// every ally within CLAIM_TILES of the group ranks the same candidates the same way and only the
// first one goes: Vesper's gold passive first, then other carries, then the rest, tanks and
// healers last; within a rank whoever is nearest a capture tile (on one: 0), then by champion.
// Someone already within CLAIM_TILES beats anyone further. Off: the funnel rules.
// 2026-10-22, 320 games against off: 137-183 (captures 81 a game against 87; turning away and
// coming back went up, not down: the chosen ally didn't always go). Off.
pub const CAPTURE_CLAIM: bool = false;
const CLAIM_TILES: i32 = 6;

fn claim_key(
    name: ChampionType,
    bonus_gold: bool,
    at: Vec2,
    tiles: &[Vec2],
) -> (bool, u8, i32, i32) {
    let class = class_of(name);
    let rank = if bonus_gold {
        0
    } else if class.is_funnel_carry() {
        1
    } else if funnels(class) {
        3
    } else {
        2
    };
    let gap = tiles
        .iter()
        .map(|&t| distance(t, at))
        .min()
        .unwrap_or(i32::MAX);
    (gap > CLAIM_TILES, rank, gap, name as i32)
}

// Whether an ally in sight comes before us for `group`'s capture (CAPTURE_CLAIM).
pub(super) fn claimed_by_other(group: &CampGroup) -> Result<bool> {
    let me = me()?;
    let tiles: Vec<Vec2> = group.camps().iter().map(|c| c.spawn_position()).collect();
    let bonus = |p: cambc::PassiveType| p == cambc::PassiveType::BonusGold;
    let mine = claim_key(
        me.champion_name()?,
        bonus(me.passive()?),
        me.position()?,
        &tiles,
    );
    for ally in &sight()?.allies {
        if !ally.is_alive()? || ally.is_recalling()? {
            continue;
        }
        let key = claim_key(
            ally.champion_name()?,
            bonus(ally.passive()?),
            ally.position()?,
            &tiles,
        );
        if !key.0 && key < mine {
            return Ok(true);
        }
    }
    Ok(false)
}

// Whether an AD carry ally (not us; only a gold-bonus one with `gold_only`) has any of `group`'s
// capture tiles within its attack range.
// v8 (overnight tests, 2026-10-24): when a tank or melee champion leaves a capture to the carry.
// In 25 traced games 722 captures were left to Vesper Vale: she took 66% (a median 8 rounds after),
// 10% were never captured and 2.5% went to the enemy. 0: the carry has a capture tile within its
// attack range (as before); 1: the carry stands within HANDOFF_TILES of a capture tile (it's
// basically there); 2: the carry has one in range and is nearer the capture tiles than we are.
// See rally::RALLY_COMMIT for the 2026-10-24 results. On: 1.
pub const HANDOFF_MODEL: u8 = 1;
const HANDOFF_TILES: i32 = 2;

pub(super) fn carry_covers(group: &CampGroup, gold_only: bool) -> Result<bool> {
    let my_position = me()?.position()?;
    let gap = |p: Vec2| {
        group
            .camps()
            .iter()
            .map(|c| distance(c.spawn_position(), p))
            .min()
            .unwrap_or(i32::MAX)
    };
    for ally in &sight()?.allies {
        let position = ally.position()?;
        if position == my_position
            || !crate::constants::class_of(ally.champion_name()?).is_funnel_carry()
            || (gold_only && ally.passive()? != cambc::PassiveType::BonusGold)
        {
            continue;
        }
        let range = ally.attack_range()?;
        let theirs = gap(position);
        let covers = match HANDOFF_MODEL {
            1 => theirs <= HANDOFF_TILES,
            2 => theirs <= range && theirs < gap(my_position),
            _ => theirs <= range,
        };
        if covers {
            return Ok(true);
        }
    }
    Ok(false)
}
