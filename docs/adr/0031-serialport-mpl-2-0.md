# 0031. Accept `serialport` (MPL-2.0)

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-031 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-031 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Use `tokio-serial` 5.5.0 → `serialport` 4.10.1 unmodified.
- **Reason.** MPL-2.0 is file-level copyleft; using it unmodified in an Apache-2.0 product only obliges us to keep its source
  available, which crates.io does. It is in the engine, not the renderer that loads libndi. Linux CI installs `libudev-dev`.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
