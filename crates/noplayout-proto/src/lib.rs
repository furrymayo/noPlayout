//! Control-socket messages between the engine and its child processes (PLAN §4.1, §4.2).
//!
//! Wire format: every message is a `u32` little-endian body length followed by a postcard body,
//! at most [`MAX_FRAME`] bytes. The first message on a fresh connection is always the child's
//! [`Up::Hello`]; a [`Hello`] whose `proto` differs from [`PROTO_VERSION`] is a hard error
//! (a mixed install), reported with a plain message.
//!
//! M0 carries only what the skeleton needs: `Hello`, `Ping`/`Pong` and `Shutdown`.
//! `Configure`, `Role`, `Health`, `Event` and the monitor socket arrive with M1/M2.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Control protocol version. Bump on any incompatible change to the types below.
pub const PROTO_VERSION: u32 = 2;

/// Largest accepted frame body (1 MiB, PLAN §4.1).
pub const MAX_FRAME: usize = 1024 * 1024;

/// Environment variable carrying the local-socket name a child connects to.
pub const ENV_IPC_NAME: &str = "NOPLAYOUT_IPC_NAME";
/// Environment variable carrying the per-launch token a child echoes in its `Hello`.
pub const ENV_IPC_TOKEN: &str = "NOPLAYOUT_IPC_TOKEN";

/// Map a control-socket name (as carried in [`ENV_IPC_NAME`]) to an `interprocess` local-socket
/// name. Engine and children both go through here so they always agree: a named pipe on
/// Windows, an abstract-namespace socket on Linux, a socket file in the temp directory on macOS.
pub fn socket_name(name: &str) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    use interprocess::local_socket::{GenericNamespaced, ToNsName};
    name.to_ns_name::<GenericNamespaced>()
}

/// Messages from the engine to a child process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Down {
    /// Heartbeat; the child answers with `Up::Pong { seq }`.
    Ping { seq: u64 },
    /// Close everything and exit with status 0.
    Shutdown,
    /// Fault injection K3 (dev `ctl hang`, S7a): block the program page's main thread for `ms`.
    HangPage { ms: u32 },
    /// Renderer: what to paint and where (sent once, after `Hello`).
    Configure(RendererConfig),
    /// Renderer: active (full rate) or standby (every 6th slot / 10 fps), §2.4. Sent at promotion.
    Role { active: bool },
    /// Output: read paints from this renderer incarnation's ring from now on (§2.4a, §2.13).
    Activate {
        ring: String,
        inc: u64,
        mode: DriveMode,
    },
}

/// Video format of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Format {
    pub width: u32,
    pub height: u32,
    pub rate_num: u32,
    pub rate_den: u32,
}

impl Format {
    pub const HD1080P5994: Format = Format {
        width: 1920,
        height: 1080,
        rate_num: 60000,
        rate_den: 1001,
    };
}

/// How the renderer is paced (PLAN §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriveMode {
    /// External begin-frame at `T_k − lead`.
    A,
    /// Free-running at `ceil(fps)`.
    B,
}

/// Everything a renderer needs to open its program page (PLAN §2.4, §2.14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererConfig {
    pub format: Format,
    pub mode: DriveMode,
    /// The output process's epoch (machine-wide monotonic ns), for mode A deadlines.
    pub epoch_mono_ns: u64,
    /// Mode A: begin-frame lead before `T_k`, ns (one frame to start; S1 tunes it).
    pub lead_ns: u64,
    /// Program page URL on the engine (`/host/renderer.html?channel=…&role=program`).
    pub url: String,
    /// Page identity handed to the page by script injection after load, never in the URL
    /// (E-046): the page's incarnation and single-use registration token.
    pub page_inc: u64,
    pub page_token: String,
    /// Paint ring object name; the engine has recorded it in its registry already.
    pub ring: String,
    pub active: bool,
}

/// Renderer events the engine acts on (PLAN §2.4, §2.13.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// The program page finished loading (main frame), with its HTTP status.
    PageLoaded {
        http_status: i32,
    },
    /// CEF's render process for the program page died (failure kind K2).
    PageTerminated {
        status: String,
    },
    /// A console message from the program page (conformance asserts zero errors).
    Console {
        error: bool,
        text: String,
    },
    /// The renderer could not do what `Configure` asked (ring, browser); it stays up.
    ConfigureFailed {
        why: String,
    },
    /// Output: the active ring could not be opened.
    ActivateFailed {
        ring: String,
        why: String,
    },
    /// Output, K2g (E-057): a paint probe has been outstanding 250 ms with no newer paint.
    PaintStall {
        inc: u64,
        probe_seq: u64,
    },
    /// Output, K5 (§5.4): a plane's `submit` has been in flight 250 ms (slow) or 2 s (stuck).
    SenderStall {
        plane: String,
        in_flight_ms: u64,
    },
    SenderStuck {
        plane: String,
        in_flight_ms: u64,
    },
    /// Output: once a second.
    OutputStats(OutputStats),
    /// Output: `--frames N` slots dumped; the dump is complete and closed.
    DumpComplete {
        frames: u64,
    },
}

