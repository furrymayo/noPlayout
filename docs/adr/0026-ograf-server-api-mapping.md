# 0026. OGraf Server API mapping: renderer = channel, RenderTarget = `{layer}`

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-026 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-026 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Renderer = channel; RenderTarget = `{ "layer": n }`; Server API calls act on playout directly, outside the
  rundown, and show in the On-air strip as "API: <graphic>". Spec bug O13 (`manifest` vs `metadata`) answered with both keys.
- **Reason.** Matches the spec's shapes at the pinned commit (RenderTarget must be a shallow object) and keeps third-party
  OGraf controllers working without knowing about rundowns.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
