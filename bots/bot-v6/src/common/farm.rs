// Choosing where to farm: phase filter, respawn memory, avoiding enemies and the farm plan.
use super::attack::cast_abilities_on_camps;
use super::camps::{
    ally_on_group, attack_camp_in_range, carry_covers, claimed_by_other, funnels,
    funnels_to_gold_carry, gold_carry, CAPTURE_CLAIM, FUNNEL_MELEE,
};
use super::duel::{cached_fight_view, fight_view, plan_duel};
use super::me;
use super::movement::{goals_were_unreachable, move_towards_closest};
use super::trace::{checkpoint, FARM_TARGET_DIAGNOSTICS};
use super::world::{camp_groups, distance, group_at, sight};
use cambc::{Bitboard, Camp, Result, Vec2};
use std::cell::OnceCell;

// Round each camp group (indexed as in camp_groups()) is expected back after we last saw it
// respawning. Without this, a respawning group only counts as respawning while in sight, so a
// champion at the edge of vision steps away, loses sight, steps back, and oscillates.
thread_local! {
    static RESPAWN_READY: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

// The round group `index` is back, if we know it's respawning (seen, or an ally's message).
pub fn respawn_ready(index: usize) -> Option<u32> {
    let round = cambc::round_number().ok()?;
    RESPAWN_READY.with(|r| {
        r.borrow()
            .get(index)
            .copied()
            .filter(|&ready| ready > round)
    })
}

// Start of every turn (bot.rs): take in an ally's respawn message (respawns.rs).
pub fn hear_respawns() -> Result<()> {
    let Some((index, at)) = super::respawns::listen()? else {
        return Ok(());
    };
    let news = RESPAWN_READY.with(|cell| {
        let mut ready = cell.borrow_mut();
        if ready.len() <= index {
            ready.resize(index + 1, 0);
        }
        let news = ready[index] < at;
        if news {
            ready[index] = at;
        }
        news
    });
    if news {
        super::respawns::bump_news();
        super::trace::trace("respawn-heard");
    }
    Ok(())
}

// Phase filter on top of nearest-camp farming: before FARM_GEM_PHASE_ROUND we skip gem clusters
// and monoliths (gems early don't help; gold buys items), after it we skip gold clusters (35 gold,
// no gems). If no camp of the preferred kinds is available we fall back to all camps. Scoring
// camps by value per turn instead tested worse (writeup entry 20). The filter applies to whole
// groups: skipping single camps abandoned a group whose elk was dead but whose gold cluster wasn't,
// so it could never be captured (and the champion stepped back and forth at its edge of vision).
// The opening (opening.rs) runs farming until round 400 on most maps, so the round only matters on
// Normal maps and after that. Against 150 (2026-10-21, 132 games each): 400 won 71, 550 74, 700 75
// and 66, 1000 58, 1300 62, 0 60; the filter off 57.
pub const FARM_PHASE_FILTER: bool = true;
pub const FARM_GEM_PHASE_ROUND: u32 = 550;
// The switch round depends on the map (v4): gems early only pay if we can reach the 1,000 that end
// the game before round 2,000; otherwise gold first (items, and the tiebreak) for longer. A map is
// rich when its camps' gem supply (every group at its respawn rate plus GEM_SUPPLY_OVERHEAD rounds a
// capture, monolith groups at MONOLITH_GEM_WEIGHT, half of it ours) times GEM_SHARE_TAKEN (what we
// take of it) reaches GEM_WIN_TARGET by the round limit. On the current maps that makes 00contest,
// 00fragment, 01elk-arena, 01gold-rush, 01hooks and 01concentrated-wealth poor: the ones where
// games ran to ~2,000 rounds. 0 for both: FARM_GEM_PHASE_ROUND everywhere.
// 2026-10-21 (engine 3.0.4, 35 maps, 210 games each, against 550 everywhere): 400 rich / 1000 poor
// 105 (even: the late switch hurt 01hooks, 01gold-rush, 01starvation; gems still decide the game at
// the round limit); only the no-gold-phase rule (below), 550 otherwise: 110. So 550 for both.
pub const GEM_PHASE_RICH_ROUND: u32 = 550;
pub const GEM_PHASE_POOR_ROUND: u32 = 550;
const GEM_SUPPLY_OVERHEAD: f32 = 20.0;
const GEM_SHARE_TAKEN: f32 = 0.4;
const MONOLITH_GEM_WEIGHT: f32 = 0.25;
const GEM_WIN_TARGET: f32 = 1000.0;
// Gold first only pays where there's gold-only farming (gold clusters) to put first: the gold
// phase only skips gem camps, so a map whose gold-only groups supply less than GOLD_PHASE_MIN_SUPPLY
// gold a team over the game (00quadrants has none: gems sit in nearly every group) goes gems first
// from the start.
const GOLD_PHASE_MIN_SUPPLY: f32 = 2000.0;

// The round farming switches from gold first to gems first on this map (worked out once).
pub fn gem_phase_round() -> u32 {
    thread_local! {
        static ROUND: OnceCell<u32> = const { OnceCell::new() };
    }
    if let Some(round) = ROUND.with(|r| r.get().copied()) {
        return round;
    }
    let round = if GEM_PHASE_RICH_ROUND == 0 && GEM_PHASE_POOR_ROUND == 0 {
        FARM_GEM_PHASE_ROUND
    } else {
        let limit = cambc::constants().max_rounds as f32;
        let mut supply = 0.0;
        let mut gold_only = 0.0;
        for group in camp_groups().unwrap_or(&[]) {
            let cycle = group.max_respawn_timer().max(0) as f32 + GEM_SUPPLY_OVERHEAD;
            if group
                .camps()
                .iter()
                .all(|c| c.kind() == cambc::CampType::GoldCluster)
            {
                gold_only += group.gold_value() as f32 * limit / cycle / 2.0;
            }
            let weight = if group
                .camps()
                .iter()
                .any(|c| c.kind() == cambc::CampType::Monolith)
            {
                MONOLITH_GEM_WEIGHT
            } else {
                1.0
            };
            supply += weight * group.gem_value() as f32 * limit / cycle / 2.0;
        }
        let rich = supply * GEM_SHARE_TAKEN >= GEM_WIN_TARGET;
        let gold_phase = gold_only >= GOLD_PHASE_MIN_SUPPLY;
        cambc::debug_block! {
            println!("gem phase: supply {} ours ~{} gold-only {} -> {} {}", supply as i32, (supply * GEM_SHARE_TAKEN) as i32, gold_only as i32, if rich { "rich" } else { "poor" }, if gold_phase { "" } else { "(no gold phase)" });
        }
        if !gold_phase {
            0
        } else if rich {
            GEM_PHASE_RICH_ROUND
        } else {
            GEM_PHASE_POOR_ROUND
        }
    };
    ROUND.with(|r| {
        let _ = r.set(round);
    });
    round
}
// In the gem phase, gold-only groups (gold clusters) are goals too while they're at most this many
// tiles further than the nearest gem goal: gold still buys items, so take it when it's on the way
// or the closest thing around. They join the candidates rather than replace them, so the target
// memory keeps the current target while we walk. 0: skip gold.
pub const GEM_PHASE_GOLD_DETOUR: i32 = 3;
// Groups we can't see (out of sight, or behind walls) are judged too, their camps at base stats
// (the camp-fight check counts the allies we see): only groups in sight were, so a champion walked
// to one, found it unwinnable, took the next nearest, walked there and found the same, and was back
// once the first one's loss memory ran out. On 01corridors (big groups, the way between neighbouring
// ones up one corridor and down the next) that was 53% of all turns walking and 3% hitting camps.
// A group found winnable isn't judged again for PRESUME_OK_ROUNDS rounds; one found unwinnable is
// dropped for PRESUME_LOSS_ROUNDS. With nothing left to farm, we call a rally for the nearest one
// we'd beat with a few allies. Only groups at least PRESUME_MIN_STEPS away on foot: a short walk
// is cheap, and judging on arrival counts the allies who are there by then (judging every unseen
// group lost 45-57 of 132 games, worst on small maps: 01hooks gold 2.0 against 7.6 a round).
// Off: judged once in sight. 2026-10-21, 132 games each against off: every unseen group 45 and
// 57 (after a fix); only 15+ steps away 58 (01corridors 5/6, 01islands 5/6, but 01hooks, 00lanes,
// 00interlock worse, and 60% more fuel-exhausted turns). Off; STRUGGLE_RALLY is what's used.
// 2026-10-23 (v6, with the faster allocator), on against off, 416 games head to head: 161/416
// (fuel-exhausted turns 6.5% against 3.1%); best 01corridors 7/8, 01starfish 7/8, 01hooks 6/8,
// 01islands 6/8, worst 00lost-world 0/8, 00split 0/8. Against bot-v5 on 01corridors alone 15/20
// (gems 534 against 318; off: 13/20, 284). The walking-detour ratio from our spawn doesn't tell the
// two kinds of map apart, so there's no per-map switch yet. Off.
pub const PRESUME_FAR_TARGETS: bool = false;
const PRESUME_MIN_STEPS: i32 = 15;
const PRESUME_OK_ROUNDS: u32 = 10;
const PRESUME_LOSS_ROUNDS: u32 = 30;
// With nothing we'd win in reach, call a rally for the nearest unwinnable group we can't see.
pub const FAR_RALLIES: bool = true;

thread_local! {
    static PRESUMED_OK_UNTIL: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Our stats changed (shopped, healed): judge groups afresh.
pub fn forget_presumed() {
    PRESUMED_OK_UNTIL.with(|p| p.borrow_mut().clear());
}

// Whether group `index`, which we can't see, would be a fight we lose (PRESUME_FAR_TARGETS).
fn presumed_unwinnable(index: usize, round: u32, far: impl FnOnce() -> bool) -> Result<bool> {
    if !PRESUME_FAR_TARGETS
        || PRESUMED_OK_UNTIL.with(|p| p.borrow().get(index).is_some_and(|&until| round < until))
        || !far()
    {
        return Ok(false);
    }
    if !super::collect::presumed_winnable(index)? {
        super::collect::extend_loss(index, round + PRESUME_LOSS_ROUNDS);
        super::trace::trace("presumed-unwinnable");
        return Ok(true);
    }
    PRESUMED_OK_UNTIL.with(|p| {
        let mut p = p.borrow_mut();
        if p.len() <= index {
            p.resize(index + 1, 0);
        }
        p[index] = round + PRESUME_OK_ROUNDS;
    });
    Ok(false)
}

// Nothing to farm and no rally on: call one for the nearest of `candidates` (unwinnable groups we
// can't see, with their gaps) we'd beat with up to rally::max_helpers more allies.
fn call_far_rally(candidates: &mut [(i32, usize)], round: u32) -> Result<()> {
    candidates.sort();
    for &(_, index) in candidates.iter().take(2) {
        if !super::rally::worth_checking(index, round) {
            continue;
        }
        let Some(max) = super::rally::max_helpers(&camp_groups()?[index], round)? else {
            continue;
        };
        for extra in 1..=max {
            if super::collect::winnable_with_standins(index, extra)? {
                super::trace::trace("far-rally-call");
                super::rally::call(index)?;
                if super::rally::active()? == Some(index) {
                    super::rally::set_need(extra)?;
                }
                return Ok(());
            }
        }
        super::rally::checked(index, round);
    }
    Ok(())
}
// Damaged camps within this many tiles are finished off first (see DAMAGED_CAMP_MAX_DETOUR).
const DAMAGED_CAMP_RADIUS: i32 = 8;

fn preferred_in_phase(kind: cambc::CampType, round: u32) -> bool {
    use cambc::CampType as C;
    if round < gem_phase_round() {
        !matches!(kind, C::GemCluster | C::Monolith)
    } else {
        kind != C::GoldCluster
    }
}

// Don't farm next to enemies we aren't expected to beat: skip groups a visible enemy could reach
// within FARM_ENEMY_TURNS turns (its attack range plus that much walking).
// Off since 2026-10-22: on the ladder we held our half but gave the middle away (50 losses: middle
// gems 232 a game against 524, elk groups 14 captures against 22, deaths even). Contesting instead
// of dropping won 176/320 and 178/320 against this on, and an opponent with it off beat us
// 173/320 (it also skips the fight view and duel plans the check needed: fuel-exhausted turns
// -9%). Lurking beside the group instead (LURK) and guarding respawns (RETURN_BIG) both lost.
pub const FARM_AVOID_ENEMIES: bool = false;
const FARM_ENEMY_TURNS: i32 = 2;
const FARM_ENEMY_DUEL_FUEL: u64 = 70_000;
// An enemy's 1v1 verdict (whether the groups it can reach stay goals) is kept this many rounds
// while both its health and ours stay in the same tenth of their maximum: working it out takes
// the fight view (~28k fuel: blocked tiles, a threat scan, a walking grid) plus a duel plan, on
// every plan with an enemy in sight, where reuse never applies (the plan key has every enemy's
// tile). Those were 20% of the turns that ran out of fuel. 0: worked out every plan.
pub const FARM_DUEL_KEEP_ROUNDS: u32 = 0;

thread_local! {
    // (enemy, round judged, our health tenth, its health tenth, winnable)
    static FARM_DUELS: std::cell::RefCell<Vec<(cambc::ChampionType, u32, i32, i32, bool)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn health_tenth(health: f32, max: f32) -> i32 {
    (10.0 * health / max.max(1.0)) as i32
}

// Where farming heads this turn (farm_camps).
pub struct FarmPlan {
    pub goals: Bitboard,
}

const FARM_TARGET_COOLDOWN_ROUNDS: u32 = 2;
// A farm target we find no walking path to is skipped for this many rounds: a pocket the region
// table doesn't know about, or a corridor units are standing in. Without it we kept the target
// and stood still.
const UNREACHABLE_TARGET_ROUNDS: u32 = 20;
const DAMAGED_CAMP_MAX_DETOUR: i32 = 3;

#[derive(Default)]
struct FarmTargetMemory {
    current: Option<Vec2>,
    abandoned: Vec<(Vec2, u32)>,
}

thread_local! {
    static FARM_TARGET_MEMORY: std::cell::RefCell<FarmTargetMemory> = std::cell::RefCell::new(FarmTargetMemory::default());
}

fn report_farm_target_change(
    previous: Option<Vec2>,
    target: Option<Vec2>,
    reason: &str,
    round: u32,
) {
    if !FARM_TARGET_DIAGNOSTICS || previous == target || previous.is_none() {
        return;
    }
    let point =
        |tile: Option<Vec2>| tile.map_or_else(|| "none".to_owned(), |p| format!("{},{}", p.x, p.y));
    // Free in local debug builds, skipped on the platform (debug_block).
    cambc::debug_block! {
        let who = me().and_then(|m| m.champion_name()).ok();
        println!(
            "farm-target-change {:?} r{} {} -> {} reason={}",
            who,
            round,
            point(previous),
            point(target),
            reason,
        );
    }
}

// Narrow `goals` to one target: keep the current one while it's still a goal, else the nearest on
// foot (`steps`) that isn't on cooldown.
// Fix 5: ranged champions (fragile carries) count camp groups about as close to the enemy's spawn
// as to ours (within CONTESTED_MARGIN tiles) as this many steps further when picking a target:
// both teams meet there (hedgerows: 41% of Prism's targets were the two middle cluster groups, and
// most of its deaths were on the way there or back). 0: off.
pub const CONTESTED_FARM_PENALTY: i32 = 8;
const CONTESTED_MARGIN: i32 = 2;

// Groups with elk or golems that a visible ally stands within NEAR_ALLY_TILES of count as
// NEAR_ALLY_BONUS steps closer when picking a target: of the elk fights we started alone we
// cleared 27-34%, with an ally 77-79% (42 traced games, 2026-10-21). 0: off.
// 2026-10-21, 132 games: 63-69 against off, and 18% more fuel-exhausted turns.
pub const NEAR_ALLY_BONUS: i32 = 0;
// Value pull (v6): a group counts VALUE_PULL_STEPS steps closer for each multiple of the median
// group's value (gold plus gems) above it, and further below it, at most VALUE_PULL_MAX either way.
// Champions spent 34% of their turns walking to camps and 8.7% hitting them (2026-10-23 local
// activity log, 104 games), and the nearest camp won whatever it paid. 0: off (nearest first).
// 2026-10-23, 4.0 against 0, 416 games head to head: 204/416 (gold at 300 -7%, at 500 -4%): the
// richer groups took longer to clear and were further away. Off.
pub const VALUE_PULL_STEPS: f32 = 0.0;
const VALUE_PULL_MAX: f32 = 12.0;
const NEAR_ALLY_TILES: i32 = 5;

// Pack (v4): until PACK_UNTIL_ROUND, a farm target counts as PACK_WEIGHT steps further for every
// tile it lies beyond PACK_RADIUS of the lead damage dealer (collect::publish_lead; the lead
// itself farms as usual): the team still takes the nearest camps, but drifts together instead of
// spreading over the map, so the fights it meets are fought by several of us. 0.0: off.
// 2026-10-21 against v3: 0.5 won 138/264 (6 seeds; gold +5.5%, gems tributed +8%, kills +20%,
// deaths -11%); 1.0 won 66/132.
// 2026-10-22 (v5, ladder: champions grouped on 00expanse where every camp is soloable), 0.0 against
// 0.5, one game a map, two seeds: 20/40 and 22/40, gold +1% and +20%, gems tributed +7% and +19%,
// gems lost on death 80 and 100 against 141 and 119. Off.
pub const PACK_WEIGHT: f32 = 0.0;
const PACK_RADIUS: i32 = 6;
const PACK_UNTIL_ROUND: u32 = 1200;

// Jackpot play in farming (jackpot.rs decides which groups are jackpots and whether it's on).
// A jackpot's camps count as JACKPOT_PULL_STEPS steps closer when picking a target, and are goals in
// both the gold and gem phases. 0: off.
pub const JACKPOT_PULL_STEPS: i32 = 12;
// A jackpot an enemy could reach stays a goal while we plus the allies within JACKPOT_CONTEST_TILES
// of it outnumber the enemies in sight within that of it, at least two of us (we'd otherwise give it
// up to whoever got there first). Off: dropped like any other group (enemy-can-reach-group).
pub const JACKPOT_CONTEST: bool = true;
const JACKPOT_CONTEST_TILES: i32 = 8;
// A respawning jackpot is a goal again once the walk there takes at least its remaining respawn time
// less JACKPOT_RETURN_MARGIN rounds, so we're there when it's back: the tiles beside its camps
// (standing on a camp's tile delays the whole group's respawn). Negative: off.
pub const JACKPOT_RETURN_MARGIN: i32 = 3;
// Only from this close: further away, farming on is worth more than the walk.
const JACKPOT_RETURN_TILES: i32 = 20;
// Lurk (third party): an elk or golem group an enemy we wouldn't beat one on one is fighting (in a
// live camp's range, the group awake) isn't just dropped: we wait just outside the camps' range on
// the far side from that enemy, the group between us, so the camps keep hitting it; the fight logic
// takes it once it's low, and the cleared group's capture draws it to us. On the ladder the groups
// we lost were elk groups (2026-10-22, 50 losses: top-20% groups 162 gems a game against 436),
// with deaths even. Off: dropped (enemy-can-reach-group). 2026-10-22, 320 games each: 150 against
// off; against an opponent that contests (FARM_AVOID_ENEMIES off) 136, where off won 147. Off (and
// with FARM_AVOID_ENEMIES off no group is dropped for enemies, so it never applies).
pub const LURK: bool = false;
// Only for groups within this many tiles, and with at least this share of our health.
const LURK_TILES: i32 = 12;
const LURK_MIN_HEALTH: f32 = 0.5;
// The ring we wait in: from just outside the camps' attack range to this many tiles further.
const LURK_SLACK: i32 = 2;
// Guard: respawning elk and golem groups are waited for like jackpots (JACKPOT_RETURN_MARGIN), not
// just jackpots. Off: jackpots only.
pub const RETURN_BIG: bool = false;
// Wait on the side of a respawning group facing the enemy's spawn, so they come through us to get
// it. Off: all round it. 2026-10-22, both on, 320 games: 169 against off; against a contesting
// opponent 128, where off won 147. Off.
pub const RETURN_ENEMY_SIDE: bool = false;
// Groups that run from our half into theirs (at least SPAN_MIN_CAMPS camps clearly on each side:
// 01yellow-brick-road's two 30-cluster gold lines): the opponent captured them from its end even
// when we'd cleared most of them (ladder, 2026-10-22: captures 3 against 16, 1 against 36).
// 0: off. 1: only our half's camps are goals (and the capture tiles we go to), and only enemies
// that could reach our half drop the group: we clear our end and hold its capture, which contests
// theirs. 2: as 1, but we never clear it, only hold the capture. 2026-10-22 on 01yellow-brick-road:
// 1 won 12/24 against 0; 2 won 14/24 and 21/32 (gold 12.0 a round against 10.9).
pub const SPAN_MODE: u8 = 2;
const SPAN_MIN_CAMPS: usize = 3;
// Leave a group of small camps (no elk, golem or monolith) to an ally already fighting it (within
// ALLY_SOLO_TILES of one of its live camps, at least ALLY_SOLO_HEALTH of its health) and look for
// another: two of us on a group one clears alone halves the gold a round and doubles the walking.
// Not when one of its camps is already in our attack range (passing by, hitting it costs nothing).
// Off: farm the nearest group whoever is on it. 2026-10-22, one game a map against off: 17/40,
// gold 5.20 a round against 5.77, clears 70.6 against 80.7 (the group was left, the next one
// further away). Off.
pub const ALLY_SOLO_LEAVE: bool = false;
const ALLY_SOLO_TILES: i32 = 2;
const ALLY_SOLO_HEALTH: f32 = 0.5;
// Groups nearer the enemy's spawn than ours, within ENEMY_SPAWN_AVOID_TILES of it, aren't farm
// goals before ENEMY_SPAWN_AVOID_UNTIL: the enemy respawns, shops and heals there (ladder,
// 2026-10-22: our champions were 5-10 tiles from it by round 41-48 on 01gold_rush). Jackpots
// still are. 0: off. 2026-10-22, 8 tiles, 320 games each against off: 165 and 154 (even). Off.
pub const ENEMY_SPAWN_AVOID_TILES: i32 = 0;
const ENEMY_SPAWN_AVOID_UNTIL: u32 = 2000;
// Grouping where the gems are concentrated (jackpot play on, jackpot.rs): the pack pull is
// PACK_WEIGHT_CONCENTRATED a tile, all game. 0.0: PACK_WEIGHT as everywhere else.
// 2026-10-22, 1.0 on the 8 jackpot maps: 70/160 against 0.0 (deaths 24.0 against 21.4). Off.
pub const PACK_WEIGHT_CONCENTRATED: f32 = 0.0;
// Jackpot play (the pull, contesting, waiting for its respawn, its rallies' precedence) starts at
// this round; before it a jackpot is farmed like any other group, so champions spread over the
// camps they can clear alone (on 01elk-arena the ladder opponent took the clusters all round the
// map while ours went one by one into the middle elk: 1,888 gold to 20,746, 2026-10-22). 0: from
// the start. Only on maps where at least JACKPOT_DELAY_CLUSTER_SHARE of the camps' gold a round
// comes from gold-cluster groups (soloable, all over the map): 01elk-arena 0.87, next 01hooks
// 0.56 and 00stampede 0.51. 2026-10-22 on the 8 jackpot maps (one seed, 16 games): 300 won 6/16,
// 600 6/16 (gold +28% and +14%), but 01elk-arena 4/4 while the gem jackpots (00gem-fields,
// 01concentrated-wealth, 00fragment, 00stampede) were mostly lost. With the share rule, 01elk-arena
// on fresh seeds: 2/4 against no delay (gold 6.7 a round against 6.1); 6/8 in all.
pub const JACKPOT_FROM_ROUND: u32 = 300;
const JACKPOT_DELAY_CLUSTER_SHARE: f32 = 0.7;

fn jackpot_live() -> bool {
    thread_local! {
        static DELAYED: OnceCell<bool> = const { OnceCell::new() };
    }
    let delayed = DELAYED.with(|d| {
        *d.get_or_init(|| {
            let (mut gold, mut clusters) = (0.0, 0.0);
            for group in camp_groups().unwrap_or(&[]) {
                let cycle = group.max_respawn_timer().max(0) as f32 + 20.0;
                let rate = group.gold_value() as f32 / cycle;
                gold += rate;
                if group
                    .camps()
                    .iter()
                    .all(|c| c.kind() == cambc::CampType::GoldCluster)
                {
                    clusters += rate;
                }
            }
            gold > 0.0 && clusters >= JACKPOT_DELAY_CLUSTER_SHARE * gold
        })
    });
    !delayed
        || JACKPOT_FROM_ROUND == 0
        || cambc::round_number().is_ok_and(|r| r >= JACKPOT_FROM_ROUND)
}
// Most extra allies a jackpot rally may count on (rally::RALLY_MAX_HELPERS for other groups).
const JACKPOT_MAX_HELPERS: usize = 4;
// A jackpot rally is joined when it's at most this many steps further than the nearest other camp
// (RALLY_WAIT_STEPS the other way for other rallies).
pub const JACKPOT_JOIN_EXTRA_STEPS: i32 = 25;

fn contested_penalty() -> Result<impl Fn(Vec2) -> i32> {
    let statics = farm_statics()?;
    let ranged = me()?.attack_range()? > 1;
    let live = jackpot_live();
    // Stronger than them: their side's groups come closer (press.rs).
    let pressing = super::press::pressing()?;
    let concentrated =
        PACK_WEIGHT_CONCENTRATED > 0.0 && super::jackpot::jackpots().is_ok_and(|j| j.on);
    let pack_weight = if concentrated {
        PACK_WEIGHT_CONCENTRATED
    } else {
        PACK_WEIGHT
    };
    let pack = if pack_weight > 0.0 && (concentrated || cambc::round_number()? < PACK_UNTIL_ROUND) {
        super::collect::pack_anchor()?
    } else {
        None
    };
    let mut allies = Vec::new();
    if NEAR_ALLY_BONUS > 0 {
        for ally in &sight()?.allies {
            if ally.is_alive().unwrap_or(false) && !ally.is_recalling().unwrap_or(true) {
                if let Ok(p) = ally.position() {
                    allies.push(p);
                }
            }
        }
    }
    Ok(move |tile: Vec2| {
        let index = match group_at(tile) {
            Ok(Some(index)) => index,
            _ => return 0,
        };
        let info = &statics.groups[index];
        let mut extra = 0;
        if CONTESTED_FARM_PENALTY > 0 && ranged && info.contested && !pressing {
            extra += CONTESTED_FARM_PENALTY;
        }
        if pressing && info.contested {
            extra -= super::press::PRESS_PULL_STEPS;
        }
        if info.big && allies.iter().any(|&a| distance(a, tile) <= NEAR_ALLY_TILES) {
            extra -= NEAR_ALLY_BONUS;
        }
        if info.jackpot && live {
            extra -= JACKPOT_PULL_STEPS;
        }
        if VALUE_PULL_STEPS > 0.0 && statics.median_value > 0.0 {
            let above = info.value / statics.median_value - 1.0;
            extra -= (above * VALUE_PULL_STEPS).clamp(-VALUE_PULL_MAX, VALUE_PULL_MAX) as i32;
        }
        if let Some(anchor) = pack {
            extra += ((distance(anchor, tile) - PACK_RADIUS).max(0) as f32 * pack_weight) as i32;
        }
        extra
    })
}

fn avoid_abandoned_farm_targets(
    goals: &mut Bitboard,
    round: u32,
    here: Vec2,
    reach: i32,
    unavailable_reason: Option<&'static str>,
    steps: &super::walk::StepsFrom,
) -> Result<()> {
    let penalty = contested_penalty()?;
    let candidates = goals.positions()?;
    let Some(candidates_for_target) = (!candidates.is_empty()).then_some(candidates) else {
        let previous = FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().current.take());
        report_farm_target_change(
            previous,
            None,
            unavailable_reason.unwrap_or("no-eligible-farm-goals"),
            round,
        );
        return Ok(());
    };

    let previous = FARM_TARGET_MEMORY.with(|memory| memory.borrow().current);
    let (target, changed_reason) = FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        memory.abandoned.retain(|&(_, until)| round < until);
        let previous_is_abandoned =
            previous.is_some_and(|old| memory.abandoned.iter().any(|&(tile, _)| tile == old));
        let available: Vec<Vec2> = candidates_for_target
            .iter()
            .copied()
            .filter(|&tile| !memory.abandoned.iter().any(|&(old, _)| old == tile))
            .collect();
        let target = if available.is_empty() {
            memory.abandoned.clear();
            steps.nearest_with(&candidates_for_target, &penalty)
        } else {
            let current_is_reachable = memory
                .current
                .is_some_and(|tile| distance(here, tile) <= reach);
            if !current_is_reachable {
                if let Some(current) = memory.current.filter(|tile| available.contains(tile)) {
                    Some(current)
                } else {
                    steps.nearest_with(&available, &penalty)
                }
            } else {
                steps.nearest_with(&available, &penalty)
            }
        };
        let reason = previous.filter(|&old| Some(old) != target).map(|old| {
            if !candidates_for_target.contains(&old) {
                unavailable_reason.unwrap_or("previous-target-not-in-farm-goals")
            } else if previous_is_abandoned {
                "previous-target-cooldown"
            } else if distance(here, old) <= reach {
                "previous-target-within-attack-range"
            } else {
                "closer-available-target"
            }
        });
        (target, reason)
    });

    FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        if memory.current != target {
            if let Some(previous) = memory.current {
                if distance(here, previous) > reach {
                    memory
                        .abandoned
                        .push((previous, round + FARM_TARGET_COOLDOWN_ROUNDS));
                }
            }
            memory.current = target;
        }
    });

    // The goals become just the target (clearing every other candidate a tile at a time cost two
    // host calls each); with no target they stay as they are and the cooldowns are dropped.
    match target {
        Some(tile) => {
            *goals = Bitboard::new()?;
            goals.set(tile, true)?;
        }
        None => FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().abandoned.clear()),
    }
    if let Some(reason) = changed_reason {
        report_farm_target_change(previous, target, reason, round);
    }
    Ok(())
}

