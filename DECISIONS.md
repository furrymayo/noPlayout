# noPlayout: engineering decisions

Companion to `PLAN.md`. One entry per non-obvious engineering choice: options, choice, reason, and what would change it.
Product decisions made by Caleb live in `docs/01-decisions.md` (D-nnn); entries here cite them where they apply and never
contradict them. When M0 creates `docs/adr/`, each accepted entry becomes one ADR from the GeneralDocuments template (E-035).

- **Status values.** *Decided (Caleb, D-nnn)*: Caleb has ruled, and the entry records the engineering consequences.
  *Proposed*: Franky's choice, pending cold review and Caleb's approval of the plan. *Spike-gated*: the direction is set and
  a named measurement picks between the options.
- All versions were checked on crates.io/npm on 2026-11-26 (PLAN §3).

| ID | Decision | Status |
|---|---|---|
| E-001 | Three process kinds: app, headless engine, one renderer per channel | Proposed |
| E-002 | Cargo workspace split; renderer does not link the engine core | Proposed |
| E-003 | UI framework: Tauri v2 (stable) + system webview | Proposed |
| E-004 | UI front end: Svelte 5 + TypeScript | Proposed |
| E-005 | CEF paint mode: CPU `on_paint`, shared texture off in v1 | Proposed |
| E-006 | Frame pacing: integer per-channel clock in the renderer; drive mode chosen per OS by S1 | Spike-gated |
| E-007 | NDI crate: grafton-ndi 3.0.0 behind our own trait | Proposed |
| E-008 | NDI sending happens inside the renderer process | Proposed |
| E-009 | Shaped vs unshaped defaults, per output mode | Decided (Caleb, D-010), bench-confirm in M2 |
| E-010 | NDI wire format BGRA/BGRX, explicit timecode, SDK clocking off | Proposed |
| E-011 | NDI runtime detected and linked, never bundled | Decided (Caleb, D-010) |
| E-012 | IPC: local sockets + postcard; no shared memory in v1 | Proposed |
| E-013 | Monitor frames: downscaled JPEG made in the renderer, credit flow control | Spike-gated (S3) |
| E-014 | One CEF browser process per channel | Proposed |
| E-015 | Cue preloads the graphic into a hidden slot | Proposed |
| E-016 | Web fill/key pages made from the same scene with SVG colour-matrix filters | Proposed |
| E-017 | One OGraf host runtime for every page; the CEF program page is authoritative | Proposed |
| E-018 | Pages are driven over a WebSocket host protocol, not CEF process messages | Proposed |
| E-019 | Crash recovery by replaying each instance's action log with `skipAnimation` | Proposed |
| E-020 | Settings: JSON, atomic write, last-good, validation in Rust | Proposed |
| E-021 | Take IDs: u16, unique per show, enforced in the engine | Decided (Caleb, D-005, D-010) |
| E-022 | Decoder plug-ins: compiled in, one crate per decoder | Proposed |
| E-023 | Decoders are gated on real captures; OES only in v1 | Decided (Caleb, D-011) + Proposed gate |
| E-024 | OGraf spec vendored at a commit; manifest validation never uses the network | Proposed |
| E-025 | Edit-mode preview rendered by a CEF renderer | Decided (Caleb, D-010) |
| E-026 | OGraf Server API mapping: renderer = channel, RenderTarget = `{layer}` | Proposed |
| E-027 | OpenAPI generated from code with utoipa | Proposed |
| E-028 | Intel macOS: tested until GitHub retires the runner, then build-only | Decided (Caleb, D-010) |
| E-029 | Toolchain pinned to Rust 1.100.0, edition 2024 | Proposed |
| E-030 | Pure-Rust JPEG encoder | Proposed |
| E-031 | Accept `serialport` (MPL-2.0) | Proposed |
| E-032 | cargo-deny licence allowlist; nothing copyleft in the renderer | Proposed |
| E-033 | Engine death: renderers exit; supervised restart restores, cold start asks | Proposed |
| E-034 | Network exposure default as IDEA says (open on the LAN), token optional | Proposed (flagged) |
| E-035 | ADRs in `docs/adr/`, one per accepted entry | Proposed |
| E-036 | Repo licence Apache-2.0 | Decided (Caleb, D-010) |
| E-037 | RossTalk-compatible listener in v1, off by default | Decided (Caleb, D-010) |
| E-038 | Data-source connections per computer, mappings per show; hold on loss | Decided (Caleb, D-009) |
| E-039 | TAKE onto an occupied layer replaces it with a cut | Proposed |

