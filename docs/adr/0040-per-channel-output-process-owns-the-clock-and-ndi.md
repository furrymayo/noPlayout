# 0040. Per-channel output process owns the clock and the NDI senders

- **Status:** accepted (DECISIONS.md: Proposed (rev 2), gated by S7a/S8)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-040 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-040 (561e4a6), PLAN.md, ADR 0008, ADR 0042

## Context

- **Context.** Review 1 F1: with NDI inside the renderer (E-008), a renderer crash or hang takes the NDI sources down, and a
  2 s restart budget can't cover detection + CEF start + page load + replay + receiver reacquire. IDEA §9's crash test needs
  output back in the same state within 2 s.

## Options considered

- **Options.** (a) Keep E-008 and make cold restarts faster. (b) Move NDI into the engine, frames over shared memory.
  (c) A small per-channel **output process**: clock, paint selection, one conversion, NDI senders, program monitor.

## Decision

- **Choice.** (c). Renderers write paints into a shared-memory ring (E-042); the output process reads the active renderer's
  ring at each slot and holds the last frame when none is fresh (PLAN §2.4a, §5).
- **Reason.** NDI sources stay up through any renderer failure, so receivers never reacquire and the outage is only the
  time to the first fresh paint. (b) would put per-frame real-time work and the NDI SDK into the control plane, so a stuck
  `submit_video` (F7) could stall every channel and the API. (a) cannot meet 2 s for a hang (detection alone eats most of
  it) and stays above 2 s for asset-heavy packages (est. 0.6–2.5 s, PLAN §2.13.5).

## Consequences

- **Cost.** One more process per channel and a shared-memory protocol; ≈ 497 MB/s per 1080p59.94 channel crosses a process
  boundary (one copy into the ring, which CEF's `on_paint` needs anyway). The output process holds no CEF and no graphics,
  so it is small; its own failure is K6.
- **Gate.** S8 (M1) for the ring; S7a (M1) for the warm path at paint level; S7b (M2) at a receiver.
- **Would change if.** S8 finds no ring that avoids blocking the writer on all three OSes; then the architecture is revisited
  before M2 (PLAN §2.13.5 gate).
