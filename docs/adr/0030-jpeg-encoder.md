# 0030. Pure-Rust JPEG encoder

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-030 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-030 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** `turbojpeg` (C libjpeg-turbo, fastest), `jpeg-encoder` 0.7.1 (pure Rust, SIMD feature), `image`.

## Decision

- **Choice.** `jpeg-encoder` with `simd`.
- **Reason.** No C toolchain or CMake on any lane, and 480×270 frames are small enough that speed isn't the limit. Its IJG
  licence term needs the "based in part on the work of the Independent JPEG Group" line in NOTICE.

## Consequences

- **Would change if.** S3 shows encoding as a meaningful share of renderer CPU.
