# 0061. Provisional drive mode per OS (S1a)

- **Status:** draft (provisional; final in M2 after S1b, PLAN §5.6)
- **Date:** 2026-11-27
- **Deciders:** franky (proposed)
- **Related:** ADR 0006, ADR 0005, PLAN.md §5.3, §5.5, §5.6, `docs/spikes/S1a-paint-pacing.md`,
  D-019, D-020

## Context

PLAN §5.6: S1a runs the real output clock and selector on the real paint ring at paint level,
A and B, per OS, at 59.94 and 50, and records a provisional mode per OS. S1b (M2, through NDI to
a receiver) makes it final. Timing gates bind on macOS arm64 on the M2 Mac mini (D-020);
Windows and Linux timing is deferred to post-air (D-019).

## Decision (provisional)

| OS | Provisional mode | Evidence |
|---|---|---|
| macOS arm64 | **A** | Mac Studio, 1 min per row, not reference hardware: no row passes. A misses by a few held slots per minute; B at 59.94 has a selector defect at the rate crossing (S1a report, finding 4). |
| Windows 11 | pending (D-019) | none |
| Linux | pending (D-019) | none |

The engine default stays A (`NOPLAYOUT_DRIVE_MODE` overrides it for tests). Nothing is called
receiver-proven or zero-drop.

## Open before this is final

1. The M2 mini's 10-minute rows (kit `m1-timing.command`).
2. The pacer's once-a-minute late wake (S1a finding 2): time-constraint thread policy on macOS if
   it persists on the M2.
3. Mode B's crossing flap at 59.94 (S1a finding 4): fix the selector's hysteresis before S1b.

## Consequences

M2 builds the NDI path for both modes as planned; the mode stays switchable in Advanced
diagnostics. If the M2 rows pass only in B at 59.94/29.97, those formats are marked *partial*
for the zero-drop claim (D-015).
