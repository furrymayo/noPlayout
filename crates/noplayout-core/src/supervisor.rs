//! The channel supervisor (PLAN §2.13, M1 task 8): an active and a warm standby renderer per
//! channel, the failure kinds K1–K4 (and K2g from the output process), warm promotion, the
//! standby rebuild behind it, cold restore when no standby can take over, and the loop guard.
//!
//! Detection (§2.13.1): K1 the active renderer's process exits; K2 its render subprocess dies
//! (`PageTerminated`); K2g the output reports `PaintStall` for the active incarnation; K3a the
//! active program page has not beaten for 500 ms (2 s while a `load` is pending); K4 the active
//! renderer has not answered the 100 ms control heartbeat for 500 ms.
//!
//! Restore (§2.13.3): warm promotion points the output at the standby's ring (`Activate`),
//! gives the standby `Role{active}` and makes its incarnation authoritative on the bus (the
//! bus's `seq` barrier, §2.14); the failed renderer is killed and a replacement standby starts
//! behind it, rebuilt by the bus snapshot. Without a ready standby (`NOPLAYOUT_STANDBY=0`, a
//! second fault during a rebuild, or the loop guard) a fresh renderer is cold-restored as the
//! active; the output holds its last frame throughout.
//!
//! TODO(M1): recovery fidelity record and standby eligibility (§2.13.3a, E-052, E-058) with the
//! Basketball sidecar declarations and their CI audit; K3b (poison action, quarantine); K4's
//! UI-thread latency probe (today K4 is the whole-process heartbeat); K5 stuck-sink and K6
//! output recycling. (STATUS.md)

use crate::children::{self, ChildHandle, Kind, Launch};
use crate::health::{ChildHealth, HealthState};
use crate::hostbus::HostBus;
use crate::program::{self, ProgramSpec};
use noplayout_frames::shm::Registry;
use noplayout_proto::{Down, DriveMode, Event, Format};
use serde::Serialize;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// `NOPLAYOUT_STANDBY=0` turns the warm standby off (cold path only; tests and S7a).
pub const ENV_STANDBY: &str = "NOPLAYOUT_STANDBY";
const BEAT_DEADLINE: Duration = Duration::from_millis(500);
const BEAT_DEADLINE_LOADING: Duration = Duration::from_millis(2000);
const PONG_DEADLINE: Duration = Duration::from_millis(500);
/// Loop guard (§2.13.1): a renderer that took over and fails the same way within this window
/// is not promoted back; it goes cold (2 s / 5 s / 10 s), then red.
const LOOP_WINDOW: Duration = Duration::from_secs(10);
const COLD_BACKOFF: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Failure {
    K1,
    K2,
    K2g,
    K3a,
    K4,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub kind: Failure,
    /// `warm`, `cold`, or `red` (the loop guard gave up).
    pub path: &'static str,
    pub failed_inc: u64,
    pub new_inc: Option<u64>,
    /// Engine monotonic time of detection (ns, the shared machine clock).
    pub detected_ns: u64,
    /// Detection → the output pointed at the new ring (ms).
    pub switched_ms: Option<f64>,
}

/// What `ctl supervisor` and the S7a harness read.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub active_inc: Option<u64>,
    pub active_pid: Option<u32>,
    pub standby_inc: Option<u64>,
    pub standby_pid: Option<u32>,
    /// The standby page has registered and caught up (its bus snapshot is applied).
    pub standby_ready: bool,
    pub standby_enabled: bool,
    pub promotions: u64,
    pub cold_restores: u64,
    /// "Renderer can't draw": the loop guard gave up; the output holds the last frame.
    pub red: bool,
    pub records: Vec<Record>,
}

pub type SharedView = Arc<Mutex<View>>;

/// Everything a renderer launch needs; cheap to clone into a spawned task.
#[derive(Clone)]
pub struct Env {
    pub channel: String,
    pub run: String,
    pub port: u16,
    pub data: PathBuf,
    pub format: Format,
    pub mode: DriveMode,
    pub epoch_mono_ns: u64,
    pub bus: HostBus,
    pub rings: Registry,
}

/// What woke the supervisor; pass it to [`Supervisor::handle`].
pub struct Wake(Woke);

enum Woke {
    Tick,
    ActiveExit,
    ActiveEvent(Option<Event>),
    StandbyEvent(Option<Event>),
    Launched(Result<Result<Renderer, String>, tokio::task::JoinError>),
    ColdDue,
}

struct Renderer {
    handle: ChildHandle,
    health: Arc<RwLock<ChildHealth>>,
    events: Option<mpsc::UnboundedReceiver<Event>>,
    inc: u64,
    ring: String,
    slot: &'static str,
    since: Instant,
}

