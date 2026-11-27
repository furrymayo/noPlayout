# 0025. Edit-mode preview rendered by a CEF renderer

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-010))
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-025 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-025 (561e4a6), PLAN.md, ADR 0013, docs/01-decisions.md D-010

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Reason.** What the operator designs is what Chromium renders to air. The Tauri webview is WebKit on macOS and Linux,
  which can lay out text and filters differently.

## Consequences

- **Choice (D-010).** The editor canvas shows frames from a CEF renderer at editor resolution; Svelte draws handles over them.
- **Cost.** One extra renderer process while editing; frames reach the UI through the monitor path (E-013) at a higher rate
  for interaction.
