# 0036. Repo licence Apache-2.0

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-010))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-036 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-036 (561e4a6), PLAN.md, ADR 0031, docs/01-decisions.md D-010

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options considered.** Apache-2.0, MIT, dual MIT/Apache-2.0. Anything loading libndi must not be GPL.

## Decision

- **Reason.** Explicit patent grant suits a video product; NOTICE holds the NDI trademark line ("NDI® is a registered
  trademark of Vizrt NDI AB"), the IJG credit, CEF/Chromium and OGraf notices. Compatible with every dependency in §3,
  including MPL-2.0 (E-031).

## Consequences

- **Choice (D-010).** Apache-2.0, with a NOTICE file.