thread_local! {
    static FARM_PLAN: std::cell::RefCell<Option<(u32, Vec2, std::rc::Rc<FarmPlan>)>> = const { std::cell::RefCell::new(None) };
}

// What farm_plan needs about each group that never changes, computed once. Pure Rust work is
// metered too (looping over every camp of every group several times a turn cost ~30k fuel), so a
// turn only looks at the camps of groups close enough to be in sight.
struct GroupInfo {
    centre: Vec2,
    radius: i32,            // furthest camp from the centre
    preferred: [bool; 2],   // has a camp preferred before / from FARM_GEM_PHASE_ROUND
    reachable: bool,        // we can get there: on foot, or with our dash (see regions.rs)
    gold_only: bool,        // only gold clusters (the Cluster opening, opening.rs)
    contested: bool, // about as close to the enemy's spawn as to ours (CONTESTED_FARM_PENALTY)
    big: bool,       // has elk or golems (NEAR_ALLY_BONUS)
    monolith: bool,  // has a monolith (RALLY_SIDE_STEPS)
    jackpot: bool,   // a jackpot (jackpot.rs, with jackpot play on)
    span: bool,      // runs from our half into theirs (SPAN_MODE)
    value: f32,      // gold plus gems its capture pays (VALUE_PULL_STEPS)
    near_enemy_spawn: bool, // ENEMY_SPAWN_AVOID_TILES
}

