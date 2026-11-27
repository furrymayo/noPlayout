# S2: paint-path cost and process-tree memory (PLAN §7 M1 task 7, R2, D-021)

Status: **measured, provisional, not reference hardware.** Mac Studio dev box (M4 Max, 128 GB,
macOS 26.3), Terminal session, kit `601efc8`. The binding numbers come from the M2 Mac mini
(8 GB, macOS 27.2.1; D-020, D-021) with the same kit.

## Paint path, CPU per slot (`noplayout-output --bench-s2`, 120 iterations)

| Size | Ring copy (renderer `on_paint` → slot) p50 / p95 | Take newest + FILL/KEY conversion p50 / p95 | Conversion alone p50 / p95 / max |
|---|---|---|---|
| 1080p | 0.10 / 0.11 ms | 1.01 / 1.05 ms | 1.01 / 1.04 / 1.44 ms |
| 2160p | 0.44 / 0.46 ms | 4.00 / 4.11 ms | 4.0 / 4.1 / 6.4 ms |

CEF's own readback (GPU → the `on_paint` buffer) happens inside CEF and is not separable here;
the S1a rows show it keeps up at 60 Hz. R2's line (p99 of copy + convert + submit > 8 ms per
1080p frame) is far off: about 1.2 ms per 1080p slot. 2160p at 59.94 uses about a quarter of a
period on one core, so 2160p channels will want the conversion split over threads (R2's first
mitigation) before M6.

## Memory: whole process tree, Basketball on-air set (scorebug, ticker, sponsor bug, all playing)

| Channels | Processes | RSS max | RSS mean | Swap growth | Memory pressure (max level) |
|---|---|---|---|---|---|
| 1 | 8 | 857 MiB | 830 MiB | 0 | 1 (normal) |
| 2 | 16 | 1,679 MiB | 1,626 MiB | 0 | 1 (normal) |

30 s per row. M1 runs one channel per engine, so the 2-channel row is two engines and counts the
engine twice; it is an upper bound. Summed RSS also counts the CEF framework's shared pages once
per helper, so it overstates what the tree costs; the M2 run should add `footprint` per process
(`TODO(M1)`, STATUS.md). **Not yet in these numbers:** the warm standby renderer per channel
(task 8), the UI and its preview browsers (M3/M4). The kit records swap and the kernel's memory
pressure level alongside.

## Proposed memory budget for the M2 mini (8 GB)

- **Whole noPlayout tree: ≤ 1.25 GiB at 1 channel and ≤ 2.5 GiB at 2 channels of 1080p59.94**
  with the Basketball on-air set, including the warm standby renderers and the UI with its
  preview browsers. That leaves at least 5.5 GB for macOS, Companion and the rest of the show.
- **No swap growth and memory pressure normal** for a whole show (the soak checks both).
- Defaults that follow from it: preview browsers at ≤ 10 fps and torn down when not visible;
  one warm standby per channel, never more; a 2160p channel is outside the 8 GB budget until
  measured.

Today's 830 MiB per channel without standby or UI leaves about 0.4 GiB per channel for the
standby renderer and the UI share. That is tight; the M2 numbers with `footprint` decide whether
the budget holds or the standby policy changes (for example a standby renderer without its own
GPU process). Caleb decides the budget.
