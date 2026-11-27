# 0051. Fallback completion status; required formats and platforms stay required

- **Status:** accepted (DECISIONS.md: Proposed (rev 2); Q4 decided (Caleb, D-015): all formats stay required)
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-051 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-051 (561e4a6), PLAN.md, docs/01-decisions.md D-015

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Every fallback in the risk register has a status: in scope, partial (milestone stays open), stop (Franky reports
  to Caleb with numbers) or deferral (Caleb's written approval only, recorded as a D-nnn), with an evidence owner (PLAN §6.1).
  All eight IDEA formats and all three OSes stay required; a CI lane that can't run CEF needs a `preflight` evidence file at
  every milestone commit; reports list passed, skipped and unexercised checks separately (§6.2).
- **Reason.** F9: rev 1's fallbacks ("label it experimental", "local-only") could close a milestone with a requirement unmet.

## Consequences

- **Open.** Q4 — decided by Caleb (D-015): all formats stay required.

---
