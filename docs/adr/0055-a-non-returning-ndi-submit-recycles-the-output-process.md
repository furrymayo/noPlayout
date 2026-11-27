# 0055. A non-returning NDI submit recycles the output process; slow submits keep the sender

- **Status:** accepted (DECISIONS.md: Proposed (rev 3))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-055 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-055 (561e4a6), PLAN.md

## Context

- **Context.** Review 2 N4: "recreate the sender at 2 s" had no path; the sender thread owns the `Sender` inside
  `submit_video`, and a thread blocked in a foreign call can't be cancelled.

## Options considered

- **Options.** (a) Quarantine the stuck thread and start a second same-name sender in-process. (b) Recycle the output
  process. (c) Wait forever.

## Decision

- **Choice.** (b) for stuck (≥ 2 s); nothing recreated for slow (250 ms – 2 s). Loop guard with backoff; channel red after
  3 recycles in 5 min, retries continue. (a) is tried once in S7b for information only.
- **Reason.** Process exit is the only termination the OS guarantees and frees the sender, frames and SDK state at once;
  the renderers are untouched. Cost stated: both planes drop, *est.* 1–3 s receiver reconnect on top of the ≥ 2 s, new
  timecode epoch. (a) depends on undocumented SDK behaviour and leaks the wedged state for the process lifetime.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
