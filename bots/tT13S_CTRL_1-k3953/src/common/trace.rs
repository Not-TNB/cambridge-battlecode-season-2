// Debug tracing: per-turn branch tags and fuel checkpoints (TRACE).
use std::fmt::Write as _;

// Verbose turn/fuel tracing is disabled; farm-target changes have their own concise diagnostic.
pub const TRACE: bool = false;
pub(super) const FARM_TARGET_DIAGNOSTICS: bool = false;
// With TRACE: print only turns that ran out of fuel (a checkpoint shows more fuel than the one
// before it: the turn was cut off and carried on into the next) or used at least SPILL_TRACE_FUEL.
// Formatting every turn's trace costs enough fuel to cause cut-offs of its own.
pub const TRACE_ONLY_SPILLS: bool = true;
// Profiling: the caching fuel fixes (rally decision, camp-fight verdicts, path fields) also work the
// answer out afresh and trace "<cache>-mismatch" when the cached one differs. Costs the fuel the
// caches save, so only for measuring how often they're wrong.
pub const VERIFY_FUEL_CACHES: bool = false;
const SPILL_TRACE_FUEL: u64 = 90_000;

thread_local! {
    static TRACE_TAGS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

// Tags and checkpoints are profiling only: in a debug_block they cost no fuel in local debug
// builds, and the platform's release build skips them.
pub fn trace(tag: &'static str) {
    cambc::debug_block! {
        super::activity::note(tag);
        if TRACE {
            TRACE_TAGS.with(|t| {
                let mut t = t.borrow_mut();
                t.push(' ');
                t.push_str(tag);
            });
        }
    }
}

thread_local! {
    static CHECKPOINTS: std::cell::RefCell<Vec<(&'static str, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Record fuel left at a point in the turn (formatted only when the trace is printed, so the
// measurement itself costs little).
#[allow(dead_code)] // for profiling: sprinkle in, read the trace, remove
pub fn checkpoint(tag: &'static str) {
    if TRACE {
        // Read inside the debug block: the fuel it costs isn't counted, so profiling doesn't cause
        // the cut-offs it measures.
        cambc::debug_block! {
            let fuel = cambc::fuel_remaining().unwrap_or(0);
            CHECKPOINTS.with(|c| c.borrow_mut().push((tag, fuel)));
        }
    }
}

thread_local! {
    // Fuel spent this turn per metered section (TRACE): (tag, fuel, times).
    static SPENT: std::cell::RefCell<Vec<(&'static str, u64, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Profiling (TRACE): a guard that adds the fuel spent until it's dropped to `tag`'s total for the
// turn, printed after the checkpoints. A section cut off by the turn's end isn't counted.
pub struct Meter {
    tag: &'static str,
    start: u64,
}

pub fn meter(tag: &'static str) -> Meter {
    let mut start = 0;
    if TRACE {
        cambc::debug_block! {
            start = cambc::fuel_remaining().unwrap_or(0);
        }
    }
    Meter { tag, start }
}

impl Drop for Meter {
    fn drop(&mut self) {
        if !TRACE {
            return;
        }
        let (tag, start) = (self.tag, self.start);
        cambc::debug_block! {
            let now = cambc::fuel_remaining().unwrap_or(0);
            if now <= start && start != 0 {
                let spent = start - now;
                SPENT.with(|s| {
                    let mut s = s.borrow_mut();
                    match s.iter_mut().find(|e| e.0 == tag) {
                        Some(e) => {
                            e.1 += spent;
                            e.2 += 1;
                        }
                        None => s.push((tag, spent, 1)),
                    }
                });
            }
        }
    }
}

// `f()`, metered as `tag` (see Meter).
pub fn metered<T>(tag: &'static str, f: impl FnOnce() -> T) -> T {
    let _m = meter(tag);
    f()
}

// Whether this turn's trace should be printed (see TRACE_ONLY_SPILLS); if not, it's dropped.
pub fn trace_wanted() -> bool {
    if !TRACE_ONLY_SPILLS {
        return true;
    }
    let wanted = CHECKPOINTS.with(|c| {
        let c = c.borrow();
        let spilled = c.windows(2).any(|w| w[1].1 > w[0].1);
        let used = match (c.first(), c.last()) {
            (Some(first), Some(last)) => first.1.saturating_sub(last.1),
            _ => 0,
        };
        spilled || used >= SPILL_TRACE_FUEL
    });
    if !wanted {
        TRACE_TAGS.with(|t| t.borrow_mut().clear());
        CHECKPOINTS.with(|c| c.borrow_mut().clear());
        SPENT.with(|s| s.borrow_mut().clear());
    }
    wanted
}

pub fn take_trace() -> String {
    let mut out = TRACE_TAGS.with(|t| std::mem::take(&mut *t.borrow_mut()));
    let checkpoints = CHECKPOINTS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    if !checkpoints.is_empty() {
        out.push_str(" | fuel[k left]: ");
        let mut previous = None;
        for (tag, fuel) in checkpoints {
            if let Some(before) = previous {
                // Fuel reads are refunded in debug builds, so back-to-back checkpoints can read a
                // few units higher: only a real jump means the turn ran out and resumed.
                if fuel > before + 5_000 {
                    write!(out, " -> next turn: {tag} {}k", fuel / 1000)
                        .expect("writing to String cannot fail");
                } else {
                    let change = (fuel as i64 - before as i64) / 1000;
                    write!(out, " -> {tag} {}k ({change:+}k)", fuel / 1000)
                        .expect("writing to String cannot fail");
                }
            } else {
                write!(out, "{tag} {}k", fuel / 1000).expect("writing to String cannot fail");
            }
            previous = Some(fuel);
        }
    }
    let spent = SPENT.with(|s| std::mem::take(&mut *s.borrow_mut()));
    if !spent.is_empty() {
        out.push_str(" | spent:");
        for (tag, fuel, times) in spent {
            write!(out, " {tag}={}x{times}", fuel).expect("writing to String cannot fail");
        }
    }
    out
}
