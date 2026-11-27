# 0052. Recovery fidelity per replica copy; promotion eligibility; what the 2 s same-state guarantee covers

- **Status:** accepted (DECISIONS.md: Proposed (rev 3), within Caleb's D-015; time declaration replaced by E-058, fidelity `replayed-timed` added by E-059 (rev 4))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-052 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-052 (561e4a6), PLAN.md, ADR 0058, ADR 0059, docs/01-decisions.md D-015

## Context

- **Context.** Review 2 N1: rev 2 treated an alive, `Synced` standby as holding the same state. After the first failover
  the replacement standby has *replayed* the instances already on air, so a third-party graphic's self-run clock in it is
  short by the clock's age at rebuild. The same holds for web pages that join or reconnect mid-show.

## Options considered

- **Options.** (a) Track fidelity per page and instance, declare time exposure, flag with a bound, and state exactly when
  the same-state guarantee holds. (b) A third replica started with the first two (keeps a live copy through two failures;
  +1 browser per channel, and still fails at the third, so it moves the limit rather than removing it). (c) Refuse or
  warn on import for undeclared graphics (blocks valid OGraf graphics).

## Decision

- **Choice.** (a), within Caleb's D-015 (Q1/Q2). Fidelity `live` / `checkpointed` / `replayed`, recorded by the engine
  from how it fed each page. Time exposure from `v_noplayout.time` (`"none"`, `{"selfTimedAfter": {"actions", "dataTrue"}}`)
  or `recovery: "checkpoint"`, in the manifest or a per-package `noplayout.recovery.json` sidecar (hashed into the graphic
  version); undeclared graphics are self-timed from `load`. Shortfall bound `L = t_rebuild − t_expose`, fixed at rebuild.
  Standby states Exact / Ready-inexact / Not ready, shown before failure. Exactness returns only by a new generation
  (re-take) or a checkpoint; never by the divergence check. An inexact standby is still promoted. PLAN §2.13.3a.
- **Reason.** It makes the approved limitation visible and bounded, keeps every valid OGraf graphic usable, and puts the
  operator's remedy (re-take in a break, or feed the clock from data) in front of them before the second failure.

## Consequences

- **Gate.** S7a kill → rebuild → second-kill cases; M3 web late-join clock test; R24.
- **Rev 4 amendment (review 3 N7).** The `selfTimedAfter` declaration form is withdrawn and replaced by E-058's timer list;
  exposure, `L` and flags become per timer. A fourth fidelity, `replayed-timed`, is added by E-059. The rest stands.
