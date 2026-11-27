//! Launching child processes (output, renderer) and talking to them over the control protocol
//! (PLAN §4.1, §4.2).
//!
//! Each child gets its own local-socket listener and a per-launch token in its environment
//! (`NOPLAYOUT_IPC_NAME`, `NOPLAYOUT_IPC_TOKEN`). It must connect and send a valid `Hello`
//! within its timeout (output 5 s, renderer 10 s). After that one task per child owns the
//! process and the socket: it sends a `Ping` every second, records `Pong`s in the health state,
//! notices the process exiting, and performs `Shutdown`.
//!
//! M0 keeps this deliberately small. The 100 ms heartbeat, failure kinds K1–K6, restarts and the
//! standby renderer are M1/M6 work (PLAN §2.13).
// TODO(M6): full supervisor (100 ms heartbeat, failure kinds, restart, orphan handling).
// (STATUS.md, "M0 limits")

use crate::health::{self, ChildHealth, ChildState};
use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::tokio::{Stream, prelude::*};
use noplayout_proto::{self as proto, Down, FrameError, Hello, HelloError, Pong, Up};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

/// Environment override for the renderer executable.
pub const ENV_RENDERER: &str = "NOPLAYOUT_RENDERER";
/// Environment override for the output-process executable.
pub const ENV_OUTPUT: &str = "NOPLAYOUT_OUTPUT";

/// How long a child that stopped answering pings stays `ok` before it is marked failed.
const PONG_DEADLINE: Duration = Duration::from_secs(5);
/// Control heartbeat to every child (§2.13.1: every 100 ms; K4/K6 fire after 500 ms silent).
pub const HEARTBEAT: Duration = Duration::from_millis(100);
/// How long a child may take to exit after `Shutdown` before it is killed.
const EXIT_GRACE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Output,
    Renderer,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Output => "output process",
            Kind::Renderer => "renderer",
        }
    }

    /// Time allowed between spawn and a valid `Hello` (PLAN §4.2: renderers within 10 s).
    pub fn hello_timeout(self) -> Duration {
        match self {
            Kind::Output => Duration::from_secs(5),
            Kind::Renderer => Duration::from_secs(10),
        }
    }
}

