# 0041. Warm standby renderer per channel, live-mirrored

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-015 / Q1): on every channel; the 2 s guarantee is for a single failure with an exact standby. Eligibility amended by E-052 (rev 3))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-041 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-041 (561e4a6), PLAN.md, ADR 0052, docs/01-decisions.md D-015

## Amendments recorded in DECISIONS.md

> **Rev 3.** Caleb decided Q1 as recommended (D-015): the standby stays on every channel, and ≤ 2 s same-state recovery is
> guaranteed for a single failure **when the standby is exact** (E-052). A standby rebuilt after a failover is still
> promoted (it always beats a cold restore), but it is exact only for what E-052 lists; the rest is flagged.

## Context

- **Context.** F1 + F2: a restart from scratch can't reach 2 s for a hung page or asset-heavy graphics, and replay cannot
  rebuild a third-party graphic's internal clock.

## Options considered

- **Options.** (a) Cold restart only. (b) A pre-started *empty* spare renderer (saves CEF start, still loads and replays).
  (c) A **warm standby**: a second renderer that runs the same show live (receives every action, renders at 1/6 rate,
  hosts the preview browser), promoted on failure.

## Decision

- **Choice.** (c), on every channel by default. After promotion a new standby is cold-restored behind the new active.
- **Reason.** Promotion needs no load and no replay: the standby executed the same actions, so `addPoints` effects and
  running clocks are already there (within one frame of delivery skew). Estimated outage 0.15 s for a crash, 0.65 s for a
  hang (PLAN §2.13.5). (b) only removes the CEF start from the cold path.

## Consequences

- **Cost.** A second CEF per channel: est. 150–400 MB RAM and 15–30 % more CPU per channel (S7/M6 measure; R20). Non-
  deterministic graphics can diverge (R23, divergence check). A graphic bug that hangs both copies needs the cold path
  (K3b, R21).
- **Open.** Q1 — decided by Caleb (D-015): option (a), standby on every channel; cold path best effort.
