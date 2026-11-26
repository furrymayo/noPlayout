# noPlayout: build plan

- **Status:** design draft rev 1 for cold review (Codex gpt-6.1-sol), then Caleb's approval. Nothing here is built yet.
- **Author:** Franky (CTO), card t_95b8a2c9, 2026-11-26. Branch `design/plan` (local only).
- **Inputs:** `IDEA.md` (source of truth), `docs/BUILD-BRIEF.md`, `docs/01-decisions.md` D-001…D-011 (D-009 UI answers,
  D-010 research picks, D-011 OES-only decoder are binding here), Robin's
  `2026-11-26-noplayout-foundations.md` + addendum, Robin's `2026-10-30-replayserver-ndi-license.md`, Usopp's `UI.md` rev 2
  (`~/OrgWork/projects/noplayout/design/UI.md`, mockups approved by D-007).
- **Companion file:** `DECISIONS.md` (engineering decision record, entries E-001…). Where this plan says "E-0nn", the reasoning is there.

Every version below was checked against crates.io / npm on 2026-11-26, and the APIs this plan depends on were read in the
published crate source at exactly that version (see §3, "Verified"). Nothing was compiled or run yet: frame pacing, readback
cost and IPC cost are still design inputs. Spikes S1–S6 measure them before anything is built on top.

---

## 1. Summary

noPlayout is three kinds of process on one machine:

1. **`noplayout-app`**: the Tauri v2 + Svelte window. It holds no show state. It starts and watches the engine and talks to it
   through the same HTTP/WebSocket API that Companion uses.
2. **`noplayout-engine`**: headless Rust. It owns the graphics library, shows and rundowns (with Take IDs), playout state,
   settings, data sources, the HTTP server (API, web outputs, graphics files) and the renderer supervisor.
3. **`noplayout-renderer`**, one per channel: CEF offscreen. It hosts the program page and the preview page, runs the
   channel's 60000/1001-style clock, converts pixels and sends NDI itself. Only small JPEG monitor frames leave the process.

All pages (the CEF program and preview pages, web outputs, the editor preview) run one OGraf host runtime, `ograf-host.js`,
and receive the same ordered action stream from the engine over a WebSocket. The CEF program page is the authoritative
instance whose results answer API calls. Renderer crashes are recovered by replaying each instance's action log with
`skipAnimation: true`.

Build order is IDEA §11 plus one milestone: **M4.5 Data sources** (D-008), between Playout (M4) and Edit (M5).

---

## 2. Architecture as it will be built

### 2.1 Processes

```
┌ noplayout-app (Tauri v2, one per desktop session) ─────────────────────────────────────┐
│ Svelte UI · supervises engine (spawn, restart, reattach) · file dialogs · OS hotkey info │
└──────────────┬──────────────────────────────────────────────────────────────────────────┘
               │ HTTP REST + WebSocket on 127.0.0.1:<port> with the internal UI token (E-034)
┌──────────────▼ noplayout-engine (exactly one, single-instance lock) ───────────────────┐
│ library · shows/rundown · playout state machine · settings · data sources              │
│ HTTP server (axum): /api/v1 · /ograf/v1 (Server API) · /out/* · /graphics/* · /host/*  │
│ WS: /ws/events · /ws/host/<ch> · /ws/monitor/<ch>/<src> · renderer supervisor/watchdog │
└──────┬──────────────────────────┬───────────────────────────────────────────────────────┘
       │ local socket ×2 per      │ HTTP + WS (host protocol) over loopback
       │ channel (control, frames)│
┌──────▼ noplayout-renderer --channel <id> (one per channel) ───────────────────────────┐
│ CEF browser process: program browser (channel res, clock-driven, alpha)               │
│                      preview browser (channel res, 30 fps max, monitor only)           │
│ channel clock thread → frame mailbox → convert (unpremul / key) → NDI senders          │
│ monitor tap → downscale 480×270 → JPEG → frames socket                                 │
│ CEF helper subprocesses (renderer/GPU/utility) = the same executable                   │
└────────────────────────────────────────────────────────────────────────────────────────┘
External: OBS/vMix/CasparCG browser sources → /out/*; Companion/Stream Deck → /api/v1, /ograf/v1 and the RossTalk-compatible
listener on 7788 (off by default, D-010); NDI receivers ← renderer senders; scoreboards → engine data-source transports.
```

Why the engine is its own process (E-001): a UI crash or a webview hang never drops air, the engine can run headless on a
rack PC (controlled from Companion), and the core can be tested in CI without Tauri. The app spawns the engine if none is
running, or attaches to the running one. Quitting the app asks the engine to shut down gracefully. If the app *crashes*,
the engine keeps running and the next app launch reattaches without a prompt.

Engine death (E-033): renderers lose their control socket and exit after a 3 s grace period (no orphan CEF processes). The
app's supervisor restarts the engine. A **supervised restart** restores on-air state automatically, like the renderer
watchdog. A **cold start** (machine or app relaunch) asks first, as IDEA §5.1 says ("Restore them?", Enter = Restore).
Failure condition for this design: engine restart to first restored NDI frame > 5 s (measured in M6).

### 2.2 Workspace layout (E-002)

```
noPlayout/
├─ Cargo.toml                 workspace; [workspace.dependencies] carries every pin in §3
├─ rust-toolchain.toml        1.100.0, edition 2024 (E-029)
├─ deny.toml                  cargo-deny licence + advisory gate (E-032)
├─ crates/
│  ├─ noplayout-core/         lib + [[bin]] noplayout-engine (D-001 name kept for the library)
│  ├─ noplayout-renderer/     bin: CEF host, clock, NDI sending (D-001)
│  ├─ noplayout-app/          bin: Tauri v2 shell (D-001)
│  ├─ noplayout-proto/        lib: IPC + host-WS message types, versioned; no heavy deps
│  ├─ noplayout-ndi/          lib: NdiOutput/NdiReceiver traits + grafton-ndi impl, runtime detection
│  ├─ noplayout-pixels/       lib: unpremultiply, key, shaped/unshaped fill, monitor scale + JPEG, test pattern math
│  ├─ noplayout-datasource/   lib + [[bin]] noplayout-capture: transports, framework, capture/replay format
│  ├─ noplayout-decoder-oes/  lib: the OES decoder plug-in (one crate per decoder, E-022)
│  └─ xtask/                  cargo xtask: fetch CEF, dev-bundle the renderer (macOS), run, package
├─ web/
│  ├─ host/                   TypeScript: ograf-host.js, renderer.html, out pages (program/fill/key/preview)
│  └─ ui/                     Svelte 5 app for Tauri
├─ third_party/ograf/         EBU OGraf v1 schemas + examples vendored at 945f8bb (MIT, LICENSE kept)
├─ fixtures/                  our own test graphics (alpha ramp, frame counter, test pattern) + goldens
├─ docs/  (adr/, test-shows/, how-to)   STATUS.md   README.md   LICENSE (Apache-2.0, D-010)   NOTICE
```

`noplayout-renderer` must not depend on `noplayout-core`: it only links `proto`, `ndi`, `pixels` and `cef`. That keeps the
CEF build graph small and stops the renderer from growing show logic.

### 2.3 Engine internals (`noplayout-core`)

| Module | Owns | Notes |
|---|---|---|
| `library` | import (folder/zip), validation, storage, serving | Validates the manifest against the vendored v1 schema (jsonschema, offline, E-024). Rejects `supportsRealTime: false` (v1 non-goal). Zip import uses `ZipFile::enclosed_name` (no path escape), size caps. Each package version is stored content-addressed: `library/<graphic-id>/<sha256-12>/`, served at `/graphics/<id>/<hash>/…`, so `main` and assets resolve relative to the manifest and a re-imported version never changes an on-air instance. Classification Coded / Designed. |
| `show` | shows, groups, rundown items, Take IDs, item data, per-item data-source mappings | §2.6. Autosave debounced 500 ms, atomic. |
| `playout` | per-channel layer stack, instances, cue/take/next/out/clear, action log, revisions | The single state authority. Every change bumps the channel revision and emits an event. §2.7. |
| `hostbus` | the host WebSocket: fan-out of ordered actions to every page of a channel, snapshots, result collection | §4.3. |
| `outputs` | output config per channel (web, NDI), NDI name rules, runtime presence, status lights | Builds the renderer `Configure` message. Abstract `OutputKind` so DeckLink can be added later (IDEA §8). |
| `supervisor` | spawning renderers, heartbeat watchdog (500 ms / 2 s), restart + state replay, channel restart on format/NDI-mode change | §4.2. |
| `api` | `/api/v1` (noPlayout), `/ograf/v1` (Server API), `/ws/events`, OpenAPI at `/api/docs` | §2.9. |
| `settings` | versioned JSON, validation, atomic write, last-good, migrations | §2.11. |
| `datasource` (wraps the crate) | running sources, field store, mapping to items, overrides, health | §2.8. |
| `health` | counters from renderers and sources, event log (ring of 500), Copy diagnostics | |
| `rosstalk` | RossTalk-compatible TCP 7788 listener, off by default (D-010) | §2.9.3. |

Runtime: tokio multi-thread. The playout state lives behind one actor task per channel (an mpsc command queue), so actions
on a channel are totally ordered without locks across awaits.

### 2.4 Renderer internals (`noplayout-renderer`)

Threads:

| Thread | Does | Real-time? |
|---|---|---|
| main / CEF UI | `cef::execute_process` first (helper subprocesses exit here), then `cef::initialize`; message loop. macOS: `external_message_pump` + `do_message_loop_work` driven by `on_schedule_message_pump_work`. Win/Linux: CEF's own loop (`run_message_loop`). Receives `on_paint`. | no |
| clock | owns the channel clock (§5); at each tick posts `send_external_begin_frame` to the UI thread (mode A) and wakes the sender | yes (elevated priority) |
| sender | at deadline *k*: takes the newest complete paint from the mailbox, converts, submits to NDI senders with `tc(k)`, updates counters, hands every *n*-th frame to the monitor thread | yes |
| monitor | downscale 1080p→480×270 (fast_image_resize), composite on checkerboard, JPEG (jpeg-encoder), push to frames socket with credits | no |
| ipc | control socket (heartbeat, configure, health, events) | no |

