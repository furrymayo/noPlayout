# 0037. RossTalk-compatible listener in v1, off by default

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-010))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-037 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-037 (561e4a6), PLAN.md, docs/01-decisions.md D-010

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Reason.** Existing Carbonite CG-device presets and Companion's XPression module can drive noPlayout unchanged.

## Consequences

- **Choice (D-010).** In v1 (M4), off by default, TCP 7788, CR/LF lines, XPression's command set mapped onto Take IDs,
  channels (framebuffer 0 = channel 1 on the wire) and layers; unknown commands logged, never fatal; same access scope as HTTP.
