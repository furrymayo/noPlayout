# 0064. Pixel goldens: per-OS shape goldens, premultiplied colour compare, fixed budgets

- **Status:** accepted
- **Date:** 2026-11-27
- **Deciders:** franky (frontier review t_a3bac44c of PR #2)
- **Related:** PLAN.md §7 M1 task 10, §8.1; D-019; `tests/conformance/README.md`

## Context

PLAN task 10 asks for an OS-independent golden for the shape fixtures and per-OS goldens only
where text is rendered. The M1 build found that geometry antialiasing is not OS-independent:
the CI runners' renderers differ. It compared colour premultiplied, made `gradient-shape`
per-OS, and gave every per-OS golden a budget of `W·H/2000` (1,036 px), which also hid real
changes. The windows and macos-x64 lanes had no goldens.

Measured on the committed goldens (`cargo test -p noplayout-conformance -- --ignored --nocapture
cross_os`, ±2 LSB, premultiplied):

| Case | linux vs macos-arm64 | linux vs windows | linux vs macos-x64 | macos-arm64 vs macos-x64 |
|---|---|---|---|---|
| `gradient-shape` | 2,889 px, max 52 | 0 | 0 | 2,889 px, max 52 |
| Basketball (six) | 1,425–44,859 px | 2,267–28,195 px | 1,402–28,024 px | 0–15,633 px |

The macos-arm64 runner rasterises through Metal on its GPU; the linux, windows and macos-x64
runners have no GPU and use Chromium's software path, which agree exactly on geometry. Run to
run on one lane, every case differs from its own golden by 0–2 px beyond ±2 LSB (CI and local
runs).

## Decision

1. **Per-OS goldens for geometry with edges.** `alpha-ramp` (axis-aligned patches) keeps one
   golden for every OS (`goldens/shape/`) with no pixel allowed beyond ±2 LSB.
   `gradient-shape` has a golden per `<os>-<arch>` because of the 2,889 antialiased edge pixels
   (up to 52 LSB) between Metal and the software path. This deviates from task 10's
   OS-independent shape golden.
2. **Colour is compared premultiplied** (c·α/255, as a keyer composites it), with alpha
   compared directly; tolerance ±2 LSB.
3. **Budgets beyond ±2 LSB:** `gradient-shape` **0 px** on its per-OS golden; each Basketball
   per-OS golden **≤ 32 px** (measured noise 0–2 px). The old `W·H/2000` budget is gone.
4. **Negative control.** The scorebug updated to a changed score (77–88) is compared with the
   unchanged scorebug golden and must fail the budget (measured: 2,998 px on macOS arm64). If
   the compare could not see a score change, the pixel suite fails.
5. **Goldens for all four lanes.** `windows-x86_64` and `macos-x86_64` goldens are blessed from
   those lanes' CI `conformance-<lane>` candidates after checking them by eye. Those lanes stay
   informational (PLAN §8.1) but their conformance step must pass for real; D-019 requires the
   Windows functional tests to pass.

## Consequences

- A Chromium/CEF bump or a runner image change that alters rasterisation shows as a golden
  failure on the affected lanes; refresh those goldens from the candidates after review.
- A real visual change of more than 32 text pixels (a digit, a word) cannot pass.
- The macos-arm64 goldens are GPU goldens; a self-hosted runner with a different Apple GPU may
  need its own.
