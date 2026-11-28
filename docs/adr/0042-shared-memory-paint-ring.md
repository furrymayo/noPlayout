# 0042. Shared-memory paint ring per renderer (iceoryx2 or own ring)

- **Status:** accepted (DECISIONS.md: Spike-gated (S8)); superseded in part by 0063 (transport clause)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-042 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-042 (561e4a6), PLAN.md, ADR 0040

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** One 4-slot ring per renderer, slot headers `{state, inc, paint_seq, painted_at}` changed only by CAS; the writer
  never blocks and never overwrites a slot being read; the reader holds at most one slot (PLAN §5.3). Transport: iceoryx2
  0.10.1 if S8 shows it meets that with zero extra copies and no leaked shared memory after `kill -9` on all three OSes;
  otherwise our own ring over `shm_open` / pagefile-backed `CreateFileMappingW`, mapped with memmap2 0.9.12.
- **Reason.** Full-rate frames must cross a process boundary (E-040); local sockets would add a copy and backpressure on the
  CEF paint thread. iceoryx2's API has been version-checked only; it is read in source in M1 before anything depends on it.

## Consequences

- **Gate.** S8 (M1); R22.
