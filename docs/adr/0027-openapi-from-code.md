# 0027. OpenAPI generated from code with utoipa

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-027 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-027 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** utoipa 6.0.0 + utoipa-axum 0.3.0, served at `/api/docs` with utoipa-scalar 0.4.0.
- **Reason.** The docs can't drift from the handlers. For `/ograf/v1`, a test checks our routes against the vendored
  `server-api.yaml` instead.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
