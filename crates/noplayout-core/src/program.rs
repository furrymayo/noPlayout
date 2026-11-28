//! Opening a channel's program page in a renderer (PLAN §2.4, §2.14, E-046): a fresh
//! incarnation and single-use page token on the host bus, the paint ring name recorded before
//! the renderer creates it, then `Configure`.

use crate::children::ChildHandle;
use crate::hostbus::HostBus;
use noplayout_frames::shm::Registry;
use noplayout_proto::{Down, DriveMode, Format, RendererConfig};

/// Environment override for the drive mode (`A` or `B`); mode A is preferred (PLAN §5.2).
pub const ENV_DRIVE_MODE: &str = "NOPLAYOUT_DRIVE_MODE";

pub fn drive_mode_from_env() -> DriveMode {
    match std::env::var(ENV_DRIVE_MODE).ok().as_deref() {
        Some("B" | "b") => DriveMode::B,
        _ => DriveMode::A,
    }
}

/// Dev/harness override of the channel rate, `NOPLAYOUT_RATE=50` or `60000/1001` (the output
/// process reads the same variable). Until channels have a configured format (M2), 1080p59.94.
pub const ENV_RATE: &str = "NOPLAYOUT_RATE";

pub fn format_from_env() -> noplayout_proto::Format {
    let mut f = noplayout_proto::Format::HD1080P5994;
    if let Ok(v) = std::env::var(ENV_RATE) {
        let (n, d) = v.split_once('/').unwrap_or((&v, "1"));
        match (n.parse::<u32>(), d.parse::<u32>()) {
            (Ok(n), Ok(d)) if n > 0 && d > 0 => (f.rate_num, f.rate_den) = (n, d),
            _ => tracing::warn!("ignoring a bad {ENV_RATE}={v}"),
        }
    }
    f
}

/// A page token: two UUIDv7 bodies (148 random bits), alphabet `[0-9a-f]`.
pub fn page_token() -> String {
    format!("{}{}", crate::new_token(), crate::new_token())
}

/// The program page URL on this engine.
pub fn program_url(port: u16, channel: &str) -> String {
    format!("http://127.0.0.1:{port}/host/renderer.html?channel={channel}&role=program")
}

#[derive(Debug, Clone)]
pub struct Opened {
    pub inc: u64,
    pub ring: String,
}

pub struct ProgramSpec<'a> {
    pub channel: &'a str,
    pub run: &'a str,
    pub port: u16,
    pub format: Format,
    pub mode: DriveMode,
    pub epoch_mono_ns: u64,
    pub active: bool,
}

/// Give `renderer` its program page. With `active`, the new incarnation becomes authoritative.
pub async fn open(
    bus: &HostBus,
    registry: &Registry,
    renderer: &ChildHandle,
    spec: ProgramSpec<'_>,
) -> Result<Opened, String> {
    let ch = bus
        .channel(spec.channel)
        .ok_or_else(|| format!("no channel {}", spec.channel))?;
    let token = page_token();
    let inc = ch
        .new_incarnation(token.clone())
        .await
        .map_err(|e| e.to_string())?;
    let ring = noplayout_frames::ring_name(spec.run, spec.channel, inc);
    registry
        .record(&ring)
        .map_err(|e| format!("cannot record ring {ring}: {e}"))?;
    let period = noplayout_frames::clock::Rate {
        num: spec.format.rate_num,
        den: spec.format.rate_den,
    }
    .period_ns();
    renderer
        .send(Down::Configure(RendererConfig {
            format: spec.format,
            mode: spec.mode,
            epoch_mono_ns: spec.epoch_mono_ns,
            // One frame to start (PLAN §5.2); S1 tunes it.
            lead_ns: period,
            url: program_url(spec.port, spec.channel),
            page_inc: inc,
            page_token: token,
            ring: ring.clone(),
            active: spec.active,
        }))
        .await?;
    if spec.active {
        ch.promote(inc).await;
    }
    Ok(Opened { inc, ring })
}

#[cfg(test)]
mod tests {
    #[test]
    fn page_token_alphabet_and_url() {
        let t = super::page_token();
        assert_eq!(t.len(), 64);
        assert!(t.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(
            super::program_url(8686, "1"),
            "http://127.0.0.1:8686/host/renderer.html?channel=1&role=program"
        );
    }
}
