# 0019. Crash recovery by replaying each instance's action log with `skipAnimation`

- **Status:** superseded by 0041, 0043, 0044, 0045 (DECISIONS.md: **Superseded by E-041/E-043/E-044/E-045**)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-019 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-019 (561e4a6), PLAN.md, ADR 0041, ADR 0043, ADR 0044, ADR 0045

## Amendments recorded in DECISIONS.md

> **Superseded by E-041, E-043, E-044 and E-045 (rev 2).** Review 1 F2 showed the 256-entry cap with a `goto` fallback loses
> real state: the reviewer ran the unmodified Basketball scorebug in Chromium with `homeScore: 10` and 257
> `addPoints(home, 1)` calls (score 267); emulating the capped fallback (`load(currentData)` + `goto: 0`) gives 10, because
> `addPoints` changes the graphic's own data and `customAction` returns only a status. Replaying `startClock` after a short
> wait lost 1,280.5 ms of elapsed clock time even without a cap. Merging updates is not provably equivalent for a
> third-party graphic either. Kept for the record.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Each instance keeps an ordered action log (cap 256). A page that reconnects gets `load(dataAtLoad)` and the log
  replayed with `skipAnimation: true`, consecutive updates merged. Past the cap: `load(currentData)` +
  `playAction({goto: step, skipAnimation: true})` and a Health note.
- **Reason.** OGraf has no "get state / set state", so replay is the only spec-conformant way to rebuild a graphic's look,
  including custom actions. `skipAnimation` makes it land on the final state immediately.

## Consequences

- **Gate.** M6: 20/20 crash tests restore the same graphics and steps within 2 s at an NDI receiver.
