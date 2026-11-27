# 0022. Decoder plug-ins: compiled in, one crate per decoder

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-022 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-022 (561e4a6), PLAN.md, docs/01-decisions.md D-011

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** (a) Compiled-in crates behind a trait and a static registry. (b) Dynamic libraries loaded at runtime. (c) WASM
  plug-ins. (d) A scripting language (Lua/JS) for decoders.

## Decision

- **Choice.** (a): one crate per decoder implementing `trait Decoder` (framing included), registered by one line and one cargo
  feature, with a shared conformance test that requires captures and a golden timeline for every registered decoder.
- **Reason.** D-011 wants each new decoder to be a self-contained change; (a) makes it exactly that (new crate + one registry
  line + one feature line + docs), checked in M4.5 by looking at the OES PR's diff. Rust has no stable ABI, so (b) is fragile
  across compiler versions; (c) and (d) add a runtime and a sandbox for a handful of decoders that ship with the product.
  Decoders run in `catch_unwind` so a bad one can't take the engine down.

## Consequences

- **Would change if.** Third parties need to ship decoders without rebuilding noPlayout; then (c).
