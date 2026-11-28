//! `noplayout-renderer --channel <id> --slot a|b --cache-dir <dir>`: the CEF offscreen renderer
//! (D-001, E-005, E-041). It links `proto`, `frames` and `cef` only; it never links the engine
//! core and never sends NDI (E-040).
//!
//! M1 (PLAN §7 task 4, §2.4): `execute_process` → `initialize` → `Hello` → on `Configure`, the
//! paint ring and a transparent OSR program browser on the engine's host page, its identity
//! injected after load; `on_paint` copies straight into the ring. Mode A posts external
//! begin-frames at `T_k − lead` (every slot active, every 6th standby); mode B free-runs at
//! `ceil(fps)` (10 fps standby). A paint probe forces a repaint after 100 ms without one
//! (E-057). The message loop runs on CEF's own schedule (`on_schedule_message_pump_work`).
//! The same executable serves as CEF's helper subprocesses (`--type=...`).
// TODO(M1): preview browser + monitor JPEGs on the standby slot (task 8 / M3). (STATUS.md)

mod cef_host;

use noplayout_frames::clock;
use noplayout_proto::{self as proto, Down, DriveMode, Event, Hello, Pong, RendererHello, Up};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long an orphaned renderer lingers before it shuts down (PLAN §4.2).
const ORPHAN_GRACE: Duration = Duration::from_secs(3);
/// Paint probe interval (E-057).
const PROBE_AFTER_NS: u64 = 100_000_000;

