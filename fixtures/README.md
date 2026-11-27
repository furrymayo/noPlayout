# Test fixtures

- `basketball-ograf-package/`: Caleb's Basketball OGraf Package (six graphics, README,
  `preview.html`), committed byte-identical to the extracted package that D-012 checked (source zip
  sha256 619394bb…9f38; the zip itself is not in the repo). Apache-2.0 as part of this repository;
  the bundled Barlow fonts keep the SIL Open Font License (`*/lib/fonts/OFL-Barlow.txt` stays
  beside them). `SHA256SUMS.basketball` lists every file
  (`cd fixtures/basketball-ograf-package && shasum -a 256 -c ../SHA256SUMS.basketball`).
- `oes/`: the OES scoreboard corpus (D-013); see `oes/README.md`.

M1 adds our own test graphics (alpha ramp, frame-oracle, recovery-probe, test pattern) and goldens.
