# 0032. cargo-deny licence allowlist; nothing copyleft in the renderer

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-032 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-032 (561e4a6), PLAN.md, docs/01-decisions.md D-003

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** `cargo deny check` on every push with an allowlist (Apache-2.0, MIT, BSD-2/3-Clause, ISC, 0BSD, Zlib,
  Unicode-3.0, BSL-1.0, MPL-2.0, IJG, plus whatever M0's first run shows is needed and reviewed). No GPL/AGPL/LGPL crates
  anywhere; nothing copyleft at all in `noplayout-renderer`, which loads libndi. Advisories checked nightly.
- **Reason.** Keeps the NDI licence conditions and "private later" (D-003) clean without manual audits.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