// Whether camp tile `tile` of a SPAN_MODE group is on our half (no further from our spawn).
fn our_half(tile: Vec2) -> Result<bool> {
    Ok(distance(tile, super::base::ally_spawn_centre()?)
        <= distance(tile, super::base::enemy_spawn_centre()?))
}

struct FarmStatics {
    groups: Vec<GroupInfo>,
    all: Bitboard,            // every camp's tile
    preferred: [Bitboard; 2], // camps of preferred groups, per phase
    median_value: f32,        // of the groups' values (VALUE_PULL_STEPS)
}

fn farm_statics() -> Result<&'static FarmStatics> {
    thread_local! {
        static STATICS: OnceCell<&'static FarmStatics> = const { OnceCell::new() };
    }
    if let Some(statics) = STATICS.with(|c| c.get().copied()) {
        return Ok(statics);
    }
    let mut statics = FarmStatics {
        groups: Vec::new(),
        all: Bitboard::new()?,
        preferred: [Bitboard::new()?, Bitboard::new()?],
        median_value: 0.0,
    };
    for (index, group) in camp_groups()?.iter().enumerate() {
        let n = group.camps().len().max(1) as i32;
        let (x, y) = group.camps().iter().fold((0, 0), |(x, y), c| {
            (x + c.spawn_position().x, y + c.spawn_position().y)
        });
        let centre = Vec2::new(x / n, y / n);
        let radius = group
            .camps()
            .iter()
            .map(|c| distance(c.spawn_position(), centre))
            .max()
            .unwrap_or(0);
        let mut preferred = [false; 2];
        for (phase, round) in [0, gem_phase_round()].into_iter().enumerate() {
            preferred[phase] = group
                .camps()
                .iter()
                .any(|c| preferred_in_phase(c.kind(), round));
        }
        // Jackpots are goals in both phases: the opponent takes them from round 0.
        let jackpot = super::jackpot::is_jackpot(index) && super::jackpot::in_squad();
        if jackpot && JACKPOT_PULL_STEPS > 0 {
            preferred = [true, true];
        }
        // Groups we can't get to are never goals: on ambush, champions stood still for the rest of
        // the game with a camp in a walled-off room as their target.
        let reachable = crate::regions::reachable_for_me(index)?;
        let (ally, enemy) = (
            super::base::ally_spawn_centre()?,
            super::base::enemy_spawn_centre()?,
        );
        let clearly = |near: Vec2, far: Vec2| {
            group
                .camps()
                .iter()
                .filter(|c| {
                    let p = c.spawn_position();
                    (distance(p, near) as f32) < distance(p, far) as f32 * 0.85
                })
                .count()
        };
        let span = SPAN_MODE > 0
            && clearly(ally, enemy) >= SPAN_MIN_CAMPS
            && clearly(enemy, ally) >= SPAN_MIN_CAMPS;
        for camp in group.camps().iter().filter(|_| reachable) {
            // A spanning group: our half only (SPAN_MODE 1), or none of it (2).
            if span && (SPAN_MODE >= 2 || !our_half(camp.spawn_position())?) {
                continue;
            }
            statics.all.set(camp.spawn_position(), true)?;
            for phase in 0..2 {
                if preferred[phase] {
                    statics.preferred[phase].set(camp.spawn_position(), true)?;
                }
            }
        }
        statics.groups.push(GroupInfo {
            centre,
            radius,
            preferred,
            reachable,
            gold_only: group
                .camps()
                .iter()
                .all(|c| c.kind() == cambc::CampType::GoldCluster),
            contested: distance(centre, super::base::enemy_spawn_centre()?)
                <= distance(centre, super::base::ally_spawn_centre()?) + CONTESTED_MARGIN,
            big: group
                .camps()
                .iter()
                .any(|c| matches!(c.kind(), cambc::CampType::Elk | cambc::CampType::Golem)),
            monolith: group
                .camps()
                .iter()
                .any(|c| c.kind() == cambc::CampType::Monolith),
            jackpot,
            span,
            value: (group.gold_value() + group.gem_value()) as f32,
            near_enemy_spawn: ENEMY_SPAWN_AVOID_TILES > 0
                && distance(centre, enemy) - radius <= ENEMY_SPAWN_AVOID_TILES
                && distance(centre, enemy) < distance(centre, ally),
        });
    }
    let mut values: Vec<f32> = statics.groups.iter().map(|g| g.value).collect();
    values.sort_by(f32::total_cmp);
    statics.median_value = values.get(values.len() / 2).copied().unwrap_or(0.0);
    let statics: &'static FarmStatics = Box::leak(Box::new(statics));
    STATICS.with(|c| {
        let _ = c.set(statics);
    });
    Ok(statics)
}

// A farm plan is reused for up to PLAN_REUSE_ROUNDS rounds while what it depends on in sight is
// unchanged: our tile and health, the gold/gem phase, the visible camps and their health, and
// where the visible enemies and allies are. Building one costs ~30k fuel, and we stand still on
// many turns (waiting on a capture or a respawn, or holding back). The age limit covers what isn't
// in the key: respawn timers, judged-lost camps and target cooldowns.
const PLAN_REUSE_ROUNDS: u32 = 5;

#[derive(PartialEq, Clone)]
struct PlanKey {
    claims: u32,
    news: u32, // respawn messages taken in (respawns.rs)
    rally: Option<usize>,
    opening: (bool, Option<usize>, bool), // Group on, its team target, Cluster on
    here: Vec2,
    hp: i32,
    phase: bool,
    camps: Vec<(Vec2, i32)>,
    enemies: Vec<Vec2>,
    allies: Vec<Vec2>,
}

