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

const USAGE: &str = "usage: noplayout-engine [--port N] | --self-test | --version";

#[derive(Debug)]
enum Mode {
    Run { port: u16 },
    SelfTest,
    Version,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Mode, String> {
    let mut port = DEFAULT_PORT;
    let mut self_test = false;
    let mut it = args.into_iter();
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
    let state = Arc::new(ApiState {
        health: health.clone(),
        ui_token: std::env::var(ENV_UI_TOKEN).ok().filter(|t| !t.is_empty()),
        shutdown: stop.clone(),
    });
    let server_stop = Arc::new(Notify::new());
    let server = tokio::spawn(http::serve(listener, state, server_stop.clone()));

    // One channel in M0, spawn order per PLAN §4.2: output process first, then the renderer.
    let channel = "1";
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

    tokio::select! {
        _ = stop.notified() => tracing::info!("stopping: API request"),
        r = tokio::signal::ctrl_c() => match r {
            Ok(()) => tracing::info!("stopping: interrupt"),
            Err(e) => tracing::error!("cannot listen for Ctrl+C: {e}"),
        },
    }

    for child in [renderer, output].into_iter().flatten() {
        match child.shutdown().await {
            Ok(status) => tracing::info!(kind = child.kind.label(), %status, "stopped"),
            Err(e) => tracing::warn!(kind = child.kind.label(), "{e}"),
        }
    }
    server_stop.notify_one();
    match server.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::error!("HTTP server: {e}"),
        Err(e) => tracing::error!("HTTP server task: {e}"),
    }
    tracing::info!("engine stopped");
    Ok(ExitCode::SUCCESS)
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
