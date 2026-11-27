# Test fixtures

- `basketball-ograf-package/`: Caleb's Basketball OGraf Package (six graphics, README,
  `preview.html`), committed byte-identical to the extracted package that D-012 checked (source zip
  sha256 619394bb…9f38; the zip itself is not in the repo). Apache-2.0 as part of this repository;
  the bundled Barlow fonts keep the SIL Open Font License (`*/lib/fonts/OFL-Barlow.txt` stays
  beside them). `SHA256SUMS.basketball` lists every file
  (`cd fixtures/basketball-ograf-package && shasum -a 256 -c ../SHA256SUMS.basketball`).
- `oes/`: the OES scoreboard corpus (D-013); see `oes/README.md`.

- `frame-oracle/`: the frame oracle (PLAN §5.5); geometry in `crates/noplayout-oracle/src/layout.rs`.
- `recovery-probe/probe-static`, `probe-timed`: the recovery probes (PLAN §2.13.6); `probe-static`
  must stay free of timers, rAF and time reads.

M1 still adds the alpha-ramp and gradient/shape fixtures and goldens (task 10).
