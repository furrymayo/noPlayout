# noPlayout

A free, cross-platform (Windows, macOS, Linux) broadcast graphics playout engine in Rust, built on
the EBU [OGraf](https://ograf.ebu.io/) v1 standard. Graphics render in an offscreen Chromium
(CEF) and go out over NDI® and as web outputs. See [`IDEA.md`](IDEA.md) for what it is for and
[`PLAN.md`](PLAN.md) for how it is built.

**Status:** M0 (workspace skeleton). Nothing plays out yet. See [`STATUS.md`](STATUS.md).

## Build and run (developers)

Prerequisites: Rust via rustup (the pinned toolchain in `rust-toolchain.toml` installs itself),
Node.js 24+, Git. Linux also needs the Tauri v2 system packages
(`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev`).
Windows needs the MSVC build tools and WebView2 (preinstalled on Windows 11).

```sh
cargo xtask cef        # download and export the pinned CEF (156.0.31) into target/cef, once
cargo xtask run        # build the web UI, the engine, the renderer and the app, then open the app
```

The window should read **Engine: running · Renderer: CEF 156.0.31 OK**.

Other tasks:

| Command | What it does |
|---|---|
| `cargo xtask self-test` | headless smoke: the engine starts the output and renderer processes and checks each `Hello` |
| `cargo xtask bundle` | build the renderer; on macOS, lay out its dev app bundle (CEF needs one) |
| `cargo xtask web` | `npm ci` + `npm run build` in `web/` |
| `cargo xtask preflight` | run the local checks CI can't and write `target/preflight/<os>-<arch>-<commit>.json` |
| `cargo xtask licences` | check that the renderer's dependency tree holds nothing copyleft (E-032) |
| `cargo deny check` | licence allowlist, sources and advisories (`deny.toml`) |

Local gates before a push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `cargo deny check`, and in `web/`: `npm run check` and `npm test`.

## Layout

| Path | What |
|---|---|
| `crates/noplayout-core` | engine library and the `noplayout-engine` binary (API, supervision, logs) |
| `crates/noplayout-renderer` | CEF offscreen renderer process |
| `crates/noplayout-output` | output process (NDI arrives in M2) |
| `crates/noplayout-proto` | engine ↔ child control protocol |
| `crates/noplayout-app` | Tauri v2 desktop app |
| `crates/noplayout-{frames,pixels,ndi,oracle,datasource,decoder-oes}` | skeletons for later milestones |
| `crates/xtask` | build tasks |
| `web/` | Svelte UI |
| `fixtures/` | test fixtures (byte-identical; never reformat) |
| `docs/adr/` | architecture decision records; `docs/01-decisions.md` holds product decisions |

## Licence

Apache-2.0 ([`LICENSE`](LICENSE)); third-party notices in [`NOTICE`](NOTICE).
NDI® is a registered trademark of Vizrt NDI AB. noPlayout does not include the NDI SDK.
