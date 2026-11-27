# 0011. NDI runtime detected and linked, never bundled

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-010))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-011 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-011 (561e4a6), PLAN.md, docs/01-decisions.md D-010

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Reason.** NDI's licence terms for redistribution need an NDI-approved flow (EULA flow-down, branding) that is simpler and
  safer as "the user installs NDI's runtime". It also keeps a public repo and its CI caches free of NDI binaries.

## Consequences

- **Choice (D-010).** Detect the NDI 6 runtime at startup and on "Check again"; when it's missing, show the OS-specific
  install link from NDI and keep web outputs running. Never ship NDI binaries, NDI Tools or SDK docs; the MIT-licensed headers
  aren't needed either, since grafton-ndi loads the library itself.
