# 0063. Paint ring transport: our own ring over libc `mmap` / `CreateFileMappingW` (S8)

- **Status:** accepted
- **Date:** 2026-11-27
- **Deciders:** franky (architect decision, frontier review t_a3bac44c of PR #2)
- **Supersedes in part:** ADR 0042 (its transport clause only)
- **Related:** ADR 0042, ADR 0012, DECISIONS.md E-042, PLAN.md §5.3, R22, `docs/spikes/S8-paint-ring.md`

## Context

ADR 0042 (E-042) fixed the ring protocol (one 4-slot ring per renderer, CAS-changed slot
headers, a writer that never blocks, a reader that holds at most one slot) and left the transport
to spike S8: iceoryx2 0.10.1 if it met three criteria (zero extra copies, a never-blocking
writer, no leaked shared memory after `kill -9` on all three OSes), otherwise our own ring over
`shm_open` / pagefile-backed `CreateFileMappingW`, mapped with memmap2 0.9.12.

S8 built the own ring and showed it meets all three criteria; `kill_9_writer_leaves_no_leak`
runs in CI on all four lanes. iceoryx2 was not run.

## Options considered

1. **Evaluate iceoryx2 now, then choose.** Costs a spike for a transport nothing downstream
   depends on: the §5.3 protocol is the same either way.
2. **Adopt the own ring without the iceoryx2 comparison.** The own ring is R22's mitigation and
   is already proven against the S8 criteria.

## Decision

Option 2. The paint ring transport is our own ring (`crates/noplayout-frames`): POSIX
`shm_open` plus libc `mmap` on macOS and Linux, pagefile-backed `CreateFileMappingW` /
`MapViewOfFile` via `windows-sys` on Windows. memmap2 is not used (it cannot map a Windows
pagefile-backed section by name, and libc `mmap` on the `shm_open` descriptor needs nothing more).
iceoryx2 is **not evaluated**; this decision says the own ring is sufficient, not that iceoryx2
fails.

The `iceoryx2` and `memmap2` workspace pins are removed.

Everything else in ADR 0042 stands: the ring protocol, its reason and its gate (S8, R22).

## Consequences

- No iceoryx2 dependency tree to licence-audit or maintain, and no reliance on its service and
  cleanup model, which has never been read in source.
- Leak cleanup is ours: the engine's ring registry sweeps every recorded name at incarnation
  retirement and at start (Unix); Windows sections die with their last handle.
- Revisiting iceoryx2 later needs a new ADR and the same three criteria.
