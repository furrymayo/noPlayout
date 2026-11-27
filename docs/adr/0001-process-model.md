# 0001. Three process kinds: app, headless engine, one renderer per channel

- **Status:** accepted (DECISIONS.md: Amended by E-040/E-041 (rev 2): four kinds)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-001 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-001 (561e4a6), PLAN.md, ADR 0033, ADR 0040, ADR 0041

## Amendments recorded in DECISIONS.md

> **Rev 2, amended by E-040 and E-041.** Four process kinds: app, headless engine, and per channel one **output process**
> (clock, conversion, NDI) plus **two renderers** (active and warm standby). The reasons below for (c) still hold; rev 2 adds
> that output must survive a renderer failure.

## Context

- **Context.** IDEA needs a desktop app, a per-channel offscreen Chromium and an HTTP API that Companion can use, and IDEA §6
  wants a renderer crash to recover in about 2 s.

## Options considered

- **Options.** (a) One process: Tauri app hosts the engine and CEF. (b) App + one engine that also hosts all CEF browsers.
  (c) App + headless engine + one renderer process per channel.

## Decision

- **Choice.** (c).
- **Reason.** CEF and the Tauri webview can't share a process cleanly: CEF wants to own process startup (`execute_process`)
  and its message loop, and Tauri wants the main thread. A renderer crash or hang must never take the control plane or the
  other channels with it. A headless engine also runs on a rack PC with no window, controlled from Companion, and lets CI test
  the core without Tauri.

## Consequences

- **Cost.** Three binaries to package and an IPC layer. Engine death is a single point of failure (R17, E-033).
- **Would change if.** Tauri ships a stable CEF runtime (then a and c merge for the UI side only; channels stay separate).
