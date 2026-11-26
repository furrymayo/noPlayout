eyline: an OGraf graphics playout engine

Working title: Keyline. Rename freely.

1. What we are building

A free, cross-platform (Windows / macOS / Linux) broadcast graphics engine written in Rust. It is built on the EBU OGraf standard: every graphic it makes or plays is an OGraf v1 graphic.

The app has two modes, switched with one toggle in the top bar, in the same spirit as Ross XPression:

Edit: build and modify graphics, then save them as OGraf packages.
Playout: run a show. Build a rundown, fill in data, then take graphics on and off air.

Before anything else, the user configures outputs. Version 1 supports two output types, each with full key/fill:

Web output: browser-renderable URLs, the same idea as Singular.Live output URLs, for OBS Browser Source, vMix, CasparCG, etc.
NDI output: NDI video sources on the network.
Product principles (these decide every UI argument)
Calm by default. An operator should be able to run a basketball game without opening a settings dialog. Advanced options exist, but they live behind an "Advanced" disclosure, never on the main screens.
Stable over clever. Settings are validated, versioned and recoverable. A renderer crash must never take down the UI or lose the rundown.
What you see is what goes out. Program monitors show the real output frames, not a re-render.
Standards first. Graphics are plain OGraf packages, usable in other OGraf renderers. The control API follows the OGraf Server API where one exists.
2. Background the builder must know
OGraf
Spec home: https://ograf.ebu.io and the repo https://github.com/ebu/ograf. Use v1, which is stable.
A graphic is a folder (often zipped) containing:
*.ograf.json, the manifest: id, name, main, stepCount, schema (data fields using GDD types), customActions, supportsRealTime, supportsNonRealTime, renderRequirements, thumbnails, etc.
an ES module (main) whose default export is a class extending HTMLElement (a web component).
The renderer creates the element and calls these methods:
load({ data, renderType, renderCharacteristics })
playAction({ goto?, delta?, skipAnimation? }), which returns { currentStep }
stopAction({ skipAnimation? })
updateAction({ data, skipAnimation? })
customAction({ id, payload, skipAnimation? })
dispose()
optionally goToTime and setActionsSchedule (non-real-time)
JSON schemas to validate manifests live in the spec repo at v1/specification/json-schemas/.
Reference packages for testing:
The EBU examples in v1/examples/.
The Basketball OGraf Package (6 graphics: scorebug with clock, lower third, 2-step matchup/lineups, stat card, ticker, sponsor bug). It is provided alongside this file. It must work unmodified.
Why we need a real browser engine

