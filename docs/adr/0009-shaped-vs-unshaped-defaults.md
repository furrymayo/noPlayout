# 0009. Shaped vs unshaped defaults, per output mode

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-010), bench-confirm in M2)
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-009 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-009 (561e4a6), PLAN.md, docs/01-decisions.md D-010

## Context

- **Context.** CEF paints premultiplied BGRA. NDI documents BGRA as not premultiplied. Keyers differ on separate fill/key.

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Reason.** Fill + Alpha follows NDI's own convention, which receivers expect. For separate sources, a shaped fill is the
  traditional linear-key default (CasparCG's fill is shaped) and needs no division, so it can't amplify noise in
  low-alpha edges.

## Consequences

- **Choice (D-010).** Fill + Alpha: **unshaped** (straight) BGRA, un-premultiplied from CEF; Shaped available under Advanced
  for receivers set to premultiplied. Separate Fill & Key: **shaped** FILL (CEF's premultiplied RGB, i.e. composited over
  black) plus a KEY of R=G=B=A; Unshaped one click away.
- **Gate.** M2 bench test on Caleb's switcher with the alpha ramp and antialiased text at its normal CG key setting. If
  Unshaped looks right there, the Separate default flips before M2 closes, recorded in this entry's ADR.