// Fuel fix 1: a rally's join-or-skip decision (the steps to it plus RALLY_WAIT_STEPS against the
// nearest other camp) is kept this many rounds instead of worked out every plan (~22k fuel each
// time a rally was active). 0: every plan. Alone on 8 rally maps: 12% fewer fuel-exhausted turns;
// the kept decision differed from a fresh one on 1.7% of uses.
pub const RALLY_DECISION_ROUNDS: u32 = 5;

// Side camps: while we wait at a rally (within RALLY_AT_STEPS of it, nobody fighting it yet), a
// camp group within RALLY_SIDE_STEPS on foot comes first, whatever the gold/gem phase, unless it
// has elk, golems or a monolith: a cluster or wolf respawning next to the rally is cleared while
// allies are still walking over, and the rally (still on, or called again) pulls us back after.
// Champions stood at rallies next to such camps: the cached join decision (RALLY_DECISION_ROUNDS)
// missed camps respawning in the meantime, and the phase filter hid off-phase clusters (in 44
// games, 745 waiting turns with a live camp within 8 tiles). Checked afresh every plan. 0: off.
pub const RALLY_SIDE_STEPS: i32 = 8;
const RALLY_AT_STEPS: i32 = 2;

thread_local! {
    // (rally group, round the decision expires, whether we join)
    static RALLY_DECISION: std::cell::Cell<Option<(usize, u32, bool)>> = const { std::cell::Cell::new(None) };
}

thread_local! {
    static PLAN_REUSE: std::cell::RefCell<Option<(u32, PlanKey, std::rc::Rc<FarmPlan>)>> =
        const { std::cell::RefCell::new(None) };
}

// Rally waiting: a plan that sent us to a rally is kept for up to RALLY_REUSE_ROUNDS rounds while
// that rally is still on, we haven't moved (we're there, waiting) and no enemy is in sight. The
// plan key holds every ally's tile, and allies walking in changed it every turn, so a champion
// standing at a rally rebuilt its plan (~50k fuel) every turn: 36% of the turns that ran out of
// fuel had a rally on, and none of them reused a plan (2026-10-22, 40 traced games). 0: off.
// 2026-10-22, 5 rounds, one game a map against off: 16/40, fuel-exhausted turns unchanged (323
// against 322 per 1,000 rounds): the waiting champions weren't where the fuel went. Off.
pub const RALLY_REUSE_ROUNDS: u32 = 0;

thread_local! {
    // (round built, rally group, our tile, plan)
    static RALLY_PLAN: std::cell::RefCell<Option<(u32, usize, Vec2, std::rc::Rc<FarmPlan>)>> =
        const { std::cell::RefCell::new(None) };
}

// Walking reuse: on the way to goals out of sight, the last plan is kept while nothing it looks at
// in sight has changed (the camp tiles we see and the live camps' health, the rally, the team
// target, our health, the phase) and no enemy is in sight, for up to WALK_REUSE_ROUNDS rounds. The
// key above includes our tile, so a plan (~36k fuel on average, ~60k at the 90th percentile, of a
// 100k turn) was rebuilt on 93% of farming turns, mostly while walking.
pub const WALK_REUSE: bool = true;
const WALK_REUSE_ROUNDS: u32 = 10;

struct WalkSnapshot {
    built: u32,
    key: PlanKey,
    seen: Bitboard, // camp tiles in sight, live or not (a cleared group coming into view)
    plan: std::rc::Rc<FarmPlan>,
}

thread_local! {
    static WALK_PLAN: std::cell::RefCell<Option<WalkSnapshot>> = const { std::cell::RefCell::new(None) };
}

fn seen_camp_tiles() -> Result<Bitboard> {
    sight()?.visible.intersect(&farm_statics()?.all)
}

// Whether any of `goals` is in sight.
fn goals_in_sight(goals: &Bitboard) -> Result<bool> {
    Ok(!goals.intersect(&sight()?.visible)?.is_empty()?)
}

fn walking_reuse(round: u32, key: &PlanKey) -> Result<Option<std::rc::Rc<FarmPlan>>> {
    if !WALK_REUSE || !key.enemies.is_empty() {
        return Ok(None);
    }
    let candidate = WALK_PLAN.with(|c| {
        c.borrow().as_ref().and_then(|s| {
            let k = &s.key;
            (round < s.built + WALK_REUSE_ROUNDS
                && k.enemies.is_empty()
                && (k.rally, k.opening, k.hp, k.phase, k.news)
                    == (key.rally, key.opening, key.hp, key.phase, key.news)
                && k.camps == key.camps)
                .then(|| (s.seen.clone(), s.plan.clone()))
        })
    });
    let Some((seen, plan)) = candidate else {
        return Ok(None);
    };
    // Goals in sight need a fresh plan (fight checks, captures); so do new camp tiles in sight.
    if goals_in_sight(&plan.goals)? || seen != seen_camp_tiles()? {
        return Ok(None);
    }
    Ok(Some(plan))
}

fn plan_key(here: Vec2, round: u32) -> Result<PlanKey> {
    let sight = sight()?;
    let mut camps = Vec::with_capacity(sight.camps.len());
    for camp in &sight.camps {
        camps.push((camp.spawn_position(), camp.health()? as i32));
    }
    let mut enemies = Vec::with_capacity(sight.enemies.len());
    for enemy in &sight.enemies {
        enemies.push(enemy.position()?);
    }
    let mut allies = Vec::with_capacity(sight.allies.len());
    for ally in &sight.allies {
        allies.push(ally.position()?);
    }
    let group = crate::opening::group_active()?;
    Ok(PlanKey {
        claims: super::claims::bits()?,
        news: super::respawns::news(),
        rally: super::rally::active()?,
        opening: (
            group,
            if group {
                crate::opening::team_target()?
            } else {
                None
            },
            crate::opening::cluster_active()?,
        ),
        here,
        hp: me()?.health()? as i32,
        phase: round >= gem_phase_round(),
        camps,
        enemies,
        allies,
    })
}

// Far-group cache (see the group loop in farm_plan): the bookkeeping for groups well out of sight
// is kept for up to FAR_GROUP_CACHE_ROUNDS rounds while we've moved at most FAR_CACHE_MOVE tiles
// and no enemy is in reach, and dropped as soon as any of those groups' respawn timer or loss
// memory runs out (those timers are all that can change a group we can't see). Even the fast path
// cost ~250 fuel a group, 12-15k a plan on maps with 50-60 groups. 0: off.
pub const FAR_GROUP_CACHE_ROUNDS: u32 = 5;
const FAR_CACHE_MOVE: i32 = 3;

#[derive(Clone)]
struct FarCache {
    claims: u32,
    news: u32,
    origin: Vec2,
    phase: usize,
    cluster_mode: bool,
    valid_until: u32,
    far: Vec<bool>,
    dropped: Vec<usize>,
    available: Vec<usize>,
    clusters: Vec<(usize, i32)>,
    preferred: u32,
    off_phase: Vec<(usize, i32)>,
}

thread_local! {
    static FAR_CACHE: std::cell::RefCell<Option<FarCache>> = const { std::cell::RefCell::new(None) };
}

// Drop the far-group cache (the loss memory it relied on was just cleared, collect::forget_lost_camps).
pub fn forget_far_cache() {
    FAR_CACHE.with(|c| *c.borrow_mut() = None);
}

// Fuel fix 4 (see the group loop in farm_plan). Off: every group takes the full pass. Alone on the
// 6 maps with the most groups: 14% fewer fuel-exhausted turns (group loop 23k -> 20k there).
pub const FAR_GROUP_FAST_PATH: bool = true;

