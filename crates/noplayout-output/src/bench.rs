//! Spike S2 (PLAN §7 M1 task 7): the CPU cost per slot of the paint path at 1080p and 2160p,
//! `noplayout-output --bench-s2`. Prints one JSON object. Measured: the renderer's copy of a
//! CEF paint into a ring slot (begin + memcpy + publish, the same `Writer` calls `on_paint`
//! makes), the output's read of the newest paint, and the one FILL + KEY conversion per slot.
//! CEF's own readback (GPU → the `on_paint` buffer) happens inside CEF and is not separable
//! here; the end-to-end paint rate of the real renderer comes from the S1a runs.

use crate::sink;
use noplayout_frames::{Reader, Writer};
use std::time::Instant;

fn stats(mut t: Vec<f64>) -> String {
    t.sort_by(f64::total_cmp);
    let q = |p: f64| t[((t.len() - 1) as f64 * p).round() as usize];
    format!(
        "{{\"p50Ms\":{:.3},\"p95Ms\":{:.3},\"maxMs\":{:.3}}}",
        q(0.5),
        q(0.95),
        q(1.0)
    )
}

/// A moving-graphic-like mix: transparent background, opaque areas and antialiased edges.
fn source(w: u32, h: u32) -> Vec<u8> {
    let mut src = vec![0u8; (w * h * 4) as usize];
    for (i, px) in src.chunks_mut(4).enumerate() {
        let a = match i % 7 {
            0..=2 => 0u8,
            3..=5 => 255,
            _ => 128,
        };
        px.copy_from_slice(&[a / 2, a / 3, a / 4, a]);
    }
    src
}

pub fn run(iterations: usize) -> Result<String, String> {
    let mut rows = Vec::new();
    for (w, h) in [(1920u32, 1080u32), (3840, 2160)] {
        let src = source(w, h);
        let name = format!("npb{:x}{}", std::process::id(), w);
        let mut writer = Writer::create(&name, 1, w, h).map_err(|e| e.to_string())?;
        let reader = Reader::open(&name).map_err(|e| e.to_string())?;
        let (mut copy, mut read, mut conv) = (Vec::new(), Vec::new(), Vec::new());
        for i in 0..iterations {
            let t0 = Instant::now();
            let mut slot = writer.begin().ok_or("no free ring slot")?;
            slot.pixels()[..src.len()].copy_from_slice(&src);
            slot.publish(i as u64 + 1, w, h, w * 4);
            copy.push(t0.elapsed().as_secs_f64() * 1e3);

            // What the pacer does per slot: take the newest paint and convert it once.
            let t1 = Instant::now();
            let s = reader.take_newest(u64::MAX).ok_or("no paint read")?;
            let m = s.meta;
            let c = sink::convert(s.pixels(), m.width, m.height, m.stride);
            drop(s);
            read.push(t1.elapsed().as_secs_f64() * 1e3);
            std::hint::black_box(&c);

            let t2 = Instant::now();
            std::hint::black_box(sink::convert(&src, w, h, w * 4));
            conv.push(t2.elapsed().as_secs_f64() * 1e3);
        }
        drop(reader);
        drop(writer);
        let _ = noplayout_frames::shm::unlink(&name);
        rows.push(format!(
            "{{\"width\":{w},\"height\":{h},\"ringCopy\":{},\"readAndConvert\":{},\"convert\":{}}}",
            stats(copy),
            stats(read),
            stats(conv)
        ));
    }
    Ok(format!(
        "{{\"spike\":\"S2\",\"iterations\":{iterations},\"sizes\":[{}]}}",
        rows.join(",")
    ))
}

/// The host's timer as the pacer sees it: lateness of the precise wait (`clock::wait_until`)
/// and of a plain 1 ms sleep, 500 samples each. A coalescing shell (launchd timer
/// coalescing under a gateway or SSH session) shows here as a ~5 ms sleep; the results file
/// records it so a number's provenance is never in doubt.
pub fn timer() -> String {
    use noplayout_frames::clock;
    let mut wait = Vec::new();
    let mut sleep = Vec::new();
    for _ in 0..500 {
        let late = clock::wait_until(clock::mono_ns() + 1_000_000);
        wait.push(late as f64 / 1e6);
        let t0 = Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        sleep.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    format!(
        "{{\"probe\":\"timer\",\"preciseWaitLateness\":{},\"sleep1ms\":{}}}",
        stats(wait),
        stats(sleep)
    )
}
