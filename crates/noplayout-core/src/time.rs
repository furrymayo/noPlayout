//! The engine's one time source (PLAN §4.3): UTC read once at engine start and advanced with the
//! monotonic clock, so `utcUs` never steps backwards within an engine run, plus the monotonic
//! microseconds since the run started. Every host `Action` and journal entry carries a [`Ts`].

use serde::{Deserialize, Serialize};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Monotonic part of a timestamp: the engine run and microseconds since it started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mono {
    pub run: String,
    pub us: u64,
}

/// The output slot whose deadline follows the action, in the output clock domain (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotRef {
    pub k: u64,
    pub epoch: String,
}

/// A channel's output clock as its output process announced it in `Hello` (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotClock {
    /// `epoch_mono_ns` on the machine-wide clock (`noplayout_frames::clock::mono_ns`).
    pub epoch_mono_ns: u64,
    pub rate: noplayout_frames::clock::Rate,
}

impl SlotClock {
    /// The first slot whose deadline `T_k` is after `now_ns`. `epoch` names the output
    /// process's epoch (its `epoch_mono_ns` in decimal); a respawned output process starts a
    /// new one (§5.1 item 3).
    pub fn slot_after(&self, now_ns: u64) -> SlotRef {
        SlotRef {
            k: noplayout_frames::clock::k_at_or_after(
                self.epoch_mono_ns,
                now_ns.saturating_add(1),
                self.rate,
            ),
            epoch: self.epoch_mono_ns.to_string(),
        }
    }

    pub fn slot_now(&self) -> SlotRef {
        self.slot_after(noplayout_frames::clock::mono_ns())
    }
}

/// `ts: {utcUs, mono: {run, us}, slot?: {k, epoch}}` (§4.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ts {
    pub utc_us: i64,
    pub mono: Mono,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<SlotRef>,
}

/// One per engine process. Cheap to clone.
#[derive(Debug, Clone)]
pub struct TimeSource {
    run: String,
    start: Instant,
    utc0_us: i64,
}

impl Default for TimeSource {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeSource {
    /// Start a new engine run: `engineRun` is a fresh UUIDv7 (§2.14).
    pub fn new() -> Self {
        Self::with_run(uuid::Uuid::now_v7().to_string())
    }

    pub fn with_run(run: String) -> Self {
        let start = Instant::now();
        Self {
            run,
            start,
            utc0_us: system_utc_us(),
        }
    }

    pub fn run(&self) -> &str {
        &self.run
    }

    /// Monotonic microseconds since this run started.
    pub fn mono_us(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    pub fn now(&self) -> Ts {
        let us = self.mono_us();
        Ts {
            utc_us: self
                .utc0_us
                .saturating_add(i64::try_from(us).unwrap_or(i64::MAX)),
            mono: Mono {
                run: self.run.clone(),
                us,
            },
            slot: None,
        }
    }

    /// Engine UTC minus system UTC, in microseconds (Health shows it, §4.3).
    pub fn offset_from_system_us(&self) -> i64 {
        self.now().utc_us - system_utc_us()
    }
}

fn system_utc_us() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_micros()).unwrap_or(i64::MAX),
        Err(e) => -i64::try_from(e.duration().as_micros()).unwrap_or(i64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_and_mono_never_go_backwards_and_share_the_run() {
        let t = TimeSource::new();
        let mut last = t.now();
        for _ in 0..1000 {
            let n = t.now();
            assert!(n.utc_us >= last.utc_us && n.mono.us >= last.mono.us);
            assert_eq!(n.mono.run, t.run());
            last = n;
        }
        assert!(t.offset_from_system_us().abs() < 1_000_000);
    }

    #[test]
    fn slot_after_is_the_next_deadline_strictly_after_issue() {
        let c = SlotClock {
            epoch_mono_ns: 1_000,
            rate: noplayout_frames::clock::Rate::P5994,
        };
        let t1 = noplayout_frames::clock::t_k(1_000, 1, c.rate);
        assert_eq!(c.slot_after(0).k, 0);
        assert_eq!(c.slot_after(1_000).k, 1, "T_0 is not after its own instant");
        assert_eq!(c.slot_after(t1 - 1).k, 1);
        assert_eq!(c.slot_after(t1).k, 2);
        assert_eq!(c.slot_after(t1).epoch, "1000");
    }

    #[test]
    fn serializes_to_the_page_wire_shape() {
        let ts = Ts {
            utc_us: 5,
            mono: Mono {
                run: "r".into(),
                us: 7,
            },
            slot: None,
        };
        assert_eq!(
            serde_json::to_value(&ts).unwrap(),
            serde_json::json!({"utcUs": 5, "mono": {"run": "r", "us": 7}})
        );
    }
}