CEF settings: `windowless_rendering_enabled = 1`; browser `background_color = 0x00000000` (fully transparent: CEF allows only
0x00 or 0xFF alpha); program browser `external_begin_frame_enabled = 1` in mode A, otherwise `windowless_frame_rate =
ceil(fps)`; preview browser `windowless_frame_rate = 30`; `shared_texture_enabled = 0` (CPU `on_paint`, E-005);
`root_cache_path` = absolute per-channel dir under app data (CEF's process-singleton lock forbids sharing); audio muted
(`set_audio_muted`; audio is a v1 non-goal). `on_render_process_terminated` → reload the page (the page reconnects and
replays); `on_console_message` → forwarded to the engine as an event and counted (conformance tests assert zero errors).

`on_paint` hands CEF's BGRA buffer (premultiplied, upper-left origin) to a triple buffer: copy into the free slot (8.29 MB
at 1080p, one `memcpy`), tag it with the paint counter and the begin-frame tick that requested it, publish. The copy is
mandatory: the buffer is only valid inside the callback.

### 2.5 OGraf host runtime (one runtime, five roles)

`web/host` builds `ograf-host.js` + `renderer.html`, served by the engine at `/host/`. Roles: `program` (CEF, authoritative),
`preview` (CEF preview browser and `/out/<ch>/preview`), `web-program`, `web-fill`, `web-key` (browser sources), `editor`
(Edit mode preview). The role changes only presentation (§2.10.2) and whether results are authoritative.

The runtime:
- keeps one container element per layer (z-order = layer number) plus hidden **cue slots**;
- creates the graphic element from the manifest's `main` (dynamic `import()` of the package URL), calls
  `load({data, renderType: "realtime", renderCharacteristics: {resolution, frameRate: 59.94006…}})` and **never sends
  another action to that instance before `load` resolves** (OGraf O5); actions that arrive meanwhile queue behind it;
- calls methods in the order the engine sequenced them and does not await one before starting the next (O8);
- merges partial `updateAction` data locally for its own snapshot; treats `currentStep: undefined` as "at end" (O7);
  supports `stepCount: -1` (NEXT STEP keeps sending `delta: 1` until the graphic reports the end, O6);
- passes `skipAnimation` through on every action type;
- reports each result (`statusCode`, `statusMessage`, `currentStep`, any extra payload) with the action's sequence number;
- times out a `load` after 10 s and any other action after 5 s, reporting the timeout as an error (never wedges the layer);
- in test builds only, draws the frame-index overlay used by S1 (a corner patch whose pixels encode a `requestAnimationFrame`
  counter). It is off in release builds and invisible to graphics.

TypeScript types come from npm `ograf` 1.2.1 (matches the pinned spec commit). Unit tests (vitest) run the runtime against
fake graphics that misbehave on purpose (slow load, throwing actions, overlapping calls).

### 2.6 Data model

All files are JSON with `schemaVersion` and are written atomically (§2.11). Ids are UUIDv7 strings.

```text
Settings (per computer) { schemaVersion, server {port=8686, scope: anyone|lan|localhost, accessToken?}, channels[1..=4] {
  id, name, format {width, height, rateNum, rateDen}, layers=10,
  web {enabled}, ndi {enabled, sourceName, mode: fillAlpha|separate, fillType: shaped|unshaped (default per mode, E-009)} },
  hotkeys{…}, ui{clock24h, uiScale}, rosstalk{enabled=false, port=7788},
  sourceConnections{ <show source key> → Transport {serial|tcpDial|tcpListen|udp, port/host/…} } }   (D-009)

Show { schemaVersion, id, name, groups[] {id, name, collapsed}, items[] RundownItem,
  dataSources[] {key, name, decoderId, layout}   (what the show expects; no ports or hosts, D-009),
  takeIdKeys[] {key, takeId, action: cue|cueAndTake}, onAir (written by the engine, used by restore) }

RundownItem { id, takeId: u16 (1..=9999, unique in show), name, groupId?, graphic {id, versionHash},
  data (JSON object), target {channelId, layer}, liveUpdate=true,
  mappings[] {field, from: {sourceKey, sourceField} | manual, format?, overridden: bool, overrideValue?} }
```

**Take IDs (D-005, D-010, E-021):** a `u16` per item, unique within the show (enforced by the engine, not just the UI: an
API request that would create a duplicate gets 409 with the clashing item named, and the UI shows the same text inline on the
field). Assignment rules are Usopp's §2.7 (auto-increment: next free after the item above; never copied on duplicate;
renumber with undo; paste remaps with a report). The engine keeps a
`takeId → itemId` index rebuilt on every rundown change. Take IDs are per show, not per channel.

### 2.7 Playout semantics

Channel state: `layers[1..=L]` each with at most one `Instance {id, itemId?, graphic, data, step: Option<i32>, phase:
loading|loaded|playing|stopping|ended, log: Vec<Action>}` plus `cued: Option<CueSlot>` and `revision: u64`.

| Operation | What the engine does |
|---|---|
| **Select / cue** (UI select, Take ID + Enter, `cue` API, RossTalk CUE/FOCUS/UPNEXT) | Sets the channel's cued item. The preview page loads it alone (D-007). The program page **preloads** it into a hidden cue slot with `load(data)`, so TAKE is immediate (XPression "CUE = no frame delay", E-015). Recueing a different item disposes the previous preload. |
| **TAKE** | If the target layer holds an instance: `dispose` it (cut, as an XPression take replaces). Move the preloaded instance into the layer (CSS visibility flip, no DOM move), send `updateAction(skipAnimation)` first if the item's data changed since the preload, then `playAction({})` (first call goes to step 0). If no preload is ready, load then play. |
| **NEXT STEP** | `playAction({delta: 1})` on the target instance (the selected item if on air, else the most recently taken one still on air; Usopp §2.3). Disabled at the last step, but never for `stepCount: -1`. |
| **Take & advance** (XPression NEXT) | TAKE, then cue the next rundown item. Off as a default hotkey (D-007: auto-advance off); exposed in the API and RossTalk. |
| **OUT** | `stopAction({})`, then `dispose` once it resolves (or after 5 s). The only animated exit. |
| **CLEAR LAYER / CLEAR ALL** | `stopAction({skipAnimation: true})` + `dispose` at once. Always cut (D-007). |
| **Data edit on air** | `updateAction({data: <changed keys only>})`, debounced 150 ms while typing; held as pending when Live update is off. |
| **Custom action** | `customAction({id, payload})`; `id` validated against the manifest, payload against that action's schema. |

Ordering: each channel actor assigns a sequence number to every action and appends it to the instance log. The hostbus sends
actions in that order to every connected page. Results that answer API calls come only from the `program` role. If the
program renderer is down, mutating API calls still change state (the renderer replays on return) and answer `202` with
`"renderer": "restarting"`; reads answer from engine state.

**Recovery replay (E-019):** a reconnecting page (restarted renderer, reloaded page, late web client) gets a snapshot: for
each layer, `load(dataAtLoad)` then the instance's log replayed in order with `skipAnimation: true`. Consecutive
`updateAction`s are compacted into one merged update. The log is capped at 256 entries per instance. Past the cap the
replay falls back to `load(currentData)` + `playAction({goto: step, skipAnimation: true})`, and Health records "custom actions
not replayed". `goto` reproduces the step even for `stepCount: -1`.

### 2.8 Data sources (D-006, D-008, D-009, D-011; E-022, E-023)

Built in v1 as milestone M4.5, with **one decoder: OES** (D-011). Every other console (Daktronics All Sport 5000 first, per
D-010) is a later, self-contained plug-in change. Prior art, used for protocol knowledge only: Caleb's
`unc-virtual-score-server` (MIT; `website/protocol.py` OES framing and field decoding, `website/ingestion.py`
serial/TCP/UDP readers including a Lantronix-style serial-to-UDP bridge, and its lesson on per-sender UDP framing) and, for
later, `daktronics-allsport-5000` 0.5.0 (MIT). No product naming, venue names, addresses or captures from either go into
noPlayout.

```text
Transport (bytes in)            Framer+Decoder (plug-in)          Field store            Mapping (per item)
serial  ─┐                     ┌─ oes  (v1)          ─┐          source.field →          item.field ← format
tcp-dial ─┼─ Chunk{peer,bytes,─┤  …later, one crate   ├─ Update ─ {value, changedAt, ─────  (manual override wins)
tcp-listen┤        t_mono}      │  per decoder        │          seenAt, raw}            → playout updateAction
udp      ─┤                     └─ registry, by id   ─┘                                  (coalesced ≤ 20 Hz/item)
replay   ─┘ (capture file; Advanced, labelled "Replay", tests + rehearsal)
```

- **Transports:** `serial` (tokio-serial 5.5.0 over serialport 4.10.1; by-name port selection; Linux uses
  `/dev/serial/by-id/...` so a replug onto another USB port still matches; the decoder supplies the preset, OES = 9600 baud
  as in Caleb's `ingestion.py`); `tcp-dial` (host:port, reconnect every 2 s, keep-alive); `tcp-listen` (port, optional
  accept-from address, one framer per connection); `udp` receive (port, interface, optional multicast group and accept-from
  address). UDP covers serial-to-network bridges that push datagrams (the Lantronix case): a datagram is not a frame
  boundary, so bytes go through the same framer as serial. UDP can be Quiet but never Lost (there is no connection).
