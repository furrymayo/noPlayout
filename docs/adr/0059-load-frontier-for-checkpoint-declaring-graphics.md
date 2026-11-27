# 0059. Load frontier: exact timestamped full replay before the first checkpoint and as the last fallback (`replayed-timed`)

- **Status:** accepted (DECISIONS.md: Proposed (rev 4))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-059 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-059 (561e4a6), PLAN.md

## Context

- **Context.** Review 3 N8: rev 3 did not say how a checkpoint-declaring instance is restored before its first checkpoint,
  and its fallback labels contradicted each other.

## Options considered

- **Options.** (a) Exact timestamped full replay from the load (the load frontier) until the first checkpoint, and as the
  last fallback. (b) Take an initial checkpoint right after `load` (an extra round trip on every take, and it can still
  fail, which needs (a) anyway).

## Decision

- **Choice.** (a). Restore = `load` at its original `loadTs` → every journal entry with original `ts`, `replay: true`,
  `skipAnimation: true` → live. Under the Designed time model this is step 2 with the cut at the load, so the copy is exact
  by the same declaration; fidelity `replayed-timed`, no flag. The whole journal of a checkpoint-declaring instance is kept
  for its life. Invalid or failing checkpoints are discarded, the instance is re-loaded in that page, and the restore tries
  older checkpoints, then the load frontier.
- **Reason.** No new failure mode and no extra work on the take path; the outcome is the same exactness claim the
  checkpoint path already makes, tested deterministically.

## Consequences

- **Gate.** M5 load-frontier edge tests 1–3 (before the first checkpoint incl. web join and K7; invalid-checkpoint
  fallback; out-of-order pair during a checkpoint request with exactly-once assertion). PLAN §2.13.4 step 4, §2.13.6.
