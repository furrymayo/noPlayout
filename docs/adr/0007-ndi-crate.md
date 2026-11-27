# 0007. NDI crate: grafton-ndi 3.0.0 behind our own trait

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-007 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-007 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** (a) `grafton-ndi` 3.0.0 (Apache-2.0 AND MIT, runtime-loaded, no SDK needed at build on Win/macOS/Linux).
  (b) `ndi-sdk-sys` (GPL-3.0, no macOS). (c) `ndi-sdk` (no send or receive). (d) Our own `libloading` wrapper over the
  MIT-licensed headers and `NDIlib_v6_load`.

## Decision

- **Choice.** (a), behind a `noplayout-ndi` trait (`NdiOutput`, `NdiReceiver`, runtime detection) so (d) can replace it.
- **Reason.** (a) is the only maintained crate with sending, receiving, BGRA/BGRX, explicit timecode, async submit with
  buffer reclaim, connection counts and a runtime search that matches our "detect, don't bundle" rule (exe dir → absolute
  `NDI_RUNTIME_DIR_V6` → platform paths; relative paths refused). I read these APIs in the 3.0.0 source. (b) is GPL, which
  the NDI licence and our licence rule out. (d) is about 1 kLOC we don't need yet.

## Consequences

- **Risk.** One maintainer (R4). Exit is (d), sized as one card.
