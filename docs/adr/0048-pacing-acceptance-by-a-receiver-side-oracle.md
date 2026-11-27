# 0048. Pacing acceptance by a pixel-only receiver-side oracle

- **Status:** accepted (DECISIONS.md: Proposed (rev 2); time-base check replaced by E-053 (rev 3); Q3 decided (Caleb, D-015))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-048 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-048 (561e4a6), PLAN.md, ADR 0053, docs/01-decisions.md D-015

## Amendments recorded in DECISIONS.md

> **Rev 3.** The time-base index `m` (computed in output-period units) is withdrawn: review 2 N2 showed an ideal
> 60 → 59.94 conversion gives 20 `Δm ≠ 1` transitions per 10,000 frames against 10 skips. E-053 replaces it with a
> source-domain step check and a lag band. Everything else here stands.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** A coded fixture draws its rAF counter `n`, a time-base index `m`, and an alpha-only copy of `n`; a separate
  decoder reads pixels only (never noPlayout's counters) at an NDI receiver (and on paint dumps in M1). Numeric thresholds per
  mode and rate (PLAN §5.5): mode A and mode B at 50/25 allow zero repeats, skips, tears, key mismatches and timecode errors;
  mode B at 59.94/29.97 allows exactly the declared 1000:1001 conversion skips (Δn = 2, ≥ 900 frames apart) and is never
  reported as zero-drop. Output counters must agree with the oracle. Eight injected faults must each fail the run.
- **Reason.** F4: rev 1's "no holes beyond the counted `repeated`" let a counted defect pass and trusted the sender's own tags.

## Consequences

- **Open.** Q3 — decided by Caleb (D-015): option (b), per-OS partial for the soak claim, decided after S1b.