// Plan farming: the nearest camps that aren't respawning (including ones we saw respawning
// earlier), skipping groups we'd lose to (collect::CAMP_FIGHT) or that enemies are next to.
// Rewards in sight and elk rallies come first, and a capture we're standing on is held. Tanks and
// healers still fight camps but leave the capture of a cleared group to a covering AD carry (see
// TANK_FUNNEL). Cached per round and position.
pub fn farm_plan() -> Result<std::rc::Rc<FarmPlan>> {
    let here = me()?.position()?;
    let round = cambc::round_number()?;
    if let Some(plan) = FARM_PLAN.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(r, p, _)| (*r, *p) == (round, here))
            .map(|(_, _, p)| p.clone())
    }) {
        return Ok(plan);
    }
    // A plan from the last few rounds, if nothing it looks at has changed since. A failed read
    // (a unit can drop out of vision mid-turn) just means building a new plan.
    if RALLY_REUSE_ROUNDS > 0 && sight()?.enemies.is_empty() {
        let rally = super::rally::active()?;
        let kept = RALLY_PLAN.with(|c| {
            c.borrow()
                .as_ref()
                .filter(|(built, group, at, _)| {
                    round < built + RALLY_REUSE_ROUNDS && Some(*group) == rally && *at == here
                })
                .map(|(_, _, _, plan)| plan.clone())
        });
        if let Some(plan) = kept {
            FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
            super::trace::trace("farm-plan-rally-kept");
            return Ok(plan);
        }
    }
    let key = plan_key(here, round).ok();
    if let Some(plan) = key.as_ref().and_then(|key| {
        PLAN_REUSE.with(|c| {
            c.borrow()
                .as_ref()
                .filter(|(built, k, _)| round < built + PLAN_REUSE_ROUNDS && k == key)
                .map(|(_, _, p)| p.clone())
        })
    }) {
        FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
        super::trace::trace("farm-plan-reused");
        return Ok(plan);
    }
    if let Some(key) = &key {
        if let Some(plan) = walking_reuse(round, key)? {
            FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
            super::trace::trace("farm-plan-walking");
            return Ok(plan);
        }
    }
    checkpoint("plan-key");
    let statics = farm_statics()?;
    let class = crate::constants::class_of(me()?.champion_name()?);
    let is_tank = funnels(class);
    let is_melee_funnel = funnels_to_gold_carry(class);
    let is_gold_carry = FUNNEL_MELEE && gold_carry()?;
    let sight = sight()?;
    let groups = camp_groups()?;
    // Enemy reach: (position, attack range + FARM_ENEMY_TURNS turns of walking).
    let mut potential_enemy_reach = Vec::new();
    if FARM_AVOID_ENEMIES {
        for enemy in &sight.enemies {
            potential_enemy_reach.push((
                enemy.clone(),
                enemy.position()?,
                enemy.attack_range()? + enemy.base_stats()?.base_move_speed * FARM_ENEMY_TURNS,
            ));
        }
    }
    let potentially_threatens_farm = potential_enemy_reach.iter().any(|(_, position, reach)| {
        groups.iter().any(|group| {
            group
                .camps()
                .iter()
                .any(|camp| distance(*position, camp.spawn_position()) <= *reach)
        })
    });
    // Kept verdicts first (FARM_DUEL_KEEP_ROUNDS): the fight view is only needed for the rest.
    let ours = health_tenth(me()?.health()?, me()?.max_health()?);
    let mut kept = Vec::with_capacity(potential_enemy_reach.len());
    for (enemy, _, _) in &potential_enemy_reach {
        let verdict = if FARM_DUEL_KEEP_ROUNDS > 0 && potentially_threatens_farm {
            let (name, theirs) = (
                enemy.champion_name()?,
                health_tenth(enemy.health()?, enemy.max_health()?),
            );
            FARM_DUELS.with(|d| {
                d.borrow()
                    .iter()
                    .find(|&&(n, at, o, t, _)| {
                        n == name && round < at + FARM_DUEL_KEEP_ROUNDS && (o, t) == (ours, theirs)
                    })
                    .map(|&(_, _, _, _, winnable)| winnable)
            })
        } else {
            None
        };
        kept.push(verdict);
    }
    let duel_view = if potentially_threatens_farm && kept.iter().any(Option::is_none) {
        match cached_fight_view()? {
            Some(view) => Some(view),
            None if cambc::fuel_remaining()? >= FARM_ENEMY_DUEL_FUEL => Some(fight_view()?),
            None => None,
        }
    } else {
        None
    };
    let mut enemy_reach = Vec::new();
    for ((enemy, position, reach), kept) in potential_enemy_reach.into_iter().zip(kept) {
        let enemy_is_winnable = if let Some(winnable) = kept {
            super::trace::trace("farm-duel-kept");
            winnable
        } else if let Some(view) = &duel_view {
            let winnable = plan_duel(&enemy, &view.threats, &view.allies, &view.from_me)?
                .is_some_and(|duel| duel.winnable);
            if FARM_DUEL_KEEP_ROUNDS > 0 {
                let (name, theirs) = (
                    enemy.champion_name()?,
                    health_tenth(enemy.health()?, enemy.max_health()?),
                );
                FARM_DUELS.with(|d| {
                    let mut d = d.borrow_mut();
                    d.retain(|&(n, _, _, _, _)| n != name);
                    d.push((name, round, ours, theirs, winnable));
                });
            }
            winnable
        } else {
            false
        };
        if !enemy_is_winnable {
            enemy_reach.push((position, reach));
        }
    }
    checkpoint("plan-enemies");
    // Who's around a jackpot we might contest (JACKPOT_CONTEST): allies on their feet, enemies seen.
    let mut contest_allies = Vec::new();
    let mut contest_enemies = Vec::new();
    if JACKPOT_CONTEST && !enemy_reach.is_empty() {
        for ally in &sight.allies {
            if ally.is_alive().unwrap_or(false) && !ally.is_recalling().unwrap_or(true) {
                if let Ok(p) = ally.position() {
                    contest_allies.push(p);
                }
            }
        }
        for enemy in &sight.enemies {
            if let Ok(p) = enemy.position() {
                contest_enemies.push(p);
            }
        }
    }
    let contest_ok = |index: usize| -> bool {
        let info = &statics.groups[index];
        if !JACKPOT_CONTEST || !info.jackpot || !jackpot_live() {
            return false;
        }
        let near = |p: &&Vec2| distance(**p, info.centre) <= JACKPOT_CONTEST_TILES + info.radius;
        let ours = 1 + contest_allies.iter().filter(near).count();
        let theirs = contest_enemies.iter().filter(near).count();
        // Only enemies we wouldn't beat one on one get here (enemy_reach), so it takes an ally and
        // more of us: alone (or even), champions held 00gemfight's middle against such an enemy and
        // died there (20-22 deaths a game against 1-4, 2 wins in 8, 2026-10-22).
        ours >= 2 && ours > theirs
    };
    let max_reach = enemy_reach.iter().map(|&(_, r)| r).max().unwrap_or(0);
    let vision = me()?.base_stats()?.vision_radius as i32;
    let phase = usize::from(round >= gem_phase_round());
    let previous_target = FARM_TARGET_MEMORY.with(|memory| memory.borrow().current);
    let previous_group = match previous_target {
        Some(tile) => group_at(tile)?,
        None => None,
    };
    let mut previous_target_reason = None;
    let mut damaged_camps = Vec::new();
    for camp in &sight.camps {
        if camp.health()? < camp.max_health()? {
            if let Some(index) = group_at(camp.spawn_position())? {
                damaged_camps.push((index, camp.spawn_position()));
            }
        }
    }
    let mut ready = RESPAWN_READY.with(|cell| cell.take());
    ready.resize(groups.len(), 0);
    let mut available = vec![false; groups.len()];
    let mut dropped = Vec::new(); // groups that aren't goals this turn
    let mut far_unwinnable = Vec::new(); // (gap, group) we can't see and would lose to
                                         // Ranking goals by walking distance (walk.rs): built on the first lookup, if any.
    let steps = super::walk::StepsFrom::new(here);
    // Whether group `index` is at least PRESUME_MIN_STEPS away on foot (PRESUME_FAR_TARGETS).
    let far_walk = |index: usize| {
        groups[index]
            .camps()
            .iter()
            .map(|c| c.spawn_position())
            .min_by_key(|&p| distance(p, here))
            .is_some_and(|t| steps.steps(t) >= PRESUME_MIN_STEPS)
    };
    let mut not_goals = Vec::new(); // dead camps of a group that's still partly alive
    let mut damaged = Vec::new();
    let mut rewards = Vec::new(); // capture tiles of cleared groups in sight, reward waiting
    let rally = super::rally::active()?;
    let mut rally_tiles = Vec::new(); // live camps of the rally group (rally.rs)
    let mut off_phase = Vec::new(); // available groups the phase skips: (index, nearest camp)
    let mut preferred_left = 0;
    let mut holding = false;
    // The opening (opening.rs): in Group mode the team farms one shared target group and funnels
    // captures to a damage dealer; in Cluster mode champions head for gold clusters.
    let opening_group = crate::opening::group_active()?;
    let opening_cluster = crate::opening::cluster_active()?;
    // The rally and team target as they stand, re-read only when this loop changes them (reading
    // them per group cost a host call each).
    let mut rally_now = rally;
    let mut team_now = if opening_group {
        crate::opening::team_target()?
    } else {
        None
    };
    let mut team_tiles = Vec::new(); // live camps of the team target group
    let mut clusters = Vec::new(); // available gold-cluster groups: (index, nearest camp)
                                   // Respawning jackpots we'd reach about when they're back (JACKPOT_RETURN_MARGIN).
    let mut returning: Vec<usize> = Vec::new();
    // An ally is fighting group `index` and it's small enough to leave to it (ALLY_SOLO_LEAVE).
    let my_range = me()?.attack_range()?;
    let ally_solos = |index: usize| -> Result<bool> {
        let mut live = Vec::new();
        for camp in &sight.camps {
            if group_at(camp.spawn_position())? == Some(index) {
                live.push(camp.spawn_position());
            }
        }
        if live.iter().any(|&c| distance(c, here) <= my_range) {
            return Ok(false);
        }
        for ally in &sight.allies {
            if !ally.is_alive()? || ally.is_recalling()? {
                continue;
            }
            if ally.health()? < ALLY_SOLO_HEALTH * ally.max_health()? {
                continue;
            }
            let p = ally.position()?;
            if live.iter().any(|&c| distance(c, p) <= ALLY_SOLO_TILES) {
                return Ok(true);
            }
        }
        Ok(false)
    };
    // Elk groups an enemy is fighting that we'd wait beside (LURK): (gap, group).
    let mut lurk: Vec<(i32, usize)> = Vec::new();
    let lurk_on = |index: usize, reach: &[(Vec2, i32)]| -> Result<bool> {
        if me()?.health()? < LURK_MIN_HEALTH * me()?.max_health()? {
            return Ok(false);
        }
        for camp in &sight.camps {
            if group_at(camp.spawn_position())? != Some(index) || !camp.is_awake().unwrap_or(false)
            {
                continue;
            }
            let range = camp.attack_range()?;
            if reach
                .iter()
                .any(|&(p, _)| distance(p, camp.spawn_position()) <= range + 1)
            {
                return Ok(true);
            }
        }
        Ok(false)
    };
    let returns = |index: usize, gap: i32, back: u32| -> bool {
        JACKPOT_RETURN_MARGIN >= 0
            && ((statics.groups[index].jackpot && jackpot_live())
                || (RETURN_BIG && statics.groups[index].big))
            && gap <= JACKPOT_RETURN_TILES
            && back.saturating_sub(round) as i32 <= gap.max(0) + JACKPOT_RETURN_MARGIN
    };
    // The far-group cache (FAR_GROUP_CACHE_ROUNDS): use it if still good, else build a fresh one.
    let cache_usable = FAR_GROUP_CACHE_ROUNDS > 0 && FAR_GROUP_FAST_PATH && enemy_reach.is_empty();
    let claim_bits = super::claims::bits()?;
    let far_cache = if cache_usable {
        FAR_CACHE.with(|c| c.borrow().clone()).filter(|c| {
            round < c.valid_until
                && c.claims == claim_bits
                && c.news == super::respawns::news()
                && distance(c.origin, here) <= FAR_CACHE_MOVE
                && c.phase == phase
                && c.cluster_mode == opening_cluster
                && rally_now.is_none_or(|g| !c.far.get(g).copied().unwrap_or(false))
                && team_now.is_none_or(|g| !c.far.get(g).copied().unwrap_or(false))
        })
    } else {
        None
    };
    let mut fresh = (cache_usable && far_cache.is_none()).then(|| FarCache {
        claims: claim_bits,
        news: super::respawns::news(),
        origin: here,
        phase,
        cluster_mode: opening_cluster,
        valid_until: round + FAR_GROUP_CACHE_ROUNDS,
        far: vec![false; groups.len()],
        dropped: Vec::new(),
        available: Vec::new(),
        clusters: Vec::new(),
        preferred: 0,
        off_phase: Vec::new(),
    });
    for (index, group) in groups.iter().enumerate() {
        let info = &statics.groups[index];
        if !info.reachable {
            continue;
        }
        // A group we just fled (safety::FLEE_COMMIT_ROUNDS) is no goal, not even as the rally.
        if super::safety::fled(index) {
            if previous_group == Some(index) {
                previous_target_reason = Some("fled-group");
            }
            dropped.push(index);
            continue;
        }
        if info.near_enemy_spawn && !info.jackpot && round < ENEMY_SPAWN_AVOID_UNTIL {
            if previous_group == Some(index) {
                previous_target_reason = Some("near-enemy-spawn");
            }
            dropped.push(index);
            continue;
        }
        if far_cache.as_ref().is_some_and(|c| c.far[index]) {
            continue;
        }
        // Nearest any of its camps can be.
        let gap = distance(info.centre, here) - info.radius;
        // A solo group an ally is already farming (claims.rs).
        if claim_bits != 0 && rally != Some(index) && super::claims::claimed_by_ally(index, gap)? {
            if previous_group == Some(index) {
                previous_target_reason = Some("claimed-by-ally");
            }
            dropped.push(index);
            continue;
        }
        // Fuel fix 4: a group out of sight, not the rally or team target and out of any enemy's
        // reach only needs its respawn and loss memory checked and its goal bookkeeping done; the
        // full pass below costs ~500 fuel a group, 15-25k a plan on maps with 40-50 groups.
        if FAR_GROUP_FAST_PATH
            && gap > vision
            && gap > DAMAGED_CAMP_RADIUS
            && (enemy_reach.is_empty() || gap > max_reach + vision)
            && rally != Some(index)
            && rally_now != Some(index)
            && team_now != Some(index)
        {
            let respawning = round < ready[index];
            if respawning && returns(index, gap, ready[index]) {
                returning.push(index);
            }
            // Record it in a fresh far-group cache if it'll stay far while we move FAR_CACHE_MOVE.
            // Not a jackpot: its return window (JACKPOT_RETURN_MARGIN) opens before it's back.
            let cached = fresh.as_mut().filter(|_| {
                gap > vision + FAR_CACHE_MOVE
                    && gap > DAMAGED_CAMP_RADIUS + FAR_CACHE_MOVE
                    && rally != Some(index)
                    && !info.jackpot
                    && !(RETURN_BIG && info.big)
            });
            let lost = !respawning
                && (super::collect::recently_lost(index, round)
                    || presumed_unwinnable(index, round, || far_walk(index))?);
            if lost {
                far_unwinnable.push((gap, index));
            }
            if respawning || lost {
                if previous_group == Some(index) {
                    previous_target_reason = Some(if respawning {
                        "group-respawning"
                    } else {
                        "camp-fight-unwinnable"
                    });
                }
                dropped.push(index);
                if let Some(c) = cached {
                    c.far[index] = true;
                    c.dropped.push(index);
                    let until = if respawning {
                        ready[index]
                    } else {
                        super::collect::lost_until(index)
                    };
                    c.valid_until = c.valid_until.min(until.max(round + 1));
                }
                continue;
            }
            available[index] = true;
            if opening_cluster && info.gold_only {
                clusters.push((index, gap));
            }
            if info.preferred[phase] {
                preferred_left += 1;
            } else {
                off_phase.push((index, gap));
            }
            if let Some(c) = cached {
                c.far[index] = true;
                c.available.push(index);
                if opening_cluster && info.gold_only {
                    c.clusters.push((index, gap));
                }
                if info.preferred[phase] {
                    c.preferred += 1;
                } else {
                    c.off_phase.push((index, gap));
                }
            }
            continue;
        }
        let mut seen = false;
        let mut any_live = false;
        let mut seen_dead = Vec::new(); // camps we can see are gone
        if gap <= vision {
            for camp in group.camps() {
                let spawn = camp.spawn_position();
                if !sight.visible.is_set(spawn)? {
                    continue;
                }
                seen = true;
                if sight.camps.iter().any(|c| c.spawn_position() == spawn) {
                    any_live = true;
                } else {
                    seen_dead.push(spawn);
                }
            }
        }
        if any_live {
            ready[index] = 0;
        }
        // A cleared group in sight: what we see beats a message (respawns.rs). Post the round it's
        // back if we didn't know it was respawning, or if we're on its capture.
        let seen_state = if seen && !any_live {
            Some(group.state()?)
        } else {
            None
        };
        if super::respawns::RESPAWN_SHARE {
            match seen_state {
                Some(cambc::CampGroupState::Respawning(timer)) => {
                    let back = round + timer.max(0) as u32;
                    if ready[index] + 2 < back {
                        super::respawns::post(index, back)?;
                    }
                    ready[index] = back;
                }
                Some(cambc::CampGroupState::RewardAvailable) => {
                    // An ally on the capture already posted it.
                    let posted = round < ready[index];
                    ready[index] = 0;
                    if !posted && group.camps().iter().any(|c| c.spawn_position() == here) {
                        super::respawns::post_capture(index, round)?;
                    }
                }
                _ => {}
            }
        }
        // A rally ends once its group is cleared (reward waiting) or respawning.
        if rally == Some(index) && ((seen && !any_live) || round < ready[index]) {
            super::rally::end(index)?;
            rally_now = super::rally::active()?;
        }
        // So does the team target; whoever plans next sets a new one.
        if team_now == Some(index) && ((seen && !any_live) || round < ready[index]) {
            crate::opening::end_team_target(index)?;
            team_now = crate::opening::team_target()?;
        }
        if round < ready[index] {
            if returns(index, gap, ready[index]) {
                returning.push(index);
            }
            if previous_group == Some(index) {
                previous_target_reason = Some("group-respawning");
            }
            dropped.push(index);
            continue;
        }
        // Only a seen group with no live camps can be respawning or waiting for capture.
        let mut reward = false;
        if let Some(state) = seen_state {
            match state {
                cambc::CampGroupState::Respawning(timer) => {
                    ready[index] = round + timer.max(0) as u32;
                    if returns(index, gap, ready[index]) {
                        returning.push(index);
                    }
                    if previous_group == Some(index) {
                        previous_target_reason = Some("group-respawning");
                    }
                    dropped.push(index);
                    continue;
                }
                cambc::CampGroupState::RewardAvailable
                    if opening_group && crate::opening::funnel_elsewhere(group)? =>
                {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("opening-funnel");
                    }
                    dropped.push(index);
                    continue;
                }
                // Gem-carry maps: the carry takes it (gemcarry.rs).
                cambc::CampGroupState::RewardAvailable
                    if super::gemcarry::leaves_to_carry(group)? =>
                {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("gem-carry-funnel");
                    }
                    dropped.push(index);
                    continue;
                }
                // Another champion is heading for it (capclaim.rs).
                cambc::CampGroupState::RewardAvailable
                    if super::capclaim::claimed_by_other(index, round)? =>
                {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("capture-claimed");
                    }
                    dropped.push(index);
                    continue;
                }
                // One capturer per group (camps::CAPTURE_CLAIM), in place of the two rules below.
                cambc::CampGroupState::RewardAvailable
                    if CAPTURE_CLAIM && claimed_by_other(group)? =>
                {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("capture-claim-other");
                    }
                    dropped.push(index);
                    continue;
                }
                cambc::CampGroupState::RewardAvailable
                    if !CAPTURE_CLAIM
                        && !super::gemcarry::am_carry()
                        && ((is_tank && carry_covers(group, false)?)
                            || (is_melee_funnel && carry_covers(group, true)?)) =>
                {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("ally-carry-captures-reward");
                    }
                    dropped.push(index);
                    continue;
                }
                // An ally already on the capture region is collecting this reward; don't path to
                // the occupied capture tile or compete for the split.
                cambc::CampGroupState::RewardAvailable
                    if !CAPTURE_CLAIM
                        && !super::gemcarry::am_carry()
                        && ally_on_group(group, is_gold_carry)? =>
                {
                    if previous_group == Some(index) {
                        previous_target_reason = Some("ally-captures-reward");
                    }
                    dropped.push(index);
                    continue;
                }
                cambc::CampGroupState::RewardAvailable => reward = true,
                _ => {}
            }
        }
        // A cleared group has nothing left to fight: a loss judged while it was alive doesn't count.
        // Some members dead but the group not cleared: the rest are alive out of our sight (a bush).
        let hidden_alive = super::collect::PARTIAL_GROUPS && seen && !any_live && !reward;
        let partly = super::collect::PARTIAL_GROUPS
            && (!seen_dead.is_empty() || damaged_camps.iter().any(|&(g, _)| g == index));
        let mut unwinnable = !reward
            && ((super::collect::recently_lost(index, round) && !partly)
                || (any_live && super::collect::camp_fight_winnable(index)? == Some(false))
                || (!seen
                    && rally_now != Some(index)
                    && team_now != Some(index)
                    && presumed_unwinnable(index, round, || far_walk(index))?));
        if unwinnable && !seen {
            far_unwinnable.push((gap, index));
        }
        // An elk or monolith group we'd beat with a few more allies: call them (rally.rs). Follow-only
        // escorts don't call rallies, only answer them (collect::ESCORT_FOLLOW_ONLY).
        // A jackpot may take the one rally slot from another group's rally, and count on the whole
        // team (JACKPOT_MAX_HELPERS).
        let over = info.jackpot
            && jackpot_live()
            && rally_now.is_some_and(|g| g != index && !statics.groups[g].jackpot);
        let rally_helpers = if unwinnable
            && any_live
            && (rally.is_none() || over)
            && !super::collect::follow_only()?
        {
            super::rally::max_helpers(group, round)?.map(|max| {
                if info.jackpot && jackpot_live() {
                    max.max(JACKPOT_MAX_HELPERS)
                } else {
                    max
                }
            })
        } else {
            None
        };
        if let Some(max) = rally_helpers.filter(|_| super::rally::worth_checking(index, round)) {
            let mut winnable = None;
            for extra in 1..=max {
                if super::collect::winnable_with_standins(index, extra)? {
                    winnable = Some(extra);
                    break;
                }
            }
            if let Some(extra) = winnable {
                if over {
                    super::trace::trace("rally-jackpot-over");
                    super::rally::call_over(index)?;
                } else {
                    super::rally::call(index)?;
                }
                rally_now = super::rally::active()?;
                if rally_now == Some(index) {
                    super::rally::set_need(extra)?;
                }
            } else {
                super::rally::checked(index, round);
            }
        }
        // A rally group stays a goal while we wait for allies.
        let rallying = rally_now == Some(index);
        // The team target too, while the team gathers (it attacks once the fight is winnable).
        let teaming = !rallying && team_now == Some(index);
        if rallying || teaming {
            unwinnable = false;
        }
        if unwinnable && previous_group == Some(index) {
            previous_target_reason = Some("camp-fight-unwinnable");
        }
        // While part of the group is alive, only its live (or unseen) camps are goals. A dead camp's
        // tile only becomes a goal once the whole group is cleared (standing there captures it):
        // otherwise a melee champion standing on one counts as having arrived and never walks over
        // to the live camp out of reach (seen stuck for 65 turns).
        // With its live members out of sight, its goals are the tiles we can't see, to find them.
        let is_goal =
            |c: &Camp| !((any_live || hidden_alive) && seen_dead.contains(&c.spawn_position()));
        let ours_only = |c: &Camp| !info.span || our_half(c.spawn_position()).unwrap_or(true);
        if gap <= max_reach + vision
            && !enemy_reach.is_empty()
            && group
                .camps()
                .iter()
                .filter(|c| is_goal(c) && ours_only(c))
                .any(|c| {
                    enemy_reach
                        .iter()
                        .any(|&(p, r)| distance(p, c.spawn_position()) <= r)
                })
        {
            if contest_ok(index) {
                super::trace::trace("jackpot-contest");
            } else {
                if previous_group == Some(index) {
                    previous_target_reason = Some("enemy-can-reach-group");
                }
                if LURK
                    && info.big
                    && any_live
                    && gap <= LURK_TILES
                    && lurk_on(index, &enemy_reach)?
                {
                    lurk.push((gap, index));
                }
                dropped.push(index);
                continue;
            }
        }
        if unwinnable {
            dropped.push(index);
            continue;
        }
        if ALLY_SOLO_LEAVE
            && any_live
            && !reward
            && !info.big
            && !info.monolith
            && !info.jackpot
            && rally_now != Some(index)
            && ally_solos(index)?
        {
            if previous_group == Some(index) {
                previous_target_reason = Some("ally-farming-group");
            }
            dropped.push(index);
            continue;
        }
        if let Some(tile) = previous_target.filter(|tile| {
            group
                .camps()
                .iter()
                .any(|camp| camp.spawn_position() == *tile)
        }) {
            if any_live && seen_dead.contains(&tile) {
                previous_target_reason = Some("camp-dead-while-group-still-active");
            }
        }
        if reward && group.camps().iter().any(|c| c.spawn_position() == here) {
            holding = super::collect::CAPTURE_HOLD;
        }
        if any_live || hidden_alive {
            not_goals.extend(seen_dead.iter().copied());
        }
        if hidden_alive {
            super::trace::trace("group-hidden-alive");
        }
        available[index] = true;
        if rallying {
            for camp in group.camps().iter().filter(|c| is_goal(c)) {
                rally_tiles.push(camp.spawn_position());
            }
        }
        if teaming {
            for camp in group.camps().iter().filter(|c| is_goal(c)) {
                team_tiles.push(camp.spawn_position());
            }
        }
        if opening_cluster && info.gold_only && !reward {
            clusters.push((index, gap));
        }
        if !info.preferred[phase] {
            off_phase.push((index, gap));
        }
        if reward {
            for camp in group.camps() {
                // A spanning group's capture is held from our end (SPAN_MODE).
                if info.span && !our_half(camp.spawn_position())? {
                    continue;
                }
                rewards.push(camp.spawn_position());
            }
        }
        if info.preferred[phase] {
            preferred_left += 1;
        }
        if gap <= DAMAGED_CAMP_RADIUS {
            damaged.extend(
                damaged_camps
                    .iter()
                    .filter(|&&(group_index, tile)| {
                        group_index == index && distance(tile, here) <= DAMAGED_CAMP_RADIUS
                    })
                    .map(|&(_, tile)| tile),
            );
        }
    }
    // Far groups from the cache (checked against a fresh pass with VERIFY_FUEL_CACHES).
    if let Some(cache) = far_cache {
        super::trace::trace("far-cache-used");
        if super::trace::VERIFY_FUEL_CACHES {
            let mut wrong = false;
            for &index in &cache.dropped {
                wrong |= !(round < ready[index] || super::collect::recently_lost(index, round));
            }
            for &index in &cache.available {
                wrong |= round < ready[index] || super::collect::recently_lost(index, round);
            }
            if wrong {
                super::trace::trace("far-cache-mismatch");
            }
        }
        dropped.extend(cache.dropped.iter().copied());
        for &index in &cache.available {
            available[index] = true;
        }
        clusters.extend(cache.clusters.iter().copied());
        preferred_left += cache.preferred;
        off_phase.extend(cache.off_phase.iter().copied());
        FAR_CACHE.with(|c| *c.borrow_mut() = Some(cache));
    } else if let Some(fresh) = fresh {
        FAR_CACHE.with(|c| *c.borrow_mut() = Some(fresh));
    }
    RESPAWN_READY.with(|cell| cell.replace(ready));
    // The nearest jackpot we'd get back to about as it respawns: wait on the free tiles around it.
    let mut jackpot_return = None;
    if let Some(&index) = returning
        .iter()
        .min_by_key(|&&i| distance(statics.groups[i].centre, here))
    {
        let info = &statics.groups[index];
        let mut ring = Bitboard::new()?;
        ring.draw_chebyshev(info.centre, (info.radius + 2) as f32)?;
        for camp in groups[index].camps() {
            ring.set(camp.spawn_position(), false)?;
        }
        ring.intersect_assign(&super::world::blocked_tiles()?.complement()?)?;
        // The side facing the enemy's spawn (RETURN_ENEMY_SIDE), if any of it is free.
        if RETURN_ENEMY_SIDE {
            let enemy = super::base::enemy_spawn_centre()?;
            let mut side = Bitboard::new()?;
            for tile in ring.positions()? {
                if distance(tile, enemy) <= distance(info.centre, enemy) {
                    side.set(tile, true)?;
                }
            }
            if !side.is_empty()? {
                ring = side;
            }
        }
        if !ring.is_empty()? {
            jackpot_return = Some(ring);
        }
    }
    // The nearest elk group an enemy is fighting (LURK): the free tiles just outside its live camps'
    // range, on the far side from the nearest enemy if any of those are free.
    let mut lurk_goals = None;
    if let Some(&(_, index)) = lurk.iter().min() {
        let (mut near, mut inner) = (Bitboard::new()?, Bitboard::new()?);
        for camp in &sight.camps {
            if group_at(camp.spawn_position())? != Some(index) {
                continue;
            }
            let range = camp.attack_range()?;
            near.draw_chebyshev(camp.spawn_position(), (range + 1 + LURK_SLACK) as f32)?;
            inner.draw_chebyshev(camp.spawn_position(), range as f32)?;
        }
        let mut ring = near.intersect(&inner.complement()?)?;
        ring.intersect_assign(&super::world::blocked_tiles()?.complement()?)?;
        let centre = statics.groups[index].centre;
        let enemy = enemy_reach
            .iter()
            .map(|&(p, _)| p)
            .min_by_key(|&p| distance(p, centre));
        if let Some(enemy) = enemy {
            let mut far = Bitboard::new()?;
            for tile in ring.positions()? {
                if distance(tile, enemy) > distance(centre, enemy) {
                    far.set(tile, true)?;
                }
            }
            if !far.is_empty()? {
                ring = far;
            }
        }
        if !ring.is_empty()? {
            lurk_goals = Some(ring);
        }
    }
    // Nothing we'd win, and no rally: call one for a group we can't see (PRESUME_FAR_TARGETS).
    if PRESUME_FAR_TARGETS
        && FAR_RALLIES
        && rally_now.is_none()
        && !super::collect::follow_only()?
        && !far_unwinnable.is_empty()
        && !available.iter().any(|&a| a)
    {
        call_far_rally(&mut far_unwinnable, round)?;
    }
    checkpoint("plan-groups");

    // Every camp (of the phase's preferred groups, if any is available), minus what's dropped.
    let regular_goals = || -> Result<(Bitboard, &'static str)> {
        let reason = if FARM_PHASE_FILTER && preferred_left > 0 {
            "preferred-phase-goals"
        } else {
            "ordinary-farm-goals"
        };
        let mut goals = if FARM_PHASE_FILTER && preferred_left > 0 {
            statics.preferred[phase].clone()
        } else {
            statics.all.clone()
        };
        checkpoint("goals-base");
        for &index in &dropped {
            for camp in groups[index].camps() {
                goals.set(camp.spawn_position(), false)?;
            }
        }
        if FARM_PHASE_FILTER && preferred_left > 0 && phase == 1 && GEM_PHASE_GOLD_DETOUR > 0 {
            let nearest = goals
                .positions()?
                .iter()
                .map(|&tile| distance(here, tile))
                .min()
                .unwrap_or(i32::MAX);
            for &(index, gap) in &off_phase {
                if gap <= nearest.saturating_add(GEM_PHASE_GOLD_DETOUR) {
                    for camp in groups[index].camps() {
                        goals.set(camp.spawn_position(), true)?;
                    }
                }
            }
        }
        checkpoint("goals-dropped");
        for &tile in &not_goals {
            goals.set(tile, false)?;
        }
        checkpoint("goals-not-goals");
        // Only in the gold phase: past it, clusters pulled champions off gem camps (trenches went
        // from 7/10 to 1/10).
        if !clusters.is_empty() && phase == 0 {
            prefer_clusters(&mut goals, &clusters, here)?;
        }
        Ok((goals, reason))
    };
    // A rally only wins if it's no further, counting RALLY_WAIT_STEPS for the wait, than the
    // nearest camp we'd farm without it (the rally group itself left out). Decided once per rally
    // and kept for RALLY_DECISION_ROUNDS rounds (fuel fix 1).
    let mut rally_regular = None;
    let mut rally_skipped = false;
    // Follow-only escorts (collect::ESCORT_FOLLOW_ONLY) always answer a rally: they have no camps
    // of their own to compare it with.
    let follow_only = super::collect::follow_only()?;
    // A side camp while we wait at the rally (RALLY_SIDE_STEPS).
    let mut rally_side = None;
    if RALLY_SIDE_STEPS > 0 && !rally_tiles.is_empty() && !follow_only {
        let at_rally = steps
            .nearest(&rally_tiles)
            .is_some_and(|t| steps.steps(t) <= RALLY_AT_STEPS);
        // Awake, not just damaged: an elk keeps its damage while it sleeps, so a rally group is
        // often damaged from an earlier try.
        let mut fight_on = false;
        for camp in &sight.camps {
            if group_at(camp.spawn_position())? == rally_now && camp.is_awake().unwrap_or(false) {
                fight_on = true;
                break;
            }
        }
        if at_rally && !fight_on {
            let mut side = Bitboard::new()?;
            for (index, group) in groups.iter().enumerate() {
                let info = &statics.groups[index];
                if !available[index]
                    || info.big
                    || info.monolith
                    || Some(index) == rally_now
                    || distance(info.centre, here) - info.radius > RALLY_SIDE_STEPS
                {
                    continue;
                }
                for camp in group.camps() {
                    let tile = camp.spawn_position();
                    if distance(tile, here) <= RALLY_SIDE_STEPS && !not_goals.contains(&tile) {
                        side.set(tile, true)?;
                    }
                }
            }
            let tiles = side.positions()?;
            if steps
                .nearest(&tiles)
                .is_some_and(|t| steps.steps(t) <= RALLY_SIDE_STEPS)
            {
                super::trace::trace("rally-side-camp");
                rally_side = Some(side);
            }
        }
    }
    // A rally that already has the allies it needs (rally::RALLY_CAP) isn't ours to join.
    let rally_full = !rally_tiles.is_empty() && super::rally::full()?;
    if rally_full {
        super::trace::trace("rally-full");
    }
    let rally_wanted = !rally_tiles.is_empty()
        && rally_side.is_none()
        && !rally_full
        && me()?.health()? >= super::rally::RALLY_MIN_HEALTH * me()?.max_health()?
        && (follow_only || {
            let cached = if RALLY_DECISION_ROUNDS > 0 {
                RALLY_DECISION
                    .with(|c| c.get())
                    .filter(|&(group, until, _)| Some(group) == rally_now && round < until)
                    .map(|(_, _, wanted)| wanted)
            } else {
                None
            };
            if cached.is_some() {
                super::trace::trace("rally-cached");
            }
            let wanted = match cached {
                Some(wanted) if !super::trace::VERIFY_FUEL_CACHES => wanted,
                _ => {
                    let (mut regular, reason) = regular_goals()?;
                    for &tile in &rally_tiles {
                        regular.set(tile, false)?;
                    }
                    let rally_steps = steps
                        .nearest(&rally_tiles)
                        .map_or(i32::MAX, |t| steps.steps(t));
                    let regular_steps =
                        steps.nearest(&regular.positions()?).map(|t| steps.steps(t));
                    // A help call (a fight in progress) has no wait: worth a few extra steps, if
                    // it's close enough to get there while the fight is on (rally::HELP_CALLS).
                    let help = super::rally::is_help_call()?;
                    let monolith = super::rally::MONOLITH_JOIN_EXTRA_STEPS >= 0
                        && rally_now.is_some_and(|g| super::rally::is_monolith(g).unwrap_or(false));
                    let jackpot_rally =
                        jackpot_live() && rally_now.is_some_and(|g| statics.groups[g].jackpot);
                    let wait = if help {
                        -super::rally::HELP_CALL_EXTRA_STEPS
                    } else if jackpot_rally {
                        super::trace::trace("rally-jackpot");
                        -JACKPOT_JOIN_EXTRA_STEPS
                    } else if super::collect::struggling(round) {
                        super::trace::trace("rally-struggling");
                        -super::rally::STRUGGLE_JOIN_EXTRA_STEPS
                    } else if monolith {
                        -super::rally::MONOLITH_JOIN_EXTRA_STEPS
                    } else {
                        super::rally::RALLY_WAIT_STEPS
                    };
                    let better = (help && rally_steps > super::rally::HELP_CALL_MAX_STEPS)
                        || (super::rally::RALLY_WAIT_STEPS > 0
                            && regular_steps.is_some_and(|r| r < rally_steps.saturating_add(wait)));
                    if better {
                        rally_regular = Some((regular, reason));
                    }
                    match cached {
                        Some(wanted) => {
                            if wanted == better {
                                super::trace::trace("rally-cache-mismatch");
                            }
                            wanted
                        }
                        None => {
                            if let Some(group) = rally_now {
                                RALLY_DECISION.with(|c| {
                                    c.set(Some((group, round + RALLY_DECISION_ROUNDS, !better)))
                                });
                            }
                            !better
                        }
                    }
                }
            };
            if !wanted {
                super::trace::trace("rally-skip-closer-camp");
                rally_skipped = true;
            }
            wanted
        });
    checkpoint("plan-rally-decision");
    let mut goals;
    let selection_reason;
    if holding {
        selection_reason = "capture-hold";
        goals = Bitboard::new()?;
        goals.set(here, true)?;
    } else if !rewards.is_empty() {
        // A reward we can see is taken, whatever the phase: otherwise a cleared gold cluster in
        // the gem phase was no goal, nobody captured it, and it never respawned (ambush: both
        // middle clusters, for 1,000+ rounds). Enemies that could reach it drop it above, unless
        // we'd win the 1v1 (plan_duel).
        selection_reason = "reward-in-sight";
        goals = Bitboard::new()?;
        for &tile in &rewards {
            goals.set(tile, true)?;
        }
    } else if rally_wanted {
        selection_reason = "rally";
        super::rally::join()?;
        goals = Bitboard::new()?;
        for &tile in &rally_tiles {
            goals.set(tile, true)?;
        }
        // Ranged champions wait at the edge of their attack range from the nearest elk rather
        // than next to it: in range to shoot when the fight starts, but as far back as that allows.
        let range = me()?.attack_range()?;
        if range > 1 {
            let (mut within, mut closer) = (Bitboard::new()?, Bitboard::new()?);
            for &tile in &rally_tiles {
                within.draw_chebyshev(tile, range as f32)?;
                closer.draw_chebyshev(tile, (range - 1) as f32)?;
            }
            let mut ring = within.intersect(&closer.complement()?)?;
            ring.intersect_assign(&super::world::blocked_tiles()?.complement()?)?;
            if !ring.is_empty()? {
                goals = ring;
            }
        }
    } else if let Some(ring) = lurk_goals {
        selection_reason = "elk-lurk";
        super::trace::trace("elk-lurk");
        goals = ring;
    } else if let Some(ring) = jackpot_return {
        selection_reason = "jackpot-return";
        super::trace::trace("jackpot-return");
        goals = ring;
    } else if !team_tiles.is_empty()
        && me()?.health()? >= super::rally::RALLY_MIN_HEALTH * me()?.max_health()?
    {
        selection_reason = "team-target";
        goals = Bitboard::new()?;
        for &tile in &team_tiles {
            goals.set(tile, true)?;
        }
    } else if follow_only {
        selection_reason = "escort-follow";
        goals = super::collect::escort_follow_goals()?;
    } else {
        let (mut regular, reason) = match (rally_side, rally_regular) {
            (Some(side), _) => (side, "rally-side-camp"),
            (None, Some(chosen)) => chosen,
            (None, None) => regular_goals()?,
        };
        if rally_skipped || rally_full {
            for &tile in &rally_tiles {
                regular.set(tile, false)?;
            }
        }
        goals = regular;
        selection_reason = reason;
    }
    checkpoint("plan-goals");
    let previous_was_regular_goal = match previous_target {
        Some(tile) => goals.is_set(tile)?,
        None => false,
    };
    if !holding
        && rewards.is_empty()
        && !matches!(selection_reason, "rally" | "team-target")
        && !damaged.is_empty()
    {
        let nearest_regular = steps
            .nearest(&goals.positions()?)
            .map(|tile| steps.steps(tile));
        let nearest_damaged = steps.nearest(&damaged);
        let damaged_distance = nearest_damaged.map_or(i32::MAX, |tile| steps.steps(tile));
        if nearest_regular
            .is_none_or(|regular| damaged_distance <= regular + DAMAGED_CAMP_MAX_DETOUR)
        {
            goals = Bitboard::new()?;
            if let Some(tile) = nearest_damaged {
                goals.set(tile, true)?;
            }
            if previous_was_regular_goal {
                if let Some(old) = previous_target.filter(|old| Some(*old) != nearest_damaged) {
                    previous_target_reason = Some(if damaged.contains(&old) {
                        "closer-damaged-camp"
                    } else {
                        "damaged-camp-priority"
                    });
                }
            }
        } else if previous_was_regular_goal
            && previous_target.is_some_and(|old| damaged.contains(&old))
        {
            previous_target_reason = Some("damaged-camp-too-far-from-regular-target");
        }
    }
    if let Some(old) = previous_target {
        if !goals.is_set(old)? && previous_target_reason.is_none() {
            previous_target_reason = Some(selection_reason);
        }
    }
    checkpoint("plan-damaged");
    if selection_reason == "escort-follow" {
        // Following an ally isn't a farm target: no target memory or cooldowns.
        FARM_TARGET_MEMORY.with(|memory| memory.borrow_mut().current = None);
    } else {
        avoid_abandoned_farm_targets(
            &mut goals,
            round,
            here,
            me()?.attack_range()?,
            previous_target_reason,
            &steps,
        )?;
    }
    // Group opening with no team target: ours becomes the team's.
    if opening_group && !holding && rewards.is_empty() && selection_reason != "rally" {
        let target = FARM_TARGET_MEMORY.with(|memory| memory.borrow().current);
        if let Some(index) = match target {
            Some(tile) => group_at(tile)?,
            None => None,
        } {
            crate::opening::set_team_target(index)?;
        }
    }
    // The capture we head for or hold claims it for the team (capclaim.rs).
    let claim = match selection_reason {
        "reward-in-sight" | "capture-hold" => {
            match FARM_TARGET_MEMORY.with(|memory| memory.borrow().current) {
                Some(tile)
                    if super::capclaim::CLAIM_NEAR_TILES <= 0
                        || distance(tile, here) <= super::capclaim::CLAIM_NEAR_TILES =>
                {
                    group_at(tile)?
                }
                _ => None,
            }
        }
        _ => None,
    };
    super::capclaim::heading_for(claim)?;
    checkpoint("plan-target");
    let plan = std::rc::Rc::new(FarmPlan { goals });
    RALLY_PLAN.with(|c| {
        *c.borrow_mut() = match (selection_reason, rally_now) {
            ("rally", Some(group)) if RALLY_REUSE_ROUNDS > 0 => {
                Some((round, group, here, plan.clone()))
            }
            _ => None,
        }
    });
    FARM_PLAN.with(|c| *c.borrow_mut() = Some((round, here, plan.clone())));
    // Only a plan heading out of sight can be kept while walking (and only without enemies).
    if WALK_REUSE {
        let snapshot = match &key {
            // A follow plan chases a moving ally: never kept while walking.
            Some(key)
                if key.enemies.is_empty()
                    && selection_reason != "escort-follow"
                    && !goals_in_sight(&plan.goals)? =>
            {
                Some(WalkSnapshot {
                    built: round,
                    key: key.clone(),
                    seen: seen_camp_tiles()?,
                    plan: plan.clone(),
                })
            }
            _ => None,
        };
        WALK_PLAN.with(|c| *c.borrow_mut() = snapshot);
    }
    PLAN_REUSE.with(|c| *c.borrow_mut() = key.map(|key| (round, key, plan.clone())));
    Ok(plan)
}