- **Per-peer framing state** (Caleb's lesson): one framer instance per TCP connection and per UDP sender address, never
  shared. Idle UDP peers are reaped after 300 s. Test: two interleaved senders with frames split across datagrams must decode
  both streams exactly (a shared parser splices bytes between devices).
- **Decoder plug-in contract (E-022):** a decoder is its own crate (`noplayout-decoder-<id>`) implementing `trait Decoder`:
  `id()`, `display_name()`, `serial_preset()`, `layouts()` (e.g. sport), `fields(layout) -> Vec<FieldDef {id, name, kind}>`,
  `feed(&mut self, &[u8]) -> Vec<DecodedUpdate>` (framing included, so each decoder owns its byte protocol), `stats()`. The
  registry is a static list in `noplayout-datasource` behind one cargo feature per decoder. Adding a decoder touches only:
  the new crate, one registry line, one feature line, its captures/goldens, its docs page and STATUS.md. No change to
  transports, the field store, mapping, the API or the UI (they all work from `fields()`). A shared conformance test runs
  every registered decoder against its own captures, so a new decoder cannot be listed without one. Decoders run inside
  `catch_unwind`; a panicking decoder marks its source DecodeErrors and never takes the engine down.
- **States:** Off, Waiting (connected, nothing yet), Receiving, Quiet (no data for N s, default 5), Lost (port gone, TCP
  closed/refused, bind error), DecodeErrors (receiving but >0 undecodable in the last minute), Paused. Usopp §5.3 wording.
- **Mapping and override:** one source field may feed many item fields; an item field has one source. "Send only on change"
  on by default (scoreboards repeat every message). Typing into a linked field sets `overridden` and keeps it until "Back to
  live" (Usopp Δ16); the engine never overwrites an overridden field from a source message. Formatters: as sent, `M:SS`,
  `SS.t` under a minute, ordinal (1st/2nd…), upper case.
- **Rate:** updates to one item are coalesced to at most one `updateAction` per 50 ms (clock tenths still look continuous);
  the coalescer never delays a value by more than 50 ms.
- **Source quiet or lost (D-009):** every mapped field, the game clock included, **holds its last value**, and the source's
  light goes amber "source lost" (UI.md wording). No local free-running clock. When data returns, fields update on the next
  message.
- **Capture and replay:** `noplayout-capture` records raw chunks with monotonic timestamps and peer ids into a `.npcap` file
  (magic, version, then length-prefixed records `{t_ns, peer, transport, bytes}`). The `replay` transport plays one back at
  real speed or as fast as possible, through the real framer. Captures are how decoders are tested (E-023), and they let an
  operator rehearse a show without the console.
- **Storage (D-009):** the show stores what it expects (`dataSources[] {key, name, decoderId, layout}`) and every item's
  field mappings. The connection (serial port, host, port, interface) is stored **per computer** in that machine's settings,
  keyed by the show's source key. Opening a show on a computer with no connection for a source shows it as "Not set up on
  this computer" with the Add dialog prefilled with the decoder; a saved serial port that isn't present shows "Port not
  found" with a picker. Mappings survive either way.

**OES in v1 (D-011, E-023):** ported from Caleb's MIT-licensed `protocol.py`, keeping its framing (STX, sport type byte, printable
payload, CR; sport identified by type byte and length; the protocol has no checksum, so a length or field-range mismatch
counts as a decode error) and its nine sport layouts, with its test vectors carried over. It runs over serial, TCP (dial and listen) and UDP. The gate is a
recorded OES byte stream from a real console (§7 M4.5), not a simulator.

### 2.9 Control API

#### 2.9.1 OGraf Server API (`/ograf/v1`, spec commit 945f8bb, OpenAPI 3.0.3, info.version 1.0.0)
- **Renderer = channel** (`rendererId` = channel id); **RenderTarget = `{ "layer": <n> }`** (a shallow object, as the spec
  requires). `GET /renderers/{id}` lists `renderTargets` as the layer range; `status` reflects the renderer's health.
- `/graphics`, `/graphics/{id}`, thumbnail, delete (refused with 409 while an instance or rundown item uses it).
- `target/graphicInstance/load|updateAction|playAction|stopAction|customActions/{id}|clear` map onto §2.7 directly, bypassing
  the rundown. Instances created this way show in the On-air strip as "API: <graphic name>".
- Errors use the spec's RFC 7807 style; a graphic-method failure answers 550 as the spec defines. Spec bug O13
  (`GET /graphics/{id}` requires `manifest` but lists `metadata`) is handled by returning both keys.
- The Server API has no upload, events or WebSocket. Those are noPlayout extensions under `/api/v1`.

#### 2.9.2 noPlayout API (`/api/v1`, documented at `/api/docs`, OpenAPI generated with utoipa 6.0.0)
Rundown: `POST /rundown/take`, `/rundown/next-step`, `/rundown/take-and-advance`, `/rundown/out`, `/rundown/clear-layer`,
`/rundown/clear-all`, `POST /rundown/items/{itemId}/cue`, `PATCH /rundown/items/{itemId}/data` (partial; respects overrides).
**Take ID routes** (D-005): `POST /takeid/{n}/cue`, `/takeid/{n}/take` (cue + take in one, for Stream Deck buttons),
`/takeid/{n}/next-step`, `/takeid/{n}/out`. Unknown Take ID → 404 `{"title":"No item 47 in this show"}`, nothing changes,
and Health logs it (Usopp §2.7). Library: `POST /library/import` (multipart zip), `GET /library`. Shows: open/save/list.
Settings: `GET/PUT /settings` (validated as a whole, never half-applied; returns per-field errors). Data sources:
`GET /datasources`, `GET /datasources/{id}/values`, `POST /datasources/{id}/pause|resume`, `PUT/DELETE
/rundown/items/{itemId}/fields/{field}/override`. Health: `GET /health`, `GET /diagnostics` (the Copy diagnostics bundle,
secrets removed). Outputs: `POST /outputs/test-pattern {on}`.

`/ws/events`: JSON events with the channel revision: `tally` (per item: on air / cued, with Take ID, so Companion buttons can
light), `playout`, `health`, `datasource`, `settings`. A reconnecting client sends its last revision and gets a snapshot.

#### 2.9.3 RossTalk-compatible listener (in v1, off by default: D-010; E-037)
Off by default, under Settings → Remote control → Advanced. TCP 7788, CR/LF-terminated, case-sensitive, as XPression's.
`CUE takeid:buffer:layer`, `TAKE`, `READ`, `NEXT`, `SEQI`, `SEQO`, `LAYEROFF buffer:layer`, `CLFB`, `CLRA`, `FOCUS`, `UPNEXT`,
`GPI n` map onto Take IDs, channels (XPression framebuffer, **zero-based on the wire**: framebuffer 0 = channel 1) and
layers. `GPI n` triggers a configurable Take ID action. Unknown commands are logged, never fatal. It binds to the same
access scope as the HTTP port (§2.12). Cost: about one build card plus tests. It lets existing Carbonite CG-device presets and
the Companion XPression module drive noPlayout unchanged.

### 2.10 Outputs

#### 2.10.1 NDI (renderer process; E-007…E-011)
Per channel, from one rendered frame (CEF premultiplied BGRA, `P`):