/// Output-side counters (§5.3, §5.4). Each is cumulative since the output process started.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputStats {
    pub slots: u64,
    /// Slots that resent the previous snapshot (no newer paint).
    pub held: u64,
    /// Ready paints freed unread because a newer one was taken.
    pub superseded: u64,
    /// Slots dropped because the pacer woke after `T_k + period/2`.
    pub slots_skipped: u64,
    pub submit_blocked: u64,
    pub pair_partial: u64,
    pub ring_overwrites: u64,
    /// Pacer wake-up lateness, µs: worst since start.
    pub late_max_us: u64,
    pub last_paint_seq: u64,
    pub active_inc: u64,
}

/// Messages from a child process to the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Up {
    /// First message on every connection.
    Hello(Hello),
    /// Heartbeat answer.
    Pong(Pong),
    /// Something happened the engine should know about now, not at the next heartbeat.
    Event(Event),
}

/// The first message a child sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hello {
    /// `noplayout-output`: fixes the channel clock epoch.
    Output(OutputHello),
    /// `noplayout-renderer`: reports the CEF it runs.
    Renderer(RendererHello),
}

impl Hello {
    /// Protocol version the child speaks.
    pub fn proto(&self) -> u32 {
        match self {
            Hello::Output(h) => h.proto,
            Hello::Renderer(h) => h.proto,
        }
    }

    /// Per-launch token the child received in [`ENV_IPC_TOKEN`].
    pub fn token(&self) -> &str {
        match self {
            Hello::Output(h) => &h.token,
            Hello::Renderer(h) => &h.token,
        }
    }

    /// Check protocol version and token; the error text is shown to the operator as is.
    pub fn check(&self, expected_token: &str) -> Result<(), HelloError> {
        if self.proto() != PROTO_VERSION {
            return Err(HelloError::ProtoMismatch {
                theirs: self.proto(),
                ours: PROTO_VERSION,
            });
        }
        if self.token() != expected_token {
            return Err(HelloError::BadToken);
        }
        Ok(())
    }
}

/// Why a `Hello` was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HelloError {
    #[error(
        "a noPlayout process speaks control protocol {theirs}, this engine speaks {ours}: \
         the installation is mixed, reinstall noPlayout"
    )]
    ProtoMismatch { theirs: u32, ours: u32 },
    #[error("a process connected with the wrong launch token and was refused")]
    BadToken,
}

/// The output process's channel clock epoch (PLAN §4.2, §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Epoch {
    /// Machine-wide monotonic clock at the epoch, ns (`noplayout_frames::clock::mono_ns`), so
    /// renderers compute the same `T_k`.
    pub mono_ns: u64,
    /// UTC at the epoch in 100 ns units since the Unix epoch (NDI timecode units).
    pub utc_100ns: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputHello {
    pub proto: u32,
    /// `noplayout-output` package version.
    pub version: String,
    pub pid: u32,
    pub token: String,
    pub epoch: Epoch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererHello {
    pub proto: u32,
    /// `noplayout-renderer` package version.
    pub renderer_version: String,
    /// CEF version, e.g. `156.0.31`.
    pub cef_version: String,
    /// Chromium version CEF is built on, e.g. `156.0.8078.100`.
    pub chromium_version: String,
    pub pid: u32,
    pub token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    pub seq: u64,
    /// Renderer: paints received from CEF so far. Output: always 0 in M0.
    pub last_paint_seq: u64,
}

/// Framing and decoding errors.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("control socket I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("control frame of {0} bytes exceeds the 1 MiB limit")]
    TooLarge(usize),
    #[error("control frame could not be decoded: {0}")]
    Decode(#[from] postcard::Error),
}

/// Encode `msg` as one frame: `u32` LE length + postcard body.
pub fn encode_frame<T: Serialize>(msg: &T) -> Result<Vec<u8>, FrameError> {
    let body = postcard::to_stdvec(msg)?;
    if body.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(body.len()));
    }
    let len = u32::try_from(body.len()).map_err(|_| FrameError::TooLarge(body.len()))?;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Parse a frame length prefix, enforcing [`MAX_FRAME`].
