//! The eight injected faults of PLAN §5.5 on the M1 paint-dump path: the real pacer, ring,
//! lanes and `DumpSink`, then the oracle reading the dump's pixels and timecodes. The source
//! is a synthetic painter drawing the frame-oracle patches (the reference encoder), one paint
//! per slot at `T_k − P/2`, on a virtual clock so the result does not depend on the host's
//! timer. (The real-CEF run of the fixture is spike S1a.)

use super::*;
use crate::fault::{Fault, Kind};
use crate::sink::Lane;
use noplayout_frames::Writer;
use noplayout_oracle::analyze::{Mode, Spec, analyze};
use noplayout_oracle::layout;
use noplayout_proto::DriveMode;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const W: u32 = 640;
const H: u32 = 480;

struct Painter {
    w: Writer,
    next: u64,
    epoch: u64,
    /// Injection 7: freeze the time base for 10 paints from this one.
    freeze_at: Option<u64>,
    frozen_s: u64,
}

struct Virtual {
    now: Mutex<u64>,
    painter: Mutex<Painter>,
    lanes: OnceLock<Vec<(Plane, Arc<Lane>)>>,
    /// Only the Block injection leaves a lane in `submit`; otherwise always wait for the sinks
    /// (a slow runner's disk must not turn into a dropped slot).
    block: Option<Plane>,
    /// The one `submit` (its `in_flight_since`) the Block injection holds; no other is exempt.
    blocked_submit: Mutex<Option<u64>>,
}

impl pacer::Clock for Virtual {
    fn now(&self) -> u64 {
        *self.now.lock().unwrap()
    }
    fn wait_until(&self, t: u64) -> u64 {
        // Let the sinks finish the previous slot; a lane in `submit` for 100 ms is the one
        // injection 8 blocks, and is left alone.
        if let Some(lanes) = self.lanes.get() {
            // Generous: a hosted runner's disk can hold a DumpSink past 1 s (a real dropped
            // slot then); only a hung sink should reach this.
            let until = Instant::now() + Duration::from_secs(20);
            while Instant::now() < until
                && !lanes.iter().all(|(plane, l)| {
                    let since = l.in_flight_since.load(Ordering::Acquire);
                    l.is_idle()
                        || (self.block == Some(*plane)
                            && since != 0
                            && clock::mono_ns() - since > 100_000_000
                            && *self.blocked_submit.lock().unwrap().get_or_insert(since) == since)
                })
            {
                std::thread::sleep(Duration::from_micros(200));
            }
        }
        std::thread::sleep(Duration::from_millis(2));
        let rate = Rate::P5994;
        let lead = rate.period_ns() / 2;
        let mut p = self.painter.lock().unwrap();
        while clock::t_k(p.epoch, p.next, rate) - lead <= t {
            let at = clock::t_k(p.epoch, p.next, rate) - lead;
            let mut s_us = at / 1000;
            match p.freeze_at {
                Some(f) if p.next == f => p.frozen_s = s_us,
                Some(f) if p.next > f && p.next < f + 10 => s_us = p.frozen_s,
                _ => {}
            }
            let n = p.next as u32;
            if let Some(mut slot) = p.w.begin() {
                layout::paint(&mut slot.pixels()[..(W * H * 4) as usize], W, H, n, s_us, n);
                slot.publish(at, W, H, W * 4);
            }
            p.next += 1;
        }
        *self.now.lock().unwrap() = t;
        0
    }
}

