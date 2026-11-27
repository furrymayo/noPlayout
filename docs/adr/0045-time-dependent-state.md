# 0045. Time-dependent state: declared checkpoints, flags, no silent loss

- **Status:** accepted (DECISIONS.md: Limitation decided (Caleb, D-015 / Q2); scope stated by E-052, checkpoint protocol by E-056 (rev 3))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-045 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-045 (561e4a6), PLAN.md, ADR 0052, ADR 0056, docs/01-decisions.md D-015

## Amendments recorded in DECISIONS.md

> **Rev 3.** Caleb accepted the limitation (D-015, Q2 option a): a third-party graphic's self-run state is exact on a
> failover only while the standby's copy is live, and it is flagged everywhere else. Review 2 N1 showed the limitation is
> not confined to double failures; E-052 states its real scope. "Every Designed graphic restores exactly" now rests on the
> checkpoint protocol in E-056 (consistent cut, timed tail), not on the rev 2 sketch below.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Warm promotion keeps internal clocks (the standby ran them). On the cold path, a third-party graphic's internally
  run clock is **not restored** and the instance is flagged on the On-air strip and in Health ("elapsed time not restored").
  Graphics may declare `v_noplayout: {recovery: "checkpoint"}` (OGraf v1 allows `^v_` vendor keys) and implement
  checkpoint/restore custom actions; every M5 Designed graphic does, so noPlayout's own graphics restore exactly on every
  path. S9 tries DevTools virtual time for third-party clocks; adopted only if it returns to real time cleanly.
- **Reason.** OGraf v1 has no get/set-state or seek; review 1's probe lost 1,280.5 ms of a `startClock` clock on replay. Data-
  driven clocks (OES → `gameClock`) are durable input and restore exactly.

## Consequences

- **Open.** Q2 — decided by Caleb (D-015): option (a), accept + flag + S9, scope per E-052.
