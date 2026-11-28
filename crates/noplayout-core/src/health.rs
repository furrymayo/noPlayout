//! The health document served at `GET /api/v1/health` and shown by the app.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Lifecycle of one child process as the engine sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildState {
    /// Not launched yet, or launched and not yet said `Hello`.
    Starting,
    /// Said a valid `Hello` and answers heartbeats.
    Ok,
    /// Its executable was not found.
    Missing,
    /// Failed to start, refused, or stopped answering.
    Failed,
    /// Exited after a `Shutdown`.
    Exited,
}

/// One child process (renderer or output).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildHealth {
    pub state: ChildState,
    pub pid: Option<u32>,
    /// Package version the child reported.
    pub version: Option<String>,
    /// Renderer only: CEF version, e.g. `156.0.31`.
    pub cef_version: Option<String>,
    /// Renderer only: Chromium version under CEF.
    pub chromium_version: Option<String>,
    /// Renderer only: paints CEF delivered so far (from the last `Pong`).
    pub paints: Option<u64>,
    /// Plain-language reason when `state` is `missing` or `failed`.
    pub detail: Option<String>,
}

impl ChildHealth {
    pub fn starting() -> Self {
        Self {
            state: ChildState::Starting,
            pid: None,
            version: None,
            cef_version: None,
            chromium_version: None,
            paints: None,
            detail: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineHealth {
    /// Always `running` when the engine answers.
    pub state: String,
    pub version: String,
    pub pid: u32,
    pub uptime_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    pub engine: EngineHealth,
    pub renderer: ChildHealth,
    pub output: ChildHealth,
    /// The warm standby renderer (§2.13.3), when one runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standby: Option<ChildHealth>,
    /// The channel supervisor's state and recovery records (M1 task 8, S7a).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervisor: Option<serde_json::Value>,
}

/// Shared, cheaply cloned health state. Writers hold the lock only to copy small values.
#[derive(Debug, Clone)]
pub struct HealthState {
    started: Instant,
    renderer: Arc<RwLock<ChildHealth>>,
    output: Arc<RwLock<ChildHealth>>,
    standby: Arc<RwLock<Option<ChildHealth>>>,
    supervisor: Arc<RwLock<Option<serde_json::Value>>>,
}

impl Default for HealthState {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthState {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            renderer: Arc::new(RwLock::new(ChildHealth::starting())),
            output: Arc::new(RwLock::new(ChildHealth::starting())),
            standby: Arc::new(RwLock::new(None)),
            supervisor: Arc::new(RwLock::new(None)),
        }
    }

    pub fn renderer(&self) -> Arc<RwLock<ChildHealth>> {
        self.renderer.clone()
    }

    /// The supervisor publishes its active and standby renderers here (§2.13.3).
    pub fn publish_supervisor(&self, v: serde_json::Value) {
        match self.supervisor.write() {
            Ok(mut s) => *s = Some(v),
            Err(p) => *p.into_inner() = Some(v),
        }
    }

    pub fn publish_renderers(&self, active: Option<ChildHealth>, standby: Option<ChildHealth>) {
        if let Some(a) = active {
            update(&self.renderer, |h| *h = a);
        }
        match self.standby.write() {
            Ok(mut s) => *s = standby,
            Err(p) => *p.into_inner() = standby,
        }
    }

    pub fn output(&self) -> Arc<RwLock<ChildHealth>> {
        self.output.clone()
    }

    pub fn snapshot(&self) -> Health {
        let read = |slot: &Arc<RwLock<ChildHealth>>| match slot.read() {
            Ok(h) => h.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        Health {
            engine: EngineHealth {
                state: "running".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                pid: std::process::id(),
                uptime_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            },
            renderer: read(&self.renderer),
            output: read(&self.output),
            standby: match self.standby.read() {
                Ok(h) => h.clone(),
                Err(p) => p.into_inner().clone(),
            },
            supervisor: match self.supervisor.read() {
                Ok(h) => h.clone(),
                Err(p) => p.into_inner().clone(),
            },
        }
    }
}

/// Apply `f` to a shared child-health slot, tolerating a poisoned lock.
pub fn update(slot: &Arc<RwLock<ChildHealth>>, f: impl FnOnce(&mut ChildHealth)) {
    match slot.write() {
        Ok(mut h) => f(&mut h),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_serialises_with_snake_case_states() {
        let state = HealthState::new();
        update(&state.renderer(), |h| {
            h.state = ChildState::Ok;
            h.cef_version = Some("156.0.31".into());
        });
        let json = serde_json::to_value(state.snapshot()).unwrap();
        assert_eq!(json["engine"]["state"], "running");
        assert_eq!(json["renderer"]["state"], "ok");
        assert_eq!(json["renderer"]["cef_version"], "156.0.31");
        assert_eq!(json["output"]["state"], "starting");
        let back: Health = serde_json::from_value(json).unwrap();
        assert_eq!(back.renderer.state, ChildState::Ok);
    }
}
