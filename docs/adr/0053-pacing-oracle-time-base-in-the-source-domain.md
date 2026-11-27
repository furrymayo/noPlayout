# 0053. Pacing oracle time base in the source domain, plus a lag band

- **Status:** accepted (DECISIONS.md: Proposed (rev 3), replaces E-048's `m`; model count corrected (rev 4, review 3 N9))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-053 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-053 (561e4a6), PLAN.md, ADR 0048

## Context

- **Context.** Review 2 N2: rev 2's `m` was computed in output periods, so a correct 60 → 59.94 conversion fails its row.

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** The fixture draws its rAF timestamp `s` (µs). The decoder checks the median source period, a per-step check
  `|Δs − Δn·P̂_src| ≤ P̂_src/4`, and a lag band `max − min of (tc − s) ≤ 1.25·P̂_src` (mode A: ≤ `P_out`/4). S1 may widen
  tolerances to measured jitter, never past a 1.5·`P̂_src` band.
- **Reason.** An exact-rational model (`~/OrgWork/projects/noplayout/design/oracle_model.py`) gives the ideal conversion
  10 skips, 0 step errors and a 16.67 ms band at every phase, against 20 failures for rev 2's rule, and both mutations
  (frozen time base, repeated paint) fail. PLAN §5.5.

## Consequences

- **Rev 4 amendment (review 3 N9).** Rev 3's model counted a transition twice when it both broke the step check and had
  `Δs ≤ 0`, and used the nominal period instead of the specified median. Fixed: one count per violating transition, median
  `P̂_src`, `Δs ≤ 0` reported as a separate diagnostic. The frozen-time-base mutation now shows 10 step errors (9 with
  `Δs ≤ 0`), not 19; its 165.8 ms band and every other result are unchanged, and no threshold moved.
