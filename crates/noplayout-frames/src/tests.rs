use super::*;
use crate::ring::{FREE, READING, READY, WRITING};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn unique(tag: &str) -> String {
    use std::sync::atomic::AtomicU32;
    static N: AtomicU32 = AtomicU32::new(0);
    format!(
        "npt{}-{tag}{}",
        std::process::id() % 100_000,
        N.fetch_add(1, Ordering::Relaxed)
    )
}

struct Cleanup(String);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = shm::unlink(&self.0);
    }
}

fn paint(w: &mut Writer, fill: u8, at: u64) -> PaintMeta {
    let mut s = w.begin().expect("a slot");
    s.pixels()[..16].fill(fill);
    s.publish(at, 2, 2, 8)
}

#[test]
fn name_is_short_and_safe() {
    let n = ring_name("0193ab12-ffff-7000-8000-00000000abcdef12", "1", 42);
    assert_eq!(n, "np00abcdef12-1-42");
    assert!(shm::os_name(&n).is_ok());
    assert!(shm::os_name("a/b").is_err());
    assert!(shm::os_name(&"x".repeat(31)).is_err());
    // Two engines started in the same second share a UUIDv7 time prefix, not its random tail.
    assert_ne!(
        ring_name("0193ab12-ffff-7000-8000-000000000001", "1", 1),
        ring_name("0193ab12-ffff-7000-8000-000000000002", "1", 1)
    );
}

#[test]
fn newest_ready_wins_older_are_freed_and_cutoff_applies() {
    let name = unique("sel");
    let _c = Cleanup(name.clone());
    let mut w = Writer::create(&name, 7, 2, 2).unwrap();
    let r = Reader::open(&name).unwrap();
    assert_eq!(r.inc(), 7);
    assert!(r.take_newest(u64::MAX).is_none(), "nothing painted yet");
    paint(&mut w, 1, 100);
    paint(&mut w, 2, 200);
    paint(&mut w, 3, 300);
    // Guard: a paint completed after T_k − guard is not eligible yet.
    {
        let s = r.take_newest(250).unwrap();
        assert_eq!((s.meta.paint_seq, s.meta.inc), (2, 7));
        assert_eq!(&s.pixels()[..16], &[2; 16]);
        assert_eq!(r.superseded(), 1, "paint 1 freed unread");
    }
    let s = r.take_newest(u64::MAX).unwrap();
    assert_eq!(s.meta.paint_seq, 3);
    assert_eq!(s.meta.painted_at_ns, 300);
    drop(s);
    assert_eq!(r.states(), [FREE; SLOTS]);
    assert!(
        r.take_newest(u64::MAX).is_none(),
        "taken paints are not resent"
    );
}

#[test]
fn writer_never_blocks_and_never_reclaims_the_reading_slot() {
    let name = unique("nb");
    let _c = Cleanup(name.clone());
    let mut w = Writer::create(&name, 1, 2, 2).unwrap();
    let r = Reader::open(&name).unwrap();
    paint(&mut w, 1, 1);
    let held = r.take_newest(u64::MAX).unwrap();
    assert_eq!(held.meta.paint_seq, 1);
    // 100 paints while the reader holds one slot: every begin() succeeds immediately.
    for i in 0..100u8 {
        paint(&mut w, i, 10 + i as u64);
    }
    assert!(w.overwrites() >= 97, "overwrites {}", w.overwrites());
    assert_eq!(&held.pixels()[..16], &[1; 16], "held paint untouched");
    let st = r.states();
    assert_eq!(st.iter().filter(|&&s| s == READING).count(), 1);
    assert_eq!(st.iter().filter(|&&s| s == READY).count(), 3);
    drop(held);
    assert_eq!(r.take_newest(u64::MAX).unwrap().meta.paint_seq, 101);
}

#[test]
fn unpublished_slot_returns_to_free_and_reader_reset_clears_reading() {
    let name = unique("rst");
    let _c = Cleanup(name.clone());
    let mut w = Writer::create(&name, 1, 2, 2).unwrap();
    {
        let _s = w.begin().unwrap();
    }
    paint(&mut w, 9, 5);
    let r = Reader::open(&name).unwrap();
    let s = r.take_newest(u64::MAX).unwrap();
    std::mem::forget(s); // a reader that died holding a slot
    assert!(r.states().contains(&READING));
    let r2 = Reader::open(&name).unwrap(); // respawned output process
    assert!(!r2.states().contains(&READING));
    assert!(!r2.states().contains(&WRITING));
}