fn dump_run(
    tag: &str,
    fault: Option<Fault>,
    freeze_at: Option<u64>,
) -> noplayout_oracle::analyze::Report {
    let ring = format!("npt{tag}{:x}", std::process::id());
    let dir = std::env::temp_dir().join(format!("np-oracle-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let epoch = 1_000_000_000_000u64;
    let virt = Arc::new(Virtual {
        now: Mutex::new(epoch),
        painter: Mutex::new(Painter {
            w: Writer::create(&ring, 3, W, H).unwrap(),
            next: 1,
            epoch,
            freeze_at,
            frozen_s: 0,
        }),
        lanes: OnceLock::new(),
        block: match fault {
            Some(Fault {
                kind: Kind::Block(p, _),
                ..
            }) => Some(p),
            _ => None,
        },
        blocked_submit: Mutex::new(None),
    });
    let crops = layout::crops(W, H);
    let sinks: Vec<(Plane, Box<dyn FrameSink>)> = [Plane::Fill, Plane::Key]
        .into_iter()
        .map(|p| {
            let s: Box<dyn FrameSink> = Box::new(DumpSink::create(&dir, p, crops.clone()).unwrap());
            (p, s)
        })
        .collect();
    let frames = 300;
    let (tx, rx) = mpsc::channel();
    let p = pacer::start(
        pacer::Config {
            rate: Rate::P5994,
            epoch_mono_ns: epoch,
            u0_100ns: 0,
            frames: Some(frames),
            start_after: None,
            fault,
            clock: virt.clone(),
        },
        sinks,
        tx,
    )
    .unwrap();
    let _ = virt
        .lanes
        .set(p.lanes.iter().map(|(pl, l)| (*pl, l.clone())).collect());
    *p.control.activate.lock().unwrap() = Some((ring.clone(), 3, DriveMode::A));
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::DumpComplete { .. }) => break,
            Ok(_) => {}
            Err(_) => assert!(Instant::now() < deadline, "{tag}: pacer did not finish"),
        }
    }
    p.control.stop.store(true, Ordering::Relaxed);
    let _ = noplayout_frames::shm::unlink(&ring);
    let (obs, key_tcs) = noplayout_oracle::dump::read(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let spec = Spec {
        mode: Mode::A,
        num: 60000,
        den: 1001,
        warmup: 5,
    };
    analyze(&spec, &obs, &key_tcs)
}

fn failures(r: &noplayout_oracle::analyze::Report) -> Vec<&str> {
    let mut f: Vec<&str> = r.failures.iter().map(String::as_str).collect();
    f.sort_unstable();
    f
}

type Case = (
    &'static str,
    Option<Fault>,
    Option<u64>,
    &'static [&'static str],
);

fn fault(kind: Kind) -> Option<Fault> {
    Some(Fault { kind, at: 50 })
}

#[test]
fn clean_dump_passes_the_mode_a_row() {
    let r = dump_run("clean", None, None);
    assert!(r.pass, "{r:?}");
    assert_eq!(r.frames, 295);
}

/// Each injection fails the run and trips its detector. Where a permanent one-slot shift in
/// latency is the injection's nature (repeat, skip, backwards), the lag band trips too, as
/// §5.5 lists for repeat. A bumped `tc` shows on both planes, and the receiver, pairing KEY to
/// FILL by timecode, then pairs one FILL with the next slot's KEY (`keyMismatch`).
#[test]
fn every_injected_fault_fails_with_its_detector() {
    let cases: [Case; 8] = [
        ("repeat", fault(Kind::Repeat), None, &["lagBand", "repeats"]),
        ("skip", fault(Kind::Skip), None, &["lagBand", "skips"]),
        (
            "back",
            fault(Kind::Backwards),
            None,
            &["backwards", "lagBand"],
        ),
        ("keyprev", fault(Kind::KeyPrev), None, &["keyMismatch"]),
        ("tear", fault(Kind::Tear), None, &["tears"]),
        (
            "tcbump",
            fault(Kind::TcBump),
            None,
            &["keyMismatch", "keyTimecode", "timecode"],
        ),
        ("freeze", None, Some(100), &["lagBand", "stepErrors"]),
        (
            "block",
            fault(Kind::Block(Plane::Key, 300)),
            None,
            &["keyTimecode", "unpaired"],
        ),
    ];
    for (tag, f, freeze, want) in cases {
        let r = dump_run(tag, f, freeze);
        assert!(!r.pass, "{tag} must fail: {r:?}");
        assert_eq!(failures(&r), want, "{tag}: {r:?}");
    }
}
