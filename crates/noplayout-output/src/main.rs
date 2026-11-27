//! `noplayout-output --channel <id> [--rate 60000/1001] [--dump-paints <dir> [--frames N]
//! [--dump-crop x,y,w,h]...]`: one per channel; owns the channel clock and, from M2, the NDI
//! senders (E-040). It never links CEF.
//!
//! M1 (PLAN §7 task 5, §2.4a, §5): `Hello{epoch}` on the machine-wide monotonic clock, then a
//! pacer thread that at each `T_k` takes the newest paint of the active ring (`Activate`),
//! converts it once into a FILL+KEY snapshot stamped `tc(k)`, and hands it to one sender
//! thread per plane (`DumpSink` with `--dump-paints`, else a counting sink); a 10 Hz monitor
//! raises `SenderStall`/`SenderStuck`, the pacer raises `PaintStall` (K2g). Dump options may
//! also come from `NOPLAYOUT_DUMP_PAINTS`, `NOPLAYOUT_DUMP_FRAMES`, `NOPLAYOUT_DUMP_CROP`
//! (`;`-separated rectangles), so the engine's children inherit them. If the control socket
//! closes without a `Shutdown`, exit after 3 s (orphan rule, PLAN §4.2).
// TODO(M2): NdiSink behind the same lanes; Fill+Alpha single-plane mode. (STATUS.md)
// TODO(M1): monitor tap and divergence threads (§2.4a) come with the supervisor (task 8).

mod fault;
mod pacer;
mod sink;

use interprocess::local_socket::{Stream, prelude::*};
use noplayout_frames::clock::{self, Rate};
use noplayout_proto::{self as proto, Down, Epoch, Event, Hello, OutputHello, Pong, Up};
use sink::{DumpSink, FrameSink, NullSink, Plane};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long an orphaned output process lingers before exiting (PLAN §4.2).
const ORPHAN_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, PartialEq, Eq)]
struct Options {
    channel: String,
    rate: Rate,
    dump: Option<PathBuf>,
    frames: Option<u64>,
    crop: sink::Crop,
}

fn parse_rate(s: &str) -> Result<Rate, String> {
    let (n, d) = s.split_once('/').unwrap_or((s, "1"));
    let num = n.parse::<u32>().map_err(|_| format!("bad rate {s}"))?;
    let den = d.parse::<u32>().map_err(|_| format!("bad rate {s}"))?;
    if num == 0 || den == 0 {
        return Err(format!("bad rate {s}"));
    }
    Ok(Rate { num, den })
}

fn parse_options(args: &[String], env: impl Fn(&str) -> Option<String>) -> Result<Options, String> {
    let mut o = Options {
        channel: String::new(),
        rate: Rate::P5994,
        dump: env("NOPLAYOUT_DUMP_PAINTS").map(PathBuf::from),
        frames: None,
        crop: Vec::new(),
    };
    if let Some(n) = env("NOPLAYOUT_DUMP_FRAMES") {
        o.frames = Some(
            n.parse()
                .map_err(|_| format!("bad NOPLAYOUT_DUMP_FRAMES {n}"))?,
        );
    }
    if let Some(c) = env("NOPLAYOUT_DUMP_CROP") {
        for r in c.split(';').filter(|r| !r.is_empty()) {
            o.crop.push(sink::parse_crop(r)?);
        }
    }
    let mut channel = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--channel" => channel = Some(value()?),
            "--rate" => o.rate = parse_rate(&value()?)?,
            "--dump-paints" => o.dump = Some(PathBuf::from(value()?)),
            "--frames" => {
                let v = value()?;
                o.frames = Some(v.parse().map_err(|_| format!("bad --frames {v}"))?);
            }
            "--dump-crop" => o.crop.push(sink::parse_crop(&value()?)?),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    o.channel = channel.ok_or(
        "usage: noplayout-output --channel <id> [--rate n/d] \
         [--dump-paints <dir> [--frames N] [--dump-crop x,y,w,h]...]",
    )?;
    Ok(o)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("noplayout-output: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The channel clock epoch (PLAN §5.1): machine-wide monotonic ns, so renderers compute the
/// same `T_k`, and UTC in NDI's 100 ns units.
fn epoch() -> Epoch {
    let mono_ns = clock::mono_ns();
    let utc_100ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_nanos() / 100).unwrap_or(i64::MAX))
        .unwrap_or(0);
    Epoch { mono_ns, utc_100ns }
}

