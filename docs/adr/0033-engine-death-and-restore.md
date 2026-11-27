# 0033. Engine death: renderers and output processes exit; supervised restart restores, cold start asks

- **Status:** accepted (DECISIONS.md: Proposed (rev 2 wording))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-033 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-033 (561e4a6), PLAN.md, ADR 0044

## Amendments recorded in DECISIONS.md

> **Rev 2.** Output processes exit 3 s after their control socket closes, like renderers (holding the last frame meanwhile).
> A supervised engine restart cold-restores every channel from the persisted journal (E-044). An app crash with the engine
> still running reattaches and replays nothing. Gate unchanged (≤ 5 s, R17).

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Renderers exit 3 s after their control socket closes (no orphan CEF). The app restarts the engine; a
  supervised restart restores on-air state without asking. A cold start (machine or app relaunch) asks "Restore them?"
  (Enter = Restore), as IDEA §5.1 says.
- **Reason.** After a crash the operator wants air back; after a deliberate relaunch, the state may be stale.

## Consequences

- **Gate.** M6: engine restart → first restored NDI frame ≤ 5 s.