/// What to launch.
#[derive(Debug, Clone)]
pub struct Launch {
    pub kind: Kind,
    pub exe: PathBuf,
    pub args: Vec<OsString>,
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("the {kind} was not found at {exe}")]
    Missing { kind: &'static str, exe: PathBuf },
    #[error("the {kind} could not be started: {source}")]
    Spawn {
        kind: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("the control socket for the {kind} could not be opened: {source}")]
    Socket {
        kind: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("the {kind} exited before it connected ({status})")]
    EarlyExit { kind: &'static str, status: String },
    #[error("the {kind} did not say hello within {secs} s")]
    Timeout { kind: &'static str, secs: u64 },
    #[error("the {kind} was refused: {source}")]
    Refused {
        kind: &'static str,
        #[source]
        source: HelloError,
    },
    #[error("the {kind} broke the control protocol: {detail}")]
    Protocol { kind: &'static str, detail: String },
}

enum Cmd {
    Ping(oneshot::Sender<Pong>),
    Shutdown(oneshot::Sender<Result<ExitStatus, String>>),
    /// SIGKILL / TerminateProcess, no grace (fault injection K1, superseded renderers).
    Kill,
    /// Forward one control message.
    Send(Down),
}

/// A running child that has said `Hello`.
#[derive(Debug)]
pub struct ChildHandle {
    pub kind: Kind,
    pub pid: Option<u32>,
    pub hello: Hello,
    cmd: mpsc::Sender<Cmd>,
    events: std::sync::Mutex<Option<mpsc::UnboundedReceiver<noplayout_proto::Event>>>,
    exited: tokio::sync::watch::Receiver<bool>,
    last_pong: Arc<AtomicU64>,
}

/// What `supervise` reports back to the handle.
struct Watch {
    exited: tokio::sync::watch::Sender<bool>,
    last_pong: Arc<AtomicU64>,
}

/// Marks the child exited when `supervise` returns, whichever way.
struct ExitGuard(tokio::sync::watch::Sender<bool>);

impl Drop for ExitGuard {
    fn drop(&mut self) {
        let _ = self.0.send(true);
    }
}

impl ChildHandle {
    /// Resolves once the process has exited (or its supervisor stopped); at once if it has.
    pub async fn wait_exit(&self) {
        let mut rx = self.exited.clone();
        let _ = rx.wait_for(|e| *e).await;
    }

    pub fn exit_watch(&self) -> tokio::sync::watch::Receiver<bool> {
        self.exited.clone()
    }

    pub fn has_exited(&self) -> bool {
        *self.exited.borrow()
    }

    /// Time since the child last answered the [`HEARTBEAT`] ping (K4/K6 detection, §2.13.1).
    pub fn pong_age(&self) -> Duration {
        let at = self.last_pong.load(Ordering::Acquire);
        Duration::from_nanos(noplayout_frames::clock::mono_ns().saturating_sub(at))
    }

    /// The child's `Up::Event`s, once (the supervisor takes them).
    pub fn take_events(&self) -> Option<mpsc::UnboundedReceiver<noplayout_proto::Event>> {
        self.events.lock().unwrap_or_else(|p| p.into_inner()).take()
    }

    /// One `Ping` → `Pong` round trip.
    pub async fn ping(&self, timeout: Duration) -> Result<Pong, String> {
        let (tx, rx) = oneshot::channel();
        self.cmd
            .send(Cmd::Ping(tx))
            .await
            .map_err(|_| format!("the {} is no longer connected", self.kind.label()))?;
        tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| format!("the {} did not answer a ping", self.kind.label()))?
            .map_err(|_| format!("the {} went away during a ping", self.kind.label()))
    }

    /// Kill the process without a grace period; the supervisor then reports it failed.
    pub async fn kill(&self) -> Result<(), String> {
        self.cmd
            .send(Cmd::Kill)
            .await
            .map_err(|_| format!("the {} had already stopped", self.kind.label()))
    }

    /// Send one control message.
    pub async fn send(&self, msg: Down) -> Result<(), String> {
        self.cmd
            .send(Cmd::Send(msg))
            .await
            .map_err(|_| format!("the {} is no longer connected", self.kind.label()))
    }

    /// Send `Shutdown` and wait for the process to exit (killed after a grace period).
    pub async fn shutdown(&self) -> Result<ExitStatus, String> {
        let (tx, rx) = oneshot::channel();
        self.cmd
            .send(Cmd::Shutdown(tx))
            .await
            .map_err(|_| format!("the {} had already stopped", self.kind.label()))?;
        rx.await
            .map_err(|_| format!("the {} supervisor stopped", self.kind.label()))?
    }
}

/// Default executable for `kind`: the environment override, else next to the running binary.
/// On macOS the renderer lives in its dev bundle (`noplayout-renderer.app`, made by
/// `cargo xtask bundle`), because CEF must run from inside an app bundle there.
pub fn default_exe(kind: Kind) -> PathBuf {
    let (env, name) = match kind {
        Kind::Output => (ENV_OUTPUT, "noplayout-output"),
        Kind::Renderer => (ENV_RENDERER, "noplayout-renderer"),
    };
    if let Some(path) = std::env::var_os(env) {
        return PathBuf::from(path);
    }
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    exe_in(&dir, kind, name)
}

fn exe_in(dir: &Path, kind: Kind, name: &str) -> PathBuf {
    if cfg!(target_os = "macos") && kind == Kind::Renderer {
        return dir
            .join(format!("{name}.app"))
            .join("Contents")
            .join("MacOS")
            .join(name);
    }
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

// `&Stream` reads and writes, so one `Arc<Stream>` serves the reader task and the writer.
async fn read_up(stream: &Stream) -> Result<Up, FrameError> {
    let mut rx = stream;
    let mut prefix = [0u8; 4];
    rx.read_exact(&mut prefix).await?;
    let len = proto::frame_len(prefix)?;
    let mut body = vec![0u8; len];
    rx.read_exact(&mut body).await?;
    proto::decode_body(&body)
}

async fn write_down(stream: &Stream, msg: &Down) -> Result<(), FrameError> {
    let mut tx = stream;
    let frame = proto::encode_frame(msg)?;
    tx.write_all(&frame).await?;
    tx.flush().await?;
    Ok(())
}

/// Launch `spec`, wait for its `Hello`, and start its supervising task. Progress and failures are
/// written to `slot` as well as returned.
pub async fn launch(
    spec: Launch,
    slot: Arc<RwLock<ChildHealth>>,
) -> Result<ChildHandle, LaunchError> {
    let result = launch_inner(&spec, &slot).await;
    if let Err(e) = &result {
        let state = match e {
            LaunchError::Missing { .. } => ChildState::Missing,
            _ => ChildState::Failed,
        };
        let detail = e.to_string();
        tracing::error!(kind = spec.kind.label(), exe = %spec.exe.display(), "{detail}");
        health::update(&slot, |h| {
            h.state = state;
            h.detail = Some(detail);
        });
    }
    result
}

async fn launch_inner(
    spec: &Launch,
    slot: &Arc<RwLock<ChildHealth>>,
) -> Result<ChildHandle, LaunchError> {
    let kind = spec.kind.label();
    if !spec.exe.is_file() {
        return Err(LaunchError::Missing {
            kind,
            exe: spec.exe.clone(),
        });
    }
    health::update(slot, |h| *h = ChildHealth::starting());

    let token = crate::new_token();
    let socket = format!(
        "noplayout-{}-{}.sock",
        std::process::id(),
        &crate::new_token()[20..]
    );
    let name =
        proto::socket_name(&socket).map_err(|source| LaunchError::Socket { kind, source })?;
    let listener = ListenerOptions::new()
        .name(name)
        .create_tokio()
        .map_err(|source| LaunchError::Socket { kind, source })?;

    let mut process = tokio::process::Command::new(&spec.exe)
        .args(&spec.args)
        .env(proto::ENV_IPC_NAME, &socket)
        .env(proto::ENV_IPC_TOKEN, &token)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| LaunchError::Spawn { kind, source })?;
    let pid = process.id();
    tracing::info!(kind, pid, exe = %spec.exe.display(), "launched");

    let hello_timeout = spec.kind.hello_timeout();
    let connected = tokio::select! {
        r = tokio::time::timeout(hello_timeout, accept_hello(&listener, &token, kind)) => match r {
            Ok(r) => r?,
            Err(_) => return Err(LaunchError::Timeout { kind, secs: hello_timeout.as_secs() }),
        },
        status = process.wait() => {
            let status = status.map(|s| s.to_string()).unwrap_or_else(|e| e.to_string());
            return Err(LaunchError::EarlyExit { kind, status });
        }
    };
    drop(listener);
    let (stream, hello) = connected;

    health::update(slot, |h| {
        h.state = ChildState::Ok;
        h.pid = pid;
        h.detail = None;
        match &hello {
            Hello::Output(o) => h.version = Some(o.version.clone()),
            Hello::Renderer(r) => {
                h.version = Some(r.renderer_version.clone());
                h.cef_version = Some(r.cef_version.clone());
                h.chromium_version = Some(r.chromium_version.clone());
            }
        }
    });
    tracing::info!(kind, pid, ?hello, "hello");

    let (cmd_tx, cmd_rx) = mpsc::channel(8);
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let (exit_tx, exit_rx) = tokio::sync::watch::channel(false);
    let last_pong = Arc::new(AtomicU64::new(noplayout_frames::clock::mono_ns()));
    tokio::spawn(supervise(
        spec.kind,
        process,
        stream,
        cmd_rx,
        slot.clone(),
        events_tx,
        Watch {
            exited: exit_tx,
            last_pong: last_pong.clone(),
        },
    ));
    Ok(ChildHandle {
        kind: spec.kind,
        pid,
        hello,
        cmd: cmd_tx,
        events: std::sync::Mutex::new(Some(events_rx)),
        exited: exit_rx,
        last_pong,
    })
}

async fn accept_hello(
    listener: &interprocess::local_socket::tokio::Listener,
    token: &str,
    kind: &'static str,
) -> Result<(Arc<Stream>, Hello), LaunchError> {
    let stream = listener
        .accept()
        .await
        .map_err(|source| LaunchError::Socket { kind, source })?;
    match read_up(&stream).await {
        Ok(Up::Hello(hello)) => {
            hello
                .check(token)
                .map_err(|source| LaunchError::Refused { kind, source })?;
            Ok((Arc::new(stream), hello))
        }
        Ok(other) => Err(LaunchError::Protocol {
            kind,
            detail: format!("first message was {other:?}, not Hello"),
        }),
        Err(e) => Err(LaunchError::Protocol {
            kind,
            detail: e.to_string(),
        }),
    }
}

async fn supervise(
    kind: Kind,
    mut process: tokio::process::Child,
    stream: Arc<Stream>,
    mut cmd_rx: mpsc::Receiver<Cmd>,
    slot: Arc<RwLock<ChildHealth>>,
    events: mpsc::UnboundedSender<noplayout_proto::Event>,
    watch: Watch,
) {
    let label = kind.label();
    // Every return from here on means the process is gone or being reaped.
    let _exited = ExitGuard(watch.exited);
    let pong_at = watch.last_pong;
    let (pong_tx, mut pong_rx) = mpsc::channel::<Pong>(16);
    // Reading frames is not cancel-safe, so it gets its own task.
    let reader = stream.clone();
    tokio::spawn(async move {
        loop {
            match read_up(&reader).await {
                Ok(Up::Pong(p)) => {
                    if pong_tx.send(p).await.is_err() {
                        break;
                    }
                }
                Ok(Up::Event(ev)) => {
                    tracing::debug!(kind = label, ?ev, "event");
                    let _ = events.send(ev);
                }
                Ok(other) => tracing::warn!(kind = label, ?other, "unexpected message"),
                Err(e) => {
                    tracing::debug!(kind = label, "control socket closed: {e}");
                    break;
                }
            }
        }
    });

    let mut ticker = tokio::time::interval(HEARTBEAT);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut seq: u64 = 0;
    let mut waiting: Option<(u64, oneshot::Sender<Pong>)> = None;
    let mut last_pong = Instant::now();
    let mut socket_open = true;

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(Cmd::Ping(reply)) => {
                    seq += 1;
                    waiting = Some((seq, reply));
                    if let Err(e) = write_down(&stream, &Down::Ping { seq }).await {
                        tracing::warn!(kind = label, "ping failed: {e}");
                    }
                }
                Some(Cmd::Kill) => {
                    tracing::warn!(kind = label, "killing (no grace)");
                    if let Err(e) = process.start_kill() {
                        tracing::warn!(kind = label, "kill failed: {e}");
                    }
                }
                Some(Cmd::Send(msg)) => {
                    if let Err(e) = write_down(&stream, &msg).await {
                        tracing::warn!(kind = label, "send failed: {e}");
                    }
                }
                Some(Cmd::Shutdown(reply)) => {
                    let result = shutdown(kind, &mut process, &stream).await;
                    health::update(&slot, |h| match &result {
                        Ok(status) if status.success() => {
                            h.state = ChildState::Exited;
                            h.detail = None;
                        }
                        Ok(status) => {
                            h.state = ChildState::Failed;
                            h.detail = Some(format!("exited after shutdown with {status}"));
                        }
                        Err(e) => {
                            h.state = ChildState::Failed;
                            h.detail = Some(e.clone());
                        }
                    });
                    let _ = reply.send(result);
                    return;
                }
                // Every handle dropped: kill_on_drop stops the process when we return.
                None => return,
            },
            _ = ticker.tick() => {
                if socket_open {
                    seq += 1;
                    if write_down(&stream, &Down::Ping { seq }).await.is_err() {
                        socket_open = false;
                    }
                }
                if last_pong.elapsed() > PONG_DEADLINE {
                    health::update(&slot, |h| if h.state == ChildState::Ok {
                        h.state = ChildState::Failed;
                        h.detail = Some(format!("the {label} stopped answering"));
                    });
                }
            },
            pong = pong_rx.recv(), if socket_open => match pong {
                Some(p) => {
                    last_pong = Instant::now();
                    pong_at.store(noplayout_frames::clock::mono_ns(), Ordering::Release);
                    health::update(&slot, |h| {
                        if kind == Kind::Renderer {
                            h.paints = Some(p.last_paint_seq);
                        }
                        if h.state == ChildState::Failed {
                            h.state = ChildState::Ok;
                            h.detail = None;
                        }
                    });
                    if waiting.as_ref().is_some_and(|(s, _)| *s == p.seq)
                        && let Some((_, reply)) = waiting.take()
                    {
                        let _ = reply.send(p);
                    }
                }
                None => {
                    socket_open = false;
                    tracing::warn!(kind = label, "control socket closed");
                }
            },
            status = process.wait() => {
                let detail = match status {
                    Ok(s) => format!("the {label} exited unexpectedly ({s})"),
                    Err(e) => format!("the {label} could not be watched: {e}"),
                };
                tracing::error!(kind = label, "{detail}");
                health::update(&slot, |h| {
                    h.state = ChildState::Failed;
                    h.detail = Some(detail);
                });
                // Answer a pending shutdown, if any arrives, with the same fact.
                while let Ok(cmd) = cmd_rx.try_recv() {
                    if let Cmd::Shutdown(reply) = cmd {
                        let _ = reply.send(Err(format!("the {label} had already exited")));
                    }
                }
                return;
            }
        }
    }
}

