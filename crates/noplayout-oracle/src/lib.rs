//! Test and harness only: decodes the frame-oracle patches from received pixels and judges a
//! run against PLAN §5.5's thresholds (E-048). Reads pixels and timecodes only, never
//! noPlayout's own counters or logs.
//!
//! - [`layout`]: patch geometry, shared with `fixtures/frame-oracle`, plus a reference encoder.
//! - [`decode`]: patches from FILL/KEY planes (whole or cropped).
//! - [`analyze`]: Δn classes, tears, key mismatch, timecode steps, source period, step check,
//!   lag band, and the thresholds per mode and rate.
//! - [`dump`]: reading an output-process paint dump.
//!
//! The `noplayout-oracle` binary runs it on a dump directory.
// The recovery-probe patches decode with [`decode::read_probe`]; the S7a harness that uses them
// is task 8.
// TODO(M2): the same analysis on an NDI receiver.

pub mod analyze;
pub mod decode;
pub mod dump;
pub mod layout;

#[cfg(test)]
mod tests;