---

## E-001: Process model
- **Context.** IDEA needs a desktop app, a per-channel offscreen Chromium and an HTTP API that Companion can use, and IDEA §6
  wants a renderer crash to recover in about 2 s.
- **Options.** (a) One process: Tauri app hosts the engine and CEF. (b) App + one engine that also hosts all CEF browsers.
  (c) App + headless engine + one renderer process per channel.
- **Choice.** (c).
- **Reason.** CEF and the Tauri webview can't share a process cleanly: CEF wants to own process startup (`execute_process`)
  and its message loop, and Tauri wants the main thread. A renderer crash or hang must never take the control plane or the
  other channels with it. A headless engine also runs on a rack PC with no window, controlled from Companion, and lets CI test
  the core without Tauri.
- **Cost.** Three binaries to package and an IPC layer. Engine death is a single point of failure (R17, E-033).
- **Would change if.** Tauri ships a stable CEF runtime (then a and c merge for the UI side only; channels stay separate).

## E-002: Workspace layout
- **Choice.** One Cargo workspace: `noplayout-core` (lib + engine bin), `noplayout-renderer`, `noplayout-app`,
  `noplayout-proto`, `noplayout-ndi`, `noplayout-pixels`, `noplayout-datasource`, one `noplayout-decoder-<id>` crate per decoder,
  `xtask`. `noplayout-renderer` depends only on `proto`, `ndi`, `pixels` and `cef`.
- **Reason.** CEF's build is heavy; keeping it out of the core keeps `cargo test -p noplayout-core` fast and CI cheap. The
  boundary also stops show logic from creeping into the renderer. D-001 named three crates; the extra libraries are internal.
- **Options rejected.** A single crate with features (CEF rebuilds on every change); separate repos (version skew between
  engine and renderer protocol).

## E-003: UI framework
- **Context.** IDEA asks for Tauri or "a CEF-based UI". Robin D3: Tauri+CEF exists only as `tauri-runtime-cef`
  3.0.0-alpha.7 (no stable release, depends on `cef =154.5.0`).
- **Options.** (a) Tauri v2.12.7 with the system webview (WebView2 / WKWebView / WebKitGTK). (b) Tauri 3 alpha with CEF.
  (c) A windowed cef-rs UI host of our own. (d) Native Rust UI (egui, Slint, Iced).
- **Choice.** (a).
- **Reason.** Stable and supported on all three OSes. (b) is alpha and cannot be in the same build graph as our renderer:
  `cef-dll-sys` declares `links = "cef_dll_wrapper"`, so Cargo refuses two versions (154 and our 156) in one workspace. (c)
  means writing window management, menus, dialogs and packaging ourselves. (d) would throw away Usopp's HTML/CSS mockups
  and the shared web tooling. The UI never shows graphics pixels directly: monitors are JPEG frames from the renderer and the
  edit canvas comes from CEF (E-025), so webview differences don't affect what goes to air.
- **Risks.** WebKitGTK quirks on Linux (R7), handled with `WEBKIT_DISABLE_DMABUF_RENDERER=1` and a manual check on Fedora KDE
  Wayland and an NVIDIA machine. Fallback is (c).
- **Would change if.** Tauri 3 with CEF goes stable on a CEF version we can share; re-evaluate then.

## E-004: UI front end
- **Options.** Svelte 5, React 19, Solid, plain TypeScript.
- **Choice.** Svelte 5.59.2 + TypeScript 6.0.3 + Vite 8.4.0, pinned in `package-lock.json`.
- **Reason.** Small runtime, compiled reactivity that suits a UI that updates lights and counters many times a second, and
  good Tauri templates. TypeScript 6 because svelte-check 4.7.8's peer range is `^5 || ^6` (TS 7.0.2 is out but unsupported).
- **Would change if.** Usopp's build spec needs a component library that exists only for React; not the case in UI.md rev 2.

## E-005: CEF paint mode
- **Options.** (a) CPU `on_paint`: CEF hands a premultiplied BGRA buffer, we copy it. (b) `accelerated_osr`: shared GPU
  texture via `on_accelerated_paint`, then GPU readback or GPU conversion.
