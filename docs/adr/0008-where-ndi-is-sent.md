# 0008. NDI sending happens inside the renderer process

- **Status:** superseded by 0040 (DECISIONS.md: **Superseded by E-040**)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-008 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-008 (561e4a6), PLAN.md, ADR 0013, ADR 0040

## Amendments recorded in DECISIONS.md

> **Superseded by E-040 (rev 2).** The reason below ("a renderer crash takes down that channel's NDI anyway, so (b) buys no
> resilience") stopped holding once recovery has to be ≤ 2 s with no output gap (review 1 F1): with NDI in a separate output
> process, the sources stay up and hold the last frame while a standby renderer takes over. Kept for the record.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** (a) In the renderer, next to the pixels. (b) In the engine, with frames moved over shared memory (iceoryx2 or
  memmap2).

## Decision

- **Choice.** (a).
- **Reason.** 1080p59.94 BGRA is about 497 MB/s per channel. Keeping it in-process avoids a shared-memory protocol, a copy
  and a second failure domain. A renderer crash takes down that channel's NDI anyway (it has no frames to send), so (b) buys
  no resilience. Only small JPEG monitor frames leave the renderer (E-013).

## Consequences

- **Would change if.** A future output needs frames from several channels at once (a multiviewer), or DeckLink output wants a
  separate process.
