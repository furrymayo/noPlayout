//! Per-run analysis and thresholds (PLAN §5.5). Input is what a receiver sees: per frame the
//! timecode and the decoded patches. Nothing here reads noPlayout's own counters.

use serde::Serialize;

/// One received frame (FILL), with the KEY of the same timecode if one arrived.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Observed {
    pub tc_100ns: i64,
    /// Patch A and B: `(n, s_us)`; `None` = unreadable.
    pub a: Option<(u32, u64)>,
    pub b: Option<(u32, u64)>,
    /// KEY plane: the alpha patch's `n` and the ramp error count; `None` = no KEY with this tc.
    pub key: Option<(Option<u32>, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Mode {
    A,
    B,
}

#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub mode: Mode,
    /// Output rate `num/den`.
    pub num: u32,
    pub den: u32,
    /// Frames ignored at the start.
    pub warmup: usize,
}

impl Spec {
    pub fn period_us(&self) -> f64 {
        f64::from(self.den) * 1e6 / f64::from(self.num)
    }
    fn period_100ns(&self) -> f64 {
        self.period_us() * 10.0
    }
    /// Nominal source period: the output period in mode A, `1/ceil(fps)` in mode B.
    pub fn nominal_src_us(&self) -> f64 {
        match self.mode {
            Mode::A => self.period_us(),
            Mode::B => 1e6 / f64::from(self.num.div_ceil(self.den)),
        }
    }
    /// 59.94 / 29.97 and friends: mode B rate-converts 1000:1001.
    fn fractional(&self) -> bool {
        self.den == 1001
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub frames: usize,
    pub repeats: u32,
    pub skips: u32,
    pub backwards: u32,
    pub tears: u32,
    pub key_mismatch: u32,
    /// KEY frames missing for a FILL timecode (one plane only).
    pub unpaired: u32,
    pub ramp_errors: u32,
    pub timecode_errors: u32,
    /// KEY-plane timecode steps that are not one period.
    pub key_timecode_errors: u32,
    pub step_errors: u32,
    pub ds_nonpositive: u32,
    pub unreadable: u32,
    pub src_period_us: f64,
    pub src_period_ok: bool,
    pub lag_band_ms: f64,
    pub lag_band_limit_ms: f64,
    /// Skip indices (frame numbers after warm-up), for the B row's spacing rule.
    pub skip_at: Vec<usize>,
    pub failures: Vec<String>,
    pub pass: bool,
}

/// Analyse FILL frames in arrival order plus the KEY plane's timecodes in arrival order.
pub fn analyze(spec: &Spec, frames: &[Observed], key_tcs: &[i64]) -> Report {
    let mut r = Report::default();
    let rows: Vec<&Observed> = frames.iter().skip(spec.warmup).collect();
    r.frames = rows.len();
    let p100 = spec.period_100ns();
    let tc_ok = |d: i64| (d as f64 - p100).abs() <= 1.0;

    // Decoded (n, s) per frame: A when readable.
    let mut ns: Vec<Option<(i64, f64)>> = Vec::with_capacity(rows.len());
    for f in &rows {
        match (f.a, f.b) {
            (Some(a), Some(b)) => {
                if a != b {
                    r.tears += 1;
                }
            }
            _ => r.unreadable += 1,
        }
        match &f.key {
            None => r.unpaired += 1,
            Some((alpha, ramp)) => {
                r.ramp_errors += ramp;
                match (alpha, f.a) {
                    (Some(n), Some((na, _))) if *n != na => r.key_mismatch += 1,
                    (None, _) => r.unreadable += 1,
                    _ => {}
                }
            }
        }
        ns.push(f.a.map(|(n, s)| (i64::from(n), s as f64)));
    }

    // Timecode steps; a frame entered by a wrong step is left out of the lag band.
    let mut bad_tc = vec![false; rows.len()];
    for j in 1..rows.len() {
        if !tc_ok(rows[j].tc_100ns - rows[j - 1].tc_100ns) {
            r.timecode_errors += 1;
            bad_tc[j] = true;
        }
    }
    let keys: Vec<i64> = key_tcs.iter().copied().skip(spec.warmup).collect();
    r.key_timecode_errors = keys.windows(2).filter(|w| !tc_ok(w[1] - w[0])).count() as u32;

    // Δn classes and the source period.
    let pairs: Vec<(usize, i64, f64)> = (1..rows.len())
        .filter_map(|j| match (ns[j - 1], ns[j]) {
            (Some((n0, s0)), Some((n1, s1))) => Some((j, n1 - n0, s1 - s0)),
            _ => None,
        })
        .collect();
    let mut ratios: Vec<f64> = pairs
        .iter()
        .filter(|p| p.1 >= 1)
        .map(|p| p.2 / p.1 as f64)
        .collect();
    ratios.sort_by(f64::total_cmp);
    let p_hat = ratios.get(ratios.len() / 2).copied().unwrap_or(0.0);
    r.src_period_us = p_hat;
    r.src_period_ok = (p_hat - spec.nominal_src_us()).abs() <= spec.nominal_src_us() * 0.005;
    for &(j, dn, ds) in &pairs {
        match dn {
            0 => r.repeats += 1,
            d if d < 0 => r.backwards += 1,
            d if d >= 2 => {
                r.skips += 1;
                r.skip_at.push(j);
            }
            _ => {}
        }
        if dn >= 1 && (ds - dn as f64 * p_hat).abs() > p_hat / 4.0 {
            r.step_errors += 1;
        }
        if dn >= 1 && ds <= 0.0 {
            r.ds_nonpositive += 1;
        }
    }

    // Lag band in µs: tc − s, constant offset irrelevant.
    let lags: Vec<f64> = (0..rows.len())
        .filter(|&j| !bad_tc[j])
        .filter_map(|j| ns[j].map(|(_, s)| rows[j].tc_100ns as f64 / 10.0 - s))
        .collect();
    let band = match (
        lags.iter().copied().reduce(f64::min),
        lags.iter().copied().reduce(f64::max),
    ) {
        (Some(lo), Some(hi)) => hi - lo,
        _ => 0.0,
    };
    r.lag_band_ms = band / 1000.0;
    let limit_us = match spec.mode {
        Mode::A => spec.period_us() / 4.0,
        Mode::B => 1.25 * p_hat,
    };
    r.lag_band_limit_ms = limit_us / 1000.0;

    // Thresholds (§5.5 table).
    let mut fail = |what: &str, bad: bool| {
        if bad {
            r.failures.push(what.to_owned());
        }
    };
    fail("repeats", r.repeats > 0);
    fail("backwards", r.backwards > 0);
    fail("tears", r.tears > 0);
    fail("keyMismatch", r.key_mismatch > 0 || r.ramp_errors > 0);
    fail("unpaired", r.unpaired > 0);
    fail("timecode", r.timecode_errors > 0);
    fail("keyTimecode", r.key_timecode_errors > 0);
    fail("stepErrors", r.step_errors > 0);
    fail("unreadable", r.unreadable > 0);
    fail("sourcePeriod", !r.src_period_ok);
    fail("lagBand", band > limit_us + 1.0);
    let skips_ok = match spec.mode {
        Mode::A => r.skips == 0,
        Mode::B if spec.fractional() => {
            let max = r.frames.div_ceil(1000) as u32 + 1;
            let all_two = pairs.iter().filter(|p| p.1 >= 2).all(|p| p.1 == 2);
            let spaced = r.skip_at.windows(2).all(|w| w[1] - w[0] >= 900);
            r.skips <= max && all_two && spaced
        }
        Mode::B => r.skips == 0,
    };
    fail("skips", !skips_ok);
    r.pass = r.failures.is_empty();
    r
}