- **Choice.** (a) in v1; (b) stays off (the `cef` crate feature `accelerated_osr` is not enabled).
- **Reason.** NDI needs CPU memory anyway, so (b) only pays if conversion also moves to the GPU, which means per-OS
  D3D11/Metal/Vulkan code. (a) is one code path on every OS and costs one 8.3 MB copy per 1080p frame, inside budget on paper.
  Robin flagged per-OS CEF OSR issues (#4166 incomplete frames on macOS x86_64, #4357), which S1 checks for (a) first.
- **Would change if.** S2 or the M6 soak shows copy + convert over 8 ms p99 per 1080p frame, or 2160p can't hold rate (R2,
  R3). Then (b) becomes a v1.x ADR.

## E-006: Frame pacing
- **Context.** Output must run at exactly 60000/1001 (or 50, 30000/1001, 25) with no drift, and FILL and KEY must be one
  frame apart never. CEF #4311: external begin-frames carry `Now()` and a fixed 60 Hz interval and are dropped if one is
  already pending.
- **Options.** (a) Engine-wide clock driving all channels. (b) One clock per channel in the renderer, CEF driven by
  `send_external_begin_frame` (mode A). (c) One clock per channel, CEF free-running at `windowless_frame_rate`, the sender
  sampling the newest paint at each deadline (mode B, the CasparCG model). (d) NDI SDK clocking (`clock_video = true`).
- **Choice.** One integer clock per channel in the renderer (`T_k = epoch + floor(k·den·1e9/num)`, absolute-deadline sleeps
  plus a short spin), with mode A where S1 shows it clean and mode B elsewhere, chosen per OS and recorded in an ADR. SDK
  clocking off; explicit timecode on every frame (E-010).
- **Reason.** The clock lives where the pixels are, so no frame-rate data crosses a process boundary. Integer deadlines never
  drift. A gives one paint per output frame when it works; B always works but can supersede or repeat frames and needs
  counters. SDK clocking would give a second, competing pacing authority and blocks the send thread.
- **Pass/fail.** S1: 10 minutes per OS at 59.94 and at 50, measured at a real NDI receiver with a burned-in counter: no holes
  or doubles beyond the counted `repeated`, `fill_key_tc_mismatch = 0`.
