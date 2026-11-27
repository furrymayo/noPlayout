# 0057. PaintStall detected by a forced-repaint probe, not by rAF

- **Status:** accepted (DECISIONS.md: Proposed (rev 3), spike-gated (S7a))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-057 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-057 (561e4a6), PLAN.md

## Context

- **Context.** Review 2 N6: "no paint while rAF advances" cannot tell a static scene from a stalled compositor; a static
  scene has no damage and no paint, and rAF says nothing about pixels.

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** After 100 ms without a paint the renderer calls `BrowserHost::invalidate(PaintElementType::default())` and
  records the probe in the ring header; the output process raises `PaintStall` if no paint newer than the probe arrives
  within 250 ms (≤ 400 ms from the last paint). rAF is diagnostics only. Loop guard on repeat stalls after promotion.
  Cost: one full-view repaint per 100 ms on a fully static scene (≈ 83 MB/s at 1080p).
- **Reason.** A healthy page answers an invalidate whatever its content, a stalled one cannot, and the judge is the reader of
  the ring, not the renderer. `invalidate` and `was_resized` exist in the cef 156.7.0 bindings for every target built;
  behaviour under external begin-frame is the S7a check (R25).

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
