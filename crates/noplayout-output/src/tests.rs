use super::*;
use crate::fault::{Fault, Kind};
use crate::sink::{Converted, Counters, Lane, Snapshot};
use noplayout_frames::{PaintMeta, Writer};
use noplayout_proto::DriveMode;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

#[test]
fn options_from_args_and_env() {
    let args: Vec<String> = ["--channel", "1", "--rate", "50", "--dump-crop", "0,0,8,8"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let env = |k: &str| match k {
        "NOPLAYOUT_DUMP_PAINTS" => Some("d".to_owned()),
        "NOPLAYOUT_DUMP_FRAMES" => Some("12".to_owned()),
        "NOPLAYOUT_DUMP_CROP" => Some("1,2,3,4;5,6,7,8".to_owned()),
        _ => None,
    };
    let o = parse_options(&args, env).unwrap();
    assert_eq!(o.channel, "1");
    assert_eq!(o.rate, Rate { num: 50, den: 1 });
    assert_eq!(o.dump, Some(PathBuf::from("d")));
    assert_eq!(o.frames, Some(12));
    assert_eq!(o.crop, vec![[1, 2, 3, 4], [5, 6, 7, 8], [0, 0, 8, 8]]);
    assert_eq!(parse_rate("60000/1001").unwrap(), Rate::P5994);
    assert!(parse_rate("0/1").is_err());
    assert!(parse_options(&[], |_| None).is_err());
}

#[test]
fn convert_unpremultiplies_and_extracts_key() {
    // BGRA premultiplied: opaque red, half-transparent white, fully transparent.
    let src = [0, 0, 255, 255, 128, 128, 128, 128, 9, 9, 9, 0];
    let c = sink::convert(&src, 3, 1, 12);
    assert_eq!(c.key, vec![255, 128, 0]);
    assert_eq!(&c.fill[0..4], &[0, 0, 255, 255]);
    assert_eq!(&c.fill[4..8], &[255, 255, 255, 255]);
    assert_eq!(&c.fill[8..12], &[0, 0, 0, 255]);
}

fn unique(tag: &str) -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    format!("npo{tag}{:x}", n ^ std::process::id())
}

type Row = (u64, i64, u64, bool, u8, u8);

#[derive(Clone, Default)]
struct Record(Arc<Mutex<Vec<Row>>>);

impl FrameSink for Record {
    fn submit(&mut self, plane: Plane, s: &Snapshot) -> std::io::Result<()> {
        if plane == Plane::Key {
            let fill = s.plane(Plane::Fill)[0];
            let key = s.plane(Plane::Key)[1];
            self.0
                .lock()
                .unwrap()
                .push((s.k, s.tc_100ns, s.paint_seq, s.held, fill, key));
        }
        Ok(())
    }
}

/// A writer that publishes a paint every millisecond, named by a per-paint value v, so
/// FILL and KEY both identify the paint.
fn painter(name: &str, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    let mut w = Writer::create(name, 7, 8, 8).unwrap();
    std::thread::spawn(move || {
        let mut n: u64 = 0;
        while !stop.load(Ordering::Relaxed) {
            n += 1;
            if let Some(mut s) = w.begin() {
                let v = 50 + (n % 200) as u8;
                for px in s.pixels()[..8 * 8 * 4].chunks_mut(4) {
                    px.copy_from_slice(&[0, 0, 0, v]);
                }
                // Pixel 0 opaque with blue = v (FILL names the paint); the rest alpha = v
                // (KEY names it).
                s.pixels()[..4].copy_from_slice(&[v, 0, 0, 255]);
                s.publish(clock::mono_ns(), 8, 8, 32);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    })
}

fn run_pacer(fault: Option<Fault>, frames: u64) -> (Vec<Row>, Vec<Event>) {
    let name = unique("p");
    let stop = Arc::new(AtomicBool::new(false));
    let paint = painter(&name, stop.clone());
    let rec = Record::default();
    let (tx, rx) = mpsc::channel();
    let p = pacer::start(
        pacer::Config {
            rate: Rate::P50,
            epoch_mono_ns: clock::mono_ns(),
            u0_100ns: 1_000,
            frames: Some(frames),
            start_after: None,
            fault,
            clock: Arc::new(pacer::RealClock),
        },
        vec![
            (Plane::Fill, Box::new(NullSink) as Box<dyn FrameSink>),
            (Plane::Key, Box::new(rec.clone())),
        ],
        tx,
    )
    .unwrap();
    *p.control.activate.lock().unwrap() = Some((name.clone(), 7, DriveMode::B));
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::DumpComplete { frames: f }) => {
                assert_eq!(f, frames);
                break;
            }
            Ok(e) => events.push(e),
            Err(_) => assert!(std::time::Instant::now() < deadline, "pacer did not finish"),
        }
    }
    p.control.stop.store(true, Ordering::Relaxed);
    stop.store(true, Ordering::Relaxed);
    paint.join().unwrap();
    let _ = noplayout_frames::shm::unlink(&name);
    let v = rec.0.lock().unwrap().clone();
    (v, events)
}

