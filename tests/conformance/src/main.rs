//! `noplayout-conformance [conformance | negative | pixels | recovery | all] [--bless] [--only NAME]
//! [--bin DIR] [--host DIR]`: M1 tasks 9 and 10 against the built engine, output process and CEF
//! renderer (`cargo xtask conformance` builds them first). Exit 0 when everything passes,
//! including the negative controls failing as they must. Candidate PNGs
//! of failed or missing goldens, and `report.json`, go to `target/conformance/`; `--bless`
//! writes the goldens instead.

mod conformance;
mod engine;
mod pixels;
mod recovery;
#[cfg(test)]
mod tests;

use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

const EBU: [&str; 9] = [
    "bar-chart",
    "headline",
    "l3rd-name",
    "minimal",
    "ograf-logo",
    "renderer-test",
    "responsive-lower-third",
    "scoreboard",
    "weather",
];
const BASKETBALL: [&str; 6] = [
    "bball-lower-third",
    "bball-matchup",
    "bball-scorebug",
    "bball-sponsor-bug",
    "bball-stat-card",
    "bball-ticker",
];
/// Basketball per-OS goldens may differ in this many pixels beyond ±2 LSB (text rasterisation
/// noise; measured 0–2 px run to run, ADR 0064).
const TEXT_GOLDEN_BUDGET: usize = 32;
/// The scorebug with a changed score, compared with the unchanged golden, must fail (ADR 0064).
const CHANGED_SCORE: &str = "bball-scorebug-changed-score";

/// Pixels a case may differ from its golden beyond ±2 LSB: geometry goldens (`shape`, and the
/// per-OS `gradient-shape`) none, Basketball text goldens [`TEXT_GOLDEN_BUDGET`].
fn budget(group: &str, name: &str) -> usize {
    if group == "shape" || name == "gradient-shape" {
        0
    } else {
        TEXT_GOLDEN_BUDGET
    }
}

/// A8's negative controls: the fixture's `fault` and the text its detector's failure must carry.
const NEGATIVE: [(&str, &str); 4] = [
    (
        "console-error",
        "console error: negative-control: deliberate console.error",
    ),
    (
        "uncaught",
        "negative-control: deliberate uncaught exception",
    ),
    ("status", "\"statusCode\":500"),
    ("wrong-step", "play #1: currentStep 1, expected 0"),
];

/// A negative control passes when the harness failed the graphic with the intended detector.
fn negative_verdict(fault: &str, detector: &str, failures: &[String]) -> Vec<String> {
    if failures.iter().any(|f| f.contains(detector)) {
        vec![]
    } else {
        vec![format!(
            "the harness did not fail `{fault}` with its detector ({detector}); got {failures:?}"
        )]
    }
}

struct Args {
    what: String,
    bless: bool,
    only: Option<String>,
    root: PathBuf,
    bin: PathBuf,
    host: PathBuf,
}

