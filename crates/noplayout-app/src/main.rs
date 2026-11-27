//! `noplayout-app`: the Tauri v2 desktop shell (D-001, E-003).
//!
//! On start it attaches to an engine already answering on the port, or spawns
//! `noplayout-engine` next to its own executable with a fresh launch token. The window shows the
//! engine's health (PLAN §7 M0 task 5). On exit it stops the engine only if it started it.
//
// TODO(M4): settings-driven port, Output Setup, real UI to Usopp's mockups. (STATUS.md)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use noplayout_core::client;
use noplayout_core::health::Health;
use serde::Serialize;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Manager, RunEvent};

/// Environment override for the engine executable.
const ENV_ENGINE: &str = "NOPLAYOUT_ENGINE";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    /// This app started the engine and will stop it.
    Spawned,
    /// An engine was already answering; this app leaves it running.
    Attached,
    /// No engine is running and starting one failed.
    None,
}

#[derive(Debug, Clone, Serialize)]
struct EngineStatus {
    mode: Mode,
    port: u16,
    health: Option<Health>,
    error: Option<String>,
}

#[derive(Debug)]
struct Engine {
    mode: Mode,
    port: u16,
    child: Option<Child>,
    token: Option<String>,
    error: Option<String>,
}

fn engine_exe() -> PathBuf {
    if let Some(p) = std::env::var_os(ENV_ENGINE) {
        return PathBuf::from(p);
    }
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_default();
    dir.join(format!("noplayout-engine{}", std::env::consts::EXE_SUFFIX))
}

/// Attach to a running engine, or start one and wait until it answers.
fn start_or_attach(port: u16) -> Engine {
    let quick = Duration::from_millis(500);
    if client::get_health(port, quick).is_ok() {
        return Engine {
            mode: Mode::Attached,
            port,
            child: None,
            token: None,
            error: None,
        };
    }
    let exe = engine_exe();
    let token = noplayout_core::new_token();
    let spawned = Command::new(&exe)
        .args(["--port", &port.to_string()])
        .env(noplayout_core::ENV_UI_TOKEN, &token)
        .spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            return Engine {
                mode: Mode::None,
                port,
                child: None,
                token: None,
                error: Some(format!(
                    "could not start the engine at {}: {e}",
                    exe.display()
                )),
            };
        }
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if client::get_health(port, quick).is_ok() {
            return Engine {
                mode: Mode::Spawned,
                port,
                child: Some(child),
                token: Some(token),
                error: None,
            };
        }
        if let Ok(Some(status)) = child.try_wait() {
            let why = if status.code() == Some(3) {
                "another engine is already running with this data directory".to_owned()
            } else {
                format!("it exited with {status}")
            };
            return Engine {
                mode: Mode::None,
                port,
                child: None,
                token: None,
                error: Some(format!("the engine did not start: {why}")),
            };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Engine {
        mode: Mode::Spawned,
        port,
        child: Some(child),
        token: Some(token),
        error: Some(format!("the engine has not answered on port {port} yet")),
    }
}

/// Stop the engine if this app started it: ask over the API, then wait, then kill.
fn stop(engine: &mut Engine) {
    let (Some(mut child), Some(token)) = (engine.child.take(), engine.token.take()) else {
        return;
    };
    if let Err(e) = client::post_shutdown(engine.port, &token, Duration::from_secs(2)) {
        eprintln!("noplayout-app: shutdown request failed: {e}");
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    eprintln!("noplayout-app: the engine did not stop within 15 s; killing it");
    let _ = child.kill();
    let _ = child.wait();
}

#[tauri::command]
async fn engine_status(state: tauri::State<'_, Mutex<Engine>>) -> Result<EngineStatus, String> {
    let (mode, port, error) = {
        let e = state.lock().map_err(|_| "engine state lock poisoned")?;
        (e.mode, e.port, e.error.clone())
    };
    let health = tauri::async_runtime::spawn_blocking(move || {
        client::get_health(port, Duration::from_secs(2))
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(match health {
        Ok(h) => EngineStatus {
            mode,
            port,
            health: Some(h),
            error: None,
        },
        Err(e) => EngineStatus {
            mode,
            port,
            health: None,
            error: Some(error.unwrap_or_else(|| e.to_string())),
        },
    })
}

fn main() {
    let engine = start_or_attach(noplayout_core::DEFAULT_PORT);
    if let Some(e) = &engine.error {
        eprintln!("noplayout-app: {e}");
    }
    let app = tauri::Builder::default()
        .manage(Mutex::new(engine))
        .invoke_handler(tauri::generate_handler![engine_status])
        .build(tauri::generate_context!())
        .expect("the noPlayout window could not be created");
    app.run(|handle, event| {
        if let RunEvent::Exit = event
            && let Some(state) = handle.try_state::<Mutex<Engine>>()
            && let Ok(mut engine) = state.lock()
        {
            stop(&mut engine);
        }
    });
}