async fn launch(env: Env, slot: &'static str, active: bool) -> Result<Renderer, String> {
    let health = Arc::new(RwLock::new(ChildHealth::starting()));
    let cache = crate::paths::cef_cache_dir(&env.data, &env.channel, slot);
    let handle = children::launch(
        Launch {
            kind: Kind::Renderer,
            exe: children::default_exe(Kind::Renderer),
            args: vec![
                "--channel".into(),
                env.channel.clone().into(),
                "--slot".into(),
                slot.into(),
                OsString::from("--cache-dir"),
                cache.into_os_string(),
            ],
        },
        health.clone(),
    )
    .await
    .map_err(|e| e.to_string())?;
    let events = handle.take_events();
    let opened = program::open(
        &env.bus,
        &env.rings,
        &handle,
        ProgramSpec {
            channel: &env.channel,
            run: &env.run,
            port: env.port,
            format: env.format,
            mode: env.mode,
            epoch_mono_ns: env.epoch_mono_ns,
            active,
        },
    )
    .await?;
    tracing::info!(
        slot,
        active,
        inc = opened.inc,
        ring = %opened.ring,
        "program page configured"
    );
    Ok(Renderer {
        handle,
        health,
        events,
        inc: opened.inc,
        ring: opened.ring,
        slot,
        since: Instant::now(),
    })
}

