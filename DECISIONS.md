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
| E-001 | Three process kinds: app, headless engine, one renderer per channel | Amended by E-040/E-041 (rev 2): four kinds |
| E-002 | Cargo workspace split; renderer does not link the engine core | Proposed |
| E-003 | UI framework: Tauri v2 (stable) + system webview | Proposed |
| E-004 | UI front end: Svelte 5 + TypeScript | Proposed |
| E-005 | CEF paint mode: CPU `on_paint`, shared texture off in v1 | Proposed |
| E-006 | Frame pacing: integer per-channel clock; drive mode chosen per OS by S1 | Spike-gated; clock moved to the output process (E-040), pass/fail replaced by E-048 |
| E-007 | NDI crate: grafton-ndi 3.0.0 behind our own trait | Proposed |
| E-008 | NDI sending happens inside the renderer process | **Superseded by E-040** |
| E-009 | Shaped vs unshaped defaults, per output mode | Decided (Caleb, D-010), bench-confirm in M2 |
| E-010 | NDI wire format BGRA/BGRX, explicit timecode, SDK clocking off | Proposed |
| E-011 | NDI runtime detected and linked, never bundled | Decided (Caleb, D-010) |
| E-012 | IPC: local sockets + postcard; no shared memory in v1 | **Superseded in part by E-042** (local sockets + postcard stay for control) |
| E-013 | Monitor frames: downscaled JPEG, credit flow control | Spike-gated (S3); program monitor now made in the output process |
| E-014 | One CEF browser process per channel | Amended by E-041: two per channel (active + standby) |
| E-015 | Cue preloads the graphic into a hidden slot | Proposed |
| E-016 | Web fill/key pages made from the same scene with SVG colour-matrix filters | Proposed |
| E-017 | One OGraf host runtime for every page; the CEF program page is authoritative | Amended by E-046 (authority bound to incarnation + token) |
| E-018 | Pages are driven over a WebSocket host protocol, not CEF process messages | Proposed |
| E-019 | Crash recovery by replaying each instance's action log with `skipAnimation` | **Superseded by E-041/E-043/E-044/E-045** |
| E-020 | Settings: JSON, atomic write, last-good, validation in Rust | Amended by E-050 (apply transaction) |
| E-021 | Take IDs: u16, unique per show, enforced in the engine | Decided (Caleb, D-005, D-010) |
| E-022 | Decoder plug-ins: compiled in, one crate per decoder | Proposed |
| E-023 | Decoders are gated on real captures; OES only in v1 | Decided (Caleb, D-011); **gate replaced by D-013** for OES |
| E-024 | OGraf spec vendored at a commit; manifest validation never uses the network | Proposed |
| E-025 | Edit-mode preview rendered by a CEF renderer | Decided (Caleb, D-010) |
| E-026 | OGraf Server API mapping: renderer = channel, RenderTarget = `{layer}` | Proposed |
| E-027 | OpenAPI generated from code with utoipa | Proposed |
| E-028 | Intel macOS: tested until GitHub retires the runner, then build-only | Decided (Caleb, D-010) |
| E-029 | Toolchain pinned to Rust 1.100.0, edition 2024 | Proposed |
| E-030 | Pure-Rust JPEG encoder | Proposed |
| E-031 | Accept `serialport` (MPL-2.0) | Proposed |
| E-032 | cargo-deny licence allowlist; nothing copyleft in the renderer | Proposed |
| E-033 | Engine death: renderers and output processes exit; supervised restart restores, cold start asks | Proposed (rev 2 wording) |
| E-034 | Network exposure default as IDEA says (open on the LAN), token optional | Proposed (flagged) |
| E-035 | ADRs in `docs/adr/`, one per accepted entry | Proposed |
| E-036 | Repo licence Apache-2.0 | Decided (Caleb, D-010) |
| E-037 | RossTalk-compatible listener in v1, off by default | Decided (Caleb, D-010) |
| E-038 | Data-source connections per computer, mappings per show; hold on loss | Decided (Caleb, D-009) |
| E-039 | TAKE onto an occupied layer replaces it with a cut | Proposed |
| E-040 | Per-channel output process owns the clock and the NDI senders | Proposed (rev 2), gated by S7a/S8 |
| E-041 | Warm standby renderer per channel, live-mirrored | Decided (Caleb, D-015 / Q1): on every channel; the 2 s guarantee is for a single failure with an exact standby. Eligibility amended by E-052 (rev 3) |
| E-042 | Shared-memory paint ring per renderer (iceoryx2 or own ring) | Spike-gated (S8) |
| E-043 | Failure kinds, detectors and the recovery measurement | Proposed (rev 2); K2g detector amended by E-057, K5 by E-055 (rev 3) |
| E-044 | Uncapped on-air journal; no compaction for third-party graphics | Proposed (rev 2) |
| E-045 | Time-dependent state: declared checkpoints, flags, no silent loss | Limitation decided (Caleb, D-015 / Q2); scope stated by E-052, checkpoint protocol by E-056 (rev 3) |
| E-046 | Identities, authority binding and ordered result commit | Proposed (rev 2) |
| E-047 | Server timestamps on the host protocol | Proposed (rev 2) |
| E-048 | Pacing acceptance by a pixel-only receiver-side oracle | Proposed (rev 2); time-base check replaced by E-053 (rev 3); Q3 decided (Caleb, D-015) |
| E-049 | Paint identity, single conversion, per-sender threads | Proposed (rev 2); stuck-sender handling replaced by E-055 (rev 3) |
| E-050 | Settings apply transaction with one owner and an effective revision | Proposed (rev 2); commit point and post-apply failures defined by E-054 (rev 3) |
| E-051 | Fallback completion status; required formats and platforms stay required | Proposed (rev 2); Q4 decided (Caleb, D-015): all formats stay required |
| E-052 | Recovery fidelity per replica copy; promotion eligibility; what the 2 s same-state guarantee covers | Proposed (rev 3), within Caleb's D-015 |
| E-053 | Pacing oracle time base in the source domain, plus a lag band | Proposed (rev 3), replaces E-048's `m` |
| E-054 | Settings: one commit point, defined outcome for every persistence and rollback failure | Proposed (rev 3) |
| E-055 | A non-returning NDI submit recycles the output process; slow submits keep the sender | Proposed (rev 3) |
| E-056 | Declared checkpoint protocol: quiescent cut, frontier seq, timed tail inside restore | Proposed (rev 3) |
| E-057 | PaintStall detected by a forced-repaint probe, not by rAF | Proposed (rev 3), spike-gated (S7a) |

