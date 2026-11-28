# S1a: paint-level pacing, mode A vs mode B (PLAN §5.6, M1 task 7)

Status: **harness built; first numbers provisional, not reference hardware.** Every number below
comes from the Mac Studio dev box (M4 Max, 128 GB, macOS 26.3) in a Terminal (Aqua) session,
1 minute per row. The binding runs are 10 minutes per row on the M2 Mac mini (D-020, D-021,
macOS 27.2.x). Windows and Linux timing: `pending (D-019)`.

## What runs

`m1-timing.command` (from `cargo xtask kit`; no Rust toolchain needed on the target Mac) runs,
for each of 59.94 and 50 and each of modes A and B, `scripts/dev-oracle-run.sh`: engine, output
process and the real CEF renderer play `fixtures/frame-oracle`; the output process paces with
its channel clock and selector and dumps every slot's FILL and KEY patch crops through the real
per-plane sender threads (`DumpSink`); `noplayout-oracle` decodes the dump's pixels and
timecodes and applies the §5.5 row for the mode and rate. The results JSON records the machine,
the session type (`launchctl managername`), a timer probe, and per row the oracle report; each
row's `engine.log` carries the output's 1 Hz counters.

The timer probe is how a number's provenance is checked: in the Mac Studio's Terminal session
the precise wait was 0.000 ms late at p95 and a 1 ms sleep took 1.27 ms; from the gateway shell
the same probe gave 1.5 ms and 5.0 ms (timer coalescing), and pacing from that shell fails on
the timer alone.

## Provisional results (Mac Studio, Terminal session, 1 min per row, kit `601efc8`)

| Row | Repeats | Skips | tc errors | Step errors | Lag band (limit) | Unreadable / tears / key | Pacer: held, slots skipped, late max | Row |
|---|---|---|---|---|---|---|---|---|
| A 59.94 | 3 | 3 | 1 | 6 | 33.6 ms (4.2) | 0 / 0 / 0 | 3, 1, 9.4 ms | fail |
| B 59.94 | 85 | 89 | 1 | 0 | 61.1 ms (20.9) | 0 / 0 / 0 | 83, 1, 8.8 ms | fail |
| A 50 | 2 | 1 | 1 | 1 | 40.6 ms (5.0) | 0 / 0 / 0 | 2, 1, 18.7 ms | fail |
| B 50 | 0 | 1 | 1 | 0 | 0.1 ms (25.0) | 0 / 0 / 0 | 0, 2, 25.7 ms | fail |

4,080 frames per row after warm-up. The source period was within 0.5 % in every row (16.7 ms
and 20.0 ms; Chromium coarsens rAF timestamps to 100 µs, which the step check tolerates).

## Reading

1. **The decode path is sound.** No unreadable patch, tear, KEY/FILL mismatch or ramp error in
   any row: FILL and KEY of every slot come from one paint, through the real ring and lanes.
2. **One late pacer wake per minute fails every row.** Each row has one or two slots the pacer
   woke for more than half a period late (`slots_skipped`, late max 9–26 ms), hence one timecode
   gap. The OS timer probe was clean, so this is a scheduling stall of the pacer thread under
   load. On the M2 run this decides whether the pacer needs the macOS time-constraint thread
   policy (`THREAD_TIME_CONSTRAINT_POLICY`, the Core Audio approach) instead of a QoS class
   (`TODO(M1)` in `noplayout-frames::clock`, STATUS.md).
3. **Mode A** otherwise shows 2–3 held slots a minute and a lag band of two periods: now and then
   a begin-frame's paint lands after `T_k − 1 ms`. Small, but the A row allows none.
4. **Mode B at 59.94 flaps at the 60 → 59.94 crossing.** 85 repeats and 89 skips a minute
   against the expected ~4 skips and no repeats: CEF's free-running paint times jitter by more
   than the selector's 1 ms hysteresis allowance, so near the crossing it alternates between
   holding and skipping. This is a selector finding to fix before S1b (M2): widen the hysteresis
   window to the measured jitter (PLAN §5.3 allows up to 1.5 · `P̂_src` for the band) or
   phase-lock the selection. **Mode B at 50** is clean apart from finding 2.

## Provisional mode

No row passes on this machine, so nothing is claimed. **Provisional macOS mode: A** (the
engine default), because A's failures are a handful per minute while B at 59.94 has a selector
defect. To be decided on the M2 mini's 10-minute runs; the ADR draft is
`docs/adr/0061-provisional-drive-mode.md`. Windows, Linux: `pending (D-019)`.

## Running it on the M2 mini

`cargo xtask kit` → `target/kit/noplayout-m1-kit-aarch64-apple-darwin.tar.gz`. Copy it over,
unpack, and run `m1-timing.command` from the `noplayout` user's GUI session (Finder or
`open -a Terminal`), between shows only (about 45 minutes with the 10-minute defaults). One
results JSON and one log land in `results/<host>-<UTC time>/`. Open question for that first
run (Rayleigh, D-021): whether CEF OSR works from an SSH (non-Aqua) session; the results
record the session type either way.
