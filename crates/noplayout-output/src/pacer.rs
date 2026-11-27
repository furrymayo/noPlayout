//! The pacer (PLAN §2.4a, §5.1, §5.3): the channel clock, paint selection from the active
//! ring, the one conversion per slot, and the hand-off to the per-plane lanes. Plus the 10 Hz
//! progress monitor (§5.4) and the paint-probe check (K2g, E-057).

use crate::fault::{Fault, Kind};
use crate::sink::{self, Converted, Counters, FrameSink, Lane, Plane, Snapshot};
use noplayout_frames::clock::{self, Rate};
use noplayout_frames::{PaintMeta, Reader};
use noplayout_proto::{DriveMode, Event, OutputStats};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Selection guard: a paint must be complete 1 ms before `T_k` (§5.3).
pub const GUARD_NS: u64 = 1_000_000;
/// Mode B: after a crossing skip, no repeat in the next 900 slots (§5.3).
pub const HYSTERESIS_SLOTS: u64 = 900;
/// K2g: a probe outstanding this long with no newer paint is a `PaintStall` (§2.13.1).
pub const PROBE_STALL_NS: u64 = 250_000_000;
/// K5 thresholds (§5.4).
pub const SENDER_SLOW_NS: u64 = 250_000_000;
pub const SENDER_STUCK_NS: u64 = 2_000_000_000;

pub struct Config {
    pub rate: Rate,
    pub epoch_mono_ns: u64,
    pub u0_100ns: i64,
    /// Stop after this many slots that carried a paint (`--frames N`).
    pub frames: Option<u64>,
    pub fault: Option<Fault>,
}

/// What the control thread asks of the pacer.
#[derive(Default)]
pub struct Control {
    pub activate: Mutex<Option<(String, u64, DriveMode)>>,
    pub stop: AtomicBool,
}

#[derive(Default)]
pub struct Stats {
    pub slots: AtomicU64,
    pub held: AtomicU64,
    pub slots_skipped: AtomicU64,
    pub late_max_ns: AtomicU64,
    pub last_paint_seq: AtomicU64,
    pub active_inc: AtomicU64,
    superseded_base: AtomicU64,
    overwrites_base: AtomicU64,
    superseded_now: AtomicU64,
    overwrites_now: AtomicU64,
}

impl Stats {
    pub fn snapshot(&self, c: &Counters) -> OutputStats {
        let r = |a: &AtomicU64| a.load(Ordering::Relaxed);
        OutputStats {
            slots: r(&self.slots),
            held: r(&self.held),
            superseded: r(&self.superseded_base) + r(&self.superseded_now),
            slots_skipped: r(&self.slots_skipped),
            submit_blocked: r(&c.submit_blocked),
            pair_partial: r(&c.pair_partial),
            ring_overwrites: r(&self.overwrites_base) + r(&self.overwrites_now),
            late_max_us: r(&self.late_max_ns) / 1000,
            last_paint_seq: r(&self.last_paint_seq),
            active_inc: r(&self.active_inc),
        }
    }
}

pub struct Pacer {
    pub control: Arc<Control>,
    pub stats: Arc<Stats>,
    pub lanes: Vec<(Plane, Arc<Lane>)>,
}

struct Active {
    reader: Reader,
    inc: u64,
    mode: DriveMode,
    probe_answered: u64,
    probe_reported: u64,
}

/// Start the lanes, the pacer thread and the monitor thread. `events` carries `Up::Event`s.
pub fn start(
    cfg: Config,
    sinks: Vec<(Plane, Box<dyn FrameSink>)>,
    events: Sender<Event>,
) -> std::io::Result<Pacer> {
    let control = Arc::new(Control::default());
    let stats = Arc::new(Stats::default());
    let counters = Arc::new(Counters::default());
    let block_k = Arc::new(AtomicU64::new(u64::MAX));
    let mut lanes = Vec::new();
    let mut joins = Vec::new();
    for (plane, s) in sinks {
        let lane = Arc::new(Lane::default());
        let hook = match cfg.fault.map(|f| f.kind) {
            Some(Kind::Block(p, ms)) if p == plane => {
                Some(crate::fault::submit_hook(p, ms, block_k.clone()))
            }
            _ => None,
        };
        joins.push(lane.spawn(plane, s, hook)?);
        lanes.push((plane, lane));
    }
    let pacer = Pacer {
        control: control.clone(),
        stats: stats.clone(),
        lanes: lanes.clone(),
    };
    let ev = events.clone();
    let (st, ct, ln) = (stats.clone(), counters.clone(), lanes.clone());
    std::thread::Builder::new()
        .name("pacer".into())
        .spawn(move || {
            if !clock::elevate_current_thread() {
                eprintln!("noplayout-output: pacer runs at normal priority");
            }
            let sent = run(&cfg, &control, &st, &ct, &ln, &ev, &block_k);
            for (_, l) in &ln {
                l.stop();
            }
            for j in joins {
                match j.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => eprintln!("noplayout-output: sink close: {e}"),
                    Err(_) => eprintln!("noplayout-output: sender thread panicked"),
                }
            }
            if cfg.frames.is_some() {
                let _ = ev.send(Event::DumpComplete { frames: sent });
            }
        })?;
    let (st, ct, ln, ctl) = (
        stats.clone(),
        counters.clone(),
        lanes.clone(),
        pacer.control.clone(),
    );
    std::thread::Builder::new()
        .name("monitor".into())
        .spawn(move || monitor(&ctl, &st, &ct, &ln, &events))?;
    Ok(pacer)
}

