//! Output-side fault injection (PLAN §5.5 injections 1–6 and 8). Built only with the
//! `fault-inject` feature; a release build refuses `NOPLAYOUT_FAULT_INJECT`.
//!
//! `NOPLAYOUT_FAULT_INJECT=<kind>@<k>[:<ms>]`, kinds: `repeat`, `skip`, `backwards`,
//! `key-prev`, `tear`, `tc-bump`, `block-fill`, `block-key`. With any fault set the pacer runs a
//! one-slot delay line (constant extra latency the oracle cannot see) so that `skip` and `tear`
//! have a later paint to use; `repeat`/`skip`/`backwards` change that delay permanently
//! (+1, −1, +3), so each produces exactly one discontinuity.

use crate::sink::Plane;
use std::sync::Arc;

pub const ENV: &str = "NOPLAYOUT_FAULT_INJECT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Repeat,
    Skip,
    Backwards,
    KeyPrev,
    Tear,
    TcBump,
    Block(Plane, u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fault {
    pub kind: Kind,
    /// Output slot index, counted from the first slot that carried a paint.
    pub at: u64,
}

/// Called by a sender thread before `submit` (injection 8).
pub type SubmitHook = Arc<dyn Fn(Plane, u64) + Send + Sync>;

pub fn parse(s: &str) -> Result<Fault, String> {
    let bad = || format!("bad {ENV} value {s}: expected <kind>@<slot>[:<ms>]");
    let (kind, rest) = s.split_once('@').ok_or_else(bad)?;
    let (at, ms) = match rest.split_once(':') {
        Some((a, m)) => (a, Some(m.parse::<u64>().map_err(|_| bad())?)),
        None => (rest, None),
    };
    let at = at.parse::<u64>().map_err(|_| bad())?;
    let kind = match (kind, ms) {
        ("repeat", None) => Kind::Repeat,
        ("skip", None) => Kind::Skip,
        ("backwards", None) => Kind::Backwards,
        ("key-prev", None) => Kind::KeyPrev,
        ("tear", None) => Kind::Tear,
        ("tc-bump", None) => Kind::TcBump,
        ("block-fill", Some(ms)) => Kind::Block(Plane::Fill, ms),
        ("block-key", Some(ms)) => Kind::Block(Plane::Key, ms),
        _ => return Err(bad()),
    };
    Ok(Fault { kind, at })
}

/// The configured fault, if any. Without the feature a set variable is an error.
pub fn from_env() -> Result<Option<Fault>, String> {
    let Ok(v) = std::env::var(ENV) else {
        return Ok(None);
    };
    if !cfg!(feature = "fault-inject") {
        return Err(format!(
            "{ENV} is set but this noplayout-output was built without the fault-inject feature"
        ));
    }
    parse(&v).map(Some)
}

/// Injection 8: block `plane`'s sender for `ms` before submitting the slot that carries
/// index `at` (slot numbers in the hook are output `k`; the pacer translates).
pub fn submit_hook(plane: Plane, ms: u64, k_at: Arc<std::sync::atomic::AtomicU64>) -> SubmitHook {
    Arc::new(move |p, k| {
        if p == plane && k == k_at.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!(
                "noplayout-output: fault-inject: blocking {} submit for {ms} ms",
                p.label()
            );
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_kind() {
        assert_eq!(
            parse("repeat@300").unwrap(),
            Fault {
                kind: Kind::Repeat,
                at: 300
            }
        );
        assert_eq!(
            parse("block-fill@10:300").unwrap().kind,
            Kind::Block(Plane::Fill, 300)
        );
        assert!(parse("block-fill@10").is_err());
        assert!(parse("repeat@x").is_err());
        assert!(parse("melt@3").is_err());
        for k in ["skip", "backwards", "key-prev", "tear", "tc-bump"] {
            assert!(parse(&format!("{k}@5")).is_ok(), "{k}");
        }
    }
}
