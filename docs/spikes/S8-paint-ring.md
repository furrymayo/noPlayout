# S8: paint ring transport (E-042, R22)

Status: **own ring adopted for M1; iceoryx2 not evaluated in code.** Timing figures below are
from the Mac Studio dev box and are **provisional, not reference hardware** (D-019/D-020: the
binding runs are on the M2 mini).

## Question

E-042: does iceoryx2 0.10.1 carry 8.29 MB 1080p BGRA paints from renderer to output process with
zero extra copies, a writer that never blocks, and no leaked shared memory after `kill -9` of the
writer on all three OSes? If not, use our own 4-slot ring over platform shared memory.

## What was built

`crates/noplayout-frames`: `shm` (POSIX `shm_open`/`mmap`, Windows pagefile-backed
`CreateFileMappingW`), `ring` (PLAN §5.3 protocol: 4 slots, `{state, inc, paint_seq,
painted_at}` headers changed only by atomics, `gen << 8 | state` words so a reclaim between a
reader's look and its compare-and-swap cannot be mistaken for the same paint), and `Registry`
(every ring name is recorded before creation; the engine sweeps them when an incarnation retires
and at start).

## Criteria and evidence

| Criterion | Own ring | How shown |
|---|---|---|
| Zero extra copies | One copy: CEF's `on_paint` buffer straight into the ring slot (the copy PLAN §2.4 calls mandatory). The reader converts from the slot in place. | `WriteSlot::pixels()` hands out the shared slot; no staging buffer exists. |
| Writer never blocks | `begin()` is a bounded lock-free loop over 4 slots; no waits, no allocation. It reclaims the oldest READY slot when none is FREE and never a READING one. | `writer_never_blocks_and_never_reclaims_the_reading_slot` (100 paints while the reader holds a slot); `concurrent_writer_and_reader_see_consistent_paints` (~800k paints, no torn reads). |
| No leak after `kill -9` | Windows: the section dies with its last handle. Unix: the registry sweep unlinks it; afterwards `shm_open` returns `ENOENT` (and `/dev/shm/<name>` is gone on Linux). | `kill_9_writer_leaves_no_leak`: child writer killed mid-copy; the reader ignores the WRITING slot, still takes the last whole paint, and nothing remains after the sweep. Runs in CI on all four lanes. |

### Provisional measurement (Mac Studio, gateway shell, `cargo test --release -p noplayout-frames -- --ignored s8_measure --nocapture`)

```
{"s8":"own-ring","paints":144,"taken":105,"overwrites":0,"superseded":37,
 "begin_us":{"p50":0.21,"p99":0.83,"max":0.83},
 "begin_copy_publish_us":{"p50":834.6,"p99":1731.1,"max":7826.3},"bytes_per_paint":8294400}
```

- `begin()` (the only part that could block) is sub-microsecond.
- The 8.29 MB copy is ~0.8 ms p50; the max includes first-touch page faults on fresh slots.
- The paint rate (144 in 5 s rather than ~600) shows the gateway shell's timer coalescing on this
  box (`sleep(8 ms)` stretched), which is why timing gates do not bind here.

## iceoryx2

Not exercised. Reasons for not adopting it in M1:

- The own ring meets all three S8 criteria above with ~500 lines and two dependencies already in
  the lock file (`libc`, `windows-sys`); iceoryx2 would add a large dependency tree and its own
  service/cleanup model that PLAN notes has never been read in source.
- R22's mitigation is exactly this ring, with the same §5.3 protocol, so nothing downstream
  depends on the choice.

**Open for the frontier review:** E-042 says "iceoryx2 if S8 shows it meets …". This card did not
run iceoryx2, so the comparison is not done; the decision recorded here is "own ring is
sufficient", not "iceoryx2 fails". Remove `iceoryx2` from the workspace pins once that is agreed.

## Pending

- Binding timing on the M2 mini (writer copy at 1080p59.94 with two channels), via the M1 timing
  kit.
