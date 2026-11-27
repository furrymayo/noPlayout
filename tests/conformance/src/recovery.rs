//! M1 task 8, functional half: the supervisor's recovery paths against the real renderer.
//! Timing (S7a's fail lines at paint level) is not asserted here; CI runners are not reference
//! hardware (D-019/D-020).
//!
//! One engine with a warm standby runs the Basketball scorebug: kill the active renderer (K1),
//! then hang its page (K3a), each must warm-promote a ready standby and keep the graphic loaded
//! with its state; two kills within 10 s trip the loop guard into a cold restore; the graphic
//! still answers `play` afterwards. A second engine without a standby (`NOPLAYOUT_STANDBY=0`)
//! must cold-restore a K1.

use crate::engine::{Engine, Opts};
use serde_json::Value;
use std::path::Path;
use std::time::{Duration, Instant};

const LAYER: &str = "10";

/// Poll the supervisor until `ok` holds (or `within` passes); the last view either way.
fn until(e: &Engine, within: Duration, ok: impl Fn(&Value) -> bool) -> (bool, Value) {
    let t0 = Instant::now();
    loop {
        let v = e.supervisor().unwrap_or(Value::Null);
        if ok(&v) {
            return (true, v);
        }
        if t0.elapsed() > within {
            return (false, v);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn records(v: &Value) -> Vec<(String, String)> {
    v["records"]
        .as_array()
        .map(|r| {
            r.iter()
                .map(|x| {
                    (
                        x["kind"].as_str().unwrap_or("?").to_owned(),
                        x["path"].as_str().unwrap_or("?").to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn loaded(e: &Engine) -> bool {
    e.ctl(&["inspect"]).is_ok_and(|v| {
        v["instances"]
            .as_array()
            .is_some_and(|i| i.iter().any(|i| i["layer"] == 10 && i["phase"] == "loaded"))
    })
}

fn ready(v: &Value) -> bool {
    v["standbyReady"] == true
}

pub struct Outcome {
    pub name: &'static str,
    pub failures: Vec<String>,
}

fn start(
    bin: &Path,
    host: &Path,
    work: &Path,
    sub: &str,
    port: u16,
    standby: bool,
    graphic: &Path,
) -> Result<Engine, String> {
    let e = Engine::start(Opts {
        bin,
        host,
        port,
        dir: work.join(sub),
        env: vec![(
            "NOPLAYOUT_STANDBY".into(),
            if standby { "1" } else { "0" }.into(),
        )],
    })?;
    let m = e.import(graphic)?;
    for args in [vec!["load", LAYER, &m], vec!["play", LAYER]] {
        let v = e.ctl(&args)?;
        if v["result"]["statusCode"] != 200 {
            return Err(format!("{}: {v}", args.join(" ")));
        }
    }
    Ok(e)
}

pub fn warm(bin: &Path, host: &Path, work: &Path, graphic: &Path) -> Outcome {
    let mut f = Vec::new();
    let r = (|| -> Result<(), String> {
        let e = start(bin, host, work, "recovery-warm", 18740, true, graphic)?;
        let check = |f: &mut Vec<String>, what: &str, want: (&str, &str), n: usize| {
            let (ok, v) = until(&e, Duration::from_secs(5), |v| records(v).len() >= n);
            let recs = records(&v);
            if !ok || recs.get(n - 1).map(|(k, p)| (k.as_str(), p.as_str())) != Some(want) {
                f.push(format!("{what}: expected record {want:?}, got {recs:?}"));
            }
            if !loaded(&e) {
                f.push(format!(
                    "{what}: the graphic is no longer loaded after recovery"
                ));
            }
        };
        for (fault, want) in [
            (vec!["kill-renderer"], ("K1", "warm")),
            (vec!["hang", "3000"], ("K3a", "warm")),
        ] {
            let (ok, v) = until(&e, Duration::from_secs(20), ready);
            if !ok {
                f.push(format!("standby not ready before {}: {v}", fault[0]));
            }
            let n = records(&v).len() + 1;
            e.ctl(&fault)?;
            check(&mut f, fault[0], want, n);
        }
        // Loop guard: a second kill within 10 s of a promotion is not promoted again.
        let (_, v) = until(&e, Duration::from_secs(20), ready);
        let n = records(&v).len();
        e.ctl(&["kill-renderer"])?;
        until(&e, Duration::from_secs(5), |v| records(v).len() > n);
        e.ctl(&["kill-renderer"])?;
        let (ok, v) = until(&e, Duration::from_secs(15), |v| {
            records(v).get(n + 1).is_some_and(|r| r.1 == "cold")
                && v["activeInc"].is_u64()
                && v["coldRestores"] == 1
        });
        if !ok {
            f.push(format!(
                "loop guard: expected warm then cold, got {:?} ({v})",
                records(&v)
            ));
        }
        std::thread::sleep(Duration::from_millis(1500));
        if !loaded(&e) {
            f.push("loop guard: the graphic is not loaded after the cold restore".into());
        }
        let p = e.ctl(&["play", LAYER])?;
        if p["result"]["statusCode"] != 200 {
            f.push(format!("play after recovery: {p}"));
        }
        if !f.is_empty() {
            f.push(e.diagnose());
        }
        e.stop()
    })();
    if let Err(e) = r {
        f.push(e);
    }
    Outcome {
        name: "recovery/warm-and-loop-guard",
        failures: f,
    }
}

pub fn cold(bin: &Path, host: &Path, work: &Path, graphic: &Path) -> Outcome {
    let mut f = Vec::new();
    let r = (|| -> Result<(), String> {
        let e = start(bin, host, work, "recovery-cold", 18741, false, graphic)?;
        e.ctl(&["kill-renderer"])?;
        let (ok, v) = until(&e, Duration::from_secs(15), |v| {
            records(v).first().is_some_and(|r| r.1 == "cold") && v["records"][0]["newInc"].is_u64()
        });
        if !ok {
            f.push(format!("expected a cold restore, got {v}"));
        }
        std::thread::sleep(Duration::from_millis(1500));
        if !loaded(&e) {
            f.push("the graphic is not loaded after the cold restore".into());
        }
        if !f.is_empty() {
            f.push(e.diagnose());
        }
        e.stop()
    })();
    if let Err(e) = r {
        f.push(e);
    }
    Outcome {
        name: "recovery/cold-no-standby",
        failures: f,
    }
}
