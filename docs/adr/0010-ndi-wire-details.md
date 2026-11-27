# 0010. NDI wire format BGRA/BGRX, explicit timecode, SDK clocking off

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-010 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-010 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** BGRA (Fill + Alpha) or BGRX (FILL, KEY), never UYVA; `clock_video(false)`; `FrameRate::new(num, den)`;
  `Timecode::from_ticks(tc(k))` with the same value on FILL and KEY, never `SYNTHESIZE`; per-frame metadata
  `<noplayout k=… paint=…/>`; `submit_video` async with a ping-pong buffer pair per sender.
- **Reason.** UYVA loses alpha in OBS DistroAV (Robin). BGRA skips our own colour conversion. Explicit timecode lets
  receivers pair FILL and KEY and makes the loopback test exact; NDI's timecode synthesizer had drift fixed only in 6.4.

## Consequences

- **Cost.** BGRA is twice the bandwidth of UYVY on the wire; acceptable on a production LAN at ≤ 4 channels of 1080p.
