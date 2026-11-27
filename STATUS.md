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

Task 4, part 2 (renderer): `Configure` over the control socket (proto v2) creates the ring
and a transparent OSR program browser on `/host/renderer.html?channel=…&role=program`;
`on_load_end` injects `{inc, token}` by script (never in the URL), so the page registers as
the authoritative incarnation; `on_paint` copies CEF's BGRA buffer straight into the ring
with `{inc, paint_seq, painted_at}` on the machine-wide monotonic clock
(`noplayout_frames::clock`, also the output epoch now). Mode A runs an elevated begin-frame
thread at `T_k − lead` (every slot active, every 6th standby); mode B sets
`windowless_frame_rate` to `ceil(fps)`/10. Paint probe (E-057) invalidates after 100 ms
without a paint and stamps the ring header. The message loop follows
`on_schedule_message_pump_work`. Audio muted; console messages and
`on_render_process_terminated` go to the engine as events; `ctl hang` busy-loops the page.
Self-test now checks a 1920×1080 paint in the ring; `scripts/dev-smoke-renderer.sh A|B`
drives load/play through a real CEF page (both modes pass on the Mac Studio). Gaps: the
standby slot and promotion are task 8 (`Role` exists, nothing sends it yet); the preview
browser and monitor JPEGs are M3 (`TODO(M1)` in the renderer); M1 uses the external pump on
every OS (`TODO(M2)`); the engine falls back to its own clock when no output process is up.

