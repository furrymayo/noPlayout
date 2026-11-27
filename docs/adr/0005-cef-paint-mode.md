# 0005. CEF paint mode: CPU `on_paint`, shared texture off in v1

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-005 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-005 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** (a) CPU `on_paint`: CEF hands a premultiplied BGRA buffer, we copy it. (b) `accelerated_osr`: shared GPU
  texture via `on_accelerated_paint`, then GPU readback or GPU conversion.

## Decision

- **Choice.** (a) in v1; (b) stays off (the `cef` crate feature `accelerated_osr` is not enabled).
- **Reason.** NDI needs CPU memory anyway, so (b) only pays if conversion also moves to the GPU, which means per-OS
  D3D11/Metal/Vulkan code. (a) is one code path on every OS and costs one 8.3 MB copy per 1080p frame, inside budget on paper.
  Robin flagged per-OS CEF OSR issues (#4166 incomplete frames on macOS x86_64, #4357), which S1 checks for (a) first.

## Consequences

- **Would change if.** S2 or the M6 soak shows copy + convert over 8 ms p99 per 1080p frame, or 2160p can't hold rate (R2,
  R3). Then (b) becomes a v1.x ADR.
