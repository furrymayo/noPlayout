//! The shared-memory paint ring between `noplayout-renderer` (writer) and `noplayout-output`
//! (reader), E-042 and PLAN §2.4, §5.3.
//!
//! Transport is our own 4-slot ring over a platform shared-memory object ([`shm`]); spike S8
//! (`docs/spikes/S8-paint-ring.md`) records why iceoryx2 was not adopted in M1.

pub mod clock;
pub mod ring;
pub mod shm;

pub use ring::{PaintMeta, ReadSlot, Reader, SLOTS, WriteSlot, Writer, ring_bytes, slot_bytes};

/// Ring object name for one renderer incarnation. Short enough for macOS (≤ 30 bytes) and
/// unique per engine run, channel and incarnation.
pub fn ring_name(run: &str, channel: &str, inc: u64) -> String {
    let run: String = run
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect();
    let channel: String = channel
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(4)
        .collect();
    format!("np{run}-{channel}-{inc}")
}

#[cfg(test)]
mod tests;
