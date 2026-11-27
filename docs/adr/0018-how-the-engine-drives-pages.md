# 0018. Pages are driven over a WebSocket host protocol, not CEF process messages

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-018 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-018 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** CEF process messages / `execute_java_script` from the renderer; a WebSocket from each page to the engine.

## Decision

- **Choice.** WebSocket host protocol (`/ws/host/<ch>`), the same for CEF and browser pages.
- **Reason.** Web outputs need it anyway, so CEF pages use the same path and the renderer stays free of OGraf logic. The
  renderer can be restarted without the engine changing how it talks to pages.

## Consequences

- **Cost.** One loopback WebSocket hop per action; negligible against animation times.