// While we wait to fight a camp group (not winnable yet: allies still coming), don't stand where its
// awake camps can hit us: goal tiles within an awake camp's range become the nearest free tiles just
// outside it, and if we're inside one already we step out. Camps don't chase, so waiting there costs
// nothing; standing next to one we weren't attacking just took its hits. Once the fight is
// winnable, attack_camp_in_range engages as before. Off: wait where the plan says.
pub const WAIT_OUTSIDE_AWAKE_CAMPS: bool = true;
// Fuel fix 6. 0: always check. Small: the check went from 2.6k to 1.9k on average.
pub const WAIT_PRECHECK_TILES: i32 = 7;

fn wait_outside_awake_camps(goals: Bitboard) -> Result<Bitboard> {
    if !WAIT_OUTSIDE_AWAKE_CAMPS {
        return Ok(goals);
    }
    let sight = sight()?;
    // Fuel fix 6: no camp within WAIT_PRECHECK_TILES (beyond the longest camp range plus a step)
    // means nothing to wait outside of yet; checked with plain distances before any host call.
    let here = me()?.position()?;
    if WAIT_PRECHECK_TILES > 0
        && !sight
            .camps
            .iter()
            .any(|c| distance(c.spawn_position(), here) <= WAIT_PRECHECK_TILES)
    {
        return Ok(goals);
    }
    let mut danger = Bitboard::new()?;
    let mut reach = 0;
    for camp in &sight.camps {
        if !camp.is_awake().unwrap_or(false) {
            continue;
        }
        let Some(index) = group_at(camp.spawn_position())? else {
            continue;
        };
        if super::collect::camp_fight_winnable(index)? == Some(true) {
            continue;
        }
        let range = camp.attack_range()?;
        danger.draw_chebyshev(camp.spawn_position(), range as f32)?;
        reach = reach.max(range);
    }
    if danger.is_empty()? {
        return Ok(goals);
    }
    let goals_in_danger = goals.intersect(&danger)?;
    if goals_in_danger.is_empty()? && !danger.is_set(here)? {
        return Ok(goals);
    }
    let free = super::world::blocked_tiles()?
        .union(&danger)?
        .complement()?;
    let mut outside = Bitboard::new()?;
    for tile in goals_in_danger.positions()? {
        outside.draw_chebyshev(tile, (reach + 1) as f32)?;
    }
    outside.intersect_assign(&free)?;
    if outside.is_empty()? {
        // No goal near the danger: just get out of it.
        outside = super::world::tiles_around(here, 2)?.intersect(&free)?;
    }
    if outside.is_empty()? {
        return Ok(goals);
    }
    super::trace::trace("wait-outside-camp");
    Ok(outside)
}

