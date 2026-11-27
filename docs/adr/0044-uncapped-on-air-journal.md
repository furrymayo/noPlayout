# 0044. Uncapped on-air journal; no compaction for third-party graphics

- **Status:** accepted (DECISIONS.md: Proposed (rev 2))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-044 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-044 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Every action dispatched to the authoritative page is journalled per instance with `seq`, `ts`, params and
  committed result; never capped, never silently truncated, persisted to `shows/<id>.onair.jsonl`. Failed/timed-out actions
  are journalled (the graphic saw them). No compaction for third-party graphics. Health warns above 50,000 entries with the
  estimated replay time; nothing is dropped. A poison action is quarantined and named on the On-air strip.
- **Reason.** F2: compaction or a cap is only safe where equivalence is established, and the engine can't establish it for
  a graphic it doesn't understand. At < 1 kB per entry and ≈ 1 ms replay per action (est., S7 measures), a whole game's
  scorebug journal stays well inside memory and a few seconds of replay.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