#[test]
fn pacer_sends_newest_paint_each_slot_with_tc_from_k() {
    let (v, _) = run_pacer(None, 40);
    assert_eq!(v.len(), 40);
    let period_100ns = 200_000; // 50 fps
    for w in v.windows(2) {
        let ((k0, tc0, s0, ..), (k1, tc1, s1, ..)) = (w[0], w[1]);
        assert!(k1 > k0);
        assert_eq!(tc1 - tc0, (k1 - k0) as i64 * period_100ns);
        assert!(s1 >= s0, "paint went backwards: {s0} -> {s1}");
    }
    for &(_, _, _, _, fill, key) in &v {
        assert_eq!(fill, key, "FILL and KEY come from the same paint");
    }
    // A paint every ms against a 20 ms slot: (almost) every slot carries a newer paint.
    let fresh = v.iter().filter(|r| !r.3).count();
    assert!(fresh >= 35, "only {fresh} of 40 slots were fresh");
}

#[test]
fn fault_repeat_and_key_prev_and_tc_bump() {
    let (v, _) = run_pacer(
        Some(Fault {
            kind: Kind::Repeat,
            at: 10,
        }),
        20,
    );
    let repeats = v.windows(2).filter(|w| w[0].2 == w[1].2).count();
    assert!(repeats >= 1, "the injected repeat shows");
    assert_eq!(v[10].2, v[9].2, "slot 10 resends slot 9's paint");

    let (v, _) = run_pacer(
        Some(Fault {
            kind: Kind::KeyPrev,
            at: 10,
        }),
        20,
    );
    let mixed: Vec<usize> = (0..v.len()).filter(|&i| v[i].4 != v[i].5).collect();
    assert_eq!(
        mixed.len(),
        1,
        "exactly one slot pairs FILL with another paint's KEY"
    );
    let i = mixed[0];
    assert_eq!(v[i].5, v[i - 1].4, "KEY is the previous slot's paint");

    let (v, _) = run_pacer(
        Some(Fault {
            kind: Kind::TcBump,
            at: 10,
        }),
        20,
    );
    // Slots may be held or skipped on a loaded machine, so compare each tc with tc(k).
    let bumped: Vec<i64> = v
        .iter()
        .map(|r| r.1 - (1_000 + r.0 as i64 * 200_000))
        .filter(|d| *d != 0)
        .collect();
    assert_eq!(bumped, vec![200_000], "exactly one tc is a period ahead");
}

#[test]
fn blocked_sender_is_slow_not_the_clock() {
    let (v, events) = run_pacer(
        Some(Fault {
            kind: Kind::Block(Plane::Key, 600),
            at: 5,
        }),
        40,
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::SenderStall { plane, .. } if plane == "key")),
        "SenderStall raised: {events:?}"
    );
    // The pacer kept its clock while the plane was blocked: no other plane's event, and the
    // blocked plane carried on afterwards. (How many slots it missed depends on the host's
    // scheduling, so the newest-wins accounting is checked deterministically below.)
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::SenderStall { plane, .. } if plane == "fill"))
    );
    assert!(v.last().unwrap().0 - v[0].0 >= 39);
}

#[test]
fn newest_wins_and_partial_pairs_are_counted() {
    let counters = Arc::new(Counters::default());
    let lane = Lane::default();
    let meta = PaintMeta {
        inc: 1,
        paint_seq: 1,
        painted_at_ns: 0,
        width: 1,
        height: 1,
        stride: 4,
    };
    let px = Arc::new(Converted {
        fill: vec![0; 4],
        key: vec![0],
    });
    for k in 0..3 {
        let s = Arc::new(Snapshot::new(
            k,
            0,
            meta,
            false,
            px.clone(),
            2,
            counters.clone(),
        ));
        lane.offer(s, &counters);
    }
    // Two displaced from this plane only (the other plane "sent" them): two partial pairs.
    assert_eq!(counters.submit_blocked.load(Ordering::Relaxed), 2);
    assert_eq!(counters.pair_partial.load(Ordering::Relaxed), 2);
}
