# 0029. Toolchain pinned to Rust 1.100.0, edition 2024

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-029 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-029 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** `rust-toolchain.toml` pins 1.100.0 (stable, 2026-11-09), edition 2024. MSRV floor from dependencies: sysinfo
  0.40.4 needs 1.95, tauri 2.12.7 needs 1.90.
- **Reason.** Reproducible builds across four CI lanes and workers' machines; bumps are deliberate PRs.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
