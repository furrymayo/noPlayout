//! `noplayout-output --channel <id>`: one per channel; owns the channel clock and, from M2, the
//! NDI senders (E-040). It never links CEF.
//!
//! M0: connect to the engine's control socket (`NOPLAYOUT_IPC_NAME`), send
//! `Hello{proto, version, pid, token, epoch}`, answer `Ping`s, exit 0 on `Shutdown`. If the
//! control socket closes without a `Shutdown`, exit after 3 s (orphan rule, PLAN §4.2).
// TODO(M1): channel clock, paint-ring reader. TODO(M2): NDI senders. (STATUS.md)

use interprocess::local_socket::{Stream, prelude::*};
use noplayout_proto::{self as proto, Down, Epoch, Hello, OutputHello, Pong, Up};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long an orphaned output process lingers before exiting (PLAN §4.2).
const ORPHAN_GRACE: Duration = Duration::from_secs(3);

fn main() -> ExitCode {
    let started = Instant::now();
    match run(started) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("noplayout-output: {e}");
            ExitCode::FAILURE
        }
    }
}

fn channel_arg() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let mut channel = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--channel" => channel = Some(args.next().ok_or("--channel needs a value")?),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    channel.ok_or_else(|| "usage: noplayout-output --channel <id>".to_owned())
}

/// The channel clock epoch. `mono_ns` is measured from this process's start (its monotonic
/// origin); `utc_100ns` is wall-clock UTC in NDI's 100 ns units.
fn epoch(started: Instant) -> Epoch {
    let utc_100ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_nanos() / 100).unwrap_or(i64::MAX))
        .unwrap_or(0);
    Epoch {
        mono_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
        utc_100ns,
    }
}

fn run(started: Instant) -> Result<ExitCode, String> {
    let channel = channel_arg()?;
    let name = std::env::var(proto::ENV_IPC_NAME).map_err(|_| {
        format!(
            "{} is not set: the output process is started by noplayout-engine",
            proto::ENV_IPC_NAME
        )
    })?;
    let token = std::env::var(proto::ENV_IPC_TOKEN)
        .map_err(|_| format!("{} is not set", proto::ENV_IPC_TOKEN))?;
    let socket = proto::socket_name(&name).map_err(|e| format!("bad socket name {name}: {e}"))?;
    let stream =
        Stream::connect(socket).map_err(|e| format!("cannot reach the engine at {name}: {e}"))?;
    // `&Stream` is both Read and Write.
    let (mut rx, mut tx) = (&stream, &stream);

    let hello = Up::Hello(Hello::Output(OutputHello {
        proto: proto::PROTO_VERSION,
        version: env!("CARGO_PKG_VERSION").into(),
        pid: std::process::id(),
        token,
        epoch: epoch(started),
    }));
    proto::write_frame(&mut tx, &hello).map_err(|e| format!("hello failed: {e}"))?;
    eprintln!("noplayout-output: channel {channel} connected");

    loop {
        match proto::read_frame::<_, Down>(&mut rx) {
            Ok(Down::Ping { seq }) => {
                let pong = Up::Pong(Pong {
                    seq,
                    last_paint_seq: 0,
                });
                if let Err(e) = proto::write_frame(&mut tx, &pong) {
                    eprintln!("noplayout-output: pong failed: {e}");
                }
            }
            Ok(Down::Shutdown) => {
                eprintln!("noplayout-output: channel {channel} shutting down");
                return Ok(ExitCode::SUCCESS);
            }
            Err(e) => {
                eprintln!(
                    "noplayout-output: control socket closed ({e}); exiting in {} s",
                    ORPHAN_GRACE.as_secs()
                );
                std::thread::sleep(ORPHAN_GRACE);
                return Ok(ExitCode::from(4));
            }
        }
    }
}
