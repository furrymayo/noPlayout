use crate::analyze::{Mode, Observed, Spec, analyze};
use crate::decode::{self, Plane};
use crate::layout;

/// FILL (straight BGRA) and KEY of a premultiplied BGRA frame, as the output process makes them.
fn planes(src: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut fill = Vec::with_capacity(src.len());
    let mut key = Vec::with_capacity(src.len() / 4);
    for p in src.chunks(4) {
        let a = u32::from(p[3]);
        for &c in &p[..3] {
            fill.push(
                (u32::from(c) * 255 + a / 2)
                    .checked_div(a)
                    .map_or(0, |v| v.min(255) as u8),
            );
        }
        fill.push(255);
        key.push(p[3]);
    }
    (fill, key)
}

fn crop(data: &[u8], w: u32, bpp: u32, crops: &[[u32; 4]]) -> Vec<u8> {
    let mut out = Vec::new();
    for &[x, y, cw, ch] in crops {
        for row in y..y + ch {
            let s = ((row * w + x) * bpp) as usize;
            out.extend_from_slice(&data[s..s + (cw * bpp) as usize]);
        }
    }
    out
}

#[test]
fn encode_decode_round_trip_whole_and_cropped() {
    let (w, h) = (1920, 1080);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let (n, s) = (0x00ab_cdef, 0x00_fedc_ba98_u64 | (1 << 39));
    layout::paint(&mut buf, w, h, n, s, n);
    let (fill, key) = planes(&buf);
    let f = Plane {
        width: w,
        height: h,
        bpp: 4,
        data: &fill,
        crops: &[],
    };
    let k = Plane {
        width: w,
        height: h,
        bpp: 1,
        data: &key,
        crops: &[],
    };
    assert_eq!(decode::read_patch(&f, layout::patch_a()), Some((n, s)));
    assert_eq!(decode::read_patch(&f, layout::patch_b(w, h)), Some((n, s)));
    assert_eq!(decode::read_alpha(&k), Some(n));
    assert_eq!(decode::ramp_errors(&k), 0);

    let crops = layout::crops(w, h);
    let (fc, kc) = (crop(&fill, w, 4, &crops), crop(&key, w, 1, &crops));
    let f = Plane {
        width: w,
        height: h,
        bpp: 4,
        data: &fc,
        crops: &crops,
    };
    let k = Plane {
        width: w,
        height: h,
        bpp: 1,
        data: &kc,
        crops: &crops,
    };
    assert_eq!(decode::read_patch(&f, layout::patch_b(w, h)), Some((n, s)));
    assert_eq!(decode::read_alpha(&k), Some(n));
    assert_eq!(decode::ramp_errors(&k), 0);

    // A flipped cell breaks parity: unreadable, never a wrong value.
    let mut bad = fill.clone();
    for y in 0..16 {
        for x in 16..32 {
            let i = ((y * w + x) * 4) as usize;
            bad[i..i + 3].iter_mut().for_each(|c| *c = 255 - *c);
        }
    }
    let f = Plane {
        width: w,
        height: h,
        bpp: 4,
        data: &bad,
        crops: &[],
    };
    assert_eq!(decode::read_patch(&f, layout::patch_a()), None);
}

/// PLAN §5.5's model (`oracle_model.py` rev 4), in exact integer arithmetic: output slot `k` at
/// `T_k`, source frame `i` stamped `s_i = φ + i·P_src`, painted 2 ms later, selector "newest
/// paint with painted_at ≤ T_k − 1 ms". Returns what a receiver would decode.
fn model(
    num: u32,
    den: u32,
    src: (i128, i128),
    phi: (i128, i128),
    mutate: Option<&str>,
) -> Vec<Observed> {
    let (sn, sd) = src;
    let (pn, pd) = phi;
    let (lat, guard, g) = (2_000_000i128, 1_000_000i128, 1_000_000_000i128);
    let (warm, frames) = (120usize, 10_000usize);
    let mut rows: Vec<(i128, i128, i128)> = (0..=(warm + frames) as i128)
        .map(|k| {
            let t = k * i128::from(den) * g / i128::from(num) + g;
            let i = ((t - guard - lat) * sn * pd - pn * g * sd).div_euclid(g * sd * pd);
            let s_ns = (pn + i * pd) * g * sd / (pd * sn);
            (t, i, s_ns)
        })
        .collect();
    match mutate {
        Some("freeze") => {
            let base = rows[warm + 500].2;
            for r in &mut rows[warm + 500..warm + 510] {
                r.2 = base;
            }
        }
        Some("repeat") => {
            rows[warm + 500].1 = rows[warm + 499].1;
            rows[warm + 500].2 = rows[warm + 499].2;
        }
        _ => {}
    }
    rows.iter()
        .map(|&(t, i, s)| {
            let n = i as u32;
            let s_us = (s / 1000) as u64;
            Observed {
                tc_100ns: (t / 100) as i64,
                a: Some((n, s_us)),
                b: Some((n, s_us)),
                key: Some((Some(n), 0)),
            }
        })
        .collect()
}

