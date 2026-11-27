# 0020. Settings: JSON, atomic write, last-good, validation in Rust

- **Status:** accepted (DECISIONS.md: Amended by E-050 (apply transaction))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-020 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-020 (561e4a6), PLAN.md, ADR 0050

## Amendments recorded in DECISIONS.md

> **Rev 2, amended by E-050.** "All-or-nothing" was only true for validation; applying a valid document can still fail
> halfway (review 1 F5). E-050 adds the apply transaction, the effective revision and the startup rules. "Last-good" now
> means last *applied*, not last loaded.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** JSON with `schemaVersion`, validated in Rust (serde + `validate()`), atomic temp-write/fsync/rename, a
  last-good copy rewritten only after a clean load, migrations that back up first, `PUT /settings` all-or-nothing.
- **Reason.** Hand-editable, diffable, and IDEA §6's "never half-applied" rule. A truncated or bad file loads last-good and
  reports exactly what was rejected (Usopp's banner).

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
