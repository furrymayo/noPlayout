//! `noplayout-engine`: the headless engine (E-001).
//!
//! ```text
//! noplayout-engine [--port N]   run the engine (default port 8686, loopback only in M0)
//! noplayout-engine --self-test  launch an output process and a renderer, check them, exit
//! ```

use noplayout_core::children::{self, Kind, Launch};
use noplayout_core::health::HealthState;
use noplayout_core::http::{self, ApiState};
use noplayout_core::instance::InstanceLock;
use noplayout_core::{DEFAULT_PORT, ENV_UI_TOKEN, logging, paths, selftest};
use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::Arc;
use tokio::sync::Notify;

/// Exit status when another engine holds the lock.
const EXIT_ALREADY_RUNNING: u8 = 3;

const USAGE: &str =
    "usage: noplayout-engine [--port N] | --self-test | --version | ctl <op> … (ctl --help)";

#[derive(Debug)]
enum Mode {
    Run { port: u16 },
    SelfTest,
    Version,
    Ctl(Vec<String>),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Mode, String> {
    let mut port = DEFAULT_PORT;
    let mut self_test = false;
    let mut it = args.into_iter().peekable();
    if it.peek().is_some_and(|a| a == "ctl") {
        return Ok(Mode::Ctl(it.skip(1).collect()));
    }
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--self-test" => self_test = true,
            "--version" | "-V" => return Ok(Mode::Version),
            "--port" => {
                let v = it.next().ok_or("--port needs a value")?;
                port = v.parse().map_err(|_| format!("--port {v}: not a port"))?;
            }
            "--help" | "-h" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    Ok(if self_test {
        Mode::SelfTest
    } else {
        Mode::Run { port }
    })
}

fn main() -> ExitCode {
    let mode = match parse_args(std::env::args().skip(1)) {
        Ok(m) => m,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("noplayout-engine: could not start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match mode {
        Mode::Ctl(args) => ctl(&args),
        Mode::Version => {
            println!("noplayout-engine {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Mode::SelfTest => rt.block_on(self_test()),
        Mode::Run { port } => match rt.block_on(run(port)) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("noplayout-engine: {e:#}");
                tracing::error!("{e:#}");
                ExitCode::FAILURE
            }
        },
    }
}

/// `noplayout-engine ctl …`: one dev-control request to a running engine (M1 task 3).
fn ctl(args: &[String]) -> ExitCode {
    let (port, req) = match noplayout_core::ctl::parse(args) {
        Ok(r) => r,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    let Some(token) = std::env::var(ENV_UI_TOKEN).ok().filter(|t| !t.is_empty()) else {
        eprintln!(
            "noplayout-engine ctl: set {ENV_UI_TOKEN} to the token the engine was started with"
        );
        return ExitCode::from(2);
    };
    let port = port.unwrap_or(DEFAULT_PORT);
    match noplayout_core::client::post_ctl(port, &token, &req, std::time::Duration::from_secs(20)) {
        Ok(body) => {
            println!("{}", body.trim());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("noplayout-engine ctl: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn self_test() -> ExitCode {
    let root = std::env::temp_dir().join(format!(
        "noplayout-selftest-{}-{}",
        std::process::id(),
        noplayout_core::new_token()
    ));
    let output = children::default_exe(Kind::Output);
    let renderer = children::default_exe(Kind::Renderer);
    println!("self-test: output   {}", output.display());
    println!("self-test: renderer {}", renderer.display());
    let report = selftest::run(output, renderer, &root).await;
    for c in &report.checks {
        println!(
            "self-test: {} {:<30} {}",
            if c.ok { "PASS" } else { "FAIL" },
            c.name,
            c.detail
        );
    }
    if let Ok(json) = serde_json::to_string(&report) {
        println!("self-test: json {json}");
    }
    let _ = std::fs::remove_dir_all(&root);
    if report.ok {
        println!("self-test: PASS");
        ExitCode::SUCCESS
    } else {
        println!("self-test: FAIL");
        ExitCode::FAILURE
    }
}

async fn run(port: u16) -> anyhow::Result<ExitCode> {
    let data = paths::data_dir()?;
    let mut lock = match InstanceLock::acquire(&data) {
        Ok(lock) => lock,
        Err(e @ noplayout_core::instance::LockError::Held { .. }) => {
            eprintln!("noplayout-engine: {e}");
            return Ok(ExitCode::from(EXIT_ALREADY_RUNNING));
        }
        Err(e) => return Err(e.into()),
    };
    let _log_guard = logging::init(&paths::log_dir(&data), "engine")?;

    let listener = http::bind(port).await?;
    let bound = listener.local_addr()?;
    lock.record(bound.port())?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        data = %data.display(),
        lock = %lock.path().display(),
        "engine starting"
    );
    // Machine-readable line for supervisors and tests (port 0 picks a free port).
    println!("noplayout-engine: listening on http://{bound}");

    let health = HealthState::new();
    let stop = Arc::new(Notify::new());
    let channel = "1";
    let time = noplayout_core::time::TimeSource::new();
    let run_id = time.run().to_string();
    // Paint rings of a crashed engine outlive it on Unix; sweep them before creating new ones.
    let rings = noplayout_frames::shm::Registry::new(data.join("rings.txt"));
    match rings.sweep() {
        Ok(0) => {}
        Ok(n) => tracing::warn!("removed {n} paint ring(s) left by an earlier engine"),
        Err(e) => tracing::warn!("cannot sweep old paint rings: {e}"),
    }
    // TODO(M4): shows/<show-id>.onair.jsonl per open show; M1 journals the dev show. Lines are
    // flushed per record and fsynced on a 200 ms debounce by the writer thread.
    let (onair, _onair_thread) =
        noplayout_core::hostbus::onair::OnAirLog::open(data.join("shows").join("dev.onair.jsonl"))?;
    let (bus_events, mut bus_rx) = tokio::sync::mpsc::unbounded_channel();
    let bus =
        noplayout_core::hostbus::HostBus::start(&[channel], time, Some(onair), Some(bus_events));
    let (faults, mut fault_rx) = tokio::sync::mpsc::unbounded_channel();
    let state = Arc::new(ApiState {
        library: Some(noplayout_core::library::Library::open(
            data.join("library"),
        )?),
        health: health.clone(),
        ui_token: std::env::var(ENV_UI_TOKEN).ok().filter(|t| !t.is_empty()),
        shutdown: stop.clone(),
        hostbus: Some(bus.clone()),
        host_dir: paths::host_dir(),
        faults: Some(faults),
    });
    let server_stop = Arc::new(Notify::new());
    let server = tokio::spawn(http::serve(listener, state, server_stop.clone()));

    // One channel in M0, spawn order per PLAN §4.2: output process first, then the renderer.
    let output = children::launch(
        Launch {
            kind: Kind::Output,
            exe: children::default_exe(Kind::Output),
            args: vec!["--channel".into(), channel.into()],
        },
        health.output(),
    )
    .await
    .ok();
    let cache = paths::cef_cache_dir(&data, channel, "a");
    let renderer = children::launch(
        Launch {
            kind: Kind::Renderer,
            exe: children::default_exe(Kind::Renderer),
            args: vec![
                "--channel".into(),
                channel.into(),
                "--slot".into(),
                "a".into(),
                OsString::from("--cache-dir"),
                cache.into_os_string(),
            ],
        },
        health.renderer(),
    )
    .await
    .ok();
    let epoch_mono_ns = match output.as_ref().map(|o| &o.hello) {
        Some(noplayout_proto::Hello::Output(h)) => h.epoch.mono_ns,
        // TODO(M1): task 8's supervisor waits for the output process; until then a renderer
        // without one paces from the engine's own clock.
        _ => noplayout_frames::clock::mono_ns(),
    };
    let mut renderer_events = None;
    if let Some(r) = &renderer {
        renderer_events = r.take_events();
        let spec = noplayout_core::program::ProgramSpec {
            channel,
            run: &run_id,
            port: bound.port(),
            format: noplayout_proto::Format::HD1080P5994,
            mode: noplayout_core::program::drive_mode_from_env(),
            epoch_mono_ns,
            active: true,
        };
        match noplayout_core::program::open(&bus, &rings, r, spec).await {
            Ok(o) => tracing::info!(inc = o.inc, ring = %o.ring, "program page configured"),
            Err(e) => tracing::error!("cannot configure the program page: {e}"),
        }
    }
    let renderer_event = async |rx: &mut Option<
        tokio::sync::mpsc::UnboundedReceiver<noplayout_proto::Event>,
    >| match rx {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    };

    // Dev-control faults and bus events. TODO(M1): task 8's supervisor owns these (standby,
    // promotion, rebuild); until then a fault acts on the single renderer directly.
    let term = terminate_signal();
    tokio::pin!(term);
    loop {
        tokio::select! {
            _ = stop.notified() => { tracing::info!("stopping: API request"); break; }
            signal = &mut term => { tracing::info!("stopping: {signal}"); break; }
            Some(f) = fault_rx.recv() => match (f, &renderer) {
                (noplayout_core::ctl::Fault::KillRenderer { .. }, Some(r)) => {
                    if let Err(e) = r.kill().await { tracing::warn!("kill-renderer: {e}"); }
                }
                (noplayout_core::ctl::Fault::Hang { ms, .. }, Some(r)) => {
                    if let Err(e) = r.send(noplayout_proto::Down::HangPage { ms }).await {
                        tracing::warn!("hang: {e}");
                    }
                }
                (f, None) => tracing::warn!(?f, "fault ignored: no renderer"),
            },
            Some(ev) = bus_rx.recv() => tracing::warn!(?ev, "bus event (no supervisor yet)"),
            Some(ev) = renderer_event(&mut renderer_events) => match ev {
                noplayout_proto::Event::Console { error: true, text } => {
                    tracing::warn!(target: "program_page", "console error: {text}");
                }
                noplayout_proto::Event::Console { text, .. } => {
                    tracing::debug!(target: "program_page", "{text}");
                }
                other => tracing::info!(?other, "renderer event"),
            },
        }
    }
    drop(bus);

    for child in [renderer, output].into_iter().flatten() {
        match child.shutdown().await {
            Ok(status) => tracing::info!(kind = child.kind.label(), %status, "stopped"),
            Err(e) => tracing::warn!(kind = child.kind.label(), "{e}"),
        }
    }
    let _ = rings.sweep();
    server_stop.notify_one();
    match server.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::error!("HTTP server: {e}"),
        Err(e) => tracing::error!("HTTP server task: {e}"),
    }
    tracing::info!("engine stopped");
    Ok(ExitCode::SUCCESS)
}

/// Resolves on Ctrl+C, and on Unix also on SIGTERM (what launchd and systemd send). If a
/// handler can't be installed, that source never fires instead of stopping the engine at once.
async fn terminate_signal() -> &'static str {
    let interrupt = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => "interrupt",
            Err(e) => {
                tracing::error!("cannot listen for Ctrl+C: {e}");
                std::future::pending().await
            }
        }
    };
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => tokio::select! {
                s = interrupt => s,
                _ = term.recv() => "SIGTERM",
            },
            Err(e) => {
                tracing::error!("cannot listen for SIGTERM: {e}");
                interrupt.await
            }
        }
    }
    #[cfg(not(unix))]
    interrupt.await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_modes() {
        assert!(matches!(
            parse_args(args(&[])),
            Ok(Mode::Run { port: DEFAULT_PORT })
        ));
        assert!(matches!(
            parse_args(args(&["--port", "9001"])),
            Ok(Mode::Run { port: 9001 })
        ));
        assert!(matches!(
            parse_args(args(&["--self-test"])),
            Ok(Mode::SelfTest)
        ));
        assert!(parse_args(args(&["--port"])).is_err());
        assert!(parse_args(args(&["--port", "x"])).is_err());
        assert!(parse_args(args(&["--bogus"])).is_err());
    }
}
