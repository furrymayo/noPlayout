# 0056. Declared checkpoint protocol: quiescent cut, frontier seq, timed tail inside restore

- **Status:** accepted (DECISIONS.md: Proposed (rev 3); bad-checkpoint fallback replaced by E-059 (rev 4))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-056 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-056 (561e4a6), PLAN.md, ADR 0059

## Context

- **Context.** Review 2 N5: rev 2 promised exact Designed clocks without a consistent cut or a time-faithful tail.

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** The host takes a checkpoint only when the instance is quiescent (all its dispatched actions resolved), never
  delaying an on-air action; the graphic captures state synchronously; the result carries `frontierSeq` and `cutTs`.
  Restore = `load` → `v_noplayout_restore{state, cutTs, nowTs}` → tail `seq > frontierSeq` with original `ts` and
  `replay: true` → live. Checkpointing graphics compute time state from action `ts` and `serverNow()`, not
  `performance.now()` deltas; every Designed graphic does so through noPlayout's runtime library, so its replay is
  time-faithful by construction. A bad checkpoint falls back to the full journal (copy becomes `replayed`, flagged).
- **Reason.** A consistent cut plus a time model that does not depend on when an action is executed is what makes "exact"
  provable; M5's property test and the second-kill test check it. PLAN §2.13.4.

## Consequences

- **Rev 4 amendment (review 3 N8).** "A bad checkpoint falls back to the full journal (copy becomes `replayed`, flagged)"
  is withdrawn: it contradicted "a checkpoint-declaring graphic is never plain replayed". A bad checkpoint is discarded
  (`checkpoint_discarded` in Health) and the restore falls back to an older checkpoint or the load frontier (E-059). The cut
  rule is made explicit for out-of-order resolution: no cut while any action of the instance is unresolved.