- **Would change if.** CEF lands `SendExternalBeginFrameWithTiming` (#4311): then A everywhere after a re-run.

## E-007: NDI crate
- **Options.** (a) `grafton-ndi` 3.0.0 (Apache-2.0 AND MIT, runtime-loaded, no SDK needed at build on Win/macOS/Linux).
  (b) `ndi-sdk-sys` (GPL-3.0, no macOS). (c) `ndi-sdk` (no send or receive). (d) Our own `libloading` wrapper over the
  MIT-licensed headers and `NDIlib_v6_load`.
- **Choice.** (a), behind a `noplayout-ndi` trait (`NdiOutput`, `NdiReceiver`, runtime detection) so (d) can replace it.
- **Reason.** (a) is the only maintained crate with sending, receiving, BGRA/BGRX, explicit timecode, async submit with
  buffer reclaim, connection counts and a runtime search that matches our "detect, don't bundle" rule (exe dir → absolute
  `NDI_RUNTIME_DIR_V6` → platform paths; relative paths refused). I read these APIs in the 3.0.0 source. (b) is GPL, which
  the NDI licence and our licence rule out. (d) is about 1 kLOC we don't need yet.
- **Risk.** One maintainer (R4). Exit is (d), sized as one card.

## E-008: Where NDI is sent
- **Options.** (a) In the renderer, next to the pixels. (b) In the engine, with frames moved over shared memory (iceoryx2 or
  memmap2).
- **Choice.** (a).
- **Reason.** 1080p59.94 BGRA is about 497 MB/s per channel. Keeping it in-process avoids a shared-memory protocol, a copy
  and a second failure domain. A renderer crash takes down that channel's NDI anyway (it has no frames to send), so (b) buys
  no resilience. Only small JPEG monitor frames leave the renderer (E-013).
- **Would change if.** A future output needs frames from several channels at once (a multiviewer), or DeckLink output wants a
  separate process.

## E-009: Shaped vs unshaped defaults
- **Context.** CEF paints premultiplied BGRA. NDI documents BGRA as not premultiplied. Keyers differ on separate fill/key.
- **Choice (D-010).** Fill + Alpha: **unshaped** (straight) BGRA, un-premultiplied from CEF; Shaped available under Advanced
  for receivers set to premultiplied. Separate Fill & Key: **shaped** FILL (CEF's premultiplied RGB, i.e. composited over
  black) plus a KEY of R=G=B=A; Unshaped one click away.
- **Reason.** Fill + Alpha follows NDI's own convention, which receivers expect. For separate sources, a shaped fill is the
  traditional linear-key default (CasparCG's fill is shaped) and needs no division, so it can't amplify noise in
  low-alpha edges.
- **Gate.** M2 bench test on Caleb's switcher with the alpha ramp and antialiased text at its normal CG key setting. If
  Unshaped looks right there, the Separate default flips before M2 closes, recorded in this entry's ADR.

## E-010: NDI wire details
- **Choice.** BGRA (Fill + Alpha) or BGRX (FILL, KEY), never UYVA; `clock_video(false)`; `FrameRate::new(num, den)`;
  `Timecode::from_ticks(tc(k))` with the same value on FILL and KEY, never `SYNTHESIZE`; per-frame metadata
  `<noplayout k=… paint=…/>`; `submit_video` async with a ping-pong buffer pair per sender.
- **Reason.** UYVA loses alpha in OBS DistroAV (Robin). BGRA skips our own colour conversion. Explicit timecode lets
  receivers pair FILL and KEY and makes the loopback test exact; NDI's timecode synthesizer had drift fixed only in 6.4.
- **Cost.** BGRA is twice the bandwidth of UYVY on the wire; acceptable on a production LAN at ≤ 4 channels of 1080p.

## E-011: NDI runtime
- **Choice (D-010).** Detect the NDI 6 runtime at startup and on "Check again"; when it's missing, show the OS-specific
  install link from NDI and keep web outputs running. Never ship NDI binaries, NDI Tools or SDK docs; the MIT-licensed headers
  aren't needed either, since grafton-ndi loads the library itself.
- **Reason.** NDI's licence terms for redistribution need an NDI-approved flow (EULA flow-down, branding) that is simpler and
  safer as "the user installs NDI's runtime". It also keeps a public repo and its CI caches free of NDI binaries.

## E-012: IPC transport and encoding
- **Options.** gRPC, JSON over local sockets, postcard over local sockets, shared memory (iceoryx2 0.10.1), CEF process
  messages.
- **Choice.** `interprocess` 2.4.10 local sockets (named pipes on Windows, Unix domain sockets elsewhere), two per channel
  (control and frames), `u32` length prefix + postcard, protocol version in `Hello`.
- **Reason.** Small messages at low rates (heartbeat 500 ms, health 1 Hz, JPEGs ≈15 fps). No code generator and no extra
  runtime; postcard is compact and serde-native. Separate sockets keep a burst of monitor frames from delaying a heartbeat.
  Shared memory is unnecessary once full-rate pixels stay in the renderer (E-008).

## E-013: Monitor frames
- **Choice.** The renderer downscales to 480×270, composites onto a checkerboard, JPEG-encodes (quality 80) at about 15 fps,
  and sends with credit-based flow control (2 credits, newest wins). The engine relays to the UI over a binary WebSocket, one
  frame in flight per client.
- **Reason.** About 0.3 MB/s per monitor instead of 120 MB/s raw; the UI decodes JPEG natively. Credits stop a slow UI from
  backing up the renderer.
- **Gate.** S3 (4 channels × program + preview). Fails if monitor work costs a renderer more than 10 % of one core or adds
  any `slots_skipped`; then lower the rate or size.

## E-014: One CEF browser process per channel
- **Options.** One renderer process hosting all channels' browsers, or one per channel.
- **Choice.** One per channel (Robin D2), each with its own `root_cache_path`.
- **Reason.** Isolation: a hung GPU process or crashed browser affects one channel. Separate clocks without cross-channel
  scheduling. CEF's process-singleton lock rules out sharing a cache directory anyway.
- **Cost.** More memory per channel (an estimate of 150–300 MB, not yet measured; M6 measures it, R14).

## E-015: Cue preloads
- **Choice.** Selecting or cueing an item loads it in the preview browser and also preloads it into a hidden cue slot in the
  program page (`load` done, not visible). TAKE flips it visible and calls `playAction`.
- **Reason.** XPression operators expect TAKE to be instant after CUE. OGraf `load` may fetch assets and take time; doing it
  at cue time removes that from the take. Data edits made after the cue are sent with `updateAction(skipAnimation)` before
  play.
- **Cost.** One extra loaded instance per channel. A graphic that starts animating in `load` (non-conformant) would be hidden
  but running; the conformance harness flags that.

## E-016: Web fill and key
- **Options.** Separate fill/key graphics; canvas readback in the page; SVG/CSS filters on the layer root; WebGL pass.
- **Choice.** SVG `feColorMatrix` filters on the layer root (`color-interpolation-filters="sRGB"`): fill keeps RGB with A=1
  (unshaped) or composites over black (shaped); key outputs R=G=B=A with A=1.
- **Reason.** Same scene, same timing, no change to graphics (IDEA §4.2), and it works in OBS's and vMix's browsers.
- **Gate.** M3 pixel test: `/key` from CEF vs the NDI KEY within 2 LSB for a ≥ 16 (R8). Fallback: WebGL pass.

## E-017: One host runtime, program authoritative
- **Choice.** Every page (CEF program and preview, web outputs, editor) runs the same `ograf-host.js` with a role. Only the
  CEF program page's results answer API calls and drive engine state; other pages' results are logged and compared.
- **Reason.** One implementation of OGraf semantics (load-before-actions, ordering, `currentStep`, `stepCount: -1`) means web
  and NDI can't disagree by design. A single authority avoids races between several pages answering one call.

## E-018: How the engine drives pages
- **Options.** CEF process messages / `execute_java_script` from the renderer; a WebSocket from each page to the engine.
- **Choice.** WebSocket host protocol (`/ws/host/<ch>`), the same for CEF and browser pages.
- **Reason.** Web outputs need it anyway, so CEF pages use the same path and the renderer stays free of OGraf logic. The
  renderer can be restarted without the engine changing how it talks to pages.
- **Cost.** One loopback WebSocket hop per action; negligible against animation times.

## E-019: Crash recovery
- **Choice.** Each instance keeps an ordered action log (cap 256). A page that reconnects gets `load(dataAtLoad)` and the log
  replayed with `skipAnimation: true`, consecutive updates merged. Past the cap: `load(currentData)` +
  `playAction({goto: step, skipAnimation: true})` and a Health note.
- **Reason.** OGraf has no "get state / set state", so replay is the only spec-conformant way to rebuild a graphic's look,
  including custom actions. `skipAnimation` makes it land on the final state immediately.
- **Gate.** M6: 20/20 crash tests restore the same graphics and steps within 2 s at an NDI receiver.

## E-020: Settings store
- **Choice.** JSON with `schemaVersion`, validated in Rust (serde + `validate()`), atomic temp-write/fsync/rename, a
  last-good copy rewritten only after a clean load, migrations that back up first, `PUT /settings` all-or-nothing.
- **Reason.** Hand-editable, diffable, and IDEA §6's "never half-applied" rule. A truncated or bad file loads last-good and
  reports exactly what was rejected (Usopp's banner).

## E-021: Take IDs
- **Choice (D-005, D-010).** `u16` 1–9999 per rundown item, unique within a show, auto-incremented, inline error on
  duplicate. Uniqueness is enforced in the engine (API answers 409 naming the clashing item), not only in the UI. API routes
  `/api/v1/takeid/{n}/cue|take|next-step|out`; RossTalk `CUE` takes a Take ID (E-037).
- **Reason.** Companion and RossTalk callers bypass the UI, so the rule must live where the data lives.

## E-022: Decoder plug-ins
- **Options.** (a) Compiled-in crates behind a trait and a static registry. (b) Dynamic libraries loaded at runtime. (c) WASM
  plug-ins. (d) A scripting language (Lua/JS) for decoders.
- **Choice.** (a): one crate per decoder implementing `trait Decoder` (framing included), registered by one line and one cargo
  feature, with a shared conformance test that requires captures and a golden timeline for every registered decoder.
- **Reason.** D-011 wants each new decoder to be a self-contained change; (a) makes it exactly that (new crate + one registry
  line + one feature line + docs), checked in M4.5 by looking at the OES PR's diff. Rust has no stable ABI, so (b) is fragile
  across compiler versions; (c) and (d) add a runtime and a sandbox for a handful of decoders that ship with the product.
  Decoders run in `catch_unwind` so a bad one can't take the engine down.
- **Would change if.** Third parties need to ship decoders without rebuilding noPlayout; then (c).

## E-023: Real captures gate decoders; OES only in v1
- **Choice (D-011 + gate).** v1 ships one decoder, OES, ported from Caleb's MIT-licensed `unc-virtual-score-server`
  (framing STX / sport type / printable payload / CR; nine sport layouts; 9600 baud). A decoder is listed in the product only
  when a recorded byte stream from a real console decodes to a golden timeline Caleb has checked. Daktronics All Sport 5000
  RTD is next (D-010) and carries the same gate; the `daktronics-allsport-5000` 0.5.0 crate has no tests and skips the
  checksum, so if used, only its field tables are trusted.
- **Reason.** "No fake features": a decoder tested only against bytes we generated proves nothing about the console. OES has
  no checksum, so field-range and length checks are what flag corruption; a real capture is the only way to tune them.

## E-024: OGraf spec vendoring and offline validation
- **Choice.** Vendor `ebu/ograf` `v1/` (schemas, examples) at commit `945f8bb` with its MIT licence; validate manifests with
  `jsonschema` 0.63.2 with **default features off** (they include `resolve-http`) and only `resolve-file`.
- **Reason.** Upstream has no tags, so a commit is the only stable pin. A playout machine is often offline, and validation
  must never stall on, or leak to, the network.

## E-025: Edit-mode preview through CEF
- **Choice (D-010).** The editor canvas shows frames from a CEF renderer at editor resolution; Svelte draws handles over them.
- **Reason.** What the operator designs is what Chromium renders to air. The Tauri webview is WebKit on macOS and Linux,
  which can lay out text and filters differently.
- **Cost.** One extra renderer process while editing; frames reach the UI through the monitor path (E-013) at a higher rate
  for interaction.

## E-026: OGraf Server API mapping
- **Choice.** Renderer = channel; RenderTarget = `{ "layer": n }`; Server API calls act on playout directly, outside the
  rundown, and show in the On-air strip as "API: <graphic>". Spec bug O13 (`manifest` vs `metadata`) answered with both keys.
- **Reason.** Matches the spec's shapes at the pinned commit (RenderTarget must be a shallow object) and keeps third-party
  OGraf controllers working without knowing about rundowns.

## E-027: OpenAPI from code
- **Choice.** utoipa 6.0.0 + utoipa-axum 0.3.0, served at `/api/docs` with utoipa-scalar 0.4.0.
- **Reason.** The docs can't drift from the handlers. For `/ograf/v1`, a test checks our routes against the vendored
  `server-api.yaml` instead.

## E-028: Intel macOS
- **Choice (D-010).** Built and tested on GitHub's `macos-15-intel` runner until it's retired (about August 2027), then
  build-only (cross-compiled on Apple Silicon, smoke-tested under Rosetta), labelled "community-tested".

## E-029: Toolchain
- **Choice.** `rust-toolchain.toml` pins 1.100.0 (stable, 2026-11-09), edition 2024. MSRV floor from dependencies: sysinfo
  0.40.4 needs 1.95, tauri 2.12.7 needs 1.90.
- **Reason.** Reproducible builds across four CI lanes and workers' machines; bumps are deliberate PRs.

## E-030: JPEG encoder
- **Options.** `turbojpeg` (C libjpeg-turbo, fastest), `jpeg-encoder` 0.7.1 (pure Rust, SIMD feature), `image`.
- **Choice.** `jpeg-encoder` with `simd`.
- **Reason.** No C toolchain or CMake on any lane, and 480×270 frames are small enough that speed isn't the limit. Its IJG
  licence term needs the "based in part on the work of the Independent JPEG Group" line in NOTICE.
- **Would change if.** S3 shows encoding as a meaningful share of renderer CPU.

## E-031: serialport (MPL-2.0)
- **Choice.** Use `tokio-serial` 5.5.0 → `serialport` 4.10.1 unmodified.
- **Reason.** MPL-2.0 is file-level copyleft; using it unmodified in an Apache-2.0 product only obliges us to keep its source
  available, which crates.io does. It is in the engine, not the renderer that loads libndi. Linux CI installs `libudev-dev`.

## E-032: Licence gate
- **Choice.** `cargo deny check` on every push with an allowlist (Apache-2.0, MIT, BSD-2/3-Clause, ISC, 0BSD, Zlib,
  Unicode-3.0, BSL-1.0, MPL-2.0, IJG, plus whatever M0's first run shows is needed and reviewed). No GPL/AGPL/LGPL crates
  anywhere; nothing copyleft at all in `noplayout-renderer`, which loads libndi. Advisories checked nightly.
- **Reason.** Keeps the NDI licence conditions and "private later" (D-003) clean without manual audits.

## E-033: Engine death and restore
- **Choice.** Renderers exit 3 s after their control socket closes (no orphan CEF). The app restarts the engine; a
  supervised restart restores on-air state without asking. A cold start (machine or app relaunch) asks "Restore them?"
  (Enter = Restore), as IDEA §5.1 says.
- **Reason.** After a crash the operator wants air back; after a deliberate relaunch, the state may be stale.
- **Gate.** M6: engine restart → first restored NDI frame ≤ 5 s.

## E-034: Network exposure
- **Choice.** As IDEA §4.2: the HTTP port (and RossTalk when enabled) binds all interfaces with no token by default, so
  Companion and OBS on other machines work out of the box. Advanced offers localhost-only, LAN-only and a token. Internal
  routes (shutdown, supervisor) are loopback-only with per-launch tokens regardless. Every state change from a non-loopback
  address is logged with its source.
- **Reason and flag.** This matches how broadcast control protocols (RossTalk, XPression) work on production LANs, but it means
  anyone on that LAN can take or clear graphics (R18). It's IDEA's call, so it's not in the open questions; Caleb can flip the
  default to LAN-only + token at no design cost.

## E-035: ADRs
- **Choice.** On approval, each accepted entry here becomes `docs/adr/NNNN-<slug>.md` from the GeneralDocuments ADR template in
  M0; later decisions get an ADR in the same PR as the change. This file stays as the index.

## E-036: Repo licence
- **Choice (D-010).** Apache-2.0, with a NOTICE file.
- **Options considered.** Apache-2.0, MIT, dual MIT/Apache-2.0. Anything loading libndi must not be GPL.
- **Reason.** Explicit patent grant suits a video product; NOTICE holds the NDI trademark line ("NDI® is a registered
  trademark of Vizrt NDI AB"), the IJG credit, CEF/Chromium and OGraf notices. Compatible with every dependency in §3,
  including MPL-2.0 (E-031).

## E-037: RossTalk-compatible listener
- **Choice (D-010).** In v1 (M4), off by default, TCP 7788, CR/LF lines, XPression's command set mapped onto Take IDs,
  channels (framebuffer 0 = channel 1 on the wire) and layers; unknown commands logged, never fatal; same access scope as HTTP.
- **Reason.** Existing Carbonite CG-device presets and Companion's XPression module can drive noPlayout unchanged.

## E-038: Data-source storage and loss behaviour
- **Choice (D-009).** The show stores which sources it expects (key, name, decoder, layout) and every field mapping; the
  connection (port, host, interface) is stored per computer in that machine's settings. When a source goes quiet or is lost,
  every mapped field, including the game clock, holds its last value and the light turns amber "source lost"; no local
  free-running clock.
- **Reason.** A show moves between machines whose serial ports and addresses differ; mappings are show content. Holding is
  predictable and needs no clock model per decoder.

## E-039: TAKE onto an occupied layer
- **Options.** Animate the old graphic out first, or cut it.
- **Choice.** `dispose` the current instance (cut) and play the new one; OUT is the only animated exit; CLEAR is always a cut
  (D-007).
- **Reason.** Matches XPression's take-replaces-layer behaviour operators know, and keeps TAKE's timing independent of the
  outgoing graphic's out animation. An operator who wants the out animation presses OUT first.
- **Would change if.** Usopp or Caleb wants "animate out, then in"; it would be a per-item option, not a new default.
