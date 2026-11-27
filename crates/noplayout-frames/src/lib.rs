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
/// unique per engine run, channel and incarnation. The run part is the run id's **last** 10
/// alphanumerics: a UUIDv7 starts with its timestamp, so its head is shared by every engine
/// started in the same minute, and two engines on one machine would open each other's ring.
pub fn ring_name(run: &str, channel: &str, inc: u64) -> String {
    let alnum: Vec<char> = run.chars().filter(char::is_ascii_alphanumeric).collect();
    let run: String = alnum[alnum.len().saturating_sub(10)..].iter().collect();
    let channel: String = channel
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(4)
        .collect();
    format!("np{run}-{channel}-{inc}")
}

#[cfg(test)]
mod tests;