fn other(slot: &'static str) -> &'static str {
    if slot == "a" { "b" } else { "a" }
}

pub struct Supervisor {
    env: Env,
    health: HealthState,
    view: SharedView,
    output: Option<Arc<ChildHandle>>,
    active: Option<Renderer>,
    standby: Option<Renderer>,
    standby_enabled: bool,
    /// A renderer being launched: `(slot, as active, task)`.
    pending: Option<(bool, tokio::task::JoinHandle<Result<Renderer, String>>)>,
    /// When the current active took over, and after which failure (the loop guard, §2.13.1).
    last_promotion: Option<(Instant, Failure)>,
    cold_attempts: usize,
    cold_due: Option<tokio::time::Instant>,
    stall_inc: Option<u64>,
    cold_slot: &'static str,
}

impl Supervisor {
    pub fn new(env: Env, health: HealthState, output: Option<Arc<ChildHandle>>) -> Self {
        let standby_enabled = std::env::var(ENV_STANDBY).map_or(true, |v| v != "0");
        let view = Arc::new(Mutex::new(View {
            standby_enabled,
            ..View::default()
        }));
        Self {
            env,
            health,
            view,
            output,
            active: None,
            standby: None,
            standby_enabled,
            pending: None,
            last_promotion: None,
            cold_attempts: 0,
            cold_due: None,
            stall_inc: None,
            cold_slot: "a",
        }
    }

    pub fn view(&self) -> SharedView {
        self.view.clone()
    }

    /// Start the first active renderer (slot `a`) and, behind it, the standby.
    pub async fn start(&mut self) {
        match launch(self.env.clone(), "a", true).await {
            Ok(r) => {
                self.activate_output(&r).await;
                self.active = Some(r);
            }
            Err(e) => {
                tracing::error!("cannot start the program renderer: {e}");
                self.cold_due = Some(tokio::time::Instant::now() + COLD_BACKOFF[0]);
            }
        }
        self.start_standby();
        self.publish().await;
    }

    fn start_standby(&mut self) {
        if !self.standby_enabled || self.standby.is_some() || self.pending.is_some() {
            return;
        }
        let Some(a) = &self.active else { return };
        let slot = other(a.slot);
        tracing::info!(slot, "starting the standby renderer");
        let env = self.env.clone();
        self.pending = Some((false, tokio::spawn(launch(env, slot, false))));
    }

    async fn activate_output(&self, r: &Renderer) {
        if let Some(out) = &self.output
            && let Err(e) = out
                .send(Down::Activate {
                    ring: r.ring.clone(),
                    inc: r.inc,
                    mode: self.env.mode,
                })
                .await
        {
            tracing::error!("cannot point the output at ring {}: {e}", r.ring);
        }
    }

    /// The renderer the dev-control faults act on.
    pub fn active_handle(&self) -> Option<&ChildHandle> {
        self.active.as_ref().map(|r| &r.handle)
    }

    /// Wait for the next thing the supervisor must act on (at most one heartbeat period).
    /// Cancel-safe: it only waits; [`Supervisor::handle`] acts.
    pub async fn wait(&mut self) -> Wake {
        let tick = tokio::time::sleep(children::HEARTBEAT);
        let exit = self.active.as_ref().map(|r| r.handle.exit_watch());
        let active_exit = async move {
            match exit {
                Some(mut rx) => {
                    let _ = rx.wait_for(|e| *e).await;
                }
                None => std::future::pending().await,
            }
        };
        let active_ev = recv(self.active.as_mut().and_then(|r| r.events.as_mut()));
        let standby_ev = recv(self.standby.as_mut().and_then(|r| r.events.as_mut()));
        let pending_ref = &mut self.pending;
        let pending = async move {
            match pending_ref {
                Some((_, t)) => t.await,
                None => std::future::pending().await,
            }
        };
        let due = self.cold_due;
        let cold_due = async move {
            match due {
                Some(t) => tokio::time::sleep_until(t).await,
                None => std::future::pending().await,
            }
        };
        Wake(tokio::select! {
            _ = tick => Woke::Tick,
            _ = active_exit => Woke::ActiveExit,
            ev = active_ev => Woke::ActiveEvent(ev),
            ev = standby_ev => Woke::StandbyEvent(ev),
            r = pending => Woke::Launched(r),
            _ = cold_due => Woke::ColdDue,
        })
    }

    /// Act on what [`Supervisor::wait`] returned. Not cancel-safe: run it to completion (in a
    /// `select!` handler, never as a raced future).
    pub async fn handle(&mut self, Wake(woke): Wake) {
        match woke {
            Woke::ActiveExit => self.fail(Failure::K1).await,
            Woke::ActiveEvent(Some(Event::PageTerminated { status })) => {
                tracing::error!("K2: the active page's render process ended ({status})");
                self.fail(Failure::K2).await;
            }
            Woke::ActiveEvent(Some(ev)) => log_event("active", ev),
            Woke::ActiveEvent(None) => {
                if let Some(r) = &mut self.active {
                    r.events = None;
                }
            }
            Woke::StandbyEvent(Some(Event::PageTerminated { status })) => {
                tracing::warn!("the standby page's render process ended ({status}); rebuilding");
                self.drop_standby().await;
            }
            Woke::StandbyEvent(Some(ev)) => log_event("standby", ev),
            Woke::StandbyEvent(None) => {
                if let Some(r) = &mut self.standby {
                    r.events = None;
                }
            }
            Woke::Launched(r) => self.launched(r).await,
            Woke::ColdDue => {
                self.cold_due = None;
                self.cold_restore();
            }
            Woke::Tick => self.check().await,
        }
        self.publish().await;
    }

    /// Output events the supervisor acts on (K2g).
    pub async fn output_event(&mut self, ev: &Event) {
        if let Event::PaintStall { inc, probe_seq } = ev
            && self.active.as_ref().is_some_and(|a| a.inc == *inc)
            && self.stall_inc != Some(*inc)
        {
            tracing::error!(inc, probe_seq, "K2g: paints stopped reaching the ring");
            self.stall_inc = Some(*inc);
            self.fail(Failure::K2g).await;
            self.publish().await;
        }
    }

    /// A superseded incarnation tried to register (§2.14): make sure that renderer is gone.
    pub async fn kill_incarnation(&mut self, inc: u64) {
        for r in [&self.active, &self.standby].into_iter().flatten() {
            if r.inc == inc {
                let _ = r.handle.kill().await;
            }
        }
    }

    /// Heartbeat checks: K4 (control heartbeat) and K3a (page beats), and the standby's health.
    async fn check(&mut self) {
        let Some(a) = &self.active else { return };
        if a.since.elapsed() > PONG_DEADLINE && a.handle.pong_age() > PONG_DEADLINE {
            tracing::error!(
                "K4: the active renderer has not answered for {:?}",
                a.handle.pong_age()
            );
            return self.fail(Failure::K4).await;
        }
        let Ok(view) = self.channel_view().await else {
            return;
        };
        let loading = view
            .instances
            .iter()
            .any(|i| matches!(i.phase, crate::hostbus::state::Phase::Loading));
        let deadline = if loading {
            BEAT_DEADLINE_LOADING
        } else {
            BEAT_DEADLINE
        };
        let page = |inc: u64| view.pages.iter().find(|p| p.inc == Some(inc));
        if a.since.elapsed() > BEAT_DEADLINE_LOADING
            && page(a.inc).is_none_or(|p| p.beat_age_ms > deadline.as_millis() as u64)
        {
            tracing::error!(
                inc = a.inc,
                "K3a: the active program page has not beaten for {deadline:?}"
            );
            return self.fail(Failure::K3a).await;
        }
        if let Some(s) = &self.standby {
            let stale = s.since.elapsed() > BEAT_DEADLINE_LOADING
                && page(s.inc)
                    .is_none_or(|p| p.beat_age_ms > BEAT_DEADLINE_LOADING.as_millis() as u64);
            if s.handle.has_exited() || stale || s.handle.pong_age() > PONG_DEADLINE * 4 {
                tracing::warn!(
                    inc = s.inc,
                    "the standby renderer is unhealthy; rebuilding it"
                );
                self.drop_standby().await;
            }
        }
        // Loop guard reset: a renderer that has been fine for the whole window.
        if self
            .last_promotion
            .is_some_and(|(t, _)| t.elapsed() > LOOP_WINDOW)
        {
            self.cold_attempts = 0;
        }
    }

    async fn channel_view(&self) -> Result<crate::hostbus::ChannelView, String> {
        self.env
            .bus
            .channel(&self.env.channel)
            .ok_or("no channel")?
            .inspect()
            .await
            .map_err(|e| e.to_string())
    }

    async fn drop_standby(&mut self) {
        if let Some(s) = self.standby.take() {
            let _ = s.handle.kill().await;
            self.retire(s).await;
        }
        self.start_standby();
    }

    /// Kill a renderer that is out of service and forget its incarnation and ring.
    async fn retire(&self, r: Renderer) {
        let _ = r.handle.kill().await;
        if let Some(ch) = self.env.bus.channel(&self.env.channel) {
            ch.remove_incarnation(r.inc).await;
        }
        // Reap in the background; the ring is unlinked by the registry sweep.
        tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_secs(5), r.handle.wait_exit()).await;
            let _ = noplayout_frames::shm::unlink(&r.ring);
        });
    }

    async fn fail(&mut self, kind: Failure) {
        let Some(failed) = self.active.take() else {
            return;
        };
        let detected_ns = noplayout_frames::clock::mono_ns();
        let t0 = Instant::now();
        let failed_inc = failed.inc;
        let looping = self
            .last_promotion
            .is_some_and(|(t, k)| k == kind && t.elapsed() < LOOP_WINDOW);
        let ready = self.standby_ready().await;
        let mut record = Record {
            kind,
            path: "warm",
            failed_inc,
            new_inc: None,
            detected_ns,
            switched_ms: None,
        };
        if ready
            && !looping
            && let Some(sb) = self.standby.take()
        {
            // Warm promotion (§2.13.3).
            self.activate_output(&sb).await;
            record.switched_ms = Some(t0.elapsed().as_secs_f64() * 1e3);
            if let Err(e) = sb.handle.send(Down::Role { active: true }).await {
                tracing::warn!("Role{{active}} to the standby: {e}");
            }
            if let Some(ch) = self.env.bus.channel(&self.env.channel) {
                ch.promote(sb.inc).await;
            }
            tracing::warn!(?kind, failed_inc, new_inc = sb.inc, "warm promotion");
            record.new_inc = Some(sb.inc);
            let mut sb = sb;
            sb.since = Instant::now();
            self.active = Some(sb);
            self.last_promotion = Some((Instant::now(), kind));
            self.view
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .promotions += 1;
            self.retire(failed).await;
            self.start_standby();
        } else {
            // Cold restore: no ready standby, or the loop guard (a repeat within 10 s).
            if looping {
                tracing::error!(?kind, "loop guard: the promoted renderer failed again");
            }
            if let Some(sb) = self.standby.take() {
                self.retire(sb).await;
            }
            let slot = failed.slot;
            self.retire(failed).await;
            if let Some((_, t)) = self.pending.take() {
                t.abort();
            }
            if self.cold_attempts >= COLD_BACKOFF.len() {
                tracing::error!(
                    "Renderer can't draw: giving up after {} cold restores",
                    self.cold_attempts
                );
                record.path = "red";
                self.view.lock().unwrap_or_else(|p| p.into_inner()).red = true;
            } else {
                record.path = "cold";
                let wait = if looping {
                    COLD_BACKOFF[self.cold_attempts]
                } else {
                    Duration::ZERO
                };
                self.cold_attempts += usize::from(looping);
                self.cold_slot = other(slot);
                self.cold_due = Some(tokio::time::Instant::now() + wait);
            }
        }
        self.view
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .records
            .push(record);
    }

    fn cold_restore(&mut self) {
        tracing::warn!(slot = self.cold_slot, "cold restore onto a fresh renderer");
        self.view
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .cold_restores += 1;
        let env = self.env.clone();
        self.pending = Some((true, tokio::spawn(launch(env, self.cold_slot, true))));
    }

    async fn launched(&mut self, r: Result<Result<Renderer, String>, tokio::task::JoinError>) {
        let Some((as_active, _)) = self.pending.take() else {
            return;
        };
        match r {
            Ok(Ok(r)) if as_active => {
                self.activate_output(&r).await;
                tracing::warn!(inc = r.inc, "cold restore: the new renderer is active");
                if let Some(rec) = self
                    .view
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .records
                    .last_mut()
                    && rec.path == "cold"
                    && rec.new_inc.is_none()
                {
                    rec.new_inc = Some(r.inc);
                    rec.switched_ms =
                        Some((noplayout_frames::clock::mono_ns() - rec.detected_ns) as f64 / 1e6);
                }
                self.active = Some(r);
                let kind = self
                    .view
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .records
                    .last()
                    .map_or(Failure::K1, |r| r.kind);
                self.last_promotion = Some((Instant::now(), kind));
                self.start_standby();
            }
            Ok(Ok(r)) => {
                tracing::info!(
                    inc = r.inc,
                    slot = r.slot,
                    "standby renderer up; rebuilding"
                );
                self.standby = Some(r);
            }
            Ok(Err(e)) => {
                tracing::error!(as_active, "renderer launch failed: {e}");
                self.after_launch_failure(as_active);
            }
            Err(e) => {
                tracing::error!(as_active, "renderer launch task failed: {e}");
                self.after_launch_failure(as_active);
            }
        }
    }

    fn after_launch_failure(&mut self, as_active: bool) {
        if as_active {
            if self.cold_attempts >= COLD_BACKOFF.len() {
                self.view.lock().unwrap_or_else(|p| p.into_inner()).red = true;
            } else {
                self.cold_due =
                    Some(tokio::time::Instant::now() + COLD_BACKOFF[self.cold_attempts]);
                self.cold_attempts += 1;
            }
        }
        // A failed standby launch is retried on the next fault-free heartbeat.
    }

    /// The standby is promotable once its page has registered, applied the bus snapshot
    /// (`Synced`) and is beating.
    async fn standby_ready(&self) -> bool {
        let Some(s) = &self.standby else { return false };
        if s.handle.has_exited() {
            return false;
        }
        self.channel_view().await.is_ok_and(|v| {
            v.pages.iter().any(|p| {
                p.inc == Some(s.inc)
                    && p.synced_seq.is_some()
                    && p.last_beat.is_some()
                    && p.beat_age_ms < BEAT_DEADLINE.as_millis() as u64
            })
        })
    }

    async fn publish(&mut self) {
        if self.standby.is_none() && self.pending.is_none() && self.active.is_some() {
            self.start_standby();
        }
        let ready = self.standby_ready().await;
        let read = |h: &Arc<RwLock<ChildHealth>>| h.read().map(|g| g.clone()).ok();
        self.health.publish_renderers(
            self.active.as_ref().and_then(|r| read(&r.health)),
            self.standby.as_ref().and_then(|r| read(&r.health)),
        );
        let mut v = self.view.lock().unwrap_or_else(|p| p.into_inner());
        v.active_inc = self.active.as_ref().map(|r| r.inc);
        v.active_pid = self.active.as_ref().and_then(|r| r.handle.pid);
        v.standby_inc = self.standby.as_ref().map(|r| r.inc);
        v.standby_pid = self.standby.as_ref().and_then(|r| r.handle.pid);
        v.standby_ready = ready;
        if let Ok(json) = serde_json::to_value(&*v) {
            self.health.publish_supervisor(json);
        }
    }

    /// Shut every renderer down (engine stop).
    pub async fn shutdown(mut self) {
        if let Some((_, t)) = self.pending.take() {
            t.abort();
        }
        for r in [self.active.take(), self.standby.take()]
            .into_iter()
            .flatten()
        {
            match r.handle.shutdown().await {
                Ok(status) => tracing::info!(slot = r.slot, %status, "renderer stopped"),
                Err(e) => tracing::warn!(slot = r.slot, "renderer: {e}"),
            }
        }
    }
}

async fn recv(rx: Option<&mut mpsc::UnboundedReceiver<Event>>) -> Option<Event> {
    match rx {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

fn log_event(role: &str, ev: Event) {
    match ev {
        Event::Console { error: true, text } => {
            tracing::warn!(target: "program_page", role, "console error: {text}");
        }
        Event::Console { text, .. } => tracing::debug!(target: "program_page", role, "{text}"),
        other => tracing::info!(role, ?other, "renderer event"),
    }
}
