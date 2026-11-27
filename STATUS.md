# noPlayout status

**Milestone:** M0, workspace skeleton (PLAN.md §7). Plan approved 2026-11-26 (D-016); macOS arm64
first (D-017).

## M1 progress (not complete)

Task 1 implements the engine library API in `noplayout_core::library::Library`:
`open`, `import_folder`, `import_zip`, and `read_asset`. The engine opens its data directory's
`library/` storage and serves validated snapshots at `/library/<full-sha256>/<asset-path>`.
`PackageVersion::manifest_url()` and `main_url()` return package-relative URLs. This is a Rust
library import API, not a new upload endpoint or UI; M1 task 3 can call it from the dev CLI.

- Vendored EBU schemas, nine examples and Server API YAML at
  `945f8bbcbcba262377544871dd4af62e54de5177`, with original licence and provenance.
- Offline schema validation, real-time requirement, contained main module and local schema
  references; immutable staged folder/ZIP imports. One graphic per import; import individual
  folders from multi-graphic collections. ADR 0060 documents path policy, limits and hashing.
- Real HTTP tests cover manifests, module dependencies, asset MIME types, HEAD, missing files,
  traversal and staging rejection. Import tests validate all nine upstream examples and six
  Basketball graphics. These are import/serving checks, not CEF runtime conformance evidence.

Task 4, part 1 (`noplayout-frames`, spike S8): own 4-slot paint ring over `shm_open`/`mmap`
and pagefile-backed `CreateFileMappingW`, the §5.3 protocol with generation-stamped slot
states, a never-blocking writer, a reader that ignores a dead writer's WRITING slot and resets
READING on respawn, and a name registry the engine sweeps (POSIX objects outlive `kill -9`).
S8 report: `docs/spikes/S8-paint-ring.md` (provisional Mac Studio numbers; iceoryx2 was not
exercised, flagged there for the frontier review).

Still pending: M1 tasks 4 (renderer OSR/ring writer/drive modes) –10 (output clock, oracle,
spikes, supervisor/recovery, CEF conformance and pixel tests). M1 stays open. Windows/Linux
functional CI evidence is pending the parent PR; timing gates remain pending per D-019. No
binding timing or memory measurements on the reference M2 mini are claimed here.

Licence gate: the pinned `jsonschema` dependency brings `borrow-or-share` (MIT-0); `deny.toml`
allows MIT-0 (permissive, no attribution clause). The upstream examples keep their bundled
dependency notices (including GSAP's standard-licence header); EBU's MIT notice does not
override those notices.

Task 2 (`web/host`): `HostRuntime` per §2.5/§4.3: load gating (O5), ordered non-awaited starts
(O8), 10 s/5 s timeouts, generation-isolated results with `seq`, `currentStep`/`stepCount: -1`
semantics, cue slots, 100 ms `Beat{n, rafCount, lagMs, lastAppliedSeq}`, `TimeSync` clock
estimate, snapshot/journal replay with original `ts`, quiescent `v_noplayout` checkpoints.
Built to `dist/host/{renderer.html,ograf-host.js}`; vitest runs it against misbehaving fake
graphics (never-resolving load, throws, out-of-order resolution). Not yet in it: the test-build
frame-oracle overlay (task 6, `TODO(M1)`), fill/key presentation (M3).

Task 3 (`noplayout_core::hostbus`, `time`, `ctl`): one actor per channel owns the sequenced
state. Every `Action` carries `ts{utcUs, mono}` from one engine `TimeSource` (non-stepping UTC);
the journal (§2.13.2) is uncapped, appended to `shows/dev.onair.jsonl` (flush per line, fsync
debounced 200 ms; reader tolerates a torn last line). Registration (§2.14): program pages need
loopback + live incarnation + constant-time token match + single use (else 4403), superseded
incarnations get 4410 and a kill event; mirror roles register freely (access scope is §2.12,
M3). Snapshot at `S` and subscription from `S + 1` happen in one actor step; results commit
in seq order per instance, standby results are held and taken at promotion (barrier `B`),
mirror disagreements count as `mirror_divergence`, stale generations are dropped. `TimeSync`
every 5 s; quiescent checkpoint results trim the snapshot journal to the tail. The engine serves
`/ws/host/<channel>` and `/host/*` (built `web/host`). Dev CLI: `NOPLAYOUT_UI_TOKEN=… noplayout-engine
ctl import|load|play|update|stop|custom|dispose|kill-renderer|hang|inspect` over the loopback
token route. Gaps: `ts.slot` is unset until the output clock exists (task 5, `TODO(M1)`);
`hang` reaches the renderer but the page busy-loop lands with task 4 (`TODO(M1)`); the
dev show path stands in for M4's show files (`TODO(M4)`).

Deliberate task-1 boundaries: engine-owned storage must not be edited externally; no global
concurrent-request memory cap, package GC, crash-left staging cleanup or power-loss durability
claim. No import UI or upload API in this slice. See ADR 0060 for the filesystem threat boundary.

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

`noplayout-pixels` (M1/M2), `noplayout-oracle` (M1),
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
