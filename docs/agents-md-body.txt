# noPlayout

**Last Updated**: 2026-11-27
**Status**: Active (M0, workspace skeleton)
**Primary OS**: Both (Windows 11 x64, macOS arm64 + x86_64, Linux x86_64); macOS arm64 first (D-017)

## Overview
A free, cross-platform OGraf v1 broadcast graphics playout engine in Rust: CEF offscreen rendering,
NDI output, web outputs, Tauri v2 desktop app. Apache-2.0, public repo during development (D-003).

## Current State
See `STATUS.md` (milestone, what works, deliberate limits, pending screenshots, CI/billing).

## Quick Reference
| Item | Value |
|------|-------|
| Spec of record | `PLAN.md` + `DECISIONS.md` (E-nnn); product decisions `docs/01-decisions.md` (D-nnn); `IDEA.md` |
| Toolchain | Rust 1.100.0 (`rust-toolchain.toml`), edition 2024; Node 24+ for `web/` |
| CEF | 156.7.0+156.0.31 (cef crate) / CEF 156.0.31; exported to `target/cef` (`cargo xtask cef`) |
| Engine API | `127.0.0.1:8686`, `GET /api/v1/health` |
| Run | `cargo xtask run` (app) · `cargo xtask self-test` (headless smoke) |
| Local gates | `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo test --workspace` · `cargo deny check` · `cargo xtask licences` · in `web/`: `npm run check`, `npm test` |

## File Map
| Need to know... | See |
|-----------------|-----|
| What it is for | `IDEA.md` |
| How it is built, milestones, CI | `PLAN.md` (§2 architecture, §3 pins, §7 milestones, §8 CI) |
| Why we made X decision | `docs/adr/README.md` (ADR index; ADR NNNN = DECISIONS.md E-NNN) |
| Product decisions from Caleb | `docs/01-decisions.md` |
| Current state, limits, pending items | `STATUS.md` |
| Crates and layout | `README.md` "Layout" |
| Test fixtures (never reformat) | `fixtures/README.md`, `fixtures/oes/README.md` |
| Licences and notices | `LICENSE`, `NOTICE`, `deny.toml` |

## Working rules
- Follow `PLAN.md` and `DECISIONS.md` exactly; no later-milestone work in a milestone's PR. Leave a
  `TODO(<milestone>)` pointing at `STATUS.md` for each deliberate limit.
- A design choice adds an ADR (`docs/adr/NNNN-title.md` + index row) in the same PR. Never rewrite an
  accepted ADR; supersede it.
- One feature branch per card; never commit to `main`; PRs say what/why/how-tested. Merges, tags and
  releases are Caleb's.
- Dependency pins live in the workspace `Cargo.toml` (PLAN §3); bumps are deliberate PRs.
- Never commit the NDI SDK, NDI Tools, or any NDI docs (D-003, D-004); `/NDI SDK*/` is gitignored.
- No secrets, IP addresses, hostnames, venue names or local paths in tracked files.
- `fixtures/` holds byte-identical copies and generated vectors: never reformat or re-encode them.
- Process boundaries (E-040): the renderer links `noplayout-proto` + `cef` only (never the engine core
  or NDI); the output process never links CEF.
- Nothing copyleft in `noplayout-renderer`'s tree (E-032): `cargo xtask licences`.
- UI/UX (layout, flows, on-screen words) comes from Usopp's approved mockups; engineers build to them.
- Timing-sensitive tests: headless macOS shells coalesce timers; don't read a sub-80 ms miss there as
  a code failure without a native rerun.

## Recent Activity
- 2026-11-27: M0 skeleton: workspace, engine/renderer/output/app, xtask, CI, fixtures, ADR backfill.
