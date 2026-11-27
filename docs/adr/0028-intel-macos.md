# 0028. Intel macOS: tested until GitHub retires the runner, then build-only

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-010))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-028 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-028 (561e4a6), PLAN.md, docs/01-decisions.md D-010

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

_Not recorded separately in DECISIONS.md; see Decision._

## Consequences

- **Choice (D-010).** Built and tested on GitHub's `macos-15-intel` runner until it's retired (about August 2027), then
  build-only (cross-compiled on Apple Silicon, smoke-tested under Rosetta), labelled "community-tested".
