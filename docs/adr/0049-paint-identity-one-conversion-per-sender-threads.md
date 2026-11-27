# 0049. Paint identity, single conversion, per-sender threads

- **Status:** accepted (DECISIONS.md: Proposed (rev 2); stuck-sender handling replaced by E-055 (rev 3))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-049 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-049 (561e4a6), PLAN.md, ADR 0055

## Amendments recorded in DECISIONS.md

> **Rev 3.** "Recreate the sender at 2 s" is withdrawn: the sender thread owns the `Sender` while it is inside
> `submit_video`, and nothing can take it back. E-055 replaces it.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Paints are named `(inc, paint_seq)`, slots `k`; nothing infers which begin-frame made a paint. The pacer converts
  the chosen paint once into an immutable FILL/KEY (or Fill+Alpha) snapshot with one `tc(k)`; each NDI sender has its own
  thread, three frames and a one-deep newest-wins queue; the pacer never calls the SDK. Progress monitor: `SenderStall` at
  250 ms, recreate at 2 s, output-process restart on a repeat within 10 s (K5).
- **Reason.** F7: rev 1 tagged paints by assumed begin-frame and let a blocking `submit_video` (possible even with clocking off,
  grafton-ndi 3.0.0 docs) delay the clock and the other plane.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
