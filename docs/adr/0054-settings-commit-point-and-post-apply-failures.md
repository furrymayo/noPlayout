# 0054. Settings: one commit point, defined outcome for every persistence and rollback failure

- **Status:** accepted (DECISIONS.md: Proposed (rev 3))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-054 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-054 (561e4a6), PLAN.md

## Context

- **Context.** Review 2 N3: rev 2 did not say what happens when persistence fails after resources were applied.

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** The `settings.json` rename is the commit point; pending stores the candidate's sha256; old resources are held
  until commit. Before the commit point every failure undoes all resources and reports `503`; after it the change stands
  and failures (directory fsync, last-good, pending delete) come back as `200` with warnings and Health amber. A failed undo
  leaves a reported **degraded** state, with the running resource set tracked separately from the committed document.
  Startup tells a committed leftover (hash match) from an interrupted apply. PLAN §2.11 failure table.
- **Reason.** Every outcome is one row the tests can assert: API view, running resources, files and next-start result.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
