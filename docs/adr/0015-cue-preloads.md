# 0015. Cue preloads the graphic into a hidden slot

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-015 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-015 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Selecting or cueing an item loads it in the preview browser and also preloads it into a hidden cue slot in the
  program page (`load` done, not visible). TAKE flips it visible and calls `playAction`.
- **Reason.** XPression operators expect TAKE to be instant after CUE. OGraf `load` may fetch assets and take time; doing it
  at cue time removes that from the take. Data edits made after the cue are sent with `updateAction(skipAnimation)` before
  play.

## Consequences

- **Cost.** One extra loaded instance per channel. A graphic that starts animating in `load` (non-conformant) would be hidden
  but running; the conformance harness flags that.