fn run() -> Result<ExitCode, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = parse_options(&args, |k| std::env::var(k).ok())?;
    let fault = fault::from_env()?;
    let channel = opts.channel.clone();
    let name = std::env::var(proto::ENV_IPC_NAME).map_err(|_| {
        format!(
            "{} is not set: the output process is started by noplayout-engine",
            proto::ENV_IPC_NAME
        )
    })?;
    let token = std::env::var(proto::ENV_IPC_TOKEN)
        .map_err(|_| format!("{} is not set", proto::ENV_IPC_TOKEN))?;
    let socket = proto::socket_name(&name).map_err(|e| format!("bad socket name {name}: {e}"))?;
    let stream = Arc::new(
        Stream::connect(socket).map_err(|e| format!("cannot reach the engine at {name}: {e}"))?,
    );

    let ep = epoch();
    // One writer thread owns the socket's send side, so frames never interleave.
    let (up_tx, up_rx) = mpsc::channel::<Up>();
    let writer = stream.clone();
    std::thread::Builder::new()
        .name("control-tx".into())
        .spawn(move || {
            let mut tx = &*writer;
            for up in up_rx {
                if let Err(e) = proto::write_frame(&mut tx, &up) {
                    eprintln!("noplayout-output: send failed: {e}");
                    return;
                }
            }
        })
        .map_err(|e| format!("cannot start the control writer: {e}"))?;
    up_tx
        .send(Up::Hello(Hello::Output(OutputHello {
            proto: proto::PROTO_VERSION,
            version: env!("CARGO_PKG_VERSION").into(),
            pid: std::process::id(),
            token,
            epoch: ep,
        })))
        .map_err(|_| "control writer stopped".to_owned())?;
    eprintln!("noplayout-output: channel {channel} connected");

    let mut sinks: Vec<(Plane, Box<dyn FrameSink>)> = Vec::new();
    for plane in [Plane::Fill, Plane::Key] {
        let s: Box<dyn FrameSink> = match &opts.dump {
            Some(dir) => Box::new(
                DumpSink::create(dir, plane, opts.crop.clone())
                    .map_err(|e| format!("cannot create dump in {}: {e}", dir.display()))?,
            ),
            None => Box::new(NullSink),
        };
        sinks.push((plane, s));
    }
    let (ev_tx, ev_rx) = mpsc::channel::<Event>();
    let fwd = up_tx.clone();
    std::thread::Builder::new()
        .name("events".into())
        .spawn(move || {
            for ev in ev_rx {
                if fwd.send(Up::Event(ev)).is_err() {
                    return;
                }
            }
        })
        .map_err(|e| format!("cannot start the event forwarder: {e}"))?;
    let pacer = pacer::start(
        pacer::Config {
            rate: opts.rate,
            epoch_mono_ns: ep.mono_ns,
            u0_100ns: ep.utc_100ns,
            frames: opts.frames,
            fault,
        },
        sinks,
        ev_tx,
    )
    .map_err(|e| format!("cannot start the pacer: {e}"))?;

    let mut rx = &*stream;
    let code = loop {
        match proto::read_frame::<_, Down>(&mut rx) {
            Ok(Down::Ping { seq }) => {
                let pong = Up::Pong(Pong {
                    seq,
                    last_paint_seq: pacer.stats.last_paint_seq.load(Ordering::Relaxed),
                });
                let _ = up_tx.send(pong);
            }
            Ok(Down::Activate { ring, inc, mode }) => {
                *pacer
                    .control
                    .activate
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = Some((ring, inc, mode));
            }
            Ok(Down::HangPage { .. } | Down::Configure(_) | Down::Role { .. }) => {
                eprintln!("noplayout-output: renderer-only message ignored");
            }
            Ok(Down::Shutdown) => {
                eprintln!("noplayout-output: channel {channel} shutting down");
                break ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!(
                    "noplayout-output: control socket closed ({e}); exiting in {} s",
                    ORPHAN_GRACE.as_secs()
                );
                std::thread::sleep(ORPHAN_GRACE);
                break ExitCode::from(4);
            }
        }
    };
    pacer.control.stop.store(true, Ordering::Relaxed);
    for (_, lane) in &pacer.lanes {
        lane.stop();
    }
    // Let the sender threads close their sinks.
    std::thread::sleep(Duration::from_millis(100));
    Ok(code)
}

#[cfg(test)]
mod tests;
