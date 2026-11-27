//! `noplayout-renderer --channel <id> --slot a|b --cache-dir <dir>`: the CEF offscreen renderer
//! (D-001, E-005, E-041). It links `proto` and `cef` only; it never links the engine core and
//! never sends NDI (E-040).
//!
//! M0 (PLAN §7 task 4): `execute_process` → `initialize` → one windowless browser on
//! `about:blank` → `Hello{cef_version}` over the control socket → answer `Ping`s with the paint
//! count → clean shutdown on `Shutdown`. The same executable serves as CEF's helper subprocesses
//! (`--type=...` on the command line), on macOS from the dev bundle's helper apps.
// TODO(M1): host protocol pages, paint ring, drive modes, standby rate. (STATUS.md)

mod cef_host;

use noplayout_proto::{self as proto, Down, Hello, Pong, RendererHello, Up};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long an orphaned renderer lingers before it shuts down (PLAN §4.2).
const ORPHAN_GRACE: Duration = Duration::from_secs(3);

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

/// Messages from the control-socket reader thread to the CEF main loop.
enum Inbox {
    Down(Down),
    Closed(String),
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
    let stream = std::sync::Arc::new(
        Stream::connect(socket).map_err(|e| format!("cannot reach the engine at {name}: {e}"))?,
    );
    let reader = stream.clone();
    // `&Stream` is both Read and Write: the reader thread and this loop share one stream.
    let mut tx = &*stream;

    let cef = cef_host::Cef::start(&opts.cache_dir)?;
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

    let (inbox_tx, inbox) = mpsc::channel::<Inbox>();
    std::thread::Builder::new()
        .name("control-rx".into())
        .spawn(move || {
            let mut rx = &*reader;
            loop {
                match proto::read_frame::<_, Down>(&mut rx) {
                    Ok(msg) => {
                        if inbox_tx.send(Inbox::Down(msg)).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = inbox_tx.send(Inbox::Closed(e.to_string()));
                        return;
                    }
                }
            }
        })
        .map_err(|e| format!("cannot start the control reader: {e}"))?;

    let mut orphaned_at: Option<Instant> = None;
    let code = loop {
        cef.pump();
        match inbox.try_recv() {
            Ok(Inbox::Down(Down::Ping { seq })) => {
                let pong = Up::Pong(Pong {
                    seq,
                    last_paint_seq: cef.paints(),
                });
                if let Err(e) = proto::write_frame(&mut tx, &pong) {
                    eprintln!("noplayout-renderer: pong failed: {e}");
                }
            }
            Ok(Inbox::Down(Down::Shutdown)) => break ExitCode::SUCCESS,
            Ok(Inbox::Down(Down::HangPage { ms })) => {
                // TODO(M1): task 4 runs a busy loop in the program page via execute_java_script.
                eprintln!("noplayout-renderer: hang {ms} ms requested; no program page yet");
            }
            Ok(Inbox::Closed(why)) => {
                eprintln!(
                    "noplayout-renderer: control socket closed ({why}); shutting down in {} s",
                    ORPHAN_GRACE.as_secs()
                );
                orphaned_at = Some(Instant::now());
            }
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => {}
        }
        if orphaned_at.is_some_and(|t| t.elapsed() >= ORPHAN_GRACE) {
            break ExitCode::from(4);
        }
        std::thread::sleep(cef_host::PUMP_INTERVAL);
    };
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
