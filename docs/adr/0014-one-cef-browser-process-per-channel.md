# 0014. One CEF browser process per channel

- **Status:** accepted (DECISIONS.md: Amended by E-041: two per channel (active + standby))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-014 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-014 (561e4a6), PLAN.md, ADR 0041

## Amendments recorded in DECISIONS.md

> **Rev 2, amended by E-041.** Two renderer processes per channel (active + standby), each with its own `root_cache_path`
> (`cef-cache/<channel>-<slot>/`). Isolation between channels is unchanged.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** One renderer process hosting all channels' browsers, or one per channel.

## Decision

- **Choice.** One per channel (Robin D2), each with its own `root_cache_path`.
- **Reason.** Isolation: a hung GPU process or crashed browser affects one channel. Separate clocks without cross-channel
  scheduling. CEF's process-singleton lock rules out sharing a cache directory anyway.

## Consequences

- **Cost.** More memory per channel (an estimate of 150–300 MB, not yet measured; M6 measures it, R14).