/// The pacer loop. Returns the number of slots that carried a paint.
fn run(
    cfg: &Config,
    control: &Control,
    stats: &Stats,
    counters: &Arc<Counters>,
    lanes: &[(Plane, Arc<Lane>)],
    events: &Sender<Event>,
    block_k: &AtomicU64,
) -> u64 {
    let (rate, epoch) = (cfg.rate, cfg.epoch_mono_ns);
    let period = rate.period_ns();
    let planes = lanes.len() as u8;
    let mut active: Option<Active> = None;
    let mut pending = None;
    // Selected paints, newest first (the delay line for fault injection; else one entry).
    let mut history: VecDeque<(PaintMeta, Arc<Converted>)> = VecDeque::new();
    let mut delay: usize = if cfg.fault.is_some() { 1 } else { 0 };
    let mut last_seq = 0u64;
    let mut hysteresis_until = 0u64;
    let mut sent = 0u64;
    let mut k = clock::k_at_or_after(epoch, clock::mono_ns() + period, rate);
    while !control.stop.load(Ordering::Relaxed) {
        let tk = clock::t_k(epoch, k, rate);
        let late = clock::wait_until(tk);
        stats.late_max_ns.fetch_max(late, Ordering::Relaxed);
        if late > period / 2 {
            // Never burst: drop this slot and any others already past (§5.3).
            let next = clock::k_at_or_after(epoch, clock::mono_ns(), rate).max(k + 1);
            stats.slots_skipped.fetch_add(next - k, Ordering::Relaxed);
            k = next;
            continue;
        }
        stats.slots.fetch_add(1, Ordering::Relaxed);

        if let Some(req) = control
            .activate
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            // The renderer creates its ring when it gets `Configure`, which may be after the
            // engine's `Activate` reaches us: retry for up to 2 s.
            pending = Some((req, clock::mono_ns() + 2_000_000_000));
        }
        if let Some(((ring, inc, mode), until)) = pending.take() {
            match Reader::open(&ring) {
                Ok(reader) => {
                    if let Some(old) = active.take() {
                        stats
                            .superseded_base
                            .fetch_add(old.reader.superseded(), Ordering::Relaxed);
                        stats
                            .overwrites_base
                            .fetch_add(old.reader.overwrites(), Ordering::Relaxed);
                    }
                    let (probe, _) = reader.probe();
                    stats.active_inc.store(inc, Ordering::Relaxed);
                    eprintln!("noplayout-output: reading ring {ring} (inc {inc}, mode {mode:?})");
                    active = Some(Active {
                        reader,
                        inc,
                        mode,
                        probe_answered: probe,
                        probe_reported: probe,
                    });
                }
                Err(_) if clock::mono_ns() < until => {
                    pending = Some(((ring, inc, mode), until));
                }
                Err(e) => {
                    let _ = events.send(Event::ActivateFailed {
                        ring,
                        why: e.to_string(),
                    });
                }
            }
        }

        let mut fresh = false;
        if let Some(a) = active.as_mut() {
            let mut slot = a.reader.take_newest(tk.saturating_sub(GUARD_NS));
            if slot.is_none() && a.mode == DriveMode::B && k < hysteresis_until {
                // Inside the hysteresis window a paint up to 1 ms late is taken (§5.3).
                slot = a.reader.take_newest(tk);
            }
            if let Some(s) = slot {
                let m = s.meta;
                let conv = sink::convert(s.pixels(), m.width, m.height, m.stride);
                drop(s);
                if a.mode == DriveMode::B && last_seq > 0 && m.paint_seq >= last_seq + 2 {
                    hysteresis_until = k + HYSTERESIS_SLOTS;
                }
                last_seq = m.paint_seq;
                stats.last_paint_seq.store(m.paint_seq, Ordering::Relaxed);
                if m.painted_at_ns > a.reader.probe().1 {
                    a.probe_answered = a.reader.probe().0;
                }
                history.push_front((m, Arc::new(conv)));
                fresh = true;
            }
            stats
                .superseded_now
                .store(a.reader.superseded(), Ordering::Relaxed);
            stats
                .overwrites_now
                .store(a.reader.overwrites(), Ordering::Relaxed);
            // K2g (E-057): a probe outstanding 250 ms with no newer paint.
            let (pseq, preq) = a.reader.probe();
            if pseq > a.probe_answered
                && pseq > a.probe_reported
                && clock::mono_ns().saturating_sub(preq) >= PROBE_STALL_NS
            {
                a.probe_reported = pseq;
                let _ = events.send(Event::PaintStall {
                    inc: a.inc,
                    probe_seq: pseq,
                });
            }
        }
        if !fresh {
            // Hold: resend the previous paint with the new tc(k) (§5.3).
            match history.front().cloned() {
                Some(prev) => history.push_front(prev),
                None => {
                    k += 1;
                    continue; // nothing painted yet
                }
            }
            stats.held.fetch_add(1, Ordering::Relaxed);
        }
        history.truncate(8);

        let mut tc = clock::tc_k(cfg.u0_100ns, k, rate);
        let mut pick = history.get(delay).or(history.back()).cloned();
        if let Some(f) = cfg.fault.filter(|f| f.at == sent) {
            eprintln!("noplayout-output: fault-inject {:?} at slot {k}", f.kind);
            match f.kind {
                Kind::Repeat => delay += 1,
                Kind::Skip => delay = delay.saturating_sub(1),
                Kind::Backwards => delay += 3,
                Kind::TcBump => {
                    tc += i64::try_from((u64::from(rate.den) * 10_000_000) / u64::from(rate.num))
                        .unwrap_or(0);
                }
                Kind::KeyPrev | Kind::Tear => {}
                Kind::Block(..) => block_k.store(k, Ordering::Relaxed),
            }
            pick = history.get(delay).or(history.back()).cloned();
            if let Some((m, conv)) = pick.clone() {
                let spliced = match f.kind {
                    Kind::KeyPrev => history.get(delay + 1).map(|(_, prev)| Converted {
                        fill: conv.fill.clone(),
                        key: prev.key.clone(),
                    }),
                    Kind::Tear => {
                        delay
                            .checked_sub(1)
                            .and_then(|d| history.get(d))
                            .map(|(_, next)| {
                                let half = conv.fill.len() / 2;
                                let mut fill = conv.fill.clone();
                                fill[half..].copy_from_slice(&next.fill[half..]);
                                let khalf = conv.key.len() / 2;
                                let mut key = conv.key.clone();
                                key[khalf..].copy_from_slice(&next.key[khalf..]);
                                Converted { fill, key }
                            })
                    }
                    _ => None,
                };
                if let Some(c) = spliced {
                    pick = Some((m, Arc::new(c)));
                }
            }
        }
        let Some((meta, conv)) = pick else {
            k += 1;
            continue;
        };
        let snap = Arc::new(Snapshot::new(
            k,
            tc,
            meta,
            !fresh,
            conv,
            planes,
            counters.clone(),
        ));
        for (_, lane) in lanes {
            lane.offer(snap.clone(), counters);
        }
        drop(snap);
        sent += 1;
        if cfg.frames.is_some_and(|n| sent >= n) {
            break;
        }
        k += 1;
    }
    sent
}