/// Command-line options of the browser (main) process.
#[derive(Debug, PartialEq, Eq)]
struct Options {
    channel: String,
    slot: String,
    cache_dir: PathBuf,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut channel = None;
    let mut slot = None;
    let mut cache_dir = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |flag: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match a.as_str() {
            "--channel" => channel = Some(value("--channel")?),
            "--slot" => slot = Some(value("--slot")?),
            "--cache-dir" => cache_dir = Some(PathBuf::from(value("--cache-dir")?)),
            // Chromium may append its own switches; they are not ours to refuse.
            other if other.starts_with("--") => {}
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    let usage = "usage: noplayout-renderer --channel <id> --slot a|b --cache-dir <dir>";
    let slot = slot.ok_or(usage)?;
    if slot != "a" && slot != "b" {
        return Err(format!("--slot must be a or b, not {slot}"));
    }
    Ok(Options {
        channel: channel.ok_or(usage)?,
        slot,
        cache_dir: cache_dir.ok_or(usage)?,
    })
}

/// `156.0.31+ge603d97+chromium-156.0.8078.100` → (`156.0.31`, `156.0.8078.100`).
fn split_cef_version(full: &str) -> (String, String) {
    let cef = full.split('+').next().unwrap_or(full).to_owned();
    let chromium = full
        .split('+')
        .find_map(|p| p.strip_prefix("chromium-"))
        .unwrap_or("unknown")
        .to_owned();
    (cef, chromium)
}

/// Everything that wakes the CEF main loop.
pub enum Wake {
    Down(Down),
    Closed(String),
    /// CEF asks for `do_message_loop_work` in `ms` (≤ 0: now).
    Pump(i64),
    /// Mode A: the begin-frame thread's deadline arrived.
    BeginFrame,
    /// A CEF callback has something for the engine.
    Event(Event),
}

/// Script for fault K3: block the page's main thread for `ms`.
fn hang_script(ms: u32) -> String {
    format!("(()=>{{const e=performance.now()+{ms};while(performance.now()<e){{}}}})()")
}

/// Mode A begin-frame thread (PLAN §2.4, §5.2): deadlines `T_k − lead` from the output epoch,
/// every slot while active, every 6th while standby. Runs at elevated priority.
fn spawn_begin_frames(
    cfg: &proto::RendererConfig,
    active: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    wake: mpsc::Sender<Wake>,
) -> std::io::Result<()> {
    let rate = clock::Rate {
        num: cfg.format.rate_num,
        den: cfg.format.rate_den,
    };
    let (epoch, lead) = (cfg.epoch_mono_ns, cfg.lead_ns);
    std::thread::Builder::new()
        .name("begin-frame".into())
        .spawn(move || {
            if !clock::elevate_current_thread() {
                eprintln!("noplayout-renderer: begin-frame thread runs at normal priority");
            }
            let mut k = clock::k_at_or_after(epoch, clock::mono_ns() + lead, rate);
            while !stop.load(Ordering::Relaxed) {
                let deadline = clock::t_k(epoch, k, rate).saturating_sub(lead);
                clock::wait_until(deadline);
                if (active.load(Ordering::Relaxed) || k.is_multiple_of(6))
                    && wake.send(Wake::BeginFrame).is_err()
                {
                    return;
                }
                // Never burst: after a stall, continue from the next future slot.
                k = (k + 1).max(clock::k_at_or_after(epoch, clock::mono_ns() + lead, rate));
            }
        })
        .map(|_| ())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if cef_host::is_subprocess(&args) {
        return cef_host::run_subprocess();
    }
    match run(&args[1..]) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("noplayout-renderer: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let opts = parse_options(args)?;
    let name = std::env::var(proto::ENV_IPC_NAME).map_err(|_| {
        format!(
            "{} is not set: the renderer is started by noplayout-engine",
            proto::ENV_IPC_NAME
        )
    })?;
    let token = std::env::var(proto::ENV_IPC_TOKEN)
        .map_err(|_| format!("{} is not set", proto::ENV_IPC_TOKEN))?;

    use interprocess::local_socket::{Stream, prelude::*};
    let socket = proto::socket_name(&name).map_err(|e| format!("bad socket name {name}: {e}"))?;
    let stream = Arc::new(
        Stream::connect(socket).map_err(|e| format!("cannot reach the engine at {name}: {e}"))?,
    );
    let reader = stream.clone();
    // `&Stream` is both Read and Write: the reader thread and this loop share one stream.
    let mut tx = &*stream;

    let (wake_tx, wake) = mpsc::channel::<Wake>();
    let mut cef = cef_host::Cef::start(&opts.cache_dir, wake_tx.clone())?;
    let (cef_version, chromium_version) = split_cef_version(cef_host::CEF_VERSION_FULL);
    let hello = Up::Hello(Hello::Renderer(RendererHello {
        proto: proto::PROTO_VERSION,
        renderer_version: env!("CARGO_PKG_VERSION").into(),
        cef_version,
        chromium_version,
        pid: std::process::id(),
        token,
    }));
    proto::write_frame(&mut tx, &hello).map_err(|e| format!("hello failed: {e}"))?;
    eprintln!(
        "noplayout-renderer: channel {} slot {} up on CEF {}",
        opts.channel,
        opts.slot,
        cef_host::CEF_VERSION_FULL
    );

    let rx_wake = wake_tx.clone();
    std::thread::Builder::new()
        .name("control-rx".into())
        .spawn(move || {
            let mut rx = &*reader;
            loop {
                match proto::read_frame::<_, Down>(&mut rx) {
                    Ok(msg) => {
                        if rx_wake.send(Wake::Down(msg)).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = rx_wake.send(Wake::Closed(e.to_string()));
                        return;
                    }
                }
            }
        })
        .map_err(|e| format!("cannot start the control reader: {e}"))?;

    let active = Arc::new(AtomicBool::new(false));
    let mut stop = Arc::new(AtomicBool::new(false));
    let mut config: Option<proto::RendererConfig> = None;
    let mut orphaned_at: Option<Instant> = None;
    let mut next_pump = Instant::now();
    let mut last_probe_ns = 0u64;
    let send = |tx: &mut &interprocess::local_socket::Stream, up: &Up| {
        if let Err(e) = proto::write_frame(tx, up) {
            eprintln!("noplayout-renderer: send failed: {e}");
        }
    };

    let code = loop {
        let timeout = next_pump.saturating_duration_since(Instant::now());
        match wake.recv_timeout(timeout) {
            Ok(Wake::Pump(ms)) => {
                let at = Instant::now() + Duration::from_millis(ms.max(0) as u64);
                next_pump = next_pump.min(at);
            }
            Ok(Wake::BeginFrame) => cef.begin_frame(),
            Ok(Wake::Event(ev)) => {
                if let Event::PageTerminated { status } = &ev {
                    eprintln!("noplayout-renderer: program page terminated: {status}");
                }
                send(&mut tx, &Up::Event(ev));
            }
            Ok(Wake::Down(Down::Ping { seq })) => {
                let pong = Up::Pong(Pong {
                    seq,
                    last_paint_seq: cef.shared.paints.load(Ordering::Relaxed),
                });
                send(&mut tx, &pong);
            }
            Ok(Wake::Down(Down::Configure(cfg))) => {
                // A replaced begin-frame thread stops at its next deadline.
                stop.store(true, Ordering::Relaxed);
                stop = Arc::new(AtomicBool::new(false));
                active.store(cfg.active, Ordering::Relaxed);
                match cef.open_program(&cfg) {
                    Ok(()) => {
                        eprintln!(
                            "noplayout-renderer: program page {} mode {:?} {}",
                            cfg.url,
                            cfg.mode,
                            if cfg.active { "active" } else { "standby" }
                        );
                        if cfg.mode == DriveMode::A
                            && let Err(e) = spawn_begin_frames(
                                &cfg,
                                active.clone(),
                                stop.clone(),
                                wake_tx.clone(),
                            )
                        {
                            send(
                                &mut tx,
                                &Up::Event(Event::ConfigureFailed {
                                    why: format!("begin-frame thread: {e}"),
                                }),
                            );
                        }
                        config = Some(cfg);
                    }
                    Err(why) => {
                        eprintln!("noplayout-renderer: configure failed: {why}");
                        send(&mut tx, &Up::Event(Event::ConfigureFailed { why }));
                    }
                }
            }
            Ok(Wake::Down(Down::Role { active: a })) => {
                active.store(a, Ordering::Relaxed);
                if let Some(cfg) = &config {
                    let rate = clock::Rate {
                        num: cfg.format.rate_num,
                        den: cfg.format.rate_den,
                    };
                    cef.set_frame_rate(cfg.mode, a, rate);
                }
            }
            Ok(Wake::Down(Down::Activate { .. })) => {
                eprintln!("noplayout-renderer: Activate is an output message; ignored");
            }
            Ok(Wake::Down(Down::HangPage { ms })) => {
                eprintln!("noplayout-renderer: hanging the program page for {ms} ms (fault K3)");
                cef.run_script(&hang_script(ms));
            }
            Ok(Wake::Down(Down::Shutdown)) => break ExitCode::SUCCESS,
            Ok(Wake::Closed(why)) => {
                eprintln!(
                    "noplayout-renderer: control socket closed ({why}); shutting down in {} s",
                    ORPHAN_GRACE.as_secs()
                );
                orphaned_at = Some(Instant::now());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break ExitCode::FAILURE,
        }
        if Instant::now() >= next_pump {
            cef.pump();
            next_pump = Instant::now() + cef_host::PUMP_FALLBACK;
        }
        // Paint probe (E-057): no paint for 100 ms → full-view invalidate, recorded in the ring.
        if cef.has_program() {
            let now = clock::mono_ns();
            let last = cef.shared.last_paint_ns.load(Ordering::Relaxed);
            if now.saturating_sub(last) >= PROBE_AFTER_NS
                && now.saturating_sub(last_probe_ns) >= PROBE_AFTER_NS
            {
                last_probe_ns = now;
                cef.invalidate();
                if let Some(w) = cef
                    .shared
                    .ring
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .as_ref()
                {
                    w.record_probe(now);
                }
                // The probe deadline may be sooner than CEF's next scheduled pump.
                next_pump = next_pump.min(Instant::now() + Duration::from_millis(1));
            }
        }
        if orphaned_at.is_some_and(|t| t.elapsed() >= ORPHAN_GRACE) {
            break ExitCode::from(4);
        }
    };
    stop.store(true, Ordering::Relaxed);
    cef.shutdown();
    eprintln!("noplayout-renderer: stopped");
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_engine_arguments() {
        let o = parse_options(&args(&[
            "--channel",
            "1",
            "--slot",
            "b",
            "--cache-dir",
            "/c",
        ]))
        .unwrap();
        assert_eq!(
            o,
            Options {
                channel: "1".into(),
                slot: "b".into(),
                cache_dir: PathBuf::from("/c"),
            }
        );
        assert!(
            parse_options(&args(&[
                "--channel",
                "1",
                "--slot",
                "c",
                "--cache-dir",
                "/c"
            ]))
            .is_err()
        );
        assert!(parse_options(&args(&["--channel", "1"])).is_err());
    }

    #[test]
    fn splits_the_cef_version_string() {
        assert_eq!(
            split_cef_version("156.0.31+ge603d97+chromium-156.0.8078.100"),
            ("156.0.31".to_owned(), "156.0.8078.100".to_owned())
        );
        assert_eq!(
            split_cef_version("1.2.3"),
            ("1.2.3".to_owned(), "unknown".to_owned())
        );
    }

    #[test]
    fn compiled_cef_matches_the_pin() {
        let (cef, _) = split_cef_version(cef_host::CEF_VERSION_FULL);
        assert_eq!(
            cef, "156.0.31",
            "cef crate pin is =156.7.0+156.0.31 (PLAN §3)"
        );
    }
}
