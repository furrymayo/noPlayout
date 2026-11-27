# Conformance, pixel and recovery tests (M1 tasks 9, 10 and 8)

`cargo xtask conformance [--release] [conformance | pixels | recovery] [--only NAME] [--bless]` builds the
engine, output process and CEF renderer and runs `noplayout-conformance` against them. On Linux,
run it under `xvfb-run -a`. The host runtime must be built (`cargo xtask web`). CI runs it on
every lane; it is required on linux and macos-arm64.

## Conformance

Every EBU example (`third_party/ograf/v1/examples`, all nine) and every Basketball graphic
(`fixtures/basketball-ograf-package`, all six) runs unmodified in one engine. So does
`fixtures/step-unbounded` (`stepCount: -1`). Each one goes through load → one `play` per step
→ update (schema defaults) → every custom action (schema defaults) → stop → dispose. The test
asserts:

- every result arrives with `statusCode` 200;
- there are zero console errors;
- `currentStep` semantics (O6/O7): the `n`-th play reports step `n − 1`, an absent `currentStep`
  means "at end", and the engine's mirror (`inspect`) agrees after every play;
- `stepCount: -1` graphics play until they report the end (at most 32 plays);
- after dispose the instance leaves the mirror.

## Pixels

Each case gets its own engine with the output process dumping settled slots
(`NOPLAYOUT_DUMP_AFTER`). The last slot's FILL (straight BGRA) and KEY are joined into a
straight-alpha RGBA PNG and compared with a golden at ±2 LSB: alpha, and colour premultiplied
(c·α/255) as a keyer composites it.

| Case | Golden | Extra check |
|---|---|---|
| `fixtures/alpha-ramp` | `goldens/shape/` (every OS, no pixel may differ) | the 16 patches' alpha within ±1 of 17·i, white (done-when (b)) |
| `fixtures/gradient-shape` | `goldens/<os>-<arch>/` (geometry antialiasing differs between rasterisers: about 2,900 edge pixels Metal vs software GL) | none |
| the six Basketball graphics (ticker paused) | `goldens/<os>-<arch>/` (text; up to 0.05 % of pixels may differ) | none |
| `scorebug-sequence` | none | 40 slots across the entry animation: frames change, corners alpha 0, mostly transparent, opaque areas, antialiased edges (done-when (b)) |

Failed or missing goldens write a candidate to `target/conformance/candidates/`, which CI
uploads as the `conformance-<lane>` artifact. To add or refresh one OS's goldens, take the
candidates from that lane's artifact (or run `--bless` on that OS). Review them by eye, then
commit them.

## Recovery (task 8, functional)

The Basketball scorebug, loaded and playing, through K1 and K3a with a ready standby (warm
promotion), two K1s within 10 s (loop guard: warm then cold), and K1 with
`NOPLAYOUT_STANDBY=0` (cold). The graphic must stay loaded through each; timing is S7a's
(`docs/spikes/S7a-recovery.md`).
