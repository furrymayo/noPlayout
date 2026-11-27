# 0013. Monitor frames: downscaled JPEG, credit flow control

- **Status:** accepted (DECISIONS.md: Spike-gated (S3); program monitor now made in the output process)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-013 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-013 (561e4a6), PLAN.md

## Amendments recorded in DECISIONS.md

> **Rev 2.** The program monitor is made in the output process from what actually goes to air; the preview monitor in the
> standby renderer. Method, rates, credits and the S3 gate are unchanged (S3 now also counts the standby's cost).

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** The renderer downscales to 480×270, composites onto a checkerboard, JPEG-encodes (quality 80) at about 15 fps,
  and sends with credit-based flow control (2 credits, newest wins). The engine relays to the UI over a binary WebSocket, one
  frame in flight per client.
- **Reason.** About 0.3 MB/s per monitor instead of 120 MB/s raw; the UI decodes JPEG natively. Credits stop a slow UI from
  backing up the renderer.

## Consequences

- **Gate.** S3 (4 channels × program + preview). Fails if monitor work costs a renderer more than 10 % of one core or adds
  any `slots_skipped`; then lower the rate or size.
