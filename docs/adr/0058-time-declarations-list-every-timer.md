# 0058. Time declarations list every timer with its entry points; exposure and flags per timer; CI declaration audit

- **Status:** accepted (DECISIONS.md: Proposed (rev 4))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-058 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-058 (561e4a6), PLAN.md

## Context

- **Context.** Review 3 N7: rev 3's `selfTimedAfter` named one set of triggers per graphic. The Basketball scorebug's
  `showMessage` with `autoHideSeconds > 0` arms a second timer that hides the message with no engine action, so a rebuilt
  copy of a data-fed scorebug that showed a timed message was classed Exact while it showed the expired message again (the
  reviewer reproduced it in Chromium with the unmodified graphic). A single flag per instance also could not say "the
  clock is exact, the message is not".

## Options considered

- **Options.** (a) Declare every timer with its entry points and track exposure per timer. (b) Keep one trigger list per
  graphic and add `showMessage` to it (fixes this case, but a data-fed scorebug then reads as an inexact clock). (c) Treat
  any graphic that has any timer as exposed from `load` (simple, but marks every data-fed scorebug inexact for life).

## Decision

- **Choice.** (a). `v_noplayout.time` is `"none"` or `{timers: [{id, kind: clock|timer, label, startedBy: {actions,
  dataTrue}}]}`; an `actions` entry may carry `payloadGt`, where a missing or non-numeric field counts as arming. A timer is
  exposed from its first matching journal entry for the instance's life, with no healing assumed: proving that a one-shot
  timer has expired in both copies would need the graphic's internal time, which is what we cannot see. Only `clock`
  timers carry the `L` bound; other timers read "may differ". Undeclared graphics get one implicit `self` timer from
  `load`. A CI audit (`noplayout.recovery.audit.json`) maps every timer-pattern match in a declared package's sources to a
  timer or a reason, and fails on drift. The recovery-probe fixture is split into a really time-free `probe-static` and a
  `probe-timed`; a false `time: none` on `probe-timed` is a negative control. PLAN §2.13.3a, §2.13.6.
- **Reason.** Each displayed claim matches one mechanism in the graphic, so the operator is told exactly what may be wrong
  and the data-fed clock keeps its exact claim. The audit makes a stale sidecar a CI failure instead of a wrong Exact.

## Consequences

- **Gate.** §2.13.6 kill → rebuild cases 5–8 and the probe declaration cases in S7a (M1); R24.
