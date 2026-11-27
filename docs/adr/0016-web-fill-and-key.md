# 0016. Web fill/key pages made from the same scene with SVG colour-matrix filters

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-016 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-016 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** Separate fill/key graphics; canvas readback in the page; SVG/CSS filters on the layer root; WebGL pass.

## Decision

- **Choice.** SVG `feColorMatrix` filters on the layer root (`color-interpolation-filters="sRGB"`): fill keeps RGB with A=1
  (unshaped) or composites over black (shaped); key outputs R=G=B=A with A=1.
- **Reason.** Same scene, same timing, no change to graphics (IDEA §4.2), and it works in OBS's and vMix's browsers.

## Consequences

- **Gate.** M3 pixel test: `/key` from CEF vs the NDI KEY within 2 LSB for a ≥ 16 (R8). Fallback: WebGL pass.