// Cluster opening: gold clusters within CLUSTER_DETOUR tiles of the nearest goal become the only
// goals, except ones an ally in sight is closer to (a cluster needs no help, and a shared capture
// is split). Nothing changes when no cluster qualifies.
const CLUSTER_DETOUR: i32 = 8;

fn prefer_clusters(goals: &mut Bitboard, clusters: &[(usize, i32)], here: Vec2) -> Result<()> {
    let statics = farm_statics()?;
    let groups = camp_groups()?;
    let nearest = goals
        .positions()?
        .iter()
        .map(|&tile| distance(here, tile))
        .min()
        .unwrap_or(i32::MAX);
    let allies = &sight()?.allies;
    let mut chosen = Bitboard::new()?;
    for &(index, gap) in clusters {
        if gap > nearest.saturating_add(CLUSTER_DETOUR) {
            continue;
        }
        let centre = statics.groups[index].centre;
        let mut taken = false;
        for ally in allies {
            if distance(ally.position()?, centre) < distance(here, centre) {
                taken = true;
                break;
            }
        }
        if taken {
            continue;
        }
        for camp in groups[index].camps() {
            if goals.is_set(camp.spawn_position())? {
                chosen.set(camp.spawn_position(), true)?;
            }
        }
    }
    if !chosen.is_empty()? {
        super::trace::trace("opening-clusters");
        *goals = chosen;
    }
    Ok(())
}

