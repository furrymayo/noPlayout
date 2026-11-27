# 0002. Cargo workspace split; renderer does not link the engine core

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-002 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-002 (561e4a6), PLAN.md, docs/01-decisions.md D-001

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options rejected.** A single crate with features (CEF rebuilds on every change); separate repos (version skew between
  engine and renderer protocol).

## Decision

- **Choice.** One Cargo workspace: `noplayout-core` (lib + engine bin), `noplayout-renderer`, `noplayout-app`,
  `noplayout-proto`, `noplayout-ndi`, `noplayout-pixels`, `noplayout-datasource`, one `noplayout-decoder-<id>` crate per decoder,
  `xtask`. `noplayout-renderer` depends only on `proto`, `ndi`, `pixels` and `cef`.
- **Reason.** CEF's build is heavy; keeping it out of the core keeps `cargo test -p noplayout-core` fast and CI cheap. The
  boundary also stops show logic from creeping into the renderer. D-001 named three crates; the extra libraries are internal.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
