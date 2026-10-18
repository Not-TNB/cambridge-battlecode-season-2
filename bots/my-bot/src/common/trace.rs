// Debug tracing: per-turn branch tags and fuel checkpoints (TRACE).
use std::fmt::Write as _;

// Verbose turn/fuel tracing is disabled; farm-target changes have their own concise diagnostic.
pub const TRACE: bool = false;
pub(super) const FARM_TARGET_DIAGNOSTICS: bool = true;

thread_local! {
    static TRACE_TAGS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

pub fn trace(tag: &str) {
    if TRACE {
        TRACE_TAGS.with(|t| {
            let mut t = t.borrow_mut();
            t.push(' ');
            t.push_str(tag);
        });
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
        CHECKPOINTS.with(|c| {
            c.borrow_mut()
                .push((tag, cambc::fuel_remaining().unwrap_or(0)))
        });
    }
}

pub fn take_trace() -> String {
    let mut out = TRACE_TAGS.with(|t| std::mem::take(&mut *t.borrow_mut()));
    let checkpoints = CHECKPOINTS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    if !checkpoints.is_empty() {
        out.push_str(" | fuel[k left]: ");
        let mut previous = None;
        for (tag, fuel) in checkpoints {
            if let Some(before) = previous {
                if fuel > before {
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
    out
}
