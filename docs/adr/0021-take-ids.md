# 0021. Take IDs: u16, unique per show, enforced in the engine

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-005, D-010))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-021 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-021 (561e4a6), PLAN.md, ADR 0037, docs/01-decisions.md D-005, docs/01-decisions.md D-010

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Reason.** Companion and RossTalk callers bypass the UI, so the rule must live where the data lives.

## Consequences

- **Choice (D-005, D-010).** `u16` 1–9999 per rundown item, unique within a show, auto-incremented, inline error on
  duplicate. Uniqueness is enforced in the engine (API answers 409 naming the clashing item), not only in the UI. API routes
  `/api/v1/takeid/{n}/cue|take|next-step|out`; RossTalk `CUE` takes a Take ID (E-037).