#[test]
fn concurrent_writer_and_reader_see_consistent_paints() {
    let name = unique("cc");
    let _c = Cleanup(name.clone());
    let mut w = Writer::create(&name, 3, 64, 64).unwrap();
    let r = Reader::open(&name).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let writer = std::thread::spawn(move || {
        let mut n = 0u64;
        let mut worst = Duration::ZERO;
        while !stop2.load(Ordering::Relaxed) {
            let t = Instant::now();
            let mut s = w.begin().expect("writer never finds the ring full");
            let fill = (n % 251) as u8;
            s.pixels()[..64 * 64 * 4].fill(fill);
            s.publish(n, 64, 64, 256);
            worst = worst.max(t.elapsed());
            n += 1;
        }
        (n, worst)
    });
    let mut last = 0;
    let mut taken = 0;
    let until = Instant::now() + Duration::from_millis(300);
    while Instant::now() < until {
        if let Some(s) = r.take_newest(u64::MAX) {
            let px = s.pixels();
            let want = ((s.meta.paint_seq - 1) % 251) as u8;
            assert!(
                px.iter().all(|&b| b == want),
                "torn paint {}",
                s.meta.paint_seq
            );
            assert!(s.meta.paint_seq > last, "paints go forward");
            last = s.meta.paint_seq;
            taken += 1;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let (n, worst) = writer.join().unwrap();
    assert!(taken > 10 && n > taken, "n {n} taken {taken}");
    eprintln!("concurrent: {n} paints, {taken} taken, worst begin+copy+publish {worst:?}");
}

/// S8 measurement (`cargo test --release -p noplayout-frames -- --ignored s8_measure`): a
/// 1080p BGRA writer at full speed against a reader taking at ~59.94 Hz. Prints one JSON line.
#[test]
#[ignore = "measurement; run explicitly"]
fn s8_measure_1080p() {
    let name = unique("s8");
    let _c = Cleanup(name.clone());
    let (w, h) = (1920u32, 1080u32);
    let mut wr = Writer::create(&name, 1, w, h).unwrap();
    let r = Reader::open(&name).unwrap();
    let src = vec![0x5au8; (w * h * 4) as usize]; // stands in for CEF's paint buffer
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let writer = std::thread::spawn(move || {
        let mut begin = Vec::new();
        let mut total = Vec::new();
        while !stop2.load(Ordering::Relaxed) {
            let t0 = Instant::now();
            let mut s = wr.begin().expect("never full");
            let t1 = Instant::now();
            s.pixels()[..src.len()].copy_from_slice(&src); // the one copy
            s.publish(0, w, h, w * 4);
            begin.push(t1 - t0);
            total.push(t0.elapsed());
            std::thread::sleep(Duration::from_millis(8)); // ~120 paints/s, 2x the output rate
        }
        (begin, total, wr.overwrites())
    });
    let mut taken = 0u64;
    let until = Instant::now() + Duration::from_secs(5);
    let mut next = Instant::now();
    while Instant::now() < until {
        next += Duration::from_nanos(16_683_350);
        if let Some(s) = r.take_newest(u64::MAX) {
            std::hint::black_box(s.pixels()[0]);
            taken += 1;
        }
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    stop.store(true, Ordering::Relaxed);
    let (mut begin, mut total, overwrites) = writer.join().unwrap();
    begin.sort();
    total.sort();
    let pct = |v: &[Duration], p: f64| v[((v.len() - 1) as f64 * p) as usize].as_secs_f64() * 1e6;
    println!(
        "{{\"s8\":\"own-ring\",\"paints\":{},\"taken\":{taken},\"overwrites\":{overwrites},\"superseded\":{},\
         \"begin_us\":{{\"p50\":{:.2},\"p99\":{:.2},\"max\":{:.2}}},\
         \"begin_copy_publish_us\":{{\"p50\":{:.1},\"p99\":{:.1},\"max\":{:.1}}},\"bytes_per_paint\":{}}}",
        total.len(),
        r.superseded(),
        pct(&begin, 0.5),
        pct(&begin, 0.99),
        pct(&begin, 1.0),
        pct(&total, 0.5),
        pct(&total, 0.99),
        pct(&total, 1.0),
        w * h * 4
    );
}

/// Child half of the `kill -9` test: create the ring, publish two paints, take a third slot
/// and spin inside it forever (a writer killed mid-copy).
#[test]
fn kill_child_writer() {
    let Ok(name) = std::env::var("NP_RING_KILL_CHILD") else {
        return;
    };
    let reg = shm::Registry::new(std::env::var("NP_RING_KILL_REG").unwrap());
    reg.record(&name).unwrap();
    let mut w = Writer::create(&name, 5, 16, 16).unwrap();
    paint(&mut w, 1, 1);
    paint(&mut w, 2, 2);
    let mut s = w.begin().unwrap();
    println!("NP_RING_READY");
    use std::io::Write;
    std::io::stdout().flush().unwrap();
    loop {
        s.pixels()[0] = s.pixels()[0].wrapping_add(1);
        std::hint::spin_loop();
    }
}

/// S8 criterion 3: `kill -9` of the writer mid-paint leaves a ring the reader can still use
/// (the WRITING slot is ignored), and after the reader goes, nothing is left behind.
#[test]
fn kill_9_writer_leaves_no_leak() {
    use std::io::BufRead;
    let name = unique("k9");
    let dir = std::env::temp_dir().join(format!("np-ring-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let reg_path = dir.join("rings.txt");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::kill_child_writer",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("NP_RING_KILL_CHILD", &name)
        .env("NP_RING_KILL_REG", &reg_path)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.stdout.take().unwrap();
    let mut lines = std::io::BufReader::new(out).lines();
    assert!(
        lines.any(|l| l.is_ok_and(|l| l.contains("NP_RING_READY"))),
        "child reached its spin"
    );
    let r = Reader::open(&name).unwrap();
    child.kill().unwrap(); // SIGKILL on Unix, TerminateProcess on Windows
    child.wait().unwrap();

    assert_eq!(r.states().iter().filter(|&&s| s == WRITING).count(), 1);
    let s = r.take_newest(u64::MAX).unwrap();
    assert_eq!(
        (s.meta.inc, s.meta.paint_seq),
        (5, 2),
        "dead writer's last whole paint"
    );
    drop(s);
    drop(r);

    // The engine's sweep (incarnation retired / next start) removes the object everywhere.
    let swept = shm::Registry::new(&reg_path).sweep().unwrap();
    assert_eq!(swept, usize::from(cfg!(unix)));
    let err = Reader::open(&name).err().expect("ring is gone");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound, "{err}");
    #[cfg(target_os = "linux")]
    assert!(!std::path::Path::new("/dev/shm").join(&name).exists());
    let _ = std::fs::remove_dir_all(dir);
}
