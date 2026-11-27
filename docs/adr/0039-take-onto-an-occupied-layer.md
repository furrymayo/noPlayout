# 0039. TAKE onto an occupied layer replaces it with a cut

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-039 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-039 (561e4a6), PLAN.md, docs/01-decisions.md D-007

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** Animate the old graphic out first, or cut it.

## Decision

- **Choice.** `dispose` the current instance (cut) and play the new one; OUT is the only animated exit; CLEAR is always a cut
  (D-007).
- **Reason.** Matches XPression's take-replaces-layer behaviour operators know, and keeps TAKE's timing independent of the
  outgoing graphic's out animation. An operator who wants the out animation presses OUT first.

## Consequences

- **Would change if.** Usopp or Caleb wants "animate out, then in"; it would be a per-item option, not a new default.

---
