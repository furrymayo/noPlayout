# 0004. UI front end: Svelte 5 + TypeScript

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-004 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-004 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** Svelte 5, React 19, Solid, plain TypeScript.

## Decision

- **Choice.** Svelte 5.59.2 + TypeScript 6.0.3 + Vite 8.4.0, pinned in `package-lock.json`.
- **Reason.** Small runtime, compiled reactivity that suits a UI that updates lights and counters many times a second, and
  good Tauri templates. TypeScript 6 because svelte-check 4.7.8's peer range is `^5 || ^6` (TS 7.0.2 is out but unsupported).

## Consequences

- **Would change if.** Usopp's build spec needs a component library that exists only for React; not the case in UI.md rev 2.
