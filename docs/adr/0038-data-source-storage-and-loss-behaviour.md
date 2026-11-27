# 0038. Data-source connections per computer, mappings per show; hold on loss

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-009))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-038 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-038 (561e4a6), PLAN.md, docs/01-decisions.md D-009

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Reason.** A show moves between machines whose serial ports and addresses differ; mappings are show content. Holding is
  predictable and needs no clock model per decoder.

## Consequences

- **Choice (D-009).** The show stores which sources it expects (key, name, decoder, layout) and every field mapping; the
  connection (port, host, interface) is stored per computer in that machine's settings. When a source goes quiet or is lost,
  every mapped field, including the game clock, holds its last value and the light turns amber "source lost"; no local
  free-running clock.
