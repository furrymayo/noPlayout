# 0047. Server timestamps on the host protocol

- **Status:** accepted (DECISIONS.md: Proposed (rev 2))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-047 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-047 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Every `Action` and journal entry carries `ts {utcUs, mono {run, us}, slot? {k, epoch}}`: UTC from one engine
  time source that never steps backwards within a run, engine monotonic time, and the channel's output slot (ties to NDI
  `tc(k)`). Informational, never used to schedule. `TimeSync` every 5 s; pages report `lagMs` (PLAN §4.3).
- **Reason.** F11: IDEA §4.2 requires server-timestamped actions. They also feed declared checkpoints, the stale-action rule at
  handoff, diagnostics and the elapsed-time flags.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