/// 10 Hz progress monitor (§5.4) and the 1 Hz stats report.
fn monitor(
    control: &Control,
    stats: &Stats,
    counters: &Counters,
    lanes: &[(Plane, Arc<Lane>)],
    events: &Sender<Event>,
) {
    let mut reported: Vec<(bool, bool)> = vec![(false, false); lanes.len()];
    let mut tick = 0u64;
    while !control.stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(100));
        let now = clock::mono_ns();
        for (i, (plane, lane)) in lanes.iter().enumerate() {
            let since = lane.in_flight_since.load(Ordering::Acquire);
            let age = if since == 0 {
                0
            } else {
                now.saturating_sub(since)
            };
            let (slow, stuck) = &mut reported[i];
            if age == 0 {
                (*slow, *stuck) = (false, false);
                continue;
            }
            let ms = age / 1_000_000;
            if age >= SENDER_STUCK_NS && !*stuck {
                *stuck = true;
                let _ = events.send(Event::SenderStuck {
                    plane: plane.label().into(),
                    in_flight_ms: ms,
                });
            } else if age >= SENDER_SLOW_NS && !*slow {
                *slow = true;
                let _ = events.send(Event::SenderStall {
                    plane: plane.label().into(),
                    in_flight_ms: ms,
                });
            }
        }
        tick += 1;
        if tick.is_multiple_of(10) {
            let _ = events.send(Event::OutputStats(stats.snapshot(counters)));
        }
    }
}
