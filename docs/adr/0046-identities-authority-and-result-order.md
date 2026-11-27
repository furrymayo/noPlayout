# 0046. Identities, authority binding and ordered result commit

- **Status:** accepted (DECISIONS.md: Proposed (rev 2))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-046 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-046 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** `engineRun` (UUIDv7), renderer incarnation `inc` (u64, increasing per channel), a 256-bit launch token per
  incarnation (env var, handed to the program page by `execute_java_script`), `(instanceId, generation)` per occupancy,
  `seq` per action. `program` registration requires loopback + current `inc` + matching token; superseded incarnations are
  refused (4410) and killed. Results commit per instance in `seq` order; results for a gone generation can't change state;
  replay results never commit. Promotion records a `seq` barrier. Snapshot-to-live handoff is atomic in the channel actor
  (PLAN §2.14).
- **Reason.** F6: with two replicas, reconnects and overlapping OGraf calls, "the program page's results are authoritative"
  is ambiguous without identities; a stale or foreign page must never be able to change on-air state.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
