# 0043. Failure kinds, detectors and the recovery measurement

- **Status:** accepted (DECISIONS.md: Proposed (rev 2); K2g detector amended by E-057, K5 by E-055 (rev 3))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-043 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-043 (561e4a6), PLAN.md, ADR 0055, ADR 0057

## Amendments recorded in DECISIONS.md

> **Rev 3.** K2g's detector is the forced-repaint probe (E-057), not "no paint while rAF advances". K5 separates a slow
> submit from a non-returning one; the non-returning case recycles the output process (E-055).

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** Nine failure rows K1–K7 (renderer exit, CEF child exit, GPU stall, page hang, poison action, UI-thread hang,
  sender stall, output-process failure, engine death), each with its own detector and budget (PLAN §2.13.1). Control
  heartbeat 100 ms, replacement at 500 ms of silence, inside IDEA's 500 ms / 2 s bounds. Outage = t1 − t0, t0 the harness's
  monotonic clock just before the fault is injected, t1 the arrival at an independent NDI receiver of the first frame whose
  pixels decode to the expected state and stay correct for 2 s. Per-stage budget with target and fail lines (§2.13.5).
- **Reason.** F1: "restart after 2 s of no Pong" plus any restore work exceeds 2 s by construction; separating kinds lets a
  process exit be handled in ≤ 20 ms and a hang in ≤ 500 ms. Failing over early is safe because promotion keeps all state.

## Consequences

- **Gate.** S7a (M1, paint level) with the Basketball on-air set on Windows, macOS arm64 and Linux: a miss of the warm-path
  fail line revisits the architecture before M2. M6: 20/20 K1 runs ≤ 2 s per reference machine.
