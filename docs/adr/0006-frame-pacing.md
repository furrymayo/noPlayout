# 0006. Frame pacing: integer per-channel clock; drive mode chosen per OS by S1

- **Status:** accepted (DECISIONS.md: Spike-gated; clock moved to the output process (E-040), pass/fail replaced by E-048)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-006 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-006 (561e4a6), PLAN.md, ADR 0010, ADR 0040, ADR 0048, ADR 0049

## Amendments recorded in DECISIONS.md

> **Rev 2.** The clock moves to the output process (E-040); renderers compute the same deadlines from the epoch it publishes.
> The paint↔slot tagging is replaced by E-049, and the pass/fail line below is replaced by E-048's oracle thresholds
> (review 1 F4: "beyond the counted `repeated`" let counted defects pass). S1 is split into S1a (M1, paint level,
> provisional) and S1b (M2, receiver, final) (F10). The options and the mode A/B choice stand.

## Context

- **Context.** Output must run at exactly 60000/1001 (or 50, 30000/1001, 25) with no drift, and FILL and KEY must be one
  frame apart never. CEF #4311: external begin-frames carry `Now()` and a fixed 60 Hz interval and are dropped if one is
  already pending.

## Options considered

- **Options.** (a) Engine-wide clock driving all channels. (b) One clock per channel in the renderer, CEF driven by
  `send_external_begin_frame` (mode A). (c) One clock per channel, CEF free-running at `windowless_frame_rate`, the sender
  sampling the newest paint at each deadline (mode B, the CasparCG model). (d) NDI SDK clocking (`clock_video = true`).

## Decision

- **Choice.** One integer clock per channel in the renderer (`T_k = epoch + floor(k·den·1e9/num)`, absolute-deadline sleeps
  plus a short spin), with mode A where S1 shows it clean and mode B elsewhere, chosen per OS and recorded in an ADR. SDK
  clocking off; explicit timecode on every frame (E-010).
- **Reason.** The clock lives where the pixels are, so no frame-rate data crosses a process boundary. Integer deadlines never
  drift. A gives one paint per output frame when it works; B always works but can supersede or repeat frames and needs
  counters. SDK clocking would give a second, competing pacing authority and blocks the send thread.
- **Pass/fail.** S1: 10 minutes per OS at 59.94 and at 50, measured at a real NDI receiver with a burned-in counter: no holes
  or doubles beyond the counted `repeated`, `fill_key_tc_mismatch = 0`.

## Consequences

- **Would change if.** CEF lands `SendExternalBeginFrameWithTiming` (#4311): then A everywhere after a re-run.
