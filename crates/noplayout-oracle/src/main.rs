//! `noplayout-oracle <dump-dir> --mode A|B [--rate 60000/1001] [--warmup 120]`: decode an
//! output-process paint dump and judge it against PLAN §5.5. Prints one JSON report; exits 0
//! on pass, 1 on fail, 2 on a usage or read error.

use noplayout_oracle::analyze::{Mode, Spec, analyze};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("noplayout-oracle: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool, String> {
    let usage = "usage: noplayout-oracle <dump-dir> --mode A|B [--rate n/d] [--warmup N]";
    let mut dir = None;
    let mut spec = Spec {
        mode: Mode::A,
        num: 60000,
        den: 1001,
        warmup: 120,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--print-crops" => {
                let v = value()?;
                let (w, h) = v
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .ok_or(format!("bad size {v}"))?;
                let c: Vec<String> = noplayout_oracle::layout::crops(w, h)
                    .iter()
                    .map(|r| format!("{},{},{},{}", r[0], r[1], r[2], r[3]))
                    .collect();
                println!("{}", c.join(";"));
                return Ok(true);
            }
            "--mode" => {
                spec.mode = match value()?.as_str() {
                    "A" | "a" => Mode::A,
                    "B" | "b" => Mode::B,
                    m => return Err(format!("bad mode {m}")),
                }
            }
            "--rate" => {
                let v = value()?;
                let (n, d) = v.split_once('/').unwrap_or((&v, "1"));
                spec.num = n.parse().map_err(|_| format!("bad rate {v}"))?;
                spec.den = d.parse().map_err(|_| format!("bad rate {v}"))?;
            }
            "--warmup" => {
                let v = value()?;
                spec.warmup = v.parse().map_err(|_| format!("bad warmup {v}"))?;
            }
            s if !s.starts_with("--") && dir.is_none() => dir = Some(PathBuf::from(s)),
            _ => return Err(usage.into()),
        }
    }
    let dir = dir.ok_or(usage)?;
    let (mut frames, mut key_tcs) = noplayout_oracle::dump::read(&dir)
        .map_err(|e| format!("cannot read the dump in {}: {e}", dir.display()))?;
    // The lead-in: slots before the page first painted a readable frame (loading). A dump that
    // never becomes readable is a failure, reported as all frames unreadable.
    let lead_in = frames.iter().position(|f| f.a.is_some()).unwrap_or(0);
    if let Some(first) = frames.get(lead_in) {
        let tc0 = first.tc_100ns;
        key_tcs.retain(|t| *t >= tc0);
    }
    frames.drain(..lead_in);
    let report = analyze(&spec, &frames, &key_tcs);
    eprintln!("noplayout-oracle: {lead_in} lead-in slots dropped");
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|e| e.to_string())?
    );
    Ok(report.pass)
}