| Mode / fill type | Sources | Pixels sent |
|---|---|---|
| Fill + Alpha (default), **unshaped** (default for this mode) | `NOPLAYOUT <name>` | BGRA, RGB un-premultiplied, A as rendered (NDI's documented convention: BGRA "is not pre-multiplied") |
| Fill + Alpha, shaped (Advanced, for receivers set to premultiplied) | same | BGRA = `P` unchanged |
| Separate Fill & Key, **shaped** (default for this mode, D-010; bench-confirmed on Caleb's switcher in M2) | `… FILL`, `… KEY` | FILL: BGRX, RGB = `P`.rgb (= composited over black); KEY: BGRX, R=G=B=A |
| Separate Fill & Key, unshaped | same | FILL: BGRX, RGB un-premultiplied; KEY as above |

- Never UYVA (OBS DistroAV drops its alpha plane). BGRA also skips our own RGB→YUV step.
- Un-premultiply: `c' = min(255, round(c·255/a))` for `a > 0`, RGB = 0 at `a = 0`, via a 64 KiB `[a][c]` lookup table, split
  over 2 worker threads per channel if one is too slow (S2 measures it; budget ≤ 3 ms per 1080p frame). KEY is a byte shuffle.
- Senders: `SenderOptions::new(name).clock_video(false)`; owned `VideoFrame`s submitted with `submit_video` (async, the
  displaced frame comes back for reuse, so FILL and KEY each keep their own ping-pong pair); `frame_rate(FrameRate::new(num,
  den))`; `timecode(Timecode::from_ticks(tc(k)))`, the same value on FILL and KEY; per-frame metadata XML
  `<noplayout k="…" paint="…"/>` for the loopback test.
- `connection_count()` polled at 1 Hz per sender → receiver counts and the amber "waiting" light.
- Runtime missing: `Ndi::new()` returns an error that lists every path tried. The renderer reports `NdiRuntimeMissing`, NDI
  outputs go red with "NDI Runtime not installed" and the OS-specific install link (Windows `ndi.link/NDIRedistV6`, macOS
  `ndi.link/NDIRedistV6Apple`, Linux: ndi.video SDK download, since the header's Linux redist URL is empty), web outputs keep
  running, and the renderer never crashes. "Check again" re-runs detection without a restart.
- Branding duties (Robin 2026-10-30, Usopp §0.5): "NDI®" on first use per screen, an `ndi.video` link beside every NDI
  selector, "NDI® is a registered trademark of Vizrt NDI AB" in About, no NDI Tools redistribution, no "NDI" in the product
  name. These are M2/M4 acceptance items, not polish.

#### 2.10.2 Web outputs (M3; E-016)
`/out/<ch>/program|fill|key|preview` are role pages of the same host runtime, subscribed to `/ws/host/<ch>`.
- `program`: transparent background (OBS/vMix pick up the alpha).
- `fill`: an opaque page. **Shaped** = content composited over black. **Unshaped** = SVG `feColorMatrix` on the layer root
  with `color-interpolation-filters="sRGB"`, matrix keeping RGB and forcing A = 1 (filter primitives work on un-premultiplied
  values, so this outputs straight RGB).
- `key`: the same filter with R = G = B = A and A = 1, over black: the matte with linear alpha as grey levels, from the same
  scene (no separate graphic, IDEA §4.2).
- The access options (anyone / LAN only / localhost only, optional `?token=`) apply to every route on the port (Usopp Δ7).
- The "browser outputs aren't frame-locked" note is shown, as IDEA requires.
- The CEF renderer can load `/out/<ch>/key` as well, so a pixel test compares the web key against the NDI KEY frame from the
  same Chromium. Tolerance and the fallback (a WebGL pass) are in R8.

#### 2.10.3 Test pattern
A built-in OGraf graphic (`fixtures/noplayout-test-pattern`): bars, an alpha ramp (16 steps, 0–255), a moving box and the
channel name. It goes through the real pipeline (CEF → convert → NDI, and every web output), so "Send test pattern" proves
the actual path rather than a synthetic one. It takes a reserved top layer and leaves the rundown untouched.

### 2.11 Settings, persistence, recovery (IDEA §6; E-020)
- Paths from `directories` 6.0.0 (`ProjectDirs::from("", "", "noPlayout")`): `settings.json`, `settings.last-good.json`,
  `shows/`, `library/`, `logs/`, `cef-cache/<channel>/`, `backups/`.
- Atomic write: temp file in the same directory → write → `fsync` → rename over → `fsync` the directory (Unix);
  `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` on Windows.
- Startup: parse → migrate (backing up the old file first) → validate. On failure load last-good and record exactly what was
  rejected for Usopp's banner. Last-good is rewritten only after a file has loaded and validated cleanly.
- Validation lives in Rust (`serde` + explicit `validate()`), returns per-field errors, and `PUT /settings` applies all or
  nothing. Changing format or NDI mode on a channel with graphics on air needs confirmation, then restarts only that
  channel's renderer, which replays state.
- On-air state is written into the show's `onAir` block at each revision (debounced 200 ms) for the restore prompt.
- Logs: `tracing` + `tracing-appender` daily rolling files, keep 14. Copy diagnostics: versions (app, CEF, NDI runtime, OS,
  GPU from CEF), settings with token fields removed, last 500 log lines, health snapshot.

### 2.12 Exposure and security (production rail 4)
- Default bind and auth follow IDEA §4.2: all interfaces, no token. Under Advanced: localhost only / LAN only / token. This is
  a broadcast control surface like RossTalk; DECISIONS E-034 records the trade-off.
- The app↔engine and renderer↔engine links use per-launch random tokens passed through environment variables, never argv.
  The token-protected internal routes (shutdown, supervisor control) are loopback-only regardless of the user setting.
- Every request that changes state from a non-loopback address is logged with its source address (no bodies, no tokens).
- Graphics are trusted local content, but the CEF pages are only allowed to navigate within the engine origin (popups and
  external navigation are blocked). Network fetches by graphics are allowed, because OGraf graphics may load fonts or data.

---

## 3. Crates and tools, pinned (verified 2026-11-26)

"Verified" means: crates.io API (version, date, licence, MSRV) **and** the named API was read in that exact published
source. Pins go in `[workspace.dependencies]`; `Cargo.lock` is committed; `cef` is pinned with `=`.

| Crate | Pin | Licence | Used by | Verified / notes |
|---|---|---|---|---|
| `cef` | `=156.7.0+156.0.31` (2026-11-19) | Apache-2.0 OR MIT | renderer | Read in crate source: `WindowInfo.{windowless_rendering_enabled, shared_texture_enabled, external_begin_frame_enabled}`, `BrowserSettings.{windowless_frame_rate, background_color}`, `Settings.{external_message_pump, multi_threaded_message_loop, root_cache_path}`, `BrowserHost::send_external_begin_frame`, `set_windowless_frame_rate`, `set_audio_muted`, `RenderHandler::on_paint(…, buffer: *const u8, width, height)`, `on_accelerated_paint`, `RequestHandler::on_render_process_terminated`, `DisplayHandler::on_console_message`, `execute_process`, `initialize`, `do_message_loop_work`, `post_task`. Default features `sandbox, build-util, resources`; `accelerated_osr` stays off (E-005). Depends on `cef-dll-sys ^156.7.0`; Cargo.lock holds it at 156.7.0+156.0.31. |
| `export-cef-dir` | `156.7.0+156.0.31` | Apache-2.0 OR MIT | xtask, CI | Same release train; fills `CEF_PATH`. `bundle-cef-app` is a bin inside `cef` (feature `build-util`). |
| `grafton-ndi` | `3.0.0` (2026-11-19), MSRV 1.88 | Apache-2.0 AND MIT | ndi | Read: `Ndi::new()`, runtime search order (exe dir → absolute `NDI_RUNTIME_DIR_V6` → platform search; relative paths refused), `SenderOptions::new(..).clock_video(bool)`, `Sender::{send_video, submit_video, submit_video_copy, reclaim_video, flush_video, connection_count, tally}`, `BorrowedVideoFrame::builder(buf, w, h, PixelFormat::Bgra).line_stride_bytes().frame_rate().timecode().metadata()`, `Timecode::from_ticks`, `FrameRate::new(num, den)`, `PixelFormat::{Bgra, Bgrx}`, `Receiver`/`ReceiverOptions` (loopback test). README: no SDK needed at build time on Win/macOS/Linux; no default features (enable none, except `tokio` if the receiver test needs it). |
| `tauri` | `2.12.7` (2026-11-18), MSRV 1.90 | Apache-2.0 OR MIT | app | `tauri-build 2.7.6`; plugins `tauri-plugin-single-instance 2.5.5` (2.x line; its newest tag is 3.0 alpha, not used). Global shortcuts are **not** used: hotkeys work only when the window has focus, which is what Stream Deck keystroke mode expects. |
| `axum` | `0.8.9`, features `ws, macros, json, query, multipart` | MIT | core | ws feature present at 0.8.9 (pulls tokio-tungstenite 0.29); tower 0.5. |
| `tower-http` | `0.7.1` | MIT | core | `fs` (ServeDir), `trace`, `limit`. Depends on tower ^0.5 like axum 0.8.9. |
| `utoipa` / `utoipa-axum` / `utoipa-scalar` | `6.0.0` / `0.3.0` / `0.4.0`, MSRV 1.88 | MIT OR Apache-2.0 | core | utoipa-axum 0.3.0 depends on axum ^0.8.4. Serves `/api/docs`. |
| `tokio` | `1.54.1` | MIT | core, renderer ipc, datasource | `rt-multi-thread, net, time, sync, macros, fs, signal`. |
| `tokio-util` | `0.7.20` | MIT | datasource | `codec` framing. |
| `interprocess` | `2.4.10` (2026-11-25), feature `tokio` | 0BSD OR Apache-2.0 | proto/core/renderer | `local_socket` with `GenericNamespaced` / `GenericFilePath` names, `ListenerOptions`; named pipes on Windows, UDS elsewhere. |
| `postcard` | `1.2.0` | MIT OR Apache-2.0 | proto | IPC message encoding (E-012). |
| `serde` / `serde_json` | `1.0.229` / `1.0.154` | MIT OR Apache-2.0 | all | |
| `jsonschema` | `0.63.2`, **`default-features = false`, features `resolve-file`** | MIT | core | Defaults pull `resolve-http` (reqwest + rustls); off so manifest validation never touches the network (E-024). |
| `zip` | `9.8.2` | MIT | core | Package import (`enclosed_name`). |
| `directories` | `6.0.0` | MIT OR Apache-2.0 | core | |
| `tracing` / `tracing-subscriber` / `tracing-appender` | `0.1.44` / `0.3.23` / `0.2.5` | MIT | all | |
| `thiserror` / `anyhow` | `2.0.21` / `1.0.105` | MIT OR Apache-2.0 | all (`anyhow` only in bins) | |
| `uuid` | `1.29.3`, `v7, serde` | Apache-2.0 OR MIT | core | |
| `fast_image_resize` | `6.2.0`, MSRV 1.87 | MIT OR Apache-2.0 | pixels | Monitor downscale. |
| `jpeg-encoder` | `0.7.1`, feature `simd` | (MIT OR Apache-2.0) AND IJG | pixels | Pure Rust, no C toolchain (E-030). The IJG licence needs the "based in part on the work of the Independent JPEG Group" line in About and docs. |
| `png` | `0.18.1` | MIT OR Apache-2.0 | renderer (PNG dump), tests (goldens) | |
| `serialport` | `4.10.1` | **MPL-2.0** | datasource (through tokio-serial) | File-level copyleft, used unmodified: compatible with an Apache/MIT product (E-031). Linux needs `libudev` at build (CI installs `libudev-dev`). |
| `tokio-serial` | `5.5.0` | MIT | datasource | serialport ^4. |
| `daktronics-allsport-5000` | not a v1 dependency (D-011) | MIT | — | Checked for the later Daktronics plug-in: 0.5.0 (2025-02-01) has `SerialRTDCodec` (tokio-util `Decoder`, SYN 0x16 / ETB 0x17), `Packet`, `RTDState::update_from_packet`, per-sport field modules; **0 tests, checksum skipped** ("don't know the hash algorithm"). If used later: field tables only, own framing, capture-gated (E-023). |
| `socket2` | `0.6.5` | MIT OR Apache-2.0 | datasource | UDP multicast join, SO_REUSEADDR, TCP keep-alive. |
| `sysinfo` | `0.40.4`, MSRV 1.95 | MIT | core | Renderer memory for Health. |
| `insta` | `1.49.0` (dev) | Apache-2.0 | tests | Snapshot tests of API JSON and decoder timelines. |
| `libloading` | `0.9.1` (transitive via cef) | ISC | — | Not used directly (grafton-ndi owns NDI loading). |

Not used, on purpose: `iceoryx2` 0.10.1 and `memmap2` 0.9.12 (shared memory only if NDI ever leaves the renderer, E-012),
`turbojpeg` (C dependency, E-030), `tauri-runtime-cef` 3.0.0-alpha.7 (alpha, pins cef 154.5.0 against our 156, and
cef-dll-sys `links` forbids two versions in one graph, E-003), `ndi-sdk-sys` (GPL-3.0, no macOS), `ndi-sdk` (no send/receive).

Toolchain and tools: Rust **1.100.0** (current stable, 2026-11-09), edition 2024 (MSRV floor set by sysinfo 1.95 and tauri
1.90). `cargo-deny` 0.22.1. `osv-scanner` (brew) before release PRs (rail 5).

JavaScript (pinned exactly in `web/package-lock.json`, `npm ci` only): `svelte` 5.59.2, `@sveltejs/vite-plugin-svelte`
7.3.2, `vite` 8.4.0, `svelte-check` 4.7.8, **`typescript` 6.0.3** (svelte-check 4.7.8's peer range is `^5 || ^6`, so TS 7.0.2
is not used yet), `@tauri-apps/api` 2.12.5, `@tauri-apps/cli` 2.12.6, `vitest` 5.1.2, `ograf` 1.2.1 (types). Node 24 LTS.

OGraf: spec + JSON schemas + examples vendored from `github.com/ebu/ograf` at `945f8bbcbcba262377544871dd4af62e54de5177`
(2026-11-26, no tags exist upstream; repo MIT). `third_party/ograf/README.md` records the commit and how to re-vendor.

Renovate (or Dependabot) proposes bumps weekly; `cef` bumps are their own PR, which must re-run S1 and the pixel tests.

---

## 4. IPC design

### 4.1 Links

| Link | Transport | Encoding | Direction / content | Rate |
|---|---|---|---|---|
| app ↔ engine | HTTP/1.1 + WebSocket, 127.0.0.1 | JSON | REST calls; `/ws/events`; monitor JPEGs over `/ws/monitor/<ch>/<program\|preview>` (binary) | events as they happen; monitors ≈15 fps |
| engine ↔ renderer **control** | `interprocess` local socket (named pipe / UDS), one per channel | `u32` LE length + postcard, max 1 MiB, versioned | down: `Configure`, `TestPattern`, `Reload`, `Shutdown`, `Ping`; up: `Hello`, `Pong`, `Health`, `Event` | heartbeat 500 ms; health 1 Hz |
| engine ← renderer **frames** | second local socket per channel | same framing; `MonitorFrame {src, k, tc, w, h, jpeg}` up, `Credit {n}` down | ≈15 fps per source, 10–40 kB each |
| engine ↔ pages (CEF program/preview, web outputs, editor) | WebSocket `/ws/host/<ch>?role=…` | JSON, sequence numbers | down: `Snapshot`, `Action`; up: `Ready`, `Result`, `PageError`, `Heartbeat` | per action |
| engine ↔ external controllers | HTTP + `/ws/events` + RossTalk-compatible TCP 7788 (off by default) | JSON (RossTalk text) | | |

Full-rate pixels never cross a process boundary: the NDI senders live in the renderer (497 MB/s per 1080p59.94 channel;
2 GB/s for four). Frames up for monitors are already JPEG, about 0.3 MB/s per monitor.

### 4.2 Engine ↔ renderer protocol
- **Spawn:** the engine starts `noplayout-renderer --channel <id>` with env `NOPLAYOUT_IPC_NAME`, `NOPLAYOUT_IPC_TOKEN` and
  `NOPLAYOUT_HOST_URL` (loopback base URL + internal token), then waits up to 10 s for `Hello{proto: 1, renderer_version,
  cef_version, pid, token}`. A protocol mismatch is a hard error with a plain message (mixed install).
- **Configure** (idempotent, may be resent): `format{w, h, num, den}`, `ndi{senders[{name, kind: fillAlpha|fill|key},
  fillType]}`, `monitor{w: 480, h: 270, quality: 80, every_n}`, `pages{program_url, preview_url}`.
- **Heartbeat:** both sides `Ping{seq}` every 500 ms; the renderer's Pong carries `last_paint_age_ms` and `last_tick_k`. The
  engine's watchdog restarts the renderer (kill the process tree, respawn, replay) when no Pong arrives for 2 s (IDEA §6), and
  logs the incident. A page that is alive but stuck (no `Heartbeat` on the host WS for 2 s while the renderer is fine) gets a
  `Reload`, not a process restart.
- **Health (1 Hz):** `paints_received, ticks, deadlines_missed, repeated, superseded, slots_skipped, fill_key_tc_mismatch,
  convert_ms_p50/p99, send_ms_p99, connection_count per sender, rss_bytes`.
- **Events:** `NdiRuntimeMissing{searched}`, `SenderError{name, reason}`, `PageTerminated{reason}`, `ConsoleError{text}`,
  `CefInitFailed{reason}`.
- **Credits** on the frames socket: the engine grants 2; the renderer sends only with a credit, dropping the oldest unsent
  monitor frame (newest wins). The engine relays to WS clients the same way (one frame in flight per client), because browser
  WebSockets have no receive backpressure.
- **Orphan rule:** a renderer whose control socket closes exits after 3 s.

### 4.3 Host protocol (pages)
- `Ready{role, hostVersion, lastRevision?}` → engine answers `Snapshot{revision, layers: [{layer, instanceId, graphicUrl,
  dataAtLoad, log: [...]}], cued?}` (§2.7 replay), then streams `Action{revision, seq, instanceId, method, params}`.
- Only `role=program` `Result`s are authoritative. Other roles' results are logged at debug level, and a disagreement
  (program says step 2, a web page says step 1) is counted as `mirror_divergence` in Health.
- Each page sends `Heartbeat{revision, frameCounter}` every 500 ms. `frameCounter` is the page's rAF count, which separates
  "static graphic" from "stalled page".

---

## 5. Frame pacing (E-006)

One clock per channel, in the renderer process, in integer arithmetic. The engine never paces frames.

1. **Rate:** `num/den` from the format (60000/1001, 50/1, 30000/1001, 25/1). `T_k = epoch_mono + floor(k · den · 10⁹ /
   num)` ns, computed from `k` every time in `u128` (never `+= 16.683 ms`; `u64` overflows after about 213 days at 59.94).
   `epoch_mono` is taken when the channel starts; `U0` = UTC at that moment in 100 ns units.
2. **Sleeping:** an absolute deadline minus 0.5 ms with the OS's precise timer (macOS `mach_wait_until`, Linux
   `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME)`, Windows high-resolution waitable timer
   `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`), then spin to `T_k`. The clock and sender threads run at elevated priority
   (macOS QoS user-interactive, Windows `THREAD_PRIORITY_TIME_CRITICAL`, Linux `SCHED_FIFO` only if permitted, otherwise nice).
3. **Drive mode A, external begin-frame (preferred):** `external_begin_frame_enabled = 1`. At `T_{k} − lead` (lead = one frame
   to start, tuned by S1) the clock posts `send_external_begin_frame` to the CEF UI thread. The paint that results is tagged
   `k+1`. CEF #4311 (open, last updated 2026-10-23) says external begin-frames carry `Now()`, no deadline and a fixed 60 Hz
   interval, and that a tick arriving while one is pending is dropped. At 59.94 the interval is 0.1 % off, which is harmless.
   At 50/25 it is 20 % off, which can affect CSS/rAF timing, so the 50 Hz formats get their own S1 run.
4. **Drive mode B, free-running (floor):** `windowless_frame_rate = ceil(fps)` (60 for 59.94, 50, 30 for 29.97, 25). CEF paints
   at its own pace into the mailbox, and the sender samples the newest complete frame at each `T_k` (CasparCG's model). At
   59.94 this supersedes about one paint every 16.7 s, which is counted.
5. **Choosing:** S1 runs A and B on all three OSes. A mode passes if 10 minutes of the frame-counter fixture plus the EBU
   scoreboard clock show, at an NDI receiver, no hole or double in the burned-in rAF counter beyond the counted `repeated`, and
   `fill_key_tc_mismatch = 0`. Ship A on every OS where it passes, B elsewhere. The choice is per OS, recorded in an ADR, and
   switchable in Advanced diagnostics (not in the operator UI). If CEF ships timed begin-frames (#4311's
   `SendExternalBeginFrameWithTiming`), A becomes the default everywhere after a re-run.
6. **At each deadline the sender:** takes the newest complete paint. If none is newer, it resends the previous converted pair
   with the new `tc(k)` (`repeated++`; a static graphic legitimately repeats, so the Health "Duplicated" counter only counts
   repeats while the page's rAF counter advanced). If several piled up, it sends the newest (`superseded++`). If the sender
   itself wakes after `T_k + period/2`, it skips the slot (`slots_skipped++`, shown as "Dropped") rather than bursting. FILL
   is never sent without KEY: both are submitted from the same immutable converted snapshot in the same slot.
7. **Timecode:** `tc(k) = U0 + floor(k · den · 10⁷ / num)` (100 ns units), written explicitly on FILL and KEY. Never
   `Timecode::SYNTHESIZE`, so NDI's synthesizer drift (fixed in 6.4) is irrelevant. `clock_video(false)` on every sender: the
   channel clock is the only pacing authority.
8. **Not frame-locked:** web outputs (IDEA's stated limitation) and two channels relative to each other (each has its own
   epoch). Genlock and cross-channel lock need the NDI Advanced SDK and are out of scope.

Known host issue: the org's Mac Studio currently has a degraded host timer (replayServer finding: about 19 ms p50 late at
59.94 since 2026-11-18). Frame-pacing measurements on that machine are void until it is fixed, so S1 and the M6 soak run on
other machines (R9).

---

## 6. Risks and their failure conditions

| # | Risk | Fails when (measurable) | Detection | Mitigation / fallback |
|---|---|---|---|---|
| R1 | External begin-frame unusable on an OS (CEF #4311 timing, #4166 incomplete frames on macOS x86_64 with the Rust crate) | S1 mode A shows holes/doubles, or a torn/incomplete frame, in the 10-min counter test | S1 harness: NDI receive + counter decode | Mode B on that OS. If B also fails at 50 Hz: ship 50 Hz formats as "experimental" in STATUS.md and keep looking; do not claim them. |
| R2 | CPU readback + conversion too slow | p99 of paint copy + convert + submit > 8 ms per 1080p frame, or any `slots_skipped` with 2 channels at 1080p59.94 on reference hardware | Health histograms (S2, M2 bench) | LUT split over threads → SIMD (std::arch) → `accelerated_osr` shared-texture path (v1.x, ADR). |
| R3 | 2160p formats (33 MB/frame, ≈1 GB/s at 29.97) | 2160p29.97 soak (30 min) drops frames on reference hardware | M6 soak | Label 2160p "needs a strong GPU" (already in UI.md) and, if it still drops, keep it out of the format list with a STATUS.md entry. |
| R4 | grafton-ndi single maintainer (615/619 commits in 2026); API breaks | a needed fix is not released within 30 days, or 4.0 breaks the API | Renovate, issue tracker | `noplayout-ndi` trait boundary; own `libloading` wrapper over the MIT headers + `NDIlib_v6_load` (about 1 kLOC). |
| R5 | cef-rs moves fast and runs ~2 Chromium majors behind upstream (156 vs 158) | a bump breaks the build or S1; a Chromium CVE matters for trusted local pages | CI on Renovate PRs | `=` pin, bump PRs gated on S1 + pixel tests; pages restricted to the engine origin (§2.12). |
| R6 | macOS bundling: CEF framework + 5 helper apps inside a Tauri-built `.app`, signing with per-helper entitlements (`allow-jit`) | the renderer helpers fail to launch from inside the app bundle, or notarization rejects | S5 (M0 dev bundle), M6 notarization dry run | Custom xtask bundling for macOS (build the Tauri app, then insert `noplayout-renderer.app` built by `bundle-cef-app` under `Contents/Frameworks/`, sign inside-out). |
| R7 | Linux: WebKitGTK UI quirks (Tauri), CEF OSR without a display | UI unusable on NVIDIA/Wayland, or CEF fails to init without X/Wayland | M0/M4 manual check on Fedora KDE Wayland + an NVIDIA box; CI under Xvfb | `WEBKIT_DISABLE_DMABUF_RENDERER=1` default on Linux; CEF `--ozone-platform=headless` test in S4; fallback (E-003): a windowed cef-rs UI host. |
| R8 | Web fill/key filter precision or browser differences (OBS's CEF, vMix) | `/key` page differs from the NDI KEY by more than 2 LSB for a ≥ 16 in the CEF pixel test | M3 pixel test | WebGL pass on the layer root. |
| R9 | OS timer jitter; the Mac Studio's degraded timer | S1 timer probe p99 > 1 ms late on a reference machine | timer probe in S1 (sleep to deadline, histogram) | Spin margin raise; run timing gates on healthy hosts only. |
| R10 | Misbehaving graphics (load never resolves, throws) | one bad graphic blocks a layer or the channel | host runtime timeouts (10 s / 5 s), conformance tests with bad fixtures | Instance marked failed with a plain message; layer stays usable; renderer unaffected. |
| R11 | No real OES recording on hand (the prior-art repo holds single datagrams, not a game) | no recorded OES byte stream of a real game by the end of M4.5 build | M4.5 gate | M4.5 is not done without it (no simulator substitutes). Recording is a 20-minute job at any venue with `noplayout-capture` during a game or practice; asked for at M4.5 start. Later decoders (Daktronics etc.) carry the same gate individually. |
| R12 | Basketball OGraf Package not supplied, or its licence forbids a public repo | not on hand when M1 ends | M1 gate | M1 is done on EBU examples; the Basketball gate moves to "required before M4 done". Kept outside the repo (env var path) until its licence is checked. |
| R13 | Hosted CI runners can't run CEF OSR (no GPU, session) on a lane | conformance/pixel jobs crash on that lane after software-GL flags (`--disable-gpu`, SwiftShader) | M1 CI | That lane builds + unit-tests only; the CEF tests run locally there, recorded in STATUS.md. Linux (Xvfb) and macOS arm64 must run them. |
| R14 | Program + preview browsers × 4 channels exhaust CPU/GPU | 4 × 1080p59.94 with previews drops on reference hardware | M6 soak variant | Preview browsers drop to 15 fps or pause when no monitor is watching; "up to 4 channels" stays, with Health showing the cost. |
| R15 | Receivers disagree on NDI alpha (vMix premultiplied option, TriCaster switch, DistroAV) | a supported receiver shows dark or bright edges with the default | M2 interop matrix (NDI Studio Monitor, OBS+DistroAV, vMix if available) | Per-mode fill-type default + hint text; documented receiver settings. |
| R16 | OS grabs Ctrl+Space (macOS input sources, Linux IBus/fcitx) | the typing-safe TAKE alternate never reaches the app | Settings → Hotkeys "Test" box (truth), plus macOS `com.apple.symbolichotkeys` keys 60/61 and a Linux IBus/fcitx process check for the warning line | User remaps; warning shown (answers Usopp's question: the Test box detects it on every OS, the warning can be predicted on macOS and Linux only). |
| R17 | Single engine process is a single point of failure for all channels | supervised engine restart → first restored NDI frame > 5 s | M6 crash test | Keep the engine small and panic-free (`panic = "abort"` is not used; per-task `catch_unwind` around plug-in decoders); restart budget measured. |
| R18 | Unauthenticated LAN control (IDEA default) | someone on the production LAN takes or clears graphics | logged source addresses | Token + scope under Advanced; documented; Caleb may flip the default (E-034). |
| R19 | Shared Anthropic budget runs out mid-milestone | Fable unavailable for review | `hermes usage` | Per model-routing: build on flash-next/Codex, frontier review queued; never silently downgraded. |

---

## 7. Milestones and tasks

Rules carried into every milestone: each ends runnable, with `cargo fmt --check`, `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo test --workspace`, `npm run check` and `npm test` green on all CI lanes, a "How to try it" section
in README.md, `STATUS.md` updated and a git tag (`m0`…`m6`, `m4.5`). **No fake features:** anything partial carries
`// TODO(Mx):` plus a STATUS.md line; NDI and CEF are never simulated in a test that claims they work. Real-output tests skip
in CI with a printed reason only when the runtime is absent, and they run locally before the milestone is called done.
Every design choice in a milestone lands with its ADR in the same PR (org ADR rule). Cards are Rayleigh's to create after
approval; build-tier routing follows `model-routing` (flash-next lane first, frontier review). The clock, NDI conversion
and recovery code deserve frontier attention because they are timing- and on-air-critical.

### M0: Workspace skeleton, CI green
Tasks:
1. Repo basics: `LICENSE` (Apache-2.0, D-010) + `NOTICE` (NDI trademark line, IJG credit, CEF/Chromium and OGraf notices),
   `README.md`, `STATUS.md`, `.gitignore`, `rust-toolchain.toml` (1.100.0),
   workspace `Cargo.toml` with all §3 pins, `deny.toml` (E-032), `.editorconfig`, `docs/adr/` with `README.md` index and one
   ADR per accepted DECISIONS.md entry (E-035). `AGENTS.md`/`CLAUDE.md` from the GeneralDocuments template: headless workers
   cannot write those files, so the body goes in a `.txt` file in the PR for Caleb or an interactive session to apply.
2. Crate skeletons (§2.2) that compile on every lane; `noplayout-proto` with `Hello/Ping/Pong` and round-trip tests.
3. `noplayout-engine`: starts, takes the single-instance lock, serves `GET /api/v1/health` on 127.0.0.1, logs to rolling files.
4. `noplayout-renderer`: `execute_process` → `initialize` → one OSR browser on `about:blank` → `Hello{cef_version}` over IPC →
   clean shutdown on `Shutdown`.
5. `noplayout-app`: Tauri v2 window with a Svelte placeholder that spawns or attaches to the engine and shows "Engine:
   running · Renderer: CEF 156.0.31 OK" from `/api/v1/health`.
6. `xtask`: `cargo xtask cef` (export-cef-dir into `target/cef/<ver>/<target>`), `cargo xtask run` (dev-bundles the renderer
   on macOS with `bundle-cef-app`, then runs the app). **Spike S5:** macOS dev bundle layout works from the workspace.
7. CI workflow (§8) on four lanes, plus `cargo deny check`.

Done when: all four CI lanes are green on the M0 commit, and on a real Windows 11 x64, macOS arm64 and Linux x86_64 desktop
`cargo xtask run` opens the noPlayout window showing "Engine: running" and the renderer's CEF version (screenshot per OS in
the PR). macOS x86_64 is proven by its CI lane running the same smoke headlessly (`noplayout-engine --self-test` spawns a
renderer and checks `Hello`).

### M1: OGraf host runtime + CEF offscreen renderer
Tasks:
1. Vendor OGraf v1 (schemas, examples) at 945f8bb; `library` import + validation + serving (E-024).
2. `web/host`: `ograf-host.js` per §2.5, `renderer.html`, vitest suite with misbehaving fake graphics.
3. Host protocol on the engine (`hostbus`) with snapshots; a dev CLI `noplayout-engine ctl load|play|update|stop|custom`
   drives it (the rundown comes in M4).
4. Renderer program browser: transparent OSR, mailbox, channel clock, mode A and B behind a flag; `--dump-png <dir> --frames N`.
5. **Spike S1** (frame drive A vs B, per OS, 59.94 and 50) and **S2** (readback + copy cost at 1080p and 2160p) with the
   frame-counter fixture; results and the per-OS mode choice recorded as an ADR.
6. Conformance harness (`tests/conformance`): every EBU example through load → play → each step → update → each custom action
   → stop → dispose in the real CEF renderer; assert return payloads, zero console errors, `currentStep` semantics (O6/O7),
   `stepCount: -1` handling.
7. Pixel tests: the alpha-ramp fixture and a gradient/shape fixture rendered to PNG and compared with goldens (shape goldens
   are OS-independent within ±2 LSB; text goldens, if any, are per OS).
8. Basketball gate (needs Caleb's package, Q1): the same conformance + PNG checks on its 6 graphics, unmodified, with the
   package outside the repo (`NOPLAYOUT_BASKETBALL_PKG`) until its licence is checked.

Done when: (a) the conformance suite passes for all nine EBU examples (`bar-chart, headline, l3rd-name, minimal, ograf-logo,
renderer-test, responsive-lower-third, scoreboard, weather`) on Linux and macOS arm64 in CI and on Windows locally, with no
console errors; (b) the EBU `scoreboard` renders and animates offscreen at 1080p59.94 and a saved PNG sequence shows correct
alpha: background alpha 0, antialiased edges between 1 and 254, and the alpha-ramp fixture's 16 patches within ±1 LSB of their
nominal values; (c) the S1/S2 ADR exists with measured numbers. IDEA's literal M1 test ("the Basketball scorebug renders and
animates offscreen") is item 8, run the day the package arrives; M4 cannot be called done without it.

### M2: NDI output: Fill+Alpha and Separate Fill & Key, test pattern, health
Tasks:
1. `noplayout-ndi`: trait, grafton-ndi implementation, runtime detection with searched paths and the OS-specific install link,
   "Check again".
2. `noplayout-pixels`: un-premultiply (LUT, threads), shaped/unshaped fill, key, with exhaustive unit tests over all (a, c)
   pairs and a criterion-style bench (`cargo bench -p noplayout-pixels`).
3. Sender thread (§5 steps 6–7), counters, metadata, `connection_count` polling.
4. Test pattern graphic (§2.10.3) and `POST /api/v1/outputs/test-pattern`.
5. Monitor tap + frames socket + `/ws/monitor` + **Spike S3** (IPC and JPEG cost at 4 channels × 2 sources).
6. Minimal operator-facing surfaces from the approved mockups: Program monitor tile and the Health panel's channel block
   (frame rate, dropped, duplicated, receivers), NDI branding (link, ®, About text).
7. NDI loopback test (`tests/ndi_loopback.rs`, real runtime; prints "skipped: NDI runtime not found (searched …)" otherwise):
   receive FILL+KEY or Fill+Alpha with `Receiver`, check resolution, `FrameRate` 60000/1001, timecode equality per pair, `k`
   continuity, and the alpha round trip `premul → straight → re-premul` within 1 LSB for a ≥ 16.
8. Interop matrix: NDI Studio Monitor (NDI Tools, not redistributed), OBS + DistroAV, vMix if a machine has it.

Done when: with the NDI 6.4 runtime installed on two machines, NDI Studio Monitor and OBS (DistroAV) on the second machine
show `NOPLAYOUT Main` keyed cleanly over a background, and `NOPLAYOUT Main FILL` / `KEY` line up when keyed (no edge
fringe on the alpha ramp); the loopback test passes a 10-minute run with `fill_key_tc_mismatch = 0` and no unexplained holes
in `k`; the test pattern appears on every NDI source; the Health block shows receiver counts that change when a receiver
connects and disconnects. Separately: NDI runtime absent → red light with the install link, web side unaffected, no crash.
Bench check (D-010): on Caleb's switcher, Separate Fill & Key with the default **Shaped** FILL keys the alpha-ramp and an
antialiased-text graphic with no dark or bright fringe at the switcher's normal CG key setting. If Unshaped looks right
instead, the default flips before M2 closes and the result is recorded in the E-009 ADR.

### M3: Web outputs
Tasks:
1. `/out/<ch>/program|fill|key|preview` role pages (§2.10.2), with access scope and token handling.
2. Host protocol completion: revisioned snapshots, reconnect with `lastRevision`, mirror-divergence counter.
3. Port-in-use detection with "next free port" (UI.md §1.5 wording) and the frame-lock note.
4. Tests: late-join (a second CEF page joins mid-show after play + 2 steps + update, and after a 2 s settle its pixels equal the
   program page's within tolerance); reconnect (kill the WS, actions continue, the page converges); `/key` vs NDI KEY pixel
   test (R8); shaped/unshaped `/fill` vs NDI FILL.
5. `docs/how-to/obs.md` with a curl script that drives the EBU scoreboard through the API.

Done when: an OBS Browser Source (current OBS release) on `/out/main/program` shows the EBU scoreboard with alpha over another
source, driven only by the documented curl script (load, play, score update, next step, custom action, stop); a browser tab
opened halfway through shows the same state without replaying animations; the late-join and key pixel tests pass in CI.

### M4: Playout mode + settings store + Output Setup wizard + OGraf Server API (+ Take IDs)
Tasks:
1. Settings store (§2.11) with migrations framework, last-good, rejected-settings report; fault tests (truncated file,
   invalid value, unknown future version, read-only directory).
2. Shows + rundown model with Take IDs (E-021): uniqueness (inline error on duplicate, D-010), auto-increment assignment,
   renumber, paste remap, undo journal.
3. Playout engine (§2.7): preload cue, TAKE, NEXT STEP, Take & advance, OUT, CLEAR LAYER/ALL (cut), live update and pending
   changes, custom actions with schema-validated payload forms, recovery replay. Preview browser in the renderer.
4. API: OGraf Server API (`/ograf/v1`) with a conformance test generated from `server-api.yaml`; `/api/v1` with Take ID routes;
   `/ws/events` with tally; OpenAPI at `/api/docs`.
5. RossTalk-compatible listener (§2.9.3; D-010, off by default), with a test transcript of the commands the Companion
   XPression module and a Carbonite CG-device preset send.
6. UI to Usopp's approved rev 2: shell + top bar lights, Output Setup wizard, Playout (rundown with Take ID column, monitors,
   on-air strip, schema-generated data form, transport with Take ID box and its pending/unknown states), Settings (Outputs,
   Hotkeys incl. Take ID keys and the Ctrl+Space Test box, Remote control incl. "Fire by Take ID"), Health, restore and
   rejected-settings prompts. Hotkey rules per UI.md §0.6 (paused while typing, typing-safe alternates).
7. Channel restart on format/NDI-mode change with confirmation; autosave; restore prompt (Enter = Restore).
8. `docs/test-shows/basketball-show.md`: a 30-step show script (pregame, scorebug on, lower thirds by Take ID, matchup steps,
   ticker, clears), plus `scripts/show-api.sh` that runs the same show through HTTP only.

Done when: from a clean profile, the wizard sets up 2 channels (Web + NDI) and the test pattern runs; the Basketball package
(and EBU examples) import; the 30-step show runs **twice**: once from the UI by keyboard only (Take ID + Enter + Space, →, Esc,
Shift+Esc) and once from `scripts/show-api.sh` alone, with NDI and web outputs showing the right graphic and step at every
checkpoint (the API run also asserts engine state and program-renderer pixels at checkpoints); a Companion button using the
documented HTTP request takes a graphic by Take ID, and with the RossTalk listener switched on, the Companion XPression module
(or `nc` sending the same lines) cues and takes by Take ID; typing a Take ID that another item already uses shows the inline
error and changes nothing; killing the app mid-show and relaunching offers Restore and Enter brings the
graphics back on their steps; truncating `settings.json` loads last-good with the banner naming what was rejected; Usopp's
review of the built screens against the mockups has no open blocking findings.

### M4.5: Data sources, OES (D-008, D-011)
Tasks:
1. `noplayout-datasource`: transports (serial, tcp-dial, tcp-listen, udp receive incl. multicast and serial-to-UDP bridges,
   replay), per-peer framers, states and backoff, raw ring (200 messages), health counters; unit tests incl. the two-sender
   interleaved-UDP test with frames split across datagrams.
2. Capture format + `noplayout-capture` CLI (record from any transport; `--info` summary).
3. Decoder plug-in contract: `trait Decoder`, static registry with one cargo feature per decoder, the shared decoder
   conformance test (every registered decoder must ship captures + a golden timeline), `catch_unwind` isolation, and
   `docs/how-to/add-a-decoder.md` (the checklist from §2.8).
4. `noplayout-decoder-oes`: ported from Caleb's `protocol.py` (all nine sport layouts it covers), its test vectors carried over,
   capture-based timeline tests.
5. Field store, mapping engine with formatters, override semantics, coalescer, hold-on-loss with the amber "source lost" light
   (D-009); per-computer connection settings vs show-saved mappings (D-009); API routes; `/ws/events` `datasource` events.
6. UI per UI.md §5 (Data sources screen, Add dialog with live check, Values, Raw data, item-first Mapping), §2.8 (linked/manual
   fields, banners, DATA lights), Health data section, "Not set up on this computer" flow.
7. Fixture: Caleb (or someone at a venue) records at least 20 minutes of a real game, or a practice with the clock and score
   running, from an OES console with `noplayout-capture`, ideally one over serial and one through the venue's UDP bridge.
   Captures stay out of the public repo (`NOPLAYOUT_CAPTURE_DIR`) unless Caleb approves a short excerpt for CI (Q2).

Done when: the recorded OES byte stream, replayed through **each transport** (serial over a pty pair on Linux, TCP dial and
listen, UDP including a two-sender interleave with frames split across datagrams), produces exactly the golden field timeline
(decoded once and checked by Caleb against the game's score sheet or video); with the capture driving a scorebug, "Main Clock →
Game clock" updates the on-air graphic within 100 ms plus one frame of the message; a manual override survives the next 100
source messages and "Back to live" restores it; stopping the stream holds every field at its last value and turns the light
amber within the Quiet timeout; on real hardware, pulling the USB serial adapter shows Lost within 1 s and replugging it
recovers without a restart; a show saved on one computer opens on another with its mappings intact and its source marked "Not
set up on this computer"; the OES PR's diff outside `crates/noplayout-decoder-oes/` is limited to the registry line, the
feature line, docs and STATUS.md (the plug-in boundary holds). A simulated decoder or synthetic stream does not count for
this gate.

Size: about 4 build cards (transports + capture + plug-in contract, OES decoder, mapping/override engine, UI) plus a review
card. A later decoder (Daktronics AS5000 next, D-010) is one card plus its capture.

### M5: Edit mode + OGraf export
Tasks:
1. Designed-graphic document model (elements, bindings, fields, segments, keyframes, custom actions), with versioned JSON.
2. Generated `graphic.mjs`: a small embedded runtime (Web Animations API) with no external dependencies; segments In / Step n
   / Out map to `playAction` steps and `stopAction`; update transitions; custom actions jump to a segment or set a field.
3. Data fields → OGraf `schema` with GDD types (Usopp Δ13 plain names in the UI, correct GDD types in the file).
4. Fonts: embed with an OS/2 `fsType` licence check; warn and fall back when embedding is not allowed.
5. Thumbnail rendered by the engine (CEF renderer frame → PNG).
6. Editor canvas: frames from a CEF renderer at editor resolution with handles drawn by Svelte over the image (E-025, D-010), the
   timeline, safe guides, grid/snap, reference image. Coded-graphic editing (name, thumbnail, default data).
7. Export: validate against the vendored v1 schema before saving; zip or folder.
8. Third-party check: SuperFly `ograf-server` (MIT) pinned to a commit in a test container, used as a smoke test only.

Done when: a lower third (bar + two bound text fields, In / Step 1 / Out, one custom action) is built with no code; its export
validates against the v1 schema; it plays in noPlayout on NDI and web, and in the pinned ograf-server via its API (scripted);
at 5 timeline points the exported graphic's CEF frames match the editor's preview frames within tolerance; a font whose
`fsType` forbids embedding shows the warning.

### M6: Hardening, watchdog, recovery, soak, packaging
Tasks:
1. Watchdog and recovery as specified (§4.2, §2.7 replay); crash tests that kill the renderer process tree, only the CEF
   render subprocess, and the engine.
2. Soak harness: 2 h at 1080p59.94, scorebug clock running (data-source replay feeding it) + ticker cycling; records Health
   every second, RSS, and NDI-receiver-side `k`/counter continuity.
3. Copy diagnostics, log rotation, port-conflict flow, all error texts from UI.md checked.
4. Packaging: Windows NSIS (and MSI if WiX works on CI), macOS `.dmg` with the custom bundling (R6) and documented signing +
   notarization steps (unsigned until Caleb provides credentials, which is an approval item), Linux AppImage + `.deb`
   (`avahi-daemon`/`libavahi-client3` as dependencies for NDI). NDI runtime detected, not bundled (E-011). Licence/NOTICE files
   (NDI EULA flow-down text, Chromium/CEF, IJG, OGraf).
5. Release workflow (tag → installers as artifacts). `osv-scanner` + `cargo deny` clean.
6. Reference hardware named by Caleb before the soak (IDEA: an Apple M-series Mac mini and a mid-range Windows PC with a
   discrete GPU; not the Mac Studio while its timer is degraded).

Done when: every IDEA §9 test passes: the conformance and pixel suites, NDI loopback, the 2-hour soak with `slots_skipped = 0`,
no unexplained holes in the receiver-side counter, and RSS growth under 5 % after the first 10 minutes on both reference
machines; the crash test (kill the renderer mid-show) restores output with the same graphics and steps within 2 s, measured at
an NDI receiver, 20 out of 20 runs; installers are produced by CI for Windows x64, macOS arm64 and x86_64, and Linux x86_64,
and each installs on a clean machine or VM and passes the M4 show smoke.

### Spikes (measure before building on them)
| ID | Question | When | Output |
|---|---|---|---|
| S1 | External begin-frame (A) vs free-run (B), per OS, 59.94 and 50 | M1 | ADR + numbers; per-OS mode |
| S2 | `on_paint` copy + conversion cost at 1080p/2160p | M1–M2 | numbers vs the 8 ms budget (R2/R3) |
| S3 | Monitor JPEG + IPC cost, 4 channels × 2 sources | M2 | CPU %, latency; go/no-go for raw fallback |
| S4 | Linux CEF OSR under Xvfb, Wayland and `--ozone-platform=headless` | M0–M1 | CI and desktop flags |
| S5 | macOS renderer helper layout from a dev tree and inside the app bundle | M0, M6 | xtask bundling |
| S6 | Ctrl+Space and other OS-grabbed keys per OS, as seen by the Tauri webview | M4 | hotkey warning table (R16) |

---

## 8. CI design (D-003)

### 8.1 While the repo is public
GitHub Actions, GitHub-hosted standard runners (free and unlimited on public repos):

| Lane | Runner | Target | Runs |
|---|---|---|---|
| linux | `ubuntu-latest` (Ubuntu 26.04) | x86_64-unknown-linux-gnu | fmt, clippy, test, build --release, cargo-deny, npm check/test, CEF conformance + pixel tests under Xvfb |
| windows | `windows-latest` (Server 2025) | x86_64-pc-windows-msvc | clippy, test, build --release, npm; CEF tests once S4/R13 prove them |
| macos-arm64 | `macos-latest` (macOS 26, M1) | aarch64-apple-darwin | clippy, test, build --release, npm, CEF conformance + pixel tests |
| macos-x64 | `macos-15-intel` (available until August 2027) | x86_64-apple-darwin | clippy, test, build --release, renderer self-test |

- Triggers: `push` to branches, `pull_request` (never `pull_request_target`). `permissions: contents: read`. No secrets in
  the public phase. Third-party actions pinned by commit SHA (checkout, rust-cache, cache, setup-node).
- Caches: `Swatinem/rust-cache` per lane; `actions/cache` for the CEF minimal distribution keyed on
  `cef-<os>-<arch>-156.7.0+156.0.31` (141–340 MB compressed per OS; four OS caches fit well inside the 10 GB free cache);
  `npm ci` cache. A cache miss runs `cargo xtask cef`.
- Linux lane installs `libudev-dev`, the WebKitGTK 4.1 dev packages Tauri v2 needs, and Xvfb.
- NDI loopback and other real-runtime tests print a skip reason in CI (the runtime can't be redistributed into a public repo
  or cache). They run locally on a machine with NDI before M2/M6 are called done.
- A `concurrency` group cancels superseded runs on the same branch. Nightly on `main`: the full CEF test set on every lane
  that supports it, plus `cargo deny check advisories`.
- Release workflow (M6): on tags `v*`, build installers per lane and attach them as workflow artifacts. Signing and
  notarization need secrets, so that step is added only with Caleb's approval and only once the repo is private (or with
  environment-protected secrets that `pull_request` runs can't reach).

### 8.2 When the repo goes private (plan, Robin's pricing)
- Private-repo rates: Linux $0.006/min, Windows $0.010/min, macOS $0.062/min; the Free plan includes $12/month. At about
  20 min × 30 pushes, Linux + Windows ≈ $9.60/month (inside the allowance); each macOS lane ≈ $37/month.
- Plan: Linux + Windows stay on hosted runners. Both macOS lanes move to a **self-hosted runner on the Mac Studio**, only after
  the repo is private (GitHub advises against self-hosted runners on public repos), in a dedicated macOS user with no org
  credentials, ephemeral work directories, and the CEF cache local. x86_64-apple-darwin builds natively by cross-compiling and
  its tests run under Rosetta (macOS 27 is Apple's last Rosetta release). Setting it up is a Caleb-approved change (runner
  registration token, a new user account, home infra coordination with vegapunk).
- Fallbacks if the Mac Studio is unavailable: macOS lanes on `main` merges and nightly only (≈ 6 runs/month ≈ $15/month for
  both), PR builds Linux + Windows only.
- The repo must have its Apache-2.0 licence (D-010) and a clean history audit (no NDI SDK, no facility data, no secrets) before the switch.

### 8.3 Intel macOS sunset (D-010, E-028)
Built and tested on `macos-15-intel` until GitHub retires it (August 2027), then build-only: cross-compiled on Apple Silicon,
smoke-tested under Rosetta, labelled "community-tested" in the README.

---

## 9. Cross-team notes (for Rayleigh to route)
- **Usopp (UI):** (1) the Shaped/Unshaped Advanced control has a per-mode default and hint (D-010). Fill + Alpha defaults to
  *Unshaped* (NDI's straight-alpha rule), hint "Choose Shaped only if your receiver is set to premultiplied (edges look bright
  or haloed)". Separate Fill & Key defaults to *Shaped*, IDEA's hint "Choose Unshaped if edges look dark on your switcher".
  The wizard mockup marks it TBD; these are the values to draw. (2) Ctrl+Space: the Test box can detect a grabbed key on every
  OS; the predictive warning line is possible on macOS (symbolic-hotkeys preference) and Linux (IBus/fcitx running) but not
  Windows. (3) API route names are fixed in §2.9.2 (`/api/v1/takeid/{n}/cue|take|next-step|out`) for Settings → Remote
  control. (4) Take & advance exists in the API and RossTalk; it has no default hotkey (D-007). (5) D-009 needs one new state
  in the data-source screens: a show's source that has no connection on this computer ("Not set up on this computer", with the
  Add dialog prefilled), plus the RossTalk on/off switch and port under Remote control → Advanced (D-010).
- **vegapunk:** nothing until the private-repo switch (§8.2 self-hosted runner) and any lab NDI machines.
- **Robin:** no new research needed for M0–M4.5. Before the first public release: re-diff the NDI 6.4 EULA against 6.3.2 (her
  noted gap).
- **Rayleigh:** D-009…D-011 were committed on `design/plan` (5cfe011, 5f2d1ab, 98016d4), not on `main` (still 48bb009). They
  reach `main` when this branch merges, or sooner if you move them; I have not touched `main`.

---

## Open questions for Caleb

D-009, D-010 and D-011 settled everything else I had. Two questions remain that change what gets built or tested; each has
my recommended answer, which the plan assumes until you say otherwise.

1. **Basketball OGraf Package: when, and under what licence?** M1 closes on the nine EBU examples; the Basketball checks run
   the day the package arrives, and M4 cannot close without it. **Recommend:** keep it outside the public repo (tests read it
   from a path you set) unless its licence allows redistribution, in which case it goes in `fixtures/` with its licence file.
2. **OES capture for M4.5: who records it, and may a short excerpt be public?** The data-source gate needs a recorded byte
   stream from a real OES console (ideally one over serial and one through a venue's UDP bridge). **Recommend:** you or a
   venue operator records 20 minutes with `noplayout-capture` once M4.5 starts; the full recording stays private, and you
   approve a 60-second excerpt (no team names on the board) to commit so CI runs the real-stream test on every push. If no
   excerpt can be public, CI runs the test only where `NOPLAYOUT_CAPTURE_DIR` is set and M4.5 is signed off locally.

## Process note
- Tier: written by Franky on the frontier model (card t_95b8a2c9), no subagents: the inputs were already researched by Robin,
  and the remaining checks were mechanical (crates.io / npm / GitHub API calls, reading published crate sources and Caleb's
  prior-art code). Rev 1 was revised in the same run for D-009…D-011, which landed mid-run.
- Verified myself on 2026-11-26: every crate pin in §3 against the crates.io API (version, date, licence, MSRV); the cef,
  grafton-ndi, daktronics-allsport-5000 and interprocess APIs this plan names, read in the downloaded `.crate` sources at those
  exact versions; axum/tower-http/utoipa-axum/tokio-serial/jsonschema dependency ranges via the crates.io dependencies API;
  the OGraf HEAD commit (945f8bb), its nine examples and the Server API paths and RenderTarget shape in `server-api.yaml` at that
  commit; npm latest versions and the svelte-check TypeScript peer range; Rust stable 1.100.0; CEF #4311 still open; OES framing
  (STX/type/CR, no checksum), nine sport parsers and 9600 baud in `unc-virtual-score-server` (MIT).
- Taken from Robin without re-checking: CEF behaviour quotes (#4166, #4357, premultiplied OSR output), the NDI alpha documentation
  and receiver behaviour, CI runner labels and pricing, the CasparCG/OBS code findings, RossTalk/XPression command semantics.
- New findings beyond Robin's report: `daktronics-allsport-5000` 0.5.0 has zero tests and skips the packet checksum;
  `jsonschema`'s default features enable HTTP schema resolution; `serialport` is MPL-2.0; TypeScript 7 is outside svelte-check's
  supported range; OES has no checksum, so decode-error detection relies on length and field ranges.
- Not done: no code compiled or run. S1–S6 are the first measurements. No build cards were created (Rayleigh creates them after
  Caleb approves). Nothing pushed; no GitHub repo created.
