// Activity log (profiling, ACTIVITY): what each champion spends its turns on. Every turn falls into
// one category (dead, recalling, hitting a camp, fighting, walking to farm, idle, ...), worked out at
// the end of the turn from the trace tags it set (trace::trace) and whether we moved or hit
// something; the counts are printed every ACTIVITY_PRINT_ROUNDS rounds. Turns cut short by running
// out of fuel show up as rounds with no turn at all ("fuel-stall"). Off in games: costs a little fuel.
use super::me;
use cambc::{Result, Vec2};
use std::cell::{Cell, RefCell};

pub const ACTIVITY: bool = false;
const ACTIVITY_PRINT_ROUNDS: u32 = 500;

thread_local! {
    static TAGS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static COUNTS: RefCell<Vec<(&'static str, u32)>> = const { RefCell::new(Vec::new()) };
    // Tags seen on turns that ended idle or waiting, to see why.
    static IDLE_TAGS: RefCell<Vec<(&'static str, u32)>> = const { RefCell::new(Vec::new()) };
    static START: Cell<Option<(u32, Vec2)>> = const { Cell::new(None) };
    static LAST_ROUND: Cell<Option<u32>> = const { Cell::new(None) };
    static PRINTED: Cell<u32> = const { Cell::new(0) };
    // The last few turns, for the death report: (round, category, tags, hp, allies within 6,
    // enemies within 8, gems).
    static RECENT: RefCell<Vec<(u32, &'static str, Vec<&'static str>, i32, usize, usize, u32)>> =
        const { RefCell::new(Vec::new()) };
    static WAS_ALIVE: Cell<bool> = const { Cell::new(false) };
}

fn bump(list: &RefCell<Vec<(&'static str, u32)>>, key: &'static str, n: u32) {
    let mut l = list.borrow_mut();
    match l.iter_mut().find(|(k, _)| *k == key) {
        Some(e) => e.1 += n,
        None => l.push((key, n)),
    }
}

// A trace tag (or an attack, see attack.rs) this turn.
pub fn note(tag: &'static str) {
    if ACTIVITY {
        TAGS.with(|t| t.borrow_mut().push(tag));
    }
}

// The start of a turn: where we stand, and the rounds lost since the last turn (fuel stalls).
pub fn start_turn() {
    if !ACTIVITY {
        return;
    }
    let round = cambc::round_number().unwrap_or(0);
    if let Some(last) = LAST_ROUND.with(|l| l.get()) {
        if round > last + 1 {
            COUNTS.with(|c| bump(c, "fuel-stall", round - last - 1));
        }
    }
    LAST_ROUND.with(|l| l.set(Some(round)));
    let here = me().and_then(|m| m.position()).ok();
    START.with(|s| s.set(here.map(|p| (round, p))));
    TAGS.with(|t| t.borrow_mut().clear());
}

// The end of a turn: classify it. `state` is set by bot.rs for turns it skips (dead, recalling, ...).
pub fn end_turn(state: Option<&'static str>) -> Result<()> {
    if !ACTIVITY {
        return Ok(());
    }
    let tags = TAGS.with(|t| std::mem::take(&mut *t.borrow_mut()));
    let has = |k: &str| tags.iter().any(|t| *t == k);
    let has_prefix = |p: &str| tags.iter().any(|t| t.starts_with(p));
    let moved = match (START.with(|s| s.get()), me().and_then(|m| m.position())) {
        (Some((_, a)), Ok(b)) => a != b,
        _ => false,
    };
    let category: &'static str = if let Some(s) = state {
        s
    } else if has("skip-dead") {
        "dead"
    } else if has("skip-recalling") {
        "recalling"
    } else if has("skip-tribute") {
        "tribute"
    } else if has("skip-teleport") || has("skip-no-actions") {
        "no-actions"
    } else if has("wait-spawn") {
        "wait-spawn"
    } else if has("hit-champion") || has("cast-enemy") {
        "fight-hit"
    } else if has("hit-camp") || has("cast-camp") {
        "camp-hit"
    } else if super::collect::capturing().unwrap_or(false) {
        "capture"
    } else if moved {
        if has("bank") || has("bank-first") || has_prefix("help-ally-before-bank") {
            "walk-bank"
        } else if has("go-home") || has("retreat") || has("walk-home") || has("low-hp")
            || has("recall-dodge")
        {
            "walk-home"
        } else if has("retreat-fight") || has("kite") || has_prefix("keep-") || has_prefix("chase")
            || has("follow-lost") || has_prefix("fight-")
        {
            "fight-move"
        } else if has_prefix("assist") {
            "assist-move"
        } else {
            "walk-farm"
        }
    } else if has("wait-outside-camp") {
        "wait-camp"
    } else if has_prefix("rally") {
        "wait-rally"
    } else if has("standoff") || has("no-nearby-target") || has_prefix("keep-") {
        "standoff"
    } else {
        "idle"
    };
    COUNTS.with(|c| bump(c, category, 1));
    // Death report: the first dead turn prints the turns before it.
    let alive = me().and_then(|m| m.is_alive()).unwrap_or(false);
    if alive {
        let (here, hp, gems) = {
            let m = me()?;
            (m.position()?, m.health()? as i32, m.gem_count()?)
        };
        let (mut allies, mut enemies) = (0, 0);
        if let Ok(sight) = super::world::sight() {
            for a in &sight.allies {
                if a.position().is_ok_and(|p| super::world::distance(p, here) <= 6) {
                    allies += 1;
                }
            }
            for e in &sight.enemies {
                if e.position().is_ok_and(|p| super::world::distance(p, here) <= 8) {
                    enemies += 1;
                }
            }
        }
        let round = cambc::round_number()?;
        RECENT.with(|r| {
            let mut r = r.borrow_mut();
            r.push((round, category, tags.clone(), hp, allies, enemies, gems));
            let n = r.len();
            if n > 5 {
                r.drain(..n - 5);
            }
        });
    } else if WAS_ALIVE.with(|w| w.get()) {
        let mut line = format!("DIED {:?} r{}", me()?.champion_name()?, cambc::round_number()?);
        RECENT.with(|r| {
            for (round, cat, tags, hp, allies, enemies, gems) in r.borrow().iter() {
                line.push_str(&format!(" || r{round} {cat} hp{hp} a{allies} e{enemies} g{gems} [{}]", tags.join(" ")));
            }
        });
        println!("{line}");
        RECENT.with(|r| r.borrow_mut().clear());
    }
    WAS_ALIVE.with(|w| w.set(alive));
    if matches!(category, "idle" | "wait-camp" | "wait-rally" | "standoff") {
        IDLE_TAGS.with(|c| {
            if tags.is_empty() {
                bump(c, "(no tags)", 1);
            }
            for t in &tags {
                bump(c, t, 1);
            }
        });
    }
    let round = cambc::round_number()?;
    let mark = round / ACTIVITY_PRINT_ROUNDS;
    if mark > PRINTED.with(|p| p.get()) || round >= 1999 && PRINTED.with(|p| p.get()) < 99 {
        PRINTED.with(|p| p.set(if round >= 1999 { 99 } else { mark }));
        let mut line = format!("ACT {:?} r{} |", me()?.champion_name()?, round);
        COUNTS.with(|c| {
            for (k, n) in c.borrow().iter() {
                line.push_str(&format!(" {k}={n}"));
            }
        });
        line.push_str(" || idle-tags:");
        IDLE_TAGS.with(|c| {
            let mut v = c.borrow().clone();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (k, n) in v.iter().take(12) {
                line.push_str(&format!(" {k}={n}"));
            }
        });
        println!("{line}");
    }
    Ok(())
}