fn spec(mode: Mode, num: u32, den: u32) -> Spec {
    Spec {
        mode,
        num,
        den,
        warmup: 120,
    }
}

const PHASES: [(i128, i128); 5] = [(0, 1), (1, 4), (1, 2), (3, 4), (999, 1000)];

#[test]
fn model_cases_pass_with_the_expected_skips() {
    // (mode, num, den, source rate, expected skips)
    let cases = [
        (Mode::A, 60000, 1001, (60000, 1001), 0),
        (Mode::B, 60000, 1001, (60, 1), 10),
        (Mode::B, 30000, 1001, (30, 1), 10),
        (Mode::B, 50, 1, (50, 1), 0),
        (Mode::B, 25, 1, (25, 1), 0),
    ];
    for (mode, num, den, src, skips) in cases {
        for phi in PHASES {
            let frames = model(num, den, src, phi, None);
            let tcs: Vec<i64> = frames.iter().map(|f| f.tc_100ns).collect();
            let r = analyze(&spec(mode, num, den), &frames, &tcs);
            let what = format!("{mode:?} {num}/{den} from {src:?} phase {phi:?}: {r:?}");
            assert!(r.pass, "{what}");
            assert_eq!(r.skips, skips, "{what}");
            assert_eq!((r.repeats, r.backwards, r.step_errors), (0, 0, 0), "{what}");
            assert!(r.src_period_ok, "{what}");
        }
    }
    // The B 59.94 band is one source period (16.67 ms, bound 20.8).
    let frames = model(60000, 1001, (60, 1), (1, 2), None);
    let r = analyze(&spec(Mode::B, 60000, 1001), &frames, &[]);
    assert!((r.lag_band_ms - 16.667).abs() < 0.01, "{}", r.lag_band_ms);
}

#[test]
fn model_mutations_fail() {
    let s = spec(Mode::B, 60000, 1001);
    let tcs = |f: &[Observed]| f.iter().map(|f| f.tc_100ns).collect::<Vec<_>>();

    let f = model(60000, 1001, (60, 1), (1, 2), Some("freeze"));
    let r = analyze(&s, &f, &tcs(&f));
    assert!(!r.pass);
    assert_eq!(r.step_errors, 10, "{r:?}");
    assert_eq!(r.ds_nonpositive, 9, "{r:?}");
    assert!(r.failures.contains(&"lagBand".to_owned()), "{r:?}");
    assert!((r.lag_band_ms - 165.8).abs() < 0.1, "{}", r.lag_band_ms);

    let f = model(60000, 1001, (60, 1), (1, 2), Some("repeat"));
    let r = analyze(&s, &f, &tcs(&f));
    assert!(!r.pass);
    assert_eq!(r.repeats, 1);
    assert_eq!(r.skips, 11);
    assert!(r.failures.contains(&"lagBand".to_owned()), "{r:?}");
    assert!((r.lag_band_ms - 32.33).abs() < 0.05, "{}", r.lag_band_ms);
}

#[test]
fn receiver_side_defects_are_caught() {
    let base = model(50, 1, (50, 1), (0, 1), None);
    let s = Spec {
        warmup: 0,
        ..spec(Mode::A, 50, 1)
    };
    let run = |f: &[Observed]| analyze(&s, f, &f.iter().map(|f| f.tc_100ns).collect::<Vec<_>>());
    assert!(run(&base[..300]).pass);

    let mut f = base[..300].to_vec();
    f[100].b = f[101].a; // tear
    f[150].key = Some((f[149].a.map(|a| a.0), 0)); // KEY from the previous paint
    f[200].tc_100ns += 1_000; // a wrong timecode
    f[250].a = None; // unreadable
    let r = run(&f);
    assert_eq!(
        (r.tears, r.key_mismatch, r.timecode_errors, r.unreadable),
        (1, 1, 2, 1),
        "{r:?}"
    );
}

#[test]
fn probe_patch_round_trip() {
    let (w, h) = (640, 360);
    let hash = decode::fnv1a(br#"{"label":"probe"}"#);
    for (counter, step) in [(0u16, None), (257, Some(0u8)), (65535, Some(3))] {
        let n = u32::from(counter) | (hash & 0xff) << 16;
        let s = u64::from(hash >> 8) | u64::from(step.map_or(0, |s| s + 1)) << 24;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        layout::paint(&mut buf, w, h, n, s, n);
        let (fill, _) = planes(&buf);
        let p = Plane {
            width: w,
            height: h,
            bpp: 4,
            data: &fill,
            crops: &[],
        };
        assert_eq!(
            decode::read_probe(&p),
            Some(decode::Probe {
                counter,
                hash,
                step
            })
        );
    }
    // The fixture's JS FNV-1a of "{}" (computed with node from the same code).
    assert_eq!(decode::fnv1a(b"{}"), 0x5465_b825);
}
