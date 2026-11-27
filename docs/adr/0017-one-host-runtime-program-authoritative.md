# 0017. One OGraf host runtime for every page; the CEF program page is authoritative

- **Status:** accepted (DECISIONS.md: Amended by E-046 (authority bound to incarnation + token))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-017 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-017 (561e4a6), PLAN.md, ADR 0046

## Amendments recorded in DECISIONS.md

> **Rev 2, amended by E-046.** "Program" is now the program page of whichever renderer the engine made active. Authority is
> bound to a loopback connection, the renderer incarnation and its launch token, never to the role string; results commit
> in `seq` order per instance (PLAN §2.14).

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Every page (CEF program and preview, web outputs, editor) runs the same `ograf-host.js` with a role. Only the
  CEF program page's results answer API calls and drive engine state; other pages' results are logged and compared.
- **Reason.** One implementation of OGraf semantics (load-before-actions, ordering, `currentStep`, `stepCount: -1`) means web
  and NDI can't disagree by design. A single authority avoids races between several pages answering one call.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
