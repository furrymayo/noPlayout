//! `noplayout-engine --self-test`: launch an output process and a renderer, check their `Hello`,
//! a heartbeat round trip, the renderer's first CEF paint and a clean `Shutdown` (PLAN §7 M0:
//! the headless smoke on lanes without a desktop). Needs no lock, port or data directory.

use crate::children::{self, Kind, Launch};
use crate::health::ChildHealth;
use noplayout_proto::Hello;
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub ok: bool,
    pub checks: Vec<Check>,
}

impl Report {
    fn push(&mut self, name: &str, result: Result<String, String>) -> bool {
        let ok = result.is_ok();
        let detail = result.unwrap_or_else(|e| e);
        self.checks.push(Check {
            name: name.to_owned(),
            ok,
            detail,
        });
        ok
    }
}

/// Run the smoke. `cache_root` holds the renderer's throwaway CEF cache.
pub async fn run(output_exe: PathBuf, renderer_exe: PathBuf, cache_root: &Path) -> Report {
    let mut report = Report {
        ok: false,
        checks: Vec::new(),
    };
    check_output(&mut report, output_exe).await;
    check_renderer(&mut report, renderer_exe, cache_root).await;
    report.ok = report.checks.iter().all(|c| c.ok);
    report
}

fn slot() -> Arc<RwLock<ChildHealth>> {
    Arc::new(RwLock::new(ChildHealth::starting()))
}

async fn check_output(report: &mut Report, exe: PathBuf) {
    let launch = Launch {
        kind: Kind::Output,
        exe,
        args: vec!["--channel".into(), "selftest".into()],
    };
    let child = match children::launch(launch, slot()).await {
        Ok(c) => c,
        Err(e) => {
            report.push("output: hello", Err(e.to_string()));
            return;
        }
    };
    let hello = match &child.hello {
        Hello::Output(h) => Ok(format!(
            "pid {} version {} epoch utc_100ns {}",
            h.pid, h.version, h.epoch.utc_100ns
        )),
        other => Err(format!("wrong hello kind: {other:?}")),
    };
    report.push("output: hello", hello);
    let ping = child.ping(Duration::from_secs(5)).await;
    report.push("output: ping", ping.map(|p| format!("pong seq {}", p.seq)));
    let exit = child.shutdown().await;
    report.push("output: shutdown", exit_ok(exit));
}

async fn check_renderer(report: &mut Report, exe: PathBuf, cache_root: &Path) {
    let cache = cache_root.join("cef-cache");
    let launch = Launch {
        kind: Kind::Renderer,
        exe,
        args: vec![
            "--channel".into(),
            "selftest".into(),
            "--slot".into(),
            "a".into(),
            OsString::from("--cache-dir"),
            cache.into_os_string(),
        ],
    };
    let child = match children::launch(launch, slot()).await {
        Ok(c) => c,
        Err(e) => {
            report.push("renderer: hello", Err(e.to_string()));
            return;
        }
    };
    let hello = match &child.hello {
        Hello::Renderer(h) => Ok(format!(
            "pid {} CEF {} (Chromium {})",
            h.pid, h.cef_version, h.chromium_version
        )),
        other => Err(format!("wrong hello kind: {other:?}")),
    };
    report.push("renderer: hello", hello);

    // A transparent OSR browser on about:blank must paint into a paint ring the engine can read
    // (the M1 path: Configure → ring → on_paint copy).
    let ring =
        noplayout_frames::ring_name(&crate::new_token(), "st", u64::from(std::process::id()));
    let registry = noplayout_frames::shm::Registry::new(cache_root.join("rings.txt"));
    let _ = registry.sweep();
    if let Err(e) = registry.record(&ring) {
        report.push("renderer: configure", Err(e.to_string()));
    }
    let configure = child
        .send(noplayout_proto::Down::Configure(
            noplayout_proto::RendererConfig {
                format: noplayout_proto::Format::HD1080P5994,
                mode: noplayout_proto::DriveMode::B,
                epoch_mono_ns: noplayout_frames::clock::mono_ns(),
                lead_ns: 16_683_333,
                url: "about:blank".into(),
                page_inc: 1,
                page_token: "selftest".into(),
                ring: ring.clone(),
                active: true,
            },
        ))
        .await;
    report.push("renderer: configure", configure.map(|()| ring.clone()));

    let deadline = Instant::now() + Duration::from_secs(10);
    let paint = loop {
        match child.ping(Duration::from_secs(5)).await {
            Ok(p) if p.last_paint_seq > 0 => {
                break Ok(format!("{} paint(s) from CEF", p.last_paint_seq));
            }
            Ok(_) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Ok(_) => break Err("no paint from CEF within 10 s".to_owned()),
            Err(e) => break Err(e),
        }
    };
    report.push("renderer: ping + first paint", paint);
    let in_ring = match noplayout_frames::Reader::open(&ring) {
        Ok(r) => match r.take_newest(u64::MAX) {
            Some(s) if (s.meta.width, s.meta.height) == (1920, 1080) => Ok(format!(
                "paint {} {}x{} in ring, alpha {}",
                s.meta.paint_seq,
                s.meta.width,
                s.meta.height,
                s.pixels()[3]
            )),
            Some(s) => Err(format!("paint is {}x{}", s.meta.width, s.meta.height)),
            None => Err("no paint in the ring".into()),
        },
        Err(e) => Err(format!("cannot open ring {ring}: {e}")),
    };
    report.push("renderer: paint ring", in_ring);
    let exit = child.shutdown().await;
    report.push("renderer: shutdown", exit_ok(exit));
    let _ = registry.sweep();
}

fn exit_ok(r: Result<std::process::ExitStatus, String>) -> Result<String, String> {
    match r {
        Ok(s) if s.success() => Ok("exited cleanly".into()),
        Ok(s) => Err(format!("exited with {s}")),
        Err(e) => Err(e),
    }
}