OGraf graphics are HTML/JS/CSS. To get frames with alpha for NDI, we must render HTML offscreen at the video frame rate and read back BGRA pixels. The plan is CEF (Chromium Embedded Framework) through the Rust cef crate (https://github.com/tauri-apps/cef-rs), using offscreen rendering (OSR) with a transparent background. It supports Linux, macOS and Windows on x86_64 and ARM64. System webviews (WebView2, WKWebView, WebKitGTK) cannot deliver alpha frames reliably, so they are not used for output rendering.

NDI
Use the NDI 6 SDK through a Rust binding: grafton-ndi, or ndi-sdk-sys / ndi-sdk (the last one loads the library dynamically). Verify each crate's current API and maintenance status before choosing, and record the choice in DECISIONS.md.
NDI video can carry alpha (BGRA / UYVA), so a single NDI source can carry fill and key together. Many switchers prefer discrete key and fill, so we also offer two separate sources.
Licensing: the NDI SDK is royalty-free but has distribution and branding requirements. Load the NDI runtime dynamically. If the runtime is missing, show a clear "Install NDI Runtime" message and link, rather than crashing. Read https://docs.ndi.video/all/developing-with-ndi/sdk/licensing before shipping.
3. Architecture
┌──────────────────────────── Keyline (one install) ─────────────────────────────┐
│                                                                                │
│  UI shell (Tauri v2 app: Rust backend + TypeScript/Svelte frontend)            │
│    Edit mode · Playout mode · Output setup · Settings                          │
│            │  (local WebSocket + REST, same API external controllers use)     │
│            ▼                                                                   │
│  keyline-core (Rust library, owned by the "engine" process)                    │
│    ├─ Graphics library   (import/validate/store OGraf packages)                │
│    ├─ Project / rundown  (shows, items, data, autosave)                        │
│    ├─ Playout state      (channels → layers → graphic instances, step, data)   │
│    ├─ Output manager     (web outputs, NDI outputs, health)                    │
│    ├─ Control API        (OGraf Server API REST + WebSocket events)            │
│    ├─ Settings store     (versioned, validated, atomic, last-known-good)       │
│    └─ Data sources       (v1: manual + API; later: serial scoreboard, JSON)    │
│            │                                                                   │
│            ▼  (IPC: commands down, frames + status up)                         │
│  keyline-renderer (separate process per channel, CEF offscreen)                │
│    Hosts renderer.html → OGraf host runtime → layers of web components         │
│    Paints BGRA frames at channel frame rate → NDI senders / preview frames     │
│                                                                                │
│  Built-in HTTP server: serves graphics, renderer pages, web outputs, UI assets │
└────────────────────────────────────────────────────────────────────────────────┘
Key decisions (the builder may challenge these, with reasons recorded in DECISIONS.md)
Rust everywhere except page content. The core, renderer host, outputs, API and settings are Rust. UI screens and graphics are web tech, because OGraf graphics are web components and the editor has to display them natively.
The renderer runs in its own process per channel. If CEF crashes, a watchdog restarts it within 2 s. The engine then re-sends the current playout state (load, current step, latest data), so graphics come back on their last step.
There is one OGraf host runtime (renderer.html + ograf-host.js). It is used by the CEF renderer, by web outputs and by the editor preview, so behaviour is identical everywhere.
Channels. A channel is one video format (resolution plus frame rate) with an ordered layer stack. Each layer holds at most one graphic instance. Outputs attach to channels. v1 supports at least 2 channels.
Frame pacing.
Each channel has a master clock at its frame rate (e.g. 59.94 = 60000/1001).
CEF OSR runs at the channel's rate. Investigate CEF external begin-frame control for frame-accurate rendering, and use it if it works on all three OSes.
NDI sends are clocked. Frames carry timecode.
Count dropped or duplicated frames and show them in the health panel.
4. Outputs and key/fill
4.1 Output setup (first launch, then Settings → Outputs)

This is a short wizard: Format → Channels → Outputs → Test.

Format: one dropdown. Choices: 1080p59.94 (default), 1080p50, 1080p29.97, 1080p25, 720p59.94, 720p50, 2160p29.97, 2160p25. Interlaced formats are out of scope for v1.
Channels: default 1, add up to 4. Each channel gets a name (e.g. "Main", "Video Board").
Outputs per channel, each a checkbox:
Web output: shows copyable URLs (see 4.2) and a port setting (default 8686) under Advanced.
NDI output: source name (default KEYLINE <channel name>) plus a mode selector:
Fill + Alpha (one NDI source with alpha). This is the default.
Separate Fill & Key (two NDI sources: <name> FILL and <name> KEY).
Advanced: fill type Shaped (premultiplied) / Unshaped (straight), with a short explanation of each. Pick the default after checking NDI's documented alpha convention, record it in DECISIONS.md, and put a hint in the UI ("Choose Unshaped if edges look dark on your switcher").
Test: a "Send test pattern" button outputs bars plus an alpha ramp and a moving box on every enabled output. Each output shows a status light: green = sending, amber = no receivers / waiting, red = error with a one-line reason.
4.2 Web output

The built-in HTTP server serves, for each channel:

URL	What it is
/out/<channel>/program	Graphics on a transparent background. For OBS / vMix browser sources (alpha comes through automatically).
/out/<channel>/fill	Fill only (same as program; shaped or unshaped per setting, see below).
/out/<channel>/key	The key (matte): white where graphics are, black elsewhere, with linear alpha preserved as grey levels. Render the same scene and derive the matte with a CSS/WebGL pass; do not build a separate graphic.
/out/<channel>/preview	Preview channel (next item cued), same formats.

How the web output works:

Web output pages are thin clients running the same OGraf host runtime. They subscribe to the channel's playout state over WebSocket and mirror every action (load, play, stop, update, custom actions), with server timestamps.
A page that joins late, or reconnects, receives a snapshot (loaded graphics, current steps, data) and appears in the correct state with skipAnimation: true.
Limitation (show this in the UI): separate browser clients are not frame-locked to each other. If someone needs perfectly locked key and fill, recommend NDI "Separate Fill & Key", which is generated from the same rendered frame.
Authentication: by default it listens on all interfaces with no auth. Under Advanced, add a "LAN only / localhost only" choice and an optional access token parameter.
4.3 NDI output
Frames come from the channel's CEF renderer as BGRA.
Fill + Alpha: send BGRA (or UYVA) on one source.
Separate Fill & Key: from the same frame, produce a FILL frame (opaque; colour per shaped/unshaped setting) and a KEY frame (alpha as luminance, opaque). Send both with identical timecode. Do the conversion in SIMD or on the GPU if needed to hold frame rate.
Report the number of NDI receivers connected to each source in the status panel.
5. Modes
5.1 Playout mode (operator)

The layout:

┌ Top bar: [EDIT | PLAYOUT]  Show name  · channel health lights · clock ───────────┐
├─────────────────────┬──────────────────────────────┬────────────────────────────┤
│ Rundown             │  PREVIEW monitor             │  PROGRAM monitor           │
│  ▸ 1 Scorebug       │  (next item, real render)    │  (real output frames)      │
│  ▸ 2 L3 – #23 …     │                              │                            │
│  ▸ 3 Matchup        ├──────────────────────────────┴────────────────────────────┤
│  …                  │  Item data panel (form generated from the OGraf schema)   │
│ [+ add graphic]     │  Custom actions as buttons · Layers on air (with Stop)    │
└─────────────────────┴───────────────────────────────────────────────────────────┘
   Transport: [TAKE ▶] [NEXT STEP ⏭] [OUT ■] [CLEAR LAYER] [CLEAR ALL]   hotkeys shown
Rundown. Items are a graphic plus a saved data set plus a target (channel and layer). Items can be reordered by dragging, duplicated, and grouped (e.g. "Pregame", "1st Half").
Workflow. Select an item to cue it on Preview. TAKE plays it on Program. NEXT STEP advances its steps (playAction with delta 1). OUT stops it.
Live data editing. Editing fields on an on-air item sends updateAction. That happens immediately, or on "Update" if the item's live-update toggle is off.
Custom actions come from the manifest. Their payload forms are generated from each action's schema.
Hotkeys:
Space = TAKE
Right arrow = NEXT STEP
Esc = OUT
Up/Down = select the previous/next rundown item
The hotkey map is configurable.
External control:
Implement the OGraf Server API (see the spec repo) for graphics, renderers, targets and actions.
Add Keyline endpoints for the rundown: take, next, out, cue <item>, update <item> data. These make Bitfocus Companion / Stream Deck integration trivial.
Publish the OpenAPI document at /api/docs.
Autosave the show every change (debounced). Reopening the app restores the rundown and data. On-air state is restored only if the user confirms ("Graphics were on air when Keyline closed. Restore them?").
5.2 Edit mode (designer)

The goal is to let a non-coder build typical broadcast graphics and export them as valid OGraf packages.

Library panel:
All graphics in the project.
Import OGraf packages (zip or folder): validate them against the v1 JSON schema and show clear errors.
Imported code-based graphics are "Coded": you can edit default data values, the name and the thumbnail, but not the design.
Graphics built in Keyline are "Designed" and fully editable.
Canvas:
Fixed to the format resolution and scaled to fit.
Toggleable action-safe and title-safe guides.
Snapping and a grid.
Optional background reference image or video still.
Elements: rectangle (fill, gradient, border, corner radius), text (font, size, weight, tracking, alignment, auto-shrink to fit a box), image, group/mask, and data-bound text/image. A clock element (count up/down, format M:SS / SS.t, driven by data or internal) is needed for scorebugs.
Data fields panel: define fields (text, number, boolean, colour, image, select, list) with defaults. Bind element properties to fields. These become the OGraf schema with GDD types.
Animation timeline:
Tracks per element, keyframes with easing presets.
Named segments: In, one segment per Step (→ stepCount), and Out.
An optional Update transition (e.g. flash a box when a bound value changes).
Preview playback with a scrubber.
Custom actions: let the designer expose a named action that jumps to a timeline segment or sets a field (e.g. "Show message").
Export:
Writes a self-contained OGraf v1 package (manifest, a generated graphic.mjs with no external dependencies, assets, thumbnail rendered by the engine).
Validates it against the official schema before saving.
Exported graphics must play correctly in a third-party OGraf renderer (test with SuperFly's ograf-server).
Fonts: bundle the fonts used by a graphic into its package (respecting font licences; warn if a font cannot be embedded).
6. Settings, stability and recovery
Settings and shows are stored as versioned JSON or TOML, with a schema, in the OS-appropriate app-data folder. Use the directories crate.
Atomic writes: write to a temp file, fsync, then rename. Keep settings.last-good.json. On a parse or validation failure at startup, load last-good and tell the user what was rejected.
Migrations: when the settings version changes, migrate forward automatically and back up the old file first.
Validation happens in Rust (serde plus explicit validation) before applying. The UI shows inline errors and never applies a half-valid config.
Live changes: changing format or NDI mode on a channel with graphics on air asks for confirmation, then restarts only that channel's renderer.
Watchdogs: a renderer heartbeat runs every 500 ms. If it is missed for 2 s, the renderer is restarted and its state re-sent. Log the incident.
Logs: use rolling log files (tracing + tracing-appender), a "Copy diagnostics" button (versions, GPU, settings without secrets, last 500 log lines), and a health panel (fps, dropped frames, NDI receiver counts, renderer memory).
Port conflicts: if the web port is taken, say so plainly and offer the next free port.
7. Data sources (designed now, built later)

The design includes a DataSource trait: a source produces partial data updates mapped onto rundown items or bound fields. v1 ships these sources:

Manual (the UI)
The HTTP API

The design must make the following easy to add next:

Serial scoreboard (Daktronics All Sport 5000 RTD and others) via the serialport crate, with a field-mapping UI (e.g. scoreboard "Main Clock" → scorebug gameClock). An MIT-licensed reference decoder exists: https://github.com/zabackary/daktronics-allsport-5000-rs.
A JSON/HTTP poll, and CSV or Google Sheets.
8. Non-goals for v1

SDI/DeckLink output, audio, video clip playback, 3D, multi-user collaborative editing, cloud hosting, interlaced formats, and non-real-time rendering (goToTime). Keep the output manager abstract so DeckLink can be added later as another output type.

9. Quality bar and tests
CI: GitHub Actions matrix: windows-latest, macos-latest (arm64) plus macOS x86_64, and ubuntu-latest. cargo build --release, cargo test, cargo clippy -D warnings and cargo fmt --check must pass on all of them.
OGraf conformance tests:
Load every graphic in the EBU examples and the Basketball package.
Run load → play → (steps) → update → custom actions → stop → dispose with no console errors.
Assert the return payloads.
Pixel tests:
Render known frames offscreen and compare them to golden PNGs within a tolerance.
Include an alpha-ramp test that proves the key output equals the fill's alpha, and that shaped/unshaped behave as documented.
NDI loopback test (when the runtime is present): send, receive with the NDI receiver API in the test, and check the resolution, frame rate, alpha and timecode match between FILL and KEY.
Soak test: a 2-hour run at 1080p59.94 with the scorebug clock running and the ticker cycling. It should drop no frames on reference hardware (Apple M-series Mac mini, and a mid-range Windows PC with a discrete GPU), and memory must stay flat.
Crash test: kill the renderer process mid-show. Output must recover to the same state within 2 s.
10. Packaging
Bundle CEF with the cef-rs tooling (export-cef-dir, bundle-cef-app).
Windows: MSI or NSIS installer.
macOS: signed and notarized .app / .dmg (document the signing steps; unsigned dev builds are fine before release).
Linux: AppImage and .deb.
The NDI runtime is detected at runtime, not bundled, unless the licence review says otherwise.
First launch opens the Output Setup wizard.
11. Milestones (build in this order; each ends runnable)
#	Milestone	Done when
M0	Workspace skeleton (keyline-core, keyline-renderer, keyline-app), CI green on all OSes	cargo run opens an empty app window on Win/Mac/Linux
M1	OGraf host runtime + CEF offscreen renderer	The Basketball scorebug renders and animates offscreen; frames saved as PNG show correct alpha
M2	NDI output: Fill+Alpha and Separate Fill & Key, test pattern, health	The NDI Studio Monitor / OBS shows Keyline sources; key and fill line up
M3	Web outputs: program/fill/key/preview URLs, state sync, late-join snapshot	OBS Browser Source plays the scorebug driven from the API
M4	Playout mode + settings store + Output Setup wizard + OGraf Server API	A full basketball show can be run from the UI and from HTTP calls
M5	Edit mode + OGraf export	A lower third designed in Keyline exports, validates, and plays in Keyline and in SuperFly ograf-server
M6	Hardening: watchdog, recovery, soak, packaging/installers	All tests in §9 pass; installers are produced for all three OSes