pub fn frame_len(prefix: [u8; 4]) -> Result<usize, FrameError> {
    let len = u32::from_le_bytes(prefix) as usize;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge(len));
    }
    Ok(len)
}

/// Decode a frame body (without its length prefix).
pub fn decode_body<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, FrameError> {
    Ok(postcard::from_bytes(body)?)
}

/// Write one frame to a blocking stream and flush it.
pub fn write_frame<W: Write, T: Serialize>(w: &mut W, msg: &T) -> Result<(), FrameError> {
    let frame = encode_frame(msg)?;
    w.write_all(&frame)?;
    w.flush()?;
    Ok(())
}

/// Read one frame from a blocking stream.
pub fn read_frame<R: Read, T: for<'de> Deserialize<'de>>(r: &mut R) -> Result<T, FrameError> {
    let mut prefix = [0u8; 4];
    r.read_exact(&mut prefix)?;
    let len = frame_len(prefix)?;
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    decode_body(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn renderer_hello() -> Hello {
        Hello::Renderer(RendererHello {
            proto: PROTO_VERSION,
            renderer_version: "0.0.1".into(),
            cef_version: "156.0.31".into(),
            chromium_version: "156.0.8078.100".into(),
            pid: 4242,
            token: "t0k3n".into(),
        })
    }

    fn output_hello() -> Hello {
        Hello::Output(OutputHello {
            proto: PROTO_VERSION,
            version: "0.0.1".into(),
            pid: 7,
            token: "t0k3n".into(),
            epoch: Epoch {
                mono_ns: 123_456_789,
                utc_100ns: 17_957_798_440_000_000,
            },
        })
    }

    fn round_trip<T>(msg: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let mut buf = Vec::new();
        write_frame(&mut buf, msg).unwrap();
        let len = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
        assert_eq!(len, buf.len() - 4, "length prefix covers exactly the body");
        read_frame(&mut Cursor::new(buf)).unwrap()
    }

    #[test]
    fn hello_round_trips() {
        for hello in [renderer_hello(), output_hello()] {
            let up = Up::Hello(hello);
            assert_eq!(round_trip(&up), up);
        }
    }

    #[test]
    fn ping_pong_shutdown_round_trip() {
        for down in [
            Down::Ping { seq: 0 },
            Down::Ping { seq: u64::MAX },
            Down::Shutdown,
        ] {
            assert_eq!(round_trip(&down), down);
        }
        let pong = Up::Pong(Pong {
            seq: 99,
            last_paint_seq: 3,
        });
        assert_eq!(round_trip(&pong), pong);
    }

    #[test]
    fn several_frames_on_one_stream_stay_in_order() {
        let mut buf = Vec::new();
        for seq in 0..5 {
            write_frame(&mut buf, &Down::Ping { seq }).unwrap();
        }
        write_frame(&mut buf, &Down::Shutdown).unwrap();
        let mut cur = Cursor::new(buf);
        for seq in 0..5 {
            assert_eq!(read_frame::<_, Down>(&mut cur).unwrap(), Down::Ping { seq });
        }
        assert_eq!(read_frame::<_, Down>(&mut cur).unwrap(), Down::Shutdown);
        assert!(matches!(
            read_frame::<_, Down>(&mut cur),
            Err(FrameError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof
        ));
    }

    #[test]
    fn oversized_length_prefix_is_refused_before_allocating() {
        let mut buf = ((MAX_FRAME + 1) as u32).to_le_bytes().to_vec();
        buf.extend_from_slice(&[0; 8]);
        assert!(matches!(
            read_frame::<_, Down>(&mut Cursor::new(buf)),
            Err(FrameError::TooLarge(n)) if n == MAX_FRAME + 1
        ));
    }

    #[test]
    fn garbage_body_is_a_decode_error() {
        let mut buf = 2u32.to_le_bytes().to_vec();
        buf.extend_from_slice(&[0xff, 0xff]);
        assert!(matches!(
            read_frame::<_, Up>(&mut Cursor::new(buf)),
            Err(FrameError::Decode(_))
        ));
    }

    #[test]
    fn hello_check_refuses_mismatch_and_bad_token() {
        assert_eq!(renderer_hello().check("t0k3n"), Ok(()));
        assert_eq!(renderer_hello().check("other"), Err(HelloError::BadToken));
        let mut old = output_hello();
        if let Hello::Output(h) = &mut old {
            h.proto = PROTO_VERSION + 1;
        }
        let err = old.check("t0k3n").unwrap_err();
        assert!(matches!(err, HelloError::ProtoMismatch { .. }));
        assert!(err.to_string().contains("mixed"), "plain message: {err}");
    }
}