async fn shutdown(
    kind: Kind,
    process: &mut tokio::process::Child,
    stream: &Stream,
) -> Result<ExitStatus, String> {
    let label = kind.label();
    if let Err(e) = write_down(stream, &Down::Shutdown).await {
        tracing::warn!(kind = label, "could not send shutdown: {e}");
    }
    match tokio::time::timeout(EXIT_GRACE, process.wait()).await {
        Ok(Ok(status)) => {
            tracing::info!(kind = label, %status, "exited");
            Ok(status)
        }
        Ok(Err(e)) => Err(format!("the {label} could not be watched: {e}")),
        Err(_) => {
            tracing::error!(
                kind = label,
                "did not exit within {EXIT_GRACE:?}; killing it"
            );
            let _ = process.kill().await;
            Err(format!(
                "the {label} did not exit within {} s and was killed",
                EXIT_GRACE.as_secs()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_lives_in_its_bundle_on_macos_only() {
        let dir = Path::new("bin");
        let renderer = exe_in(dir, Kind::Renderer, "noplayout-renderer");
        let output = exe_in(dir, Kind::Output, "noplayout-output");
        if cfg!(target_os = "macos") {
            assert_eq!(
                renderer,
                Path::new("bin/noplayout-renderer.app/Contents/MacOS/noplayout-renderer")
            );
        } else {
            assert_eq!(
                renderer,
                dir.join(format!(
                    "noplayout-renderer{}",
                    std::env::consts::EXE_SUFFIX
                ))
            );
        }
        assert_eq!(
            output,
            dir.join(format!("noplayout-output{}", std::env::consts::EXE_SUFFIX))
        );
    }

    #[tokio::test]
    async fn missing_executable_is_reported_as_missing() {
        let slot = Arc::new(RwLock::new(ChildHealth::starting()));
        let err = launch(
            Launch {
                kind: Kind::Renderer,
                exe: PathBuf::from("/definitely/not/here/noplayout-renderer"),
                args: vec![],
            },
            slot.clone(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LaunchError::Missing { .. }));
        let h = slot.read().unwrap().clone();
        assert_eq!(h.state, ChildState::Missing);
        assert!(h.detail.unwrap().contains("was not found"));
    }
}