---

## E-001: Process model
> **Rev 2, amended by E-040 and E-041.** Four process kinds: app, headless engine, and per channel one **output process**
> (clock, conversion, NDI) plus **two renderers** (active and warm standby). The reasons below for (c) still hold; rev 2 adds
> that output must survive a renderer failure.

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
> **Rev 2.** The clock moves to the output process (E-040); renderers compute the same deadlines from the epoch it publishes.
> The paint↔slot tagging is replaced by E-049, and the pass/fail line below is replaced by E-048's oracle thresholds
> (review 1 F4: "beyond the counted `repeated`" let counted defects pass). S1 is split into S1a (M1, paint level,
> provisional) and S1b (M2, receiver, final) (F10). The options and the mode A/B choice stand.

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
> **Superseded by E-040 (rev 2).** The reason below ("a renderer crash takes down that channel's NDI anyway, so (b) buys no
> resilience") stopped holding once recovery has to be ≤ 2 s with no output gap (review 1 F1): with NDI in a separate output
> process, the sources stay up and hold the last frame while a standby renderer takes over. Kept for the record.

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
> **Rev 2, superseded in part by E-042.** Control (heartbeat, configure, health, events) stays on `interprocess` local sockets
> with postcard, now one per process (output process and each renderer), with heartbeats at 100 ms. Monitor JPEGs keep a
> separate socket. Full-rate paints from renderer to output process go over the shared-memory ring of E-042, because NDI
> left the renderer (E-040).

- **Options.** gRPC, JSON over local sockets, postcard over local sockets, shared memory (iceoryx2 0.10.1), CEF process
  messages.
- **Choice.** `interprocess` 2.4.10 local sockets (named pipes on Windows, Unix domain sockets elsewhere), two per channel
  (control and frames), `u32` length prefix + postcard, protocol version in `Hello`.
- **Reason.** Small messages at low rates (heartbeat 500 ms, health 1 Hz, JPEGs ≈15 fps). No code generator and no extra
  runtime; postcard is compact and serde-native. Separate sockets keep a burst of monitor frames from delaying a heartbeat.
  Shared memory is unnecessary once full-rate pixels stay in the renderer (E-008).

## E-013: Monitor frames
> **Rev 2.** The program monitor is made in the output process from what actually goes to air; the preview monitor in the
> standby renderer. Method, rates, credits and the S3 gate are unchanged (S3 now also counts the standby's cost).

- **Choice.** The renderer downscales to 480×270, composites onto a checkerboard, JPEG-encodes (quality 80) at about 15 fps,
  and sends with credit-based flow control (2 credits, newest wins). The engine relays to the UI over a binary WebSocket, one
  frame in flight per client.
- **Reason.** About 0.3 MB/s per monitor instead of 120 MB/s raw; the UI decodes JPEG natively. Credits stop a slow UI from
  backing up the renderer.
- **Gate.** S3 (4 channels × program + preview). Fails if monitor work costs a renderer more than 10 % of one core or adds
  any `slots_skipped`; then lower the rate or size.

## E-014: One CEF browser process per channel
> **Rev 2, amended by E-041.** Two renderer processes per channel (active + standby), each with its own `root_cache_path`
> (`cef-cache/<channel>-<slot>/`). Isolation between channels is unchanged.

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
> **Rev 2, amended by E-046.** "Program" is now the program page of whichever renderer the engine made active. Authority is
> bound to a loopback connection, the renderer incarnation and its launch token, never to the role string; results commit
> in `seq` order per instance (PLAN §2.14).

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
> **Superseded by E-041, E-043, E-044 and E-045 (rev 2).** Review 1 F2 showed the 256-entry cap with a `goto` fallback loses
> real state: the reviewer ran the unmodified Basketball scorebug in Chromium with `homeScore: 10` and 257
> `addPoints(home, 1)` calls (score 267); emulating the capped fallback (`load(currentData)` + `goto: 0`) gives 10, because
> `addPoints` changes the graphic's own data and `customAction` returns only a status. Replaying `startClock` after a short
> wait lost 1,280.5 ms of elapsed clock time even without a cap. Merging updates is not provably equivalent for a
> third-party graphic either. Kept for the record.

- **Choice.** Each instance keeps an ordered action log (cap 256). A page that reconnects gets `load(dataAtLoad)` and the log
  replayed with `skipAnimation: true`, consecutive updates merged. Past the cap: `load(currentData)` +
  `playAction({goto: step, skipAnimation: true})` and a Health note.
- **Reason.** OGraf has no "get state / set state", so replay is the only spec-conformant way to rebuild a graphic's look,
  including custom actions. `skipAnimation` makes it land on the final state immediately.
- **Gate.** M6: 20/20 crash tests restore the same graphics and steps within 2 s at an NDI receiver.

## E-020: Settings store
> **Rev 2, amended by E-050.** "All-or-nothing" was only true for validation; applying a valid document can still fail
> halfway (review 1 F5). E-050 adds the apply transaction, the effective revision and the startup rules. "Last-good" now
> means last *applied*, not last loaded.

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
> **Rev 2: the OES gate is replaced by D-013.** M4.5 is done when the decoder passes a corpus ported from
> `unc-virtual-score-server` (test vectors plus simulator streams recorded once into captures), byte-for-byte over serial
> loopback, TCP and UDP. No venue recording is required. STATUS.md states D-013's limitation (the simulators share the
> decoder's protocol knowledge; a live console check is recommended at the first game, not a gate). The real-capture gate
> below still applies to later decoders (Daktronics etc.) unless Caleb decides otherwise for them.

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
> **Rev 2.** Output processes exit 3 s after their control socket closes, like renderers (holding the last frame meanwhile).
> A supervised engine restart cold-restores every channel from the persisted journal (E-044). An app crash with the engine
> still running reattaches and replays nothing. Gate unchanged (≤ 5 s, R17).

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

---

# Rev 2 entries (review 1)

## E-040: Per-channel output process owns the clock and NDI
- **Context.** Review 1 F1: with NDI inside the renderer (E-008), a renderer crash or hang takes the NDI sources down, and a
  2 s restart budget can't cover detection + CEF start + page load + replay + receiver reacquire. IDEA §9's crash test needs
  output back in the same state within 2 s.
- **Options.** (a) Keep E-008 and make cold restarts faster. (b) Move NDI into the engine, frames over shared memory.
  (c) A small per-channel **output process**: clock, paint selection, one conversion, NDI senders, program monitor.
- **Choice.** (c). Renderers write paints into a shared-memory ring (E-042); the output process reads the active renderer's
  ring at each slot and holds the last frame when none is fresh (PLAN §2.4a, §5).
- **Reason.** NDI sources stay up through any renderer failure, so receivers never reacquire and the outage is only the
  time to the first fresh paint. (b) would put per-frame real-time work and the NDI SDK into the control plane, so a stuck
  `submit_video` (F7) could stall every channel and the API. (a) cannot meet 2 s for a hang (detection alone eats most of
  it) and stays above 2 s for asset-heavy packages (est. 0.6–2.5 s, PLAN §2.13.5).
- **Cost.** One more process per channel and a shared-memory protocol; ≈ 497 MB/s per 1080p59.94 channel crosses a process
  boundary (one copy into the ring, which CEF's `on_paint` needs anyway). The output process holds no CEF and no graphics,
  so it is small; its own failure is K6.
- **Gate.** S8 (M1) for the ring; S7a (M1) for the warm path at paint level; S7b (M2) at a receiver.
- **Would change if.** S8 finds no ring that avoids blocking the writer on all three OSes; then the architecture is revisited
  before M2 (PLAN §2.13.5 gate).

## E-041: Warm standby renderer per channel
> **Rev 3.** Caleb decided Q1 as recommended (D-015): the standby stays on every channel, and ≤ 2 s same-state recovery is
> guaranteed for a single failure **when the standby is exact** (E-052). A standby rebuilt after a failover is still
> promoted (it always beats a cold restore), but it is exact only for what E-052 lists; the rest is flagged.
- **Context.** F1 + F2: a restart from scratch can't reach 2 s for a hung page or asset-heavy graphics, and replay cannot
  rebuild a third-party graphic's internal clock.
- **Options.** (a) Cold restart only. (b) A pre-started *empty* spare renderer (saves CEF start, still loads and replays).
  (c) A **warm standby**: a second renderer that runs the same show live (receives every action, renders at 1/6 rate,
  hosts the preview browser), promoted on failure.
- **Choice.** (c), on every channel by default. After promotion a new standby is cold-restored behind the new active.
- **Reason.** Promotion needs no load and no replay: the standby executed the same actions, so `addPoints` effects and
  running clocks are already there (within one frame of delivery skew). Estimated outage 0.15 s for a crash, 0.65 s for a
  hang (PLAN §2.13.5). (b) only removes the CEF start from the cold path.
- **Cost.** A second CEF per channel: est. 150–400 MB RAM and 15–30 % more CPU per channel (S7/M6 measure; R20). Non-
  deterministic graphics can diverge (R23, divergence check). A graphic bug that hangs both copies needs the cold path
  (K3b, R21).
- **Open.** Q1 — decided by Caleb (D-015): option (a), standby on every channel; cold path best effort.

## E-042: Shared-memory paint ring
- **Choice.** One 4-slot ring per renderer, slot headers `{state, inc, paint_seq, painted_at}` changed only by CAS; the writer
  never blocks and never overwrites a slot being read; the reader holds at most one slot (PLAN §5.3). Transport: iceoryx2
  0.10.1 if S8 shows it meets that with zero extra copies and no leaked shared memory after `kill -9` on all three OSes;
  otherwise our own ring over `shm_open` / pagefile-backed `CreateFileMappingW`, mapped with memmap2 0.9.12.
- **Reason.** Full-rate frames must cross a process boundary (E-040); local sockets would add a copy and backpressure on the
  CEF paint thread. iceoryx2's API has been version-checked only; it is read in source in M1 before anything depends on it.
- **Gate.** S8 (M1); R22.

## E-043: Failure kinds and the recovery measurement
> **Rev 3.** K2g's detector is the forced-repaint probe (E-057), not "no paint while rAF advances". K5 separates a slow
> submit from a non-returning one; the non-returning case recycles the output process (E-055).
- **Choice.** Nine failure rows K1–K7 (renderer exit, CEF child exit, GPU stall, page hang, poison action, UI-thread hang,
  sender stall, output-process failure, engine death), each with its own detector and budget (PLAN §2.13.1). Control
  heartbeat 100 ms, replacement at 500 ms of silence, inside IDEA's 500 ms / 2 s bounds. Outage = t1 − t0, t0 the harness's
  monotonic clock just before the fault is injected, t1 the arrival at an independent NDI receiver of the first frame whose
  pixels decode to the expected state and stay correct for 2 s. Per-stage budget with target and fail lines (§2.13.5).
- **Reason.** F1: "restart after 2 s of no Pong" plus any restore work exceeds 2 s by construction; separating kinds lets a
  process exit be handled in ≤ 20 ms and a hang in ≤ 500 ms. Failing over early is safe because promotion keeps all state.
- **Gate.** S7a (M1, paint level) with the Basketball on-air set on Windows, macOS arm64 and Linux: a miss of the warm-path
  fail line revisits the architecture before M2. M6: 20/20 K1 runs ≤ 2 s per reference machine.

## E-044: Uncapped on-air journal
- **Choice.** Every action dispatched to the authoritative page is journalled per instance with `seq`, `ts`, params and
  committed result; never capped, never silently truncated, persisted to `shows/<id>.onair.jsonl`. Failed/timed-out actions
  are journalled (the graphic saw them). No compaction for third-party graphics. Health warns above 50,000 entries with the
  estimated replay time; nothing is dropped. A poison action is quarantined and named on the On-air strip.
- **Reason.** F2: compaction or a cap is only safe where equivalence is established, and the engine can't establish it for
  a graphic it doesn't understand. At < 1 kB per entry and ≈ 1 ms replay per action (est., S7 measures), a whole game's
  scorebug journal stays well inside memory and a few seconds of replay.

## E-045: Time-dependent state
> **Rev 3.** Caleb accepted the limitation (D-015, Q2 option a): a third-party graphic's self-run state is exact on a
> failover only while the standby's copy is live, and it is flagged everywhere else. Review 2 N1 showed the limitation is
> not confined to double failures; E-052 states its real scope. "Every Designed graphic restores exactly" now rests on the
> checkpoint protocol in E-056 (consistent cut, timed tail), not on the rev 2 sketch below.
- **Choice.** Warm promotion keeps internal clocks (the standby ran them). On the cold path, a third-party graphic's internally
  run clock is **not restored** and the instance is flagged on the On-air strip and in Health ("elapsed time not restored").
  Graphics may declare `v_noplayout: {recovery: "checkpoint"}` (OGraf v1 allows `^v_` vendor keys) and implement
  checkpoint/restore custom actions; every M5 Designed graphic does, so noPlayout's own graphics restore exactly on every
  path. S9 tries DevTools virtual time for third-party clocks; adopted only if it returns to real time cleanly.
- **Reason.** OGraf v1 has no get/set-state or seek; review 1's probe lost 1,280.5 ms of a `startClock` clock on replay. Data-
  driven clocks (OES → `gameClock`) are durable input and restore exactly.
- **Open.** Q2 — decided by Caleb (D-015): option (a), accept + flag + S9, scope per E-052.

## E-046: Identities, authority and result order
- **Choice.** `engineRun` (UUIDv7), renderer incarnation `inc` (u64, increasing per channel), a 256-bit launch token per
  incarnation (env var, handed to the program page by `execute_java_script`), `(instanceId, generation)` per occupancy,
  `seq` per action. `program` registration requires loopback + current `inc` + matching token; superseded incarnations are
  refused (4410) and killed. Results commit per instance in `seq` order; results for a gone generation can't change state;
  replay results never commit. Promotion records a `seq` barrier. Snapshot-to-live handoff is atomic in the channel actor
  (PLAN §2.14).
- **Reason.** F6: with two replicas, reconnects and overlapping OGraf calls, "the program page's results are authoritative"
  is ambiguous without identities; a stale or foreign page must never be able to change on-air state.

## E-047: Server timestamps
- **Choice.** Every `Action` and journal entry carries `ts {utcUs, mono {run, us}, slot? {k, epoch}}`: UTC from one engine
  time source that never steps backwards within a run, engine monotonic time, and the channel's output slot (ties to NDI
  `tc(k)`). Informational, never used to schedule. `TimeSync` every 5 s; pages report `lagMs` (PLAN §4.3).
- **Reason.** F11: IDEA §4.2 requires server-timestamped actions. They also feed declared checkpoints, the stale-action rule at
  handoff, diagnostics and the elapsed-time flags.

## E-048: Pacing acceptance by a receiver-side oracle
> **Rev 3.** The time-base index `m` (computed in output-period units) is withdrawn: review 2 N2 showed an ideal
> 60 → 59.94 conversion gives 20 `Δm ≠ 1` transitions per 10,000 frames against 10 skips. E-053 replaces it with a
> source-domain step check and a lag band. Everything else here stands.
- **Choice.** A coded fixture draws its rAF counter `n`, a time-base index `m`, and an alpha-only copy of `n`; a separate
  decoder reads pixels only (never noPlayout's counters) at an NDI receiver (and on paint dumps in M1). Numeric thresholds per
  mode and rate (PLAN §5.5): mode A and mode B at 50/25 allow zero repeats, skips, tears, key mismatches and timecode errors;
  mode B at 59.94/29.97 allows exactly the declared 1000:1001 conversion skips (Δn = 2, ≥ 900 frames apart) and is never
  reported as zero-drop. Output counters must agree with the oracle. Eight injected faults must each fail the run.
- **Reason.** F4: rev 1's "no holes beyond the counted `repeated`" let a counted defect pass and trusted the sender's own tags.
- **Open.** Q3 — decided by Caleb (D-015): option (b), per-OS partial for the soak claim, decided after S1b.

## E-049: Paint identity, one conversion, per-sender threads
> **Rev 3.** "Recreate the sender at 2 s" is withdrawn: the sender thread owns the `Sender` while it is inside
> `submit_video`, and nothing can take it back. E-055 replaces it.
- **Choice.** Paints are named `(inc, paint_seq)`, slots `k`; nothing infers which begin-frame made a paint. The pacer converts
  the chosen paint once into an immutable FILL/KEY (or Fill+Alpha) snapshot with one `tc(k)`; each NDI sender has its own
  thread, three frames and a one-deep newest-wins queue; the pacer never calls the SDK. Progress monitor: `SenderStall` at
  250 ms, recreate at 2 s, output-process restart on a repeat within 10 s (K5).
- **Reason.** F7: rev 1 tagged paints by assumed begin-frame and let a blocking `submit_video` (possible even with clocking off,
  grafton-ndi 3.0.0 docs) delay the clock and the other plane.

## E-050: Settings apply transaction
> **Rev 3.** E-054 adds the commit point (the `settings.json` rename) and the outcome of every failure after resources
> were applied, including a failed rollback.
- **Choice.** One `settings` actor owns the document and an `effective_rev`; every writer uses `If-Match` (409 on stale).
  Apply = validate (422, nothing changes) → plan → write `settings.pending.json` → apply each step with an undo → commit
  (atomic rename to `settings.json`, then `last-good`), or roll back every applied step, move pending to `rejected/` and
  report the failed step (500/503). Last-good = last successfully applied. A crash mid-apply: startup moves pending to
  `rejected/` ("interrupted apply") and continues from the committed `settings.json`.
  Future schema versions are refused and never overwritten (PLAN §2.11).
- **Reason.** F5: validation alone doesn't make an apply all-or-nothing (a port or NDI name can fail after other changes
  took effect), and two writers could lose an update silently.
- **Gate.** The §2.11 fault matrix in M4.

## E-051: Fallback completion status
- **Choice.** Every fallback in the risk register has a status: in scope, partial (milestone stays open), stop (Franky reports
  to Caleb with numbers) or deferral (Caleb's written approval only, recorded as a D-nnn), with an evidence owner (PLAN §6.1).
  All eight IDEA formats and all three OSes stay required; a CI lane that can't run CEF needs a `preflight` evidence file at
  every milestone commit; reports list passed, skipped and unexercised checks separately (§6.2).
- **Reason.** F9: rev 1's fallbacks ("label it experimental", "local-only") could close a milestone with a requirement unmet.
- **Open.** Q4 — decided by Caleb (D-015): all formats stay required.

---

# Rev 3 entries (review 2)

## E-052: Recovery fidelity per replica copy and standby eligibility
- **Context.** Review 2 N1: rev 2 treated an alive, `Synced` standby as holding the same state. After the first failover
  the replacement standby has *replayed* the instances already on air, so a third-party graphic's self-run clock in it is
  short by the clock's age at rebuild. The same holds for web pages that join or reconnect mid-show.
- **Options.** (a) Track fidelity per page and instance, declare time exposure, flag with a bound, and state exactly when
  the same-state guarantee holds. (b) A third replica started with the first two (keeps a live copy through two failures;
  +1 browser per channel, and still fails at the third, so it moves the limit rather than removing it). (c) Refuse or
  warn on import for undeclared graphics (blocks valid OGraf graphics).
- **Choice.** (a), within Caleb's D-015 (Q1/Q2). Fidelity `live` / `checkpointed` / `replayed`, recorded by the engine
  from how it fed each page. Time exposure from `v_noplayout.time` (`"none"`, `{"selfTimedAfter": {"actions", "dataTrue"}}`)
  or `recovery: "checkpoint"`, in the manifest or a per-package `noplayout.recovery.json` sidecar (hashed into the graphic
  version); undeclared graphics are self-timed from `load`. Shortfall bound `L = t_rebuild − t_expose`, fixed at rebuild.
  Standby states Exact / Ready-inexact / Not ready, shown before failure. Exactness returns only by a new generation
  (re-take) or a checkpoint; never by the divergence check. An inexact standby is still promoted. PLAN §2.13.3a.
- **Reason.** It makes the approved limitation visible and bounded, keeps every valid OGraf graphic usable, and puts the
  operator's remedy (re-take in a break, or feed the clock from data) in front of them before the second failure.
- **Gate.** S7a kill → rebuild → second-kill cases; M3 web late-join clock test; R24.

## E-053: Pacing oracle time base in the source domain
- **Context.** Review 2 N2: rev 2's `m` was computed in output periods, so a correct 60 → 59.94 conversion fails its row.
- **Choice.** The fixture draws its rAF timestamp `s` (µs). The decoder checks the median source period, a per-step check
  `|Δs − Δn·P̂_src| ≤ P̂_src/4`, and a lag band `max − min of (tc − s) ≤ 1.25·P̂_src` (mode A: ≤ `P_out`/4). S1 may widen
  tolerances to measured jitter, never past a 1.5·`P̂_src` band.
- **Reason.** An exact-rational model (`~/OrgWork/projects/noplayout/design/oracle_model.py`) gives the ideal conversion
  10 skips, 0 step errors and a 16.67 ms band at every phase, against 20 failures for rev 2's rule, and both mutations
  (frozen time base, repeated paint) fail. PLAN §5.5.

## E-054: Settings commit point and post-apply failures
- **Context.** Review 2 N3: rev 2 did not say what happens when persistence fails after resources were applied.
- **Choice.** The `settings.json` rename is the commit point; pending stores the candidate's sha256; old resources are held
  until commit. Before the commit point every failure undoes all resources and reports `503`; after it the change stands
  and failures (directory fsync, last-good, pending delete) come back as `200` with warnings and Health amber. A failed undo
  leaves a reported **degraded** state, with the running resource set tracked separately from the committed document.
  Startup tells a committed leftover (hash match) from an interrupted apply. PLAN §2.11 failure table.
- **Reason.** Every outcome is one row the tests can assert: API view, running resources, files and next-start result.

## E-055: A non-returning NDI submit recycles the output process
- **Context.** Review 2 N4: "recreate the sender at 2 s" had no path; the sender thread owns the `Sender` inside
  `submit_video`, and a thread blocked in a foreign call can't be cancelled.
- **Options.** (a) Quarantine the stuck thread and start a second same-name sender in-process. (b) Recycle the output
  process. (c) Wait forever.
- **Choice.** (b) for stuck (≥ 2 s); nothing recreated for slow (250 ms – 2 s). Loop guard with backoff; channel red after
  3 recycles in 5 min, retries continue. (a) is tried once in S7b for information only.
- **Reason.** Process exit is the only termination the OS guarantees and frees the sender, frames and SDK state at once;
  the renderers are untouched. Cost stated: both planes drop, *est.* 1–3 s receiver reconnect on top of the ≥ 2 s, new
  timecode epoch. (a) depends on undocumented SDK behaviour and leaks the wedged state for the process lifetime.

## E-056: Declared checkpoint protocol and the Designed time model
- **Context.** Review 2 N5: rev 2 promised exact Designed clocks without a consistent cut or a time-faithful tail.
- **Choice.** The host takes a checkpoint only when the instance is quiescent (all its dispatched actions resolved), never
  delaying an on-air action; the graphic captures state synchronously; the result carries `frontierSeq` and `cutTs`.
  Restore = `load` → `v_noplayout_restore{state, cutTs, nowTs}` → tail `seq > frontierSeq` with original `ts` and
  `replay: true` → live. Checkpointing graphics compute time state from action `ts` and `serverNow()`, not
  `performance.now()` deltas; every Designed graphic does so through noPlayout's runtime library, so its replay is
  time-faithful by construction. A bad checkpoint falls back to the full journal (copy becomes `replayed`, flagged).
- **Reason.** A consistent cut plus a time model that does not depend on when an action is executed is what makes "exact"
  provable; M5's property test and the second-kill test check it. PLAN §2.13.4.

## E-057: PaintStall detected by a forced-repaint probe
- **Context.** Review 2 N6: "no paint while rAF advances" cannot tell a static scene from a stalled compositor; a static
  scene has no damage and no paint, and rAF says nothing about pixels.
- **Choice.** After 100 ms without a paint the renderer calls `BrowserHost::invalidate(PaintElementType::default())` and
  records the probe in the ring header; the output process raises `PaintStall` if no paint newer than the probe arrives
  within 250 ms (≤ 400 ms from the last paint). rAF is diagnostics only. Loop guard on repeat stalls after promotion.
  Cost: one full-view repaint per 100 ms on a fully static scene (≈ 83 MB/s at 1080p).
- **Reason.** A healthy page answers an invalidate whatever its content, a stalled one cannot, and the judge is the reader of
  the ring, not the renderer. `invalidate` and `was_resized` exist in the cef 156.7.0 bindings for every target built;
  behaviour under external begin-frame is the S7a check (R25).
