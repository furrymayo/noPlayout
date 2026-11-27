# noPlayout status

**Milestone:** M0, workspace skeleton (PLAN.md §7). Plan approved 2026-11-26 (D-016); macOS arm64
first (D-017).

## M0 "Done when"

| Item | State |
|---|---|
| Four CI lanes green on the M0 commit | see the M0 PR's checks (lanes: linux, windows, macos-arm64, macos-x64, plus cargo-deny) |
| macOS arm64 desktop: `cargo xtask run` shows "Engine: running" + the renderer's CEF version | run verified on a real Mac (health API read back `Renderer: CEF 156.0.31 OK`); **screenshot pending Caleb**: a headless agent session has no Screen Recording permission |
| Windows 11 x64 desktop screenshot | **pending, before first Windows deployment** (D-017); steps below |
| Linux x86_64 desktop screenshot | **pending, before first Linux deployment** (D-017); steps below |
| macOS x86_64 | proven by its CI lane running `cargo xtask self-test --release` headlessly |

### Desktop check (any OS)

```sh
git clone https://github.com/furrymayo/noPlayout && cd noPlayout
cargo xtask cef     # once: fetch the pinned CEF into target/cef (~300 MB)
cargo xtask run     # opens the noPlayout window
```

Expected window text: **Engine: running · Renderer: CEF 156.0.31 OK**. Take a screenshot of the
window and attach it to the M0 PR. Close the window to stop the engine.
Linux needs the Tauri v2 packages listed in README.md; Windows needs the MSVC build tools.

## What works (M0)

- `noplayout-engine`: single-instance lock on the data directory, rolling daily logs (14 kept),
  `GET /api/v1/health` on 127.0.0.1:8686, token-gated `POST /api/v1/internal/shutdown`, a child
  supervisor for one output process and one renderer, `--self-test`, clean stop on Ctrl+C/SIGTERM.
- `noplayout-renderer`: CEF 156.0.31 offscreen browser on `about:blank`, `Hello{cef_version}` over
  the local socket, Ping/Pong with a paint count, clean exit on `Shutdown`, exits when the engine
  goes away. macOS dev bundle via `cargo xtask bundle` (spike S5: works from the workspace).
- `noplayout-output`: `Hello{epoch}`, Ping/Pong, exits on `Shutdown` or when the engine goes away.
- `noplayout-app`: Tauri v2 window that attaches to a running engine or starts one, and stops it on
  exit only if it started it.
- `xtask`: `cef`, `web`, `bundle`, `self-test`, `run`, `preflight`, `licences`.
- Fixtures: Basketball OGraf package (byte-identical, D-014) and the OES corpus (D-013).

## M0 limits (deliberate; each has a `TODO(<milestone>)` in the code)

- Renderer message pump runs at a fixed cadence; the scheduled pump tied to output-clock drive modes
  is M1.
- CEF sandbox is on for macOS only; Windows (bootstrap executable) and Linux (SUID helper) are M6
  packaging work.
- Renderer: no host protocol pages, paint ring, drive modes or standby rate yet (M1).
- Output: no channel clock or ring reader (M1), no NDI (M2).
- Supervisor: 1 s heartbeat and a single launch; the full supervisor (100 ms heartbeat, failure
  kinds, restart, orphan handling) is M6 (PLAN §2.13).
- App: fixed port 8686, placeholder UI; settings, Output Setup and the real UI (Usopp's mockups) are
  M4.
- Preflight: only the CEF self-test runs; conformance, pixel goldens and S1 timing are M1, NDI
  loopback M2.
- Windows CI runs the CEF self-test but doesn't gate on it until S4/R13 (PLAN §8.1).

## Skeleton crates

`noplayout-frames` (M1, spike S8), `noplayout-pixels` (M1/M2), `noplayout-oracle` (M1),
`noplayout-ndi` (M2), `noplayout-datasource` + `noplayout-capture` (M4.5),
`noplayout-decoder-oes` (M4.5; must pass `fixtures/oes/` byte for byte).

## CI and billing

Public repo, GitHub-hosted runners (PLAN §8.1). **Actions runs:** the first push (2026-11-27) ran
every job with no billing refusal, so the account's private-repo payment problem does not reach
this public repo. The final lane results are on the M0 PR's checks.

What the first CI rounds found (fixed in M0):
- Windows file locks are mandatory, so the lock holder's pid/port now lives in `engine.holder`
  beside `engine.lock`.
- Windows checks text out as CRLF; `.gitattributes` makes text LF everywhere (fixtures excluded).
- cef-dll-sys 156.7.0 copies the CEF runtime into `<profile>/build/` instead of next to the
  binaries with this Cargo's build-dir layout; `cargo xtask bundle` now places it on Linux and
  Windows itself (and the Linux renderer carries `RUNPATH $ORIGIN`). Worth reporting upstream.