// Join an ally's camp fight (collect::ally_camp_fight_to_join): attack a camp in range, otherwise
// walk to the group's live camps and attack after the step if one is in range.
pub fn join_camp_fight(camps: &[Vec2]) -> Result<()> {
    if attack_camp_in_range()? {
        cast_abilities_on_camps(false)?;
        return Ok(());
    }
    let mut goals = Bitboard::new()?;
    for &tile in camps {
        goals.set(tile, true)?;
    }
    if move_towards_closest(goals)? {
        let me = me()?;
        let (here, range) = (me.position()?, me.attack_range()?);
        if camps.iter().any(|&c| distance(c, here) <= range) && attack_camp_in_range()? {
            cast_abilities_on_camps(true)?;
        }
    }
    Ok(())
}

fn skip_unreachable_target(round: u32) {
    // A kept plan would send us back to it.
    WALK_PLAN.with(|c| *c.borrow_mut() = None);
    PLAN_REUSE.with(|c| *c.borrow_mut() = None);
    FARM_TARGET_MEMORY.with(|memory| {
        let mut memory = memory.borrow_mut();
        if let Some(target) = memory.current.take() {
            memory
                .abandoned
                .push((target, round + UNREACHABLE_TARGET_ROUNDS));
        }
    });
}

// On the old maps it kept the last plan on only 0.9% of farming turns (2% fewer fuel-exhausted
// turns); on the 3.0 maps turns run out far more (7-28%), mostly before the move. It only ever
// reuses the last plan's own goals, so a low-fuel turn carries on with what the previous full plan
// chose and can't flip the target back and forth. 0: off.
// 2026-10-22 (v5): turns reached the farm step with ~82k fuel and a plan cost 50-90k, so 60k
// almost never kept one and 8.5% of all turns ran out (the work carries into the next turn: a lost
// turn, and the plan it finishes is as old as a kept one). 320 games each against 60k/10 rounds:
// 85k/3 168-152 (fuel-exhausted turns -17%, gems +4%); 80k/5 166-154 (-23%, but deaths 15.1
// against 13.3); 90k/2 160-160 (no change: a plan cut off in round N finishes in N+1, so with 2
// rounds it's too old by N+2 and the next one is cut off too).
pub const FARM_PLAN_MIN_FUEL: u64 = 85_000;
// Below this, skip the wait-outside check (a few bitboards and fight checks) and just move.
const WAIT_CHECK_MIN_FUEL: u64 = 30_000;
const STALE_PLAN_ROUNDS: u32 = 3;
// Whether a kept plan is used with enemies in sight: it doesn't know about ones that turned up
// since, and walked us towards them. Off: a fresh plan whenever an enemy is in sight.
pub const STALE_PLAN_WITH_ENEMIES: bool = true;

// Clear camps and capture their rewards, following farm_plan.
pub fn farm_camps() -> Result<()> {
    checkpoint("farm-start");
    super::claims::farmed();
    super::rally::check_unanswered()?;
    let attacked = attack_camp_in_range()?;
    checkpoint("farm-attack-check");
    if attacked {
        cast_abilities_on_camps(false)?;
        checkpoint("farm-cast");
        return Ok(());
    }
    // Fuel fix 2: with less than FARM_PLAN_MIN_FUEL left, building a plan (~36k on average) left
    // too little for the move itself, the most common place a turn ran out of fuel (the champion
    // then stands still). Keep walking to the last plan's goals instead, if it's recent.
    let round = cambc::round_number()?;
    let stale = if FARM_PLAN_MIN_FUEL > 0
        && cambc::fuel_remaining()? < FARM_PLAN_MIN_FUEL
        && (STALE_PLAN_WITH_ENEMIES || sight()?.enemies.is_empty())
    {
        FARM_PLAN.with(|c| {
            c.borrow()
                .as_ref()
                .filter(|(built, _, _)| round < built + STALE_PLAN_ROUNDS)
                .map(|(_, _, plan)| plan.clone())
        })
    } else {
        None
    };
    let plan = match stale {
        Some(plan) => {
            super::trace::trace("plan-low-fuel");
            plan
        }
        None => farm_plan()?,
    };
    checkpoint("farm-plan");
    // Camps don't move: the ones in sight before the step are the ones we might reach after it.
    let camps_before: Vec<Vec2> = sight()?.camps.iter().map(|c| c.spawn_position()).collect();
    let goals = if cambc::fuel_remaining()? >= WAIT_CHECK_MIN_FUEL {
        wait_outside_awake_camps(plan.goals.clone())?
    } else {
        plan.goals.clone()
    };
    checkpoint("farm-wait-outside");
    let moved = move_towards_closest(goals)?;
    if !moved && goals_were_unreachable() {
        super::trace::trace("farm-unreachable");
        skip_unreachable_target(cambc::round_number()?);
    }
    // Attack after the step only with a camp in range of where we now stand: the check rebuilds
    // sight and the camp-fight context for the new tile (~16k fuel), and this was where turns most
    // often ran out of fuel.
    if moved {
        let me = me()?;
        let (here, range) = (me.position()?, me.attack_range()?);
        if camps_before.iter().any(|&c| distance(c, here) <= range) && attack_camp_in_range()? {
            cast_abilities_on_camps(true)?;
        }
        checkpoint("farm-post-step");
    }
    Ok(())
}

// Our current farm target (claims.rs).
pub fn current_target() -> Option<Vec2> {
    FARM_TARGET_MEMORY.with(|memory| memory.borrow().current)
}