Task 5 (output process, no NDI): `Hello{epoch}` on the shared monotonic clock; an elevated
pacer thread waits for each `T_k` (precise OS timer to −0.5 ms, then spin), skips a slot it
wakes for after `T_k + period/2` (`slots_skipped`, never bursts), takes the newest READY
paint of the active ring with `painted_at ≤ T_k − 1 ms` (mode B: 900-slot hysteresis after a
crossing skip lets a paint up to 1 ms later through), holds the previous paint when none is
newer (`held`), converts once into FILL (straight BGRA) + KEY (8-bit alpha), stamps
`tc(k) = U0 + ⌊k·den·10⁷/num⌋` and offers it to one elevated sender thread per plane through
a one-deep newest-wins lane (`submit_blocked`, `pair_partial` counted exactly when a slot
reaches one plane only). `DumpSink` (`--dump-paints <dir> --frames N [--dump-crop x,y,w,h]`,
or the `NOPLAYOUT_DUMP_*` variables) writes `<plane>/<k>.raw` plus `<plane>.jsonl` with `k`,
`tc`, the paint and `held`. 10 Hz monitor: `SenderStall` (250 ms) / `SenderStuck` (2 s);
pacer: `PaintStall` when a paint probe is 250 ms unanswered (K2g); 1 Hz `OutputStats` to the
engine. The engine sends `Activate{ring, inc, mode}` after `Configure`; the output retries
the open for 2 s while the renderer creates the ring. `fault-inject` feature
(`NOPLAYOUT_FAULT_INJECT=<kind>@<slot>[:<ms>]`): repeat, skip, backwards, key-prev, tear,
tc-bump, block-fill/key (injections 1–6 and 8; 7 is the fixture's). Unit tests drive the
real pacer against an in-process ring. Gaps: the engine only logs `PaintStall`/`SenderStuck`
until the supervisor (task 8) acts on them; monitor tap and divergence threads are task 8
/ M3 (`TODO(M1)`). On the Mac Studio from a gateway shell the pacer wakes up to 64–81 ms
late and skips most slots: that is the known degraded timer, not reference evidence; timing
runs go through the Terminal `.command` path (task 7) and bind on the M2 mini (D-020).
S2 conversion cost (Mac Studio, provisional): 1080p p50 1.5 ms / p95 2.1 ms, 2160p p50
3.9 ms / p95 4.2 ms (`cargo test --release -p noplayout-output -- --ignored s2_convert`).

Task 6 (oracle + fixtures): `noplayout-oracle` decodes patches A/B, the alpha patch and the
ramp from FILL/KEY planes, whole or cropped (`layout.rs` is the geometry and the reference
encoder; `fixtures/frame-oracle/graphic.mjs` draws the same), and judges a run per §5.5: Δn
classes, tears, key mismatch, unpaired KEY, FILL and KEY timecode steps, unreadable, source
period (0.5 %), step check, lag band (A: P/4, B: 1.25·P̂_src) and the skip rule (B at
fractional rates: ≤ ⌈N/1000⌉+1, each Δn = 2, ≥ 900 apart). Binary: `noplayout-oracle <dump>
--mode A|B [--rate] [--warmup]`, `--print-crops WxH` for the dump crops; it drops the
lead-in before the first readable frame. Unit tests: synthetic round trips (whole, cropped,
a flipped cell → unreadable), the §5.5 model cases in exact integer arithmetic (A 59.94; B
59.94 / 29.97 → 10 skips per 10,000; B 50 / 25 → 0; five phases each) and the two mutations
(10-frame freeze: 10 step errors, 9 Δs ≤ 0, band 165.8 ms; repeat: band 32.3 ms). Dump path:
`noplayout-output` tests run the real pacer, ring, lanes and `DumpSink` on a virtual clock
with a synthetic oracle painter, then the oracle on the dump: clean passes the A row; each of
the eight injections fails with its detector (8 = a `DumpSink` sender blocked 300 ms).
`fixtures/recovery-probe/probe-static` (no timer, no rAF, no time read) and `probe-timed`
(+ deciseconds since load) encode counter, data hash and step; `decode::read_probe` reads
them. `scripts/dev-oracle-run.sh A|B [frames]` runs the frame oracle through the real CEF
renderer and the oracle; on the Mac Studio gateway shell it decodes every frame (0
unreadable, tears, key mismatches, ramp errors) while the timing rows fail on the degraded
timer (not evidence). `scripts/dev-check-fixtures.sh` imports and loads all three fixtures.
Gap: the 3 s stuck-sink case recycling the output process needs the supervisor (task 8,
`TODO(M1)`); the output already raises `SenderStuck` at 2 s.

Task 7 (spikes S1a, S2): `cargo xtask kit` (macOS) builds
`target/kit/noplayout-m1-kit-<target>.tar.gz`: release binaries, the renderer bundle, the host
runtime, the fixtures and one entry point, `m1-timing.command`, for a Mac with no toolchain
(D-020). It writes one `results.json` + `log.txt` per run: machine, session type, a timer probe
(`noplayout-output --bench-timer`), S2 paint-path cost (`--bench-s2`), S1a rows (A/B × 59.94/50,
10 minutes each by default, through the real renderer and the oracle) and process-tree memory at
1 and 2 channels with the Basketball on-air set, plus swap and memory pressure (D-021); earlier
results survive a kit rebuild. Engine and output take `NOPLAYOUT_RATE`. Reports:
`docs/spikes/S1a-paint-pacing.md`, `docs/spikes/S2-paint-path-and-memory.md`; ADR draft
`docs/adr/0061-provisional-drive-mode.md` (provisional macOS mode A; Windows/Linux pending,
D-019). **All S1a/S2 numbers are from the Mac Studio, Terminal session, 1 minute per row:
provisional, not reference hardware.** No S1a row passes there (one late pacer wake a minute in
every row; mode A a few held slots; mode B 59.94 flaps at the rate crossing). S2: 1080p
copy + convert ≈ 1.2 ms per slot; memory 857 MiB (1 channel) / 1,679 MiB (2 channels, upper
bound), no swap, pressure normal; budget proposed for Caleb in the S2 report. The M2-mini run
is pending an off-show window. Found and fixed on the way: ring names took the UUIDv7 run id's
time prefix, so two engines started within a minute opened each other's ring. Open
(`TODO(M1)`): pacer time-constraint policy if the M2 shows the late wake; B's crossing
hysteresis before S1b; `footprint` instead of summed RSS; a command sent before the program
page registers is lost (`"result": null`), gated by the supervisor in task 8.

Tasks 9 and 10 (conformance, pixels): `tests/conformance` (`cargo xtask conformance`; see its
README). Locally on macOS arm64 all 16 conformance runs pass: the 9 EBU examples, the 6
Basketball graphics and a `stepCount: -1` fixture. Injected faults are caught: a console
error, a non-200 result and a wrong `currentStep`. The pixel cases pass against the blessed
goldens: the alpha-ramp (16 patches at 17·i ± 1), the gradient/shape fixture, six per-OS
Basketball goldens, and the scorebug's 40-slot entry sequence (animates, transparent
background, antialiased edges). Two repeat runs were stable: 0 px beyond ±2, except 1 px in
the matchup. Linux goldens come from the first CI run's artifact. CI: required on linux and
macos-arm64, informational on windows and macos-x64 (done-when (a) allows Windows by
`preflight`). The output gained `NOPLAYOUT_DUMP_AFTER`, a dump start gate.

Still pending: M1 task 8 (supervisor/recovery and S7a). M1 stays open. Windows/Linux
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

`noplayout-pixels` (M1/M2),
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
