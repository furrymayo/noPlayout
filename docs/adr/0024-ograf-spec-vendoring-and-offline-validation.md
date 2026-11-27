# 0024. OGraf spec vendored at a commit; manifest validation never uses the network

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-024 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-024 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Vendor `ebu/ograf` `v1/` (schemas, examples) at commit `945f8bb` with its MIT licence; validate manifests with
  `jsonschema` 0.63.2 with **default features off** (they include `resolve-http`) and only `resolve-file`.
- **Reason.** Upstream has no tags, so a commit is the only stable pin. A playout machine is often offline, and validation
  must never stall on, or leak to, the network.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