fn args() -> Result<Args, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().unwrap_or(root);
    let mut a = Args {
        what: "all".into(),
        bless: false,
        only: None,
        bin: std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .ok_or("cannot locate the binaries")?,
        host: root.join("web/dist/host"),
        root,
    };
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "conformance" | "negative" | "pixels" | "recovery" | "all" => a.what = s,
            "--bless" => a.bless = true,
            "--only" => a.only = it.next(),
            "--bin" => a.bin = it.next().ok_or("--bin needs a value")?.into(),
            "--host" => a.host = it.next().ok_or("--host needs a value")?.into(),
            _ => return Err(format!("unknown argument {s}")),
        }
    }
    Ok(a)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("noplayout-conformance: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool, String> {
    let a = args()?;
    let out = a.root.join("target/conformance");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let work = std::env::temp_dir().join(format!("np-conformance-{}", std::process::id()));
    let want = |n: &str| a.only.as_deref().is_none_or(|o| o == n);
    let mut report = Vec::new();
    let mut ok = true;
    let mut line = |name: &str, fails: &[String], extra: &str| {
        ok &= fails.is_empty();
        println!(
            "{} {name}{extra}",
            if fails.is_empty() { "ok  " } else { "FAIL" }
        );
        for f in fails {
            println!("       {f}");
        }
        report.push(json!({"name": name, "failures": fails}));
    };

    if a.what == "all" || a.what == "conformance" {
        let mut graphics: Vec<PathBuf> = EBU
            .iter()
            .map(|n| a.root.join("third_party/ograf/v1/examples").join(n))
            .collect();
        graphics.extend(
            BASKETBALL
                .iter()
                .map(|n| a.root.join("fixtures/basketball-ograf-package").join(n)),
        );
        graphics.push(a.root.join("tests/conformance/fixtures/step-unbounded"));
        graphics.retain(|g| want(&g.file_name().unwrap_or_default().to_string_lossy()));
        if !graphics.is_empty() {
            let e = engine::Engine::start(engine::Opts {
                bin: &a.bin,
                host: &a.host,
                port: 18730,
                dir: work.join("conformance"),
                env: vec![],
            })?;
            for (i, g) in graphics.iter().enumerate() {
                let o = conformance::run(&e, g, 10 + i as u32, None);
                line(
                    &format!("conformance/{}", o.name),
                    &o.failures,
                    &format!(" ({} actions)", o.actions),
                );
            }
            e.stop()?;
        }
    }

    // A8: negative controls. The harness must fail a graphic that logs a console error, throws an
    // uncaught exception, answers non-200 or reports the wrong `currentStep`; each control
    // passes only if the matching detector fired.
    if a.what == "all" || a.what == "negative" {
        let controls: Vec<_> = NEGATIVE
            .iter()
            .filter(|(f, _)| want(&format!("negative-{f}")) || want("negative-control"))
            .collect();
        if !controls.is_empty() {
            let dir = a.root.join("tests/conformance/fixtures/negative-control");
            let e = engine::Engine::start(engine::Opts {
                bin: &a.bin,
                host: &a.host,
                port: 18731,
                dir: work.join("negative"),
                env: vec![],
            })?;
            for (i, (fault, detector)) in controls.into_iter().enumerate() {
                let o = conformance::run(&e, &dir, 10 + i as u32, Some(&json!({"fault": fault})));
                let fails = negative_verdict(fault, detector, &o.failures);
                line(&format!("negative/{fault}"), &fails, " (must fail)");
            }
            e.stop()?;
        }
    }

    if a.what == "all" || a.what == "pixels" {
        let os = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let fx = a.root.join("tests/conformance/fixtures");
        let bb = a.root.join("fixtures/basketball-ograf-package");
        let goldens = a.root.join("tests/conformance/goldens");
        let mut cases: Vec<(String, pixels::Case)> = Vec::new();
        for n in ["alpha-ramp", "gradient-shape"] {
            cases.push((
                // The ramp is exact on every OS; the shape fixture's geometry antialiasing differs
                // between rasterisers (Metal vs software GL: about 2,900 edge pixels), so it is
                // per-OS like the text.
                if n == "alpha-ramp" {
                    "shape".into()
                } else {
                    os.clone()
                },
                pixels::Case {
                    name: n,
                    dir: fx.join(n),
                    data: None,
                    update: None,
                    settle: Duration::from_millis(500),
                    frames: 2,
                    before_play: false,
                },
            ));
        }
        for n in BASKETBALL {
            cases.push((
                os.clone(),
                pixels::Case {
                    name: n,
                    dir: bb.join(n),
                    // The ticker cycles items on a timer; paused, it shows its first item.
                    data: (n == "bball-ticker").then(|| json!({"paused": true})),
                    update: None,
                    settle: Duration::from_millis(3000),
                    frames: 2,
                    before_play: false,
                },
            ));
        }
        for (group, c) in cases.iter().filter(|(_, c)| want(c.name)) {
            let name = format!("pixels/{group}/{}", c.name);
            let cap = match pixels::capture(&a.bin, &a.host, &work, c) {
                Ok(cap) => cap,
                Err(e) => {
                    line(&name, &[e], "");
                    continue;
                }
            };
            let Some(img) = cap.frames.last() else {
                line(&name, &["no frames dumped".into()], "");
                continue;
            };
            let mut fails = if c.name == "alpha-ramp" {
                pixels::check_ramp(img)
            } else {
                vec![]
            };
            let golden = goldens.join(group).join(format!("{}.png", c.name));
            let candidate = out
                .join("candidates")
                .join(group)
                .join(format!("{}.png", c.name));
            let mut extra = String::new();
            if a.bless {
                pixels::write_png(&golden, img)?;
                extra = " (blessed)".into();
            } else if !golden.exists() {
                pixels::write_png(&candidate, img)?;
                fails.push(format!(
                    "no golden {group}/{}.png; candidate written",
                    c.name
                ));
            } else {
                let budget = budget(group, c.name);
                let (bad, worst) = pixels::diff(img, &pixels::read_png(&golden)?, 2);
                extra = format!(" ({bad} px beyond ±2, max diff {worst})");
                if bad > budget {
                    pixels::write_png(&candidate, img)?;
                    fails.push(format!(
                        "{bad} pixels beyond ±2 LSB (budget {budget}); candidate written"
                    ));
                }
            }
            line(&name, &fails, &extra);
        }
        // Negative control: a changed score must not pass the scorebug's golden.
        if want(CHANGED_SCORE) && !a.bless {
            let name = format!("pixels/{os}/{CHANGED_SCORE}");
            let golden = goldens.join(&os).join("bball-scorebug.png");
            let c = pixels::Case {
                name: CHANGED_SCORE,
                dir: bb.join("bball-scorebug"),
                data: None,
                update: Some(json!({"awayScore": 77, "homeScore": 88})),
                settle: Duration::from_millis(3000),
                frames: 2,
                before_play: false,
            };
            let mut extra = " (must differ from bball-scorebug's golden)".to_owned();
            let fails = match (
                pixels::read_png(&golden),
                pixels::capture(&a.bin, &a.host, &work, &c),
            ) {
                (Err(e), _) => vec![format!("no golden to compare with: {e}")],
                (_, Err(e)) => vec![e],
                (Ok(g), Ok(cap)) => match cap.frames.last() {
                    None => vec!["no frames dumped".into()],
                    Some(img) => {
                        let (bad, _) = pixels::diff(img, &g, 2);
                        let budget = budget(&os, "bball-scorebug");
                        extra = format!(" ({bad} px beyond ±2; must exceed {budget})");
                        if bad > budget {
                            vec![]
                        } else {
                            vec![format!(
                                "a changed score differs in only {bad} px (budget {budget}): the golden compare cannot see it"
                            )]
                        }
                    }
                },
            };
            line(&name, &fails, &extra);
        }
        if want("scorebug-sequence") {
            let c = pixels::Case {
                name: "scorebug-sequence",
                dir: bb.join("bball-scorebug"),
                data: None,
                update: None,
                settle: Duration::from_millis(300),
                frames: 40,
                before_play: true,
            };
            let fails = match pixels::capture(&a.bin, &a.host, &work, &c) {
                Ok(cap) => pixels::check_scorebug(&cap.frames),
                Err(e) => vec![e],
            };
            line(
                "pixels/scorebug-sequence",
                &fails,
                " (40 slots across the entry animation)",
            );
        }
    }

    if a.what == "all" || a.what == "recovery" {
        let sb = a
            .root
            .join("fixtures/basketball-ograf-package/bball-scorebug");
        for (n, f) in [
            (
                "warm-and-loop-guard",
                recovery::warm as fn(&Path, &Path, &Path, &Path) -> recovery::Outcome,
            ),
            ("cold-no-standby", recovery::cold),
        ] {
            if want(n) {
                let o = f(&a.bin, &a.host, &work, &sb);
                line(o.name, &o.failures, "");
            }
        }
    }

    let _ = std::fs::remove_dir_all(&work);
    let text = serde_json::to_string_pretty(&json!({"ok": ok, "results": report}))
        .map_err(|e| e.to_string())?;
    std::fs::write(out.join("report.json"), text).map_err(|e| e.to_string())?;
    Ok(ok)
}
