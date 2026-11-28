# S7a — recovery at paint level (M1 task 8)

**Status: supervisor built and functionally tested; the paint-level timing harness and the gate run are pending.**
Not measured on reference hardware (D-019, D-020: the gate binds on the M2 mini; Windows/Linux timing is post-air).

## What exists (M1 task 8)

`crates/noplayout-core/src/supervisor.rs` runs an active and a warm standby renderer per channel (§2.13.3):

| Kind | Detector (§2.13.1) | Implemented |
|---|---|---|
| K1 | active renderer process exits | yes |
| K2 | active page's render process ends (`PageTerminated`) | yes (not yet injected by a test) |
| K2g | output `PaintStall` for the active incarnation (paint probe, E-057) | yes (wired; not yet injected) |
| K3a | active page silent for 500 ms (2 s while a `load` is pending) | yes |
| K3b | poison action → quarantine | **pending** `TODO(M1)` |
| K4 | control heartbeat (100 ms) unanswered for 500 ms | whole-process heartbeat only; UI-thread latency probe **pending** |
| K5/K6 | stuck sink → recycle output | **pending** (logged only) |

Restore: warm promotion (`Activate` to the standby's ring, `Role{active}`, bus `promote` → `seq` barrier, §2.14), a replacement standby rebuilt from the bus snapshot behind it, cold restore when no standby is ready, and the loop guard (same failure within 10 s of a takeover → cold with 2 s / 5 s / 10 s backoff, then red). `NOPLAYOUT_STANDBY=0` disables the standby. The supervisor's state and one record per recovery (kind, warm/cold/red, failed/new incarnation, detection time on the shared monotonic clock, detection → output switch) are published in `/api/v1/health` under `supervisor`.

## Functional evidence (CI, every lane; required on linux and macos-arm64)

`cargo xtask conformance recovery` (`tests/conformance/src/recovery.rs`), Basketball scorebug loaded and playing:

- K1 (kill) with a ready standby → warm promotion; graphic still loaded on the new incarnation.
- K3a (`hang 3000`) with a ready standby → warm promotion; graphic still loaded.
- K1, then K1 again within 10 s → warm, then cold (loop guard); graphic reloaded; `play` answers 200.
- K1 with `NOPLAYOUT_STANDBY=0` → cold restore; graphic reloaded.

Mac Studio dev probe (`scripts/dev-recovery-probe.sh`, gateway shell, **provisional, not reference hardware, not a timing result**): K3a detected at the 500 ms deadline; warm switch (detection → `Activate` sent) under 0.1 ms; cold restore ≈ 2.2 s after a 2 s loop-guard backoff.

## Pending for the S7a gate (done-when (d))

`TODO(M1)`, all on the M2 mini from the kit's Terminal `.command`:
- The paint-level harness: t0 immediately before injection; the oracle times the first frame of the new incarnation at the output. Fail lines: K1 ≤ 500 ms, K3a/K4 ≤ 1,000 ms, 20 of 20 runs.
- K2 and K2g injection (render-process kill, forced GPU stall with rAF advancing and stopped); the static-scene control showing no false `PaintStall`; `invalidate` under external begin-frame.
- The recovery fidelity record and standby eligibility (§2.13.3a, E-052, E-058), the two Basketball recovery tests, and the kill → rebuild → second-kill exact/flagged cases with the standby indicator checked before each kill.
- Cold-path times recorded.

| OS | Warm fail lines | Cold times | Fidelity cases |
|---|---|---|---|
| macOS arm64 (M2 mini) | pending | pending | pending |
| Windows 11 | pending (D-019) | pending (D-019) | pending (D-019) |
| Linux | pending (D-019) | pending (D-019) | pending (D-019) |
