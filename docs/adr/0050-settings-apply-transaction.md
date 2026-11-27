# 0050. Settings apply transaction with one owner and an effective revision

- **Status:** accepted (DECISIONS.md: Proposed (rev 2); commit point and post-apply failures defined by E-054 (rev 3))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-050 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-050 (561e4a6), PLAN.md, ADR 0054

## Amendments recorded in DECISIONS.md

> **Rev 3.** E-054 adds the commit point (the `settings.json` rename) and the outcome of every failure after resources
> were applied, including a failed rollback.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** One `settings` actor owns the document and an `effective_rev`; every writer uses `If-Match` (409 on stale).
  Apply = validate (422, nothing changes) → plan → write `settings.pending.json` → apply each step with an undo → commit
  (atomic rename to `settings.json`, then `last-good`), or roll back every applied step, move pending to `rejected/` and
  report the failed step (500/503). Last-good = last successfully applied. A crash mid-apply: startup moves pending to
  `rejected/` ("interrupted apply") and continues from the committed `settings.json`.
  Future schema versions are refused and never overwritten (PLAN §2.11).
- **Reason.** F5: validation alone doesn't make an apply all-or-nothing (a port or NDI name can fail after other changes
  took effect), and two writers could lose an update silently.

## Consequences

- **Gate.** The §2.11 fault matrix in M4.
