# noPlayout: build plan

- **Status:** design draft **rev 2** for cold review 2 (card t_0d2dd9ce), then Caleb's approval. Nothing here is built yet.
  Rev 1 (87908b9) got REWORK in cold review 1 (`~/OrgWork/projects/noplayout/reviews/plan-review-1.md`, 11 findings, 4
  blocking). Every finding is answered in **§ Review 1 disposition** at the end; the requirement questions it raised are in
  **§ Open questions for Caleb**.
- **Author:** Franky (CTO). Rev 1 card t_95b8a2c9; rev 2 card t_63bf63cf, 2026-11-26. Branch `design/plan` (local only),
  with `main` (D-012…D-014) merged in at 96a24c2.
- **Inputs:** `IDEA.md` (source of truth), `docs/BUILD-BRIEF.md`, `docs/01-decisions.md` **D-001…D-014** (D-009 UI answers,
  D-010 research picks, D-011 OES-only decoder, D-012/D-014 Basketball package as a public Apache-2.0 fixture with OFL fonts,
  D-013 OES test corpus from `unc-virtual-score-server` are binding here), Robin's `2026-11-26-noplayout-foundations.md` +
  addendum, Robin's `2026-10-30-replayserver-ndi-license.md`, Usopp's **`UI.md` rev 3 (final build target)** +
  `noplayout-mockups.pdf` (24 pages) at `~/OrgWork/projects/noplayout/design/`, cold review 1.
- **Companion file:** `DECISIONS.md` (engineering decision record, entries E-001…E-051). Where this plan says "E-0nn", the
  reasoning is there. Rev 2 supersedes E-008, E-012 (in part) and E-019, and amends E-001, E-006, E-013, E-014, E-017,
  E-020, E-023 and E-033; the old text is kept and marked.

Every version below was checked against crates.io / npm on 2026-11-26, and the APIs this plan depends on were read in the
published crate source at exactly that version (see §3, "Verified"), except where §3 says "version-checked only". Nothing was
compiled or run yet: frame pacing, readback cost, IPC cost and **recovery time** are still design inputs. Spikes S1–S8 measure
them before anything is built on top (§7, "Spikes"); the recovery spike runs in M1, not M6.

---

## 1. Summary

noPlayout is four kinds of process on one machine (rev 2 adds the output process and the standby renderer, E-040/E-041):

1. **`noplayout-app`**: the Tauri v2 + Svelte window. It holds no show state. It starts and watches the engine and talks to it
   through the same HTTP/WebSocket API that Companion uses.
2. **`noplayout-engine`**: headless Rust. It owns the graphics library, shows and rundowns (with Take IDs), playout state,
   the on-air action journal, settings, data sources, the HTTP server (API, web outputs, graphics files) and the supervisor.
3. **`noplayout-output`**, one per channel: small, no CEF. It owns the channel's 60000/1001-style clock, reads finished
   paints from shared memory, converts pixels and sends NDI with explicit timecode. It keeps sending (holding the last good
   frame) while a renderer fails over, so NDI receivers never lose the source.
4. **`noplayout-renderer`**, **two per channel**: CEF offscreen. One is **active** (its paints go to air), the other is a
   **warm standby** that receives the same live action stream and keeps the same graphics running, painting at a low rate.
   The standby also hosts the channel's preview page. If the active renderer dies or hangs, the standby is promoted in
   about a frame or two and a new standby is started behind it.

All pages (the CEF program pages, preview, web outputs, the editor preview) run one OGraf host runtime, `ograf-host.js`,
and receive the same ordered, timestamped action stream from the engine over a WebSocket. Only the active renderer's program
page is authoritative, and only pages holding that renderer's launch token can claim the role. Recovery has an explicit
contract (§2.13): a live standby restores everything the graphics show, including running internal clocks; a cold restart
replays the full on-air journal (never silently truncated) and states plainly what it cannot rebuild.

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
│ WS: /ws/events · /ws/host/<ch> · /ws/monitor/<ch>/<src> · supervisor/watchdog · journal │
└──────┬───────────────────────────────┬──────────────────────────────┬──────────────────┘
       │ control socket               │ control socket ×2             │ HTTP + WS (host
       │ (one per channel)            │ (active, standby)             │ protocol), loopback
┌──────▼ noplayout-output --channel <id> ─┐   ┌──────▼ noplayout-renderer --channel <id> --slot a|b ───┐
│ channel clock (pacing authority)        │   │ CEF browser process (own root_cache_path per slot)    │
│ shm reader: newest READY paint ◄────────┼───┤ program browser: writes paints into its shm ring      │
│ convert (unpremul / key) → pair snapshot│   │   (active: every slot; standby: every 6th slot)        │
│ sender threads: FILL, KEY (or F+A)      │   │ preview browser (standby slot only): 30 fps max        │
│ sender-progress monitor · oracle tap    │   │ preview monitor tap → JPEG → engine                    │
│ program monitor tap → JPEG → engine     │   │ CEF helper subprocesses = the same executable          │
│ replica-divergence check (a vs b)       │   └────────────────────────────────────────────────────────┘
└─────────────────────────────────────────┘     two per channel: one ACTIVE, one WARM STANDBY (E-041)
External: OBS/vMix/CasparCG browser sources → /out/*; Companion/Stream Deck → /api/v1, /ograf/v1 and the RossTalk-compatible
listener on 7788 (off by default, D-010); NDI receivers ← output-process senders; scoreboards → engine data-source transports.
```

**Why rev 2 splits output from rendering (E-040, E-041; review 1 F1/F2).** In rev 1 the NDI senders lived in the renderer,
so a renderer death removed the NDI sources, receivers had to rediscover them, and a fresh CEF had to rebuild the graphics'
state from a log that could not hold all of it. Now:
- the **output process** (no CEF, small) owns the channel clock and the NDI senders. It survives any renderer failure and keeps
  sending the last good frame with fresh timecode, so receivers never lose the source;
- two **renderer processes** per channel run the same program page and receive the same live action stream. The active one
  feeds air; the standby paints at a sixth of the rate and is promoted in one or two frames when the active one fails. The
  standby has executed every action itself, in real time, so its state includes things no log can rebuild (a graphic's own
  running clock, counters changed by custom actions). After promotion a new standby is started behind it (§2.13).
- Full-rate pixels now cross one process boundary, through shared memory written in the `on_paint` copy that rev 1 already
  had to make (E-042), so there is no extra copy.

Why the engine is its own process (E-001): a UI crash or a webview hang never drops air, the engine can run headless on a
rack PC (controlled from Companion), and the core can be tested in CI without Tauri. The app spawns the engine if none is
running, or attaches to the running one. Quitting the app asks the engine to shut down gracefully. If the app *crashes*,
the engine keeps running and the next app launch reattaches without a prompt.

Engine death (E-033): renderers and output processes lose their control sockets. Output processes keep sending the last good
frame for a 3 s grace period, then everything exits (no orphan CEF processes). The app's supervisor restarts the engine. A
**supervised restart** restores on-air state automatically from the persisted journal (§2.13, cold path). A **cold start**
(machine or app relaunch) asks first, as IDEA §5.1 says ("Restore them?", Enter = Restore). An **app crash with the engine
still running** is neither: the app reattaches and nothing is replayed (the engine never stopped).
Failure condition for this design: engine restart to first restored NDI frame > 5 s (measured in M6; R17).

### 2.2 Workspace layout (E-002)

```
noPlayout/
├─ Cargo.toml                 workspace; [workspace.dependencies] carries every pin in §3
├─ rust-toolchain.toml        1.100.0, edition 2024 (E-029)
├─ deny.toml                  cargo-deny licence + advisory gate (E-032)
├─ crates/
│  ├─ noplayout-core/         lib + [[bin]] noplayout-engine (D-001 name kept for the library)
│  ├─ noplayout-renderer/     bin: CEF host; writes paints to shared memory; never sends NDI (D-001)
│  ├─ noplayout-output/       bin: channel clock, shm reader, conversion, NDI senders, oracle tap (E-040)
│  ├─ noplayout-app/          bin: Tauri v2 shell (D-001)
│  ├─ noplayout-proto/        lib: IPC + host-WS message types, versioned; no heavy deps
│  ├─ noplayout-frames/       lib: the shared-memory paint ring protocol (E-042), used by renderer and output
│  ├─ noplayout-ndi/          lib: NdiOutput/NdiReceiver traits + grafton-ndi impl, runtime detection
│  ├─ noplayout-pixels/       lib: unpremultiply, key, shaped/unshaped fill, monitor scale + JPEG, test pattern math
│  ├─ noplayout-oracle/       lib (test + harness only): decodes the frame-oracle and recovery-probe patches from pixels
│  ├─ noplayout-datasource/   lib + [[bin]] noplayout-capture: transports, framework, capture/replay format
│  ├─ noplayout-decoder-oes/  lib: the OES decoder plug-in (one crate per decoder, E-022)
│  └─ xtask/                  cargo xtask: fetch CEF, dev-bundle the renderer (macOS), run, package, preflight (§8.2)
├─ web/
│  ├─ host/                   TypeScript: ograf-host.js, renderer.html, out pages (program/fill/key/preview)
│  └─ ui/                     Svelte 5 app for Tauri
├─ third_party/ograf/         EBU OGraf v1 schemas + examples vendored at 945f8bb (MIT, LICENSE kept)
├─ fixtures/                  our own test graphics (alpha ramp, frame-oracle, recovery-probe, test pattern, designed/*) +
│                             goldens; basketball-ograf-package/ (Caleb's, Apache-2.0, Barlow fonts OFL; D-014)
├─ docs/  (adr/, test-shows/, how-to)   STATUS.md   README.md   LICENSE (Apache-2.0, D-010)   NOTICE
```

`noplayout-renderer` must not depend on `noplayout-core`: it only links `proto`, `frames` and `cef` (plus `pixels` for the
preview JPEG). `noplayout-output` links `proto`, `frames`, `ndi` and `pixels`, and never CEF. That keeps the CEF build graph
small, keeps show logic out of both, and lets the output process start in well under a second.

### 2.3 Engine internals (`noplayout-core`)

| Module | Owns | Notes |
|---|---|---|
| `library` | import (folder/zip), validation, storage, serving | Validates the manifest against the vendored v1 schema (jsonschema, offline, E-024). Rejects `supportsRealTime: false` (v1 non-goal). Zip import uses `ZipFile::enclosed_name` (no path escape), size caps. Each package version is stored content-addressed: `library/<graphic-id>/<sha256-12>/`, served at `/graphics/<id>/<hash>/…`, so `main` and assets resolve relative to the manifest and a re-imported version never changes an on-air instance. Classification Coded / Designed. |
| `show` | shows, groups, rundown items, Take IDs, item data, per-item data-source mappings | §2.6. Autosave debounced 500 ms, atomic. |
| `playout` | per-channel layer stack, instances (with generation), cue/take/next/out/clear, the on-air journal, revisions, result commit | The single state authority. Every change bumps the channel revision and emits an event. §2.7, §2.13, §2.14. |
| `hostbus` | the host WebSocket: registration and authority (§2.14), ordered timestamped actions to every page of a channel, snapshot/live handoff, result collection | §4.3. |
| `outputs` | output config per channel (web, NDI), NDI name rules, runtime presence, status lights | Builds the output-process `Configure` message. Abstract `OutputKind` so DeckLink can be added later (IDEA §8). |
| `supervisor` | spawning output processes and renderers (active + standby), incarnations and launch tokens, failure detection per kind, promotion, standby rebuild, channel restart on format/NDI-mode change | §2.13, §4.2. |
| `api` | `/api/v1` (noPlayout), `/ograf/v1` (Server API), `/ws/events`, OpenAPI at `/api/docs` | §2.9. |
| `settings` | versioned JSON, validation, prepare/commit/rollback apply transaction, effective revision, last-good, migrations | §2.11. |
| `datasource` (wraps the crate) | running sources, field store, mapping to items, overrides, health | §2.8. |
| `health` | counters from renderers and sources, event log (ring of 500), Copy diagnostics | |
| `rosstalk` | RossTalk-compatible TCP 7788 listener, off by default (D-010) | §2.9.3. |

Runtime: tokio multi-thread. The playout state lives behind one actor task per channel (an mpsc command queue), so actions
on a channel are totally ordered without locks across awaits.

### 2.4 Renderer internals (`noplayout-renderer`)

Two per channel, `--slot a` and `--slot b`. Which one is **active** is the engine's decision (§2.13); the process only knows
its current role from the last `Role{active|standby}` message. Threads:

| Thread | Does | Real-time? |
|---|---|---|
| main / CEF UI | `cef::execute_process` first (helper subprocesses exit here), then `cef::initialize`; message loop. macOS: `external_message_pump` + `do_message_loop_work` driven by `on_schedule_message_pump_work`. Win/Linux: CEF's own loop (`run_message_loop`). Receives `on_paint`. | no |
| begin-frame (mode A only) | computes the same `T_k` as the output process from the channel epoch it was given (§5.1) and posts `send_external_begin_frame` to the UI thread at `T_k − lead`: every slot when active, every 6th slot when standby | yes (elevated priority) |
| ipc | control socket (heartbeat, configure, role, health, events) | no |
| preview monitor (standby only) | downscale the preview browser's paints to 480×270, JPEG, send with credits | no |

CEF settings: `windowless_rendering_enabled = 1`; browser `background_color = 0x00000000` (fully transparent: CEF allows only
0x00 or 0xFF alpha); program browser `external_begin_frame_enabled = 1` in mode A, otherwise `windowless_frame_rate =
ceil(fps)` when active and `10` when standby (`set_windowless_frame_rate` switches it at promotion); preview browser (standby
slot only) `windowless_frame_rate = 30`; `shared_texture_enabled = 0` (CPU `on_paint`, E-005); `root_cache_path` = absolute
per-channel, **per-slot** dir (CEF's process-singleton lock forbids sharing); audio muted (`set_audio_muted`; audio is a v1
non-goal). `on_render_process_terminated` → `Event::PageTerminated` to the engine at once (failure kind K2, §2.13);
`on_console_message` → forwarded and counted (conformance tests assert zero errors).

`on_paint` copies CEF's BGRA buffer (premultiplied, upper-left origin; only valid inside the callback, so the copy is
mandatory, one `memcpy` of 8.29 MB at 1080p) **straight into a free slot of this renderer's shared-memory paint ring** (E-042)
and publishes it with its **paint identity**: `{renderer_incarnation, paint_seq, painted_at_mono_ns}`. `paint_seq` counts
`on_paint` calls; it is **not** tied to any begin-frame or output slot, because `on_paint` carries no begin-frame id and CEF
may coalesce or drop begin-frames (#4311). The output slot `k` is assigned later by the output process (§5.3).

### 2.4a Output process internals (`noplayout-output`, one per channel; E-040)

| Thread | Does | Real-time? |
|---|---|---|
| pacer | owns the channel clock (§5); at each deadline `T_k` claims the newest complete paint from the **active** renderer's ring, converts it once into an immutable pair snapshot (FILL+KEY, or Fill+Alpha), stamps `tc(k)` and hands it to each sender thread's one-deep queue (newest wins) | yes (elevated) |
| sender ×1 or ×2 | one thread per NDI sender: `submit_video` of the snapshot for its plane; records `submit_started/returned_mono` and the last `k` submitted (§5.4) | yes |
| progress monitor | 10 Hz: per-sender submit age, pacer lateness, ring health of both renderers; raises `SenderStall`, `PacerLate`, `ProducerStale` | no |
| monitor | program monitor tap (what actually went to air): every *n*-th snapshot → 480×270, checkerboard, JPEG → engine with credits | no |
| divergence | every 6th slot, compares 64×36 luma thumbnails of the active and standby paints for the same slot (mode A) or for the nearest paints (mode B); persistent difference → `ReplicaDiverged` (§2.13.4) | no |
| ipc | control socket to the engine: `Configure`, `Activate{renderer_incarnation}`, `TestPattern`, `Shutdown`, `Ping`; up: `Hello{epoch}`, `Pong`, `Health`, `Event` | no |

While no fresh paint is available from the active renderer (failover in progress, or a static graphic), the pacer **holds**:
it resends the last converted snapshot with the new `tc(k)`. NDI receivers therefore never see the source disappear during a
renderer failure; they see the last good frame held. Held slots during a failover are counted separately (`held_failover`)
from ordinary repeats.

### 2.5 OGraf host runtime (one runtime, five roles)

`web/host` builds `ograf-host.js` + `renderer.html`, served by the engine at `/host/`. Roles: `program` (CEF program browser of
the active **or** standby renderer; only the active one's results are authoritative, §2.14), `preview` (CEF preview browser
and `/out/<ch>/preview`), `web-program`, `web-fill`, `web-key` (browser sources), `editor` (Edit mode preview). The role
changes only presentation (§2.10.2) and whether results can be authoritative; it never grants authority by itself (§2.14).

The runtime:
- keeps one container element per layer (z-order = layer number) plus hidden **cue slots**;
- creates the graphic element from the manifest's `main` (dynamic `import()` of the package URL), calls
  `load({data, renderType: "realtime", renderCharacteristics: {resolution, frameRate: 59.94006…}})` and **never sends
  another action to that instance before `load` resolves** (OGraf O5); actions that arrive meanwhile queue behind it;
- calls methods in the order the engine sequenced them and does not await one before starting the next (O8);
- merges partial `updateAction` data locally for its own snapshot; treats `currentStep: undefined` as "at end" (O7);
  supports `stepCount: -1` (NEXT STEP keeps sending `delta: 1` until the graphic reports the end, O6);
- passes `skipAnimation` through on every action type;
- reports each result (`statusCode`, `statusMessage`, `currentStep`, any extra payload) with the action's `seq` and
  `instanceId`/`generation`, in whatever order the graphic's promises resolve (the engine orders them, §2.14);
- times out a `load` after 10 s and any other action after 5 s, reporting the timeout as an error (never wedges the layer);
- sends a **liveness beat** every 100 ms from a `setTimeout` loop on the page's main thread (`Beat{n, rafCount}`), so a
  JS main-thread hang is visible within a few hundred ms (§2.13.1, kind K3), and `rafCount` separates "static graphic" from
  "stalled page";
- in test builds only, draws the frame-oracle overlay (§5.5) when a test asks for it. It is off in release builds and
  invisible to graphics.

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

Channel state: `layers[1..=L]` each with at most one `Instance {id, generation, itemId?, graphic, data, step: Option<i32>,
phase: loading|loaded|playing|stopping|ended, journal}` plus `cued: Option<CueSlot>` and `revision: u64`. `generation` is a
per-channel counter bumped on every load into any layer or cue slot, so `(instanceId, generation)` names one occupancy
exactly, even after a layer is reused.

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

Ordering: each channel actor assigns a sequence number `seq` and a server timestamp `ts` (§4.3) to every action and appends it
to the instance's journal before sending it. The hostbus sends actions in `seq` order to every connected page. The rules for
results, authority, snapshots and incarnations are in §2.14. If no renderer is active (both failed at once), mutating API
calls still change engine state and the journal, and answer `202` with `"renderer": "restarting"`; reads answer from engine
state.

**Recovery** (what is restored after a renderer, page or engine failure, how fast, and what cannot be restored) is specified
in §2.13. Rev 1's capped log with `goto` fallback (E-019) is withdrawn: it lost real graphic state (review 1 F2).

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
counts as a decode error) and its nine sport layouts, with its test vectors carried over. It runs over serial, TCP (dial and listen) and UDP. **Rev 2 (D-013):**
the gate is a test corpus ported from `unc-virtual-score-server` (`tests/test_protocol.py`, `tests/test_ingestion.py`
incl. per-sender UDP framing, and the `simulators/` per-sport streams, recorded once into `.npcap` files), byte-for-byte over
serial loopback, TCP and UDP. No venue recording is needed. STATUS.md states the limitation D-013 names: the simulators were
written from the same protocol knowledge as the decoder, so a live console check is recommended (not a gate) at the first game.

**Later source types stay possible (review 1 requirement gap).** JSON/HTTP poll and CSV/Sheets (IDEA §7, D-006) are not byte
streams, so they will not be `Decoder`s. They get a second, documented plug-in kind, `trait Poller { fn poll(&mut self) ->
Result<Vec<FieldUpdate>> }`, run by the same framework on a timer, feeding the same field store, mapping, override and health
paths. v1 ships the trait, its docs page and one in-repo test implementation (a local-file JSON poller used only in tests);
the real HTTP/CSV/Sheets pollers are deferred by D-006/D-011.

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
Settings: `GET /settings` (returns the document and its effective revision as `ETag`), `PUT /settings` with `If-Match:
<revision>` (validated as a whole and applied as one transaction, §2.11: `200` with the new revision, `409` if the revision is
stale, `422` with per-field errors, `500`/`503` with the failed apply step after a rollback). Data sources:
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

#### 2.10.1 NDI (output process; E-007…E-011, E-040)
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
  displaced frame comes back for reuse; a borrowed builder is never passed to the owned submit, per review 1's API audit), `frame_rate(FrameRate::new(num,
  den))` (returns `Result`, checked once at configure); `timecode(Timecode::from_ticks(tc(k)))`, the same value on FILL and
  KEY; per-frame metadata XML `<noplayout k="…" inc="…" paint="…"/>` for diagnostics. **Acceptance tests never trust this
  metadata**; they decode the pixels (§5.5).
- `submit_video` can block under SDK backpressure even with clocking off (grafton-ndi 3.0.0 docs; review 1 F7), so each
  sender has its own thread and the pacer never calls it (§5.4).
- `connection_count()` (returns `Result<u32>`) polled at 1 Hz per sender → receiver counts and the amber "waiting" light.
- Runtime missing: `Ndi::new()` returns an error that lists every path tried. The output process reports `NdiRuntimeMissing`,
  NDI outputs go red with "NDI Runtime not installed" and the OS-specific install link (Windows `ndi.link/NDIRedistV6`, macOS
  `ndi.link/NDIRedistV6Apple`, Linux: ndi.video SDK download, since the header's Linux redist URL is empty), web outputs keep
  running, and nothing crashes. "Check again" re-runs detection without a restart.
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
- "Send test pattern" also drives every web output (the test pattern is an OGraf graphic on the host stream), and M3 tests
  it on each web role (review 1 requirement gap).
- The CEF renderer can load `/out/<ch>/key` as well, so a pixel test compares the web key against the NDI KEY frame from the
  same Chromium. Tolerance and the fallback (a WebGL pass) are in R8.

#### 2.10.3 Test pattern
A built-in OGraf graphic (`fixtures/noplayout-test-pattern`): bars, an alpha ramp (16 steps, 0–255), a moving box and the
channel name. It goes through the real pipeline (CEF → convert → NDI, and every web output), so "Send test pattern" proves
the actual path rather than a synthetic one. It takes a reserved top layer and leaves the rundown untouched.

### 2.11 Settings, persistence, recovery (IDEA §6; E-020, E-050)
- Paths from `directories` 6.0.0 (`ProjectDirs::from("", "", "noPlayout")`): `settings.json`, `settings.last-good.json`,
  `settings.pending.json` (only during an apply), `rejected/`, `shows/`, `library/`, `logs/`, `cef-cache/<channel>-<slot>/`,
  `backups/`.
- Atomic write (`atomic_write(path, bytes)`): temp file in the same directory → write → `fsync` → rename over → `fsync` the
  directory (Unix); `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` on Windows.
- Validation lives in Rust (`serde` + explicit `validate()`), returns per-field errors.

**One owner, one effective revision.** A single `settings` actor owns the effective settings and an `effective_rev: u64`.
Every writer (the UI's form saves, `PUT /settings`, the wizard, a Take-ID-key edit) goes through it with the revision it read
(`If-Match`); a stale revision gets `409` and the caller re-reads. Writes are therefore serialized and no write is lost
silently.

**Apply transaction** (E-050). For a candidate document:
1. **Validate** (pure). Failure → `422`, nothing changes.
2. **Plan.** Diff against effective settings into ordered resource steps, each with an undo: bind a new HTTP listener;
   reconfigure or restart a channel (output process + renderers) with new format/NDI names/mode; start/stop data-source
   transports; RossTalk listener on/off; hotkey map (no resource).
3. **Persist pending.** `atomic_write(settings.pending.json)`. `settings.json` is untouched.
4. **Apply** the steps in order. New resources are acquired **before** old ones are released wherever possible (a new port
   is bound before the old listener closes; a new NDI sender name is created before the old one is dropped). If any step
   fails, the applied steps are undone in reverse (channels restarted with their old config, the new listener closed),
   `settings.pending.json` is moved to `rejected/settings-<utc>.json` with a `.reason.txt` naming the failed step, and the
   response is `500`/`503` with that step. Effective settings and `effective_rev` stay exactly as before.
5. **Commit.** `atomic_write(settings.json)` from the candidate, then `atomic_write(settings.last-good.json)`, delete
   pending, `effective_rev += 1`, emit `settings` event. Old listeners close 2 s later (in-flight requests finish). The app
   follows a port change through the event.

**Last-good** means *the last settings that were successfully applied* (live or at startup), never merely "parsed". It is
first created on the first successful startup apply (including the wizard's first save on a clean profile).

**Startup.**
- `settings.pending.json` present → an apply was interrupted (crash or power loss). It is moved to `rejected/` with reason
  "interrupted apply"; startup continues with `settings.json`. A crash after step 5's first write leaves the new
  `settings.json` and a stale last-good, which is fine: it is applied normally and last-good is rewritten.
- Parse → if `schemaVersion` is older: copy the file to `backups/settings.v<N>.<utc>.json` first, migrate in memory, validate,
  apply, then commit as the current version. If the migrated document fails validation or apply, the backup and the
  original stay, and the last-good path below is used.
- `schemaVersion` newer than this build: **refuse and never overwrite** `settings.json`. Load last-good if its version is
  supported (migrating it in memory if older), otherwise built-in defaults; a banner says which, and settings writes are
  held in memory until the user confirms "Replace settings written by a newer noPlayout" (then the newer file is first copied
  to `backups/`).
- Parse or validation failure: load last-good, record exactly what was rejected for Usopp's banner, and keep the rejected file
  in `rejected/`.
- On every path the effective state, the file left on disk and what the next restart will load are defined; the M4 fault
  matrix (§7) checks each.

Other persistence:
- Changing format or NDI mode on a channel with graphics on air needs confirmation, then restarts only that channel's output
  process and renderers, which restore through the cold path (§2.13.3).
- On-air state (the journals, §2.13.2) is appended to `shows/<show-id>.onair.jsonl` (fsync debounced 200 ms) and referenced
  from the show's `onAir` block, for the restore prompt and for supervised engine restarts.
- Logs: `tracing` + `tracing-appender` daily rolling files, keep 14. Copy diagnostics: versions (app, CEF, NDI runtime, OS,
  GPU from CEF), settings with token fields removed, last 500 log lines **with tokens and `?token=` query values redacted**,
  health snapshot.

### 2.12 Exposure and security (production rail 4)
- Default bind and auth follow IDEA §4.2: all interfaces, no token. Under Advanced: localhost only / LAN only / token. This is
  a broadcast control surface like RossTalk; DECISIONS E-034 records the trade-off.
- The app↔engine and renderer↔engine links use per-launch random tokens passed through environment variables, never argv.
  The token-protected internal routes (shutdown, supervisor control) are loopback-only regardless of the user setting.
- Every request that changes state from a non-loopback address is logged with its source address (no bodies, no tokens).
- Graphics are trusted local content, but the CEF pages are only allowed to navigate within the engine origin (popups and
  external navigation are blocked). Network fetches by graphics are allowed, because OGraf graphics may load fonts or data.
- Host-protocol authority (which page's results count) is not a user-facing access setting; it is bound to launch tokens and
  renderer incarnations in §2.14, and holds even on an intentionally open LAN.

### 2.13 Recovery contract (E-043, E-044, E-045; review 1 F1, F2)

IDEA §3/§6/§9: a crashed renderer is restarted and its state re-sent; heartbeat every 500 ms, restart when missed for 2 s;
crash test "kill the renderer process mid-show; output must recover to the same state within 2 s". This section defines
what "recover", "same state" and "within 2 s" mean, per failure kind, and what noPlayout can and cannot guarantee for an
arbitrary OGraf package.

#### 2.13.1 Failure kinds, detection, measurement origin

| Kind | What fails | Detected by | Detection budget | Recovery path |
|---|---|---|---|---|
| **K1** | active renderer's CEF browser process exits (crash, `kill -9`, `TerminateProcess`) | engine's child-process handle (tokio `Child::wait`: SIGCHLD/pidfd on Unix, process handle on Windows) | ≤ 20 ms | warm promotion |
| **K2** | a CEF child of the active renderer exits (render subprocess killed or crashed) | `on_render_process_terminated` → `Event::PageTerminated` | ≤ 50 ms | warm promotion; the old renderer is killed and rebuilt as standby |
| **K2g** | the active renderer's GPU process crashes or stalls (Chromium relaunches it; paints stop) | output process: no new paint for 250 ms while the page's `Beat` reports `rafCount` advancing (`PaintStall`) | ≤ 300 ms | warm promotion |
| **K3a** | the active page's JS main thread hangs; the standby keeps beating | engine: no `Beat` from the active page for 500 ms (2,000 ms while a `load` is pending on that page, since graphics may do heavy synchronous setup) | ≤ 500 ms (≤ 2 s during load) | warm promotion |
| **K3b** | **poison action**: an action that hangs every replica (the same graphic bug runs in the standby too) | K3a on the active, then the standby misses its beats within 1 s, and both stopped after the same last-dispatched `seq` | ≤ 1.5 s | cold restore **without** that action (quarantined, §2.13.3) |
| **K4** | the active renderer's CEF UI thread or whole process hangs (no crash) | control heartbeat every 100 ms; the ipc thread also posts a task to the CEF UI thread every 100 ms and reports its latency. No Pong, or UI-thread latency > 500 ms | ≤ 500 ms | warm promotion; old process killed |
| **K5** | one NDI sender's `submit_video` blocks (SDK backpressure, a stuck receiver) | output process progress monitor: submit in flight > 250 ms (`SenderStall`) | ≤ 250 ms | the other plane continues; after 2 s that sender is recreated under the same name; a second stall within 10 s restarts the output process (K6) |
| **K6** | the output process exits or hangs | engine child handle, or no Pong for 500 ms | ≤ 500 ms | respawn the output process (no CEF; renderers keep painting into their rings); NDI sources reappear and receivers reconnect |
| **K7** | the engine exits | the app's supervisor | as E-033 | supervised engine restart, cold restore of every channel (R17, 5 s budget) |

IDEA's watchdog numbers are kept as upper bounds and tightened: the control heartbeat runs every 100 ms (≤ 500 ms as IDEA
says), and a renderer is replaced after 500 ms of missed heartbeat, which is always before IDEA's 2 s. Failing over early is
safe because promotion keeps all state; the cost of a false alarm is a few held frames.

**Measurement origin (t0)** for every recovery test: the harness's monotonic clock reading taken immediately before it
injects the fault (the `kill`/`TerminateProcess` call, `SIGSTOP`, the dispatch `ts` of a test `hang` action, or a
`fault-inject` hook in a test build). **Restored (t1):** arrival time, at an independent NDI receiver process on the same
machine (same monotonic clock), of the first frame whose pixels decode (§5.5, recovery-probe and Basketball checks) to the
expected state and after which the state stays correct for 2 s. **Outage = t1 − t0.** Also recorded per run: whether the
receiver saw any gap in frame arrival > 100 ms (source continuity; must be none for K1–K4), held frames, and which path ran.

#### 2.13.2 What the engine keeps: the on-air journal

For every live instance the engine keeps a **journal**: `load` (graphic id + version hash, `dataAtLoad`,
`renderCharacteristics`) followed by **every action dispatched to the authoritative page**, in `seq` order, each with its
`ts` (§4.3), params and committed result. It is **never capped and never silently truncated.**
- Actions the engine rejected before dispatch (unknown custom action id, payload fails its schema) are not journalled; the
  graphic never saw them.
- Actions that failed or timed out in the graphic **are** journalled and replayed, because the graphic saw them and may have
  partly applied them. A replay result whose status class differs from the original is counted as `replay_divergence`.
- **No compaction** of a third-party graphic's journal: merging consecutive `updateAction`s is not provably equivalent (a
  graphic may count updates or react to intermediate values). Compaction happens only where equivalence is declared by the
  graphic itself (§2.13.4).
- Size: a journal entry is typically < 1 kB. Health shows each instance's journal length and the estimated cold-restore
  replay time (measured replay rate from S7). Above 50,000 entries for one instance Health warns ("Restoring Scorebug after a
  crash would take about 9 s"); nothing is dropped.
- Persistence: appended to `shows/<show-id>.onair.jsonl` (one line per entry, fsync debounced 200 ms) and dropped for an
  instance when it is disposed. The show's `onAir` block points at it. Engine restarts and the cold-start restore prompt
  read it.

#### 2.13.3 Restore paths and what each guarantees

- **Warm promotion** (K1, K2, K2g, K3a, K4). The engine sends `Activate{inc}` to the output process (it reads the standby's
  ring from the next slot) and `Role{active}` to the standby (full paint rate; its program page's results become
  authoritative at a `seq` barrier, §2.14). Until the first fresh paint arrives, the output holds the last good frame. The
  failed renderer is killed (whole process tree) and a **new standby** is started and cold-restored behind it (Health
  "Standby: rebuilding", then "ready"). The preview browser always lives in the standby: at promotion the new active closes
  its preview browser and the new standby opens one, so the Preview monitor shows "Preview restarting…" for the rebuild
  (seconds; it is not on air).
- **Cold restore** (no ready standby: a second fault during a rebuild, K3b, K7, a format change, standby turned off). A new
  renderer gets a snapshot: per layer, `load(dataAtLoad)` then the full journal replayed in order with
  `skipAnimation: true`, then the live stream (§2.14 handoff). For **K3b** the action in flight when both replicas hung is
  marked `quarantined` and left out of the replay; the instance is restored to its state before that action, and the On-air
  strip shows "‘Show message’ froze this graphic and was skipped". The output holds the last good frame throughout.
- **Late join / reconnect** of a web page or the preview: same as cold restore, into that page only. Never authoritative.

| State class | Warm promotion, standby saw the instance live ("exact") | Warm promotion, standby joined later ("rebuilt") | Cold restore, late join |
|---|---|---|---|
| Durable inputs: graphic version, data incl. data-source fields, layers, cue, step/phase as committed | exact | exact | exact |
| Effects of every dispatched action, e.g. Basketball `addPoints` ×257 changing the score through its own `updateAction` | exact (executed live) | exact (uncapped journal) | exact (uncapped journal), for graphics that are deterministic given the same action sequence |
| Time-dependent internal state: a clock started by `startClock`, timers, animations started by an action | exact within action-delivery skew between replicas (target ≤ 1 frame, measured in S7) | not restored, flagged, until the replica-divergence check shows it matches on screen | **not restored**, flagged "elapsed time not restored: startClock 1 min 12 s ago" on the On-air strip and in Health (see §2.13.4 for Designed graphics and the S9 attempt) |
| Non-deterministic graphics (random numbers, external fetches) | as live | may differ; divergence flagged | may differ; divergence flagged when detected |

The flags are the point: when noPlayout cannot rebuild something, the operator is told on the On-air strip, not only in a
log. A Health counter alone (rev 1) is not enough. (All operator-facing strings in §2.13 are placeholders; Usopp owns the
wording and placement, §9.)

#### 2.13.4 Time-dependent state, declared checkpoints, replica divergence

- OGraf v1 has no way to ask a real-time graphic for its internal state or to seek its clock. noPlayout therefore cannot
  restore elapsed time inside an arbitrary third-party graphic on a cold path. Graphics whose clocks are **driven by data**
  (the Basketball scorebug with `clockRunning: false` and `gameClock` pushed from the OES source, D-008) restore exactly,
  because the clock value is durable input. Q2 asks Caleb to accept this limitation for internally-run clocks.
- **Declared checkpoints.** The OGraf manifest schema at 945f8bb allows vendor keys matching `^v_.*` (checked in
  `v1/specification/json-schemas/graphics/schema.json`). A graphic may declare `"v_noplayout": {"recovery": "checkpoint"}`
  and implement two custom actions: `v_noplayout_checkpoint` (returns its internal state as JSON in the result payload) and
  `v_noplayout_restore {state, checkpointTs, nowTs}`. For such graphics the engine checkpoints every 10 s or 100 actions
  and replays `load + restore + journal tail`, which is equivalent by the graphic's own declaration. With the server
  timestamps (§4.3) a restored clock continues at the correct elapsed time. **Every Designed graphic exported by M5 declares
  and implements this**, so noPlayout's own graphics restore exactly on every path. `"v_noplayout": {"updateCompaction":
  true}` likewise allows merging consecutive updates. Third-party graphics without these keys get the full journal.
- **S9 (spike, not relied on):** time-faithful cold replay through the DevTools virtual-time API (`Emulation.setVirtualTimePolicy`
  via `BrowserHost::execute_dev_tools_method`, present in cef 156.7.0): replay the journal with virtual time advanced by the
  recorded `ts` deltas, then return to real time. Fails if the switch back to real time makes `performance.now()` jump or
  breaks animations; then the limitation above stands.
- **Replica divergence.** Every 6th slot the output process compares 64×36 luma thumbnails of active and standby paints. A
  difference above tolerance for 2 s marks the standby "diverged" (amber). A diverged standby is rebuilt; if it diverges again
  for the same instance, that instance is labelled non-deterministic in Health and the standby keeps running (it is still
  the better recovery option).

#### 2.13.5 Time budget per stage (estimates until S7 measures them)

| Stage | Warm, K1 | Warm, K3a/K4 | Cold restore |
|---|---|---|---|
| Detection | ≤ 20 ms | ≤ 500 ms | per kind |
| Decide + `Activate`/`Role` messages | ≤ 5 ms | ≤ 5 ms | — |
| Spawn renderer + CEF initialize | — | — | est. 300–800 ms |
| Page + host runtime + graphics load (Basketball package: 6 graphics, 4.6 MB on disk, 111 files of which 75 are fonts/images, from loopback HTTP) | — | — | est. 200–1,000 ms |
| Journal replay with `skipAnimation` | — | — | est. ≤ 1 ms per action (≈ 0.3 s for 257), measured |
| First fresh paint at full rate | ≤ 3 slots (50 ms) | ≤ 3 slots | ≤ 3 slots |
| Output picks it up | ≤ 1 slot (17 ms) | ≤ 1 slot | ≤ 1 slot |
| NDI transport + receiver decode | ≤ 50 ms, measured | ≤ 50 ms | ≤ 50 ms |
| **Total, target / fail** | **≤ 150 ms / > 500 ms** | **≤ 650 ms / > 1,000 ms** | **≤ 2,000 ms target, not guaranteed** (est. 0.6–2.5 s) |

The IDEA crash test (one kill mid-show) always takes the warm path, so the 2 s bound is met with a wide margin if S7 confirms
the estimates. The cold path is reached only after a second fault within a few seconds, a poison action, an engine death or a
deliberate channel restart; its time is measured and reported, and Q1 asks Caleb whether ≤ 2 s must hold there too.

**Gate (not a fallback):** S7 runs in M1 on Windows 11, macOS arm64 and Linux (and the macOS Intel CI lane, informational),
with the Basketball show's on-air set (scorebug + ticker + sponsor bug). If the warm path misses its fail line on a supported
OS, the architecture is revisited before M2 starts. A failed M6 crash test is not an acceptable way to find out.

#### 2.13.6 Recovery tests (each named again in its milestone)
- Basketball `addPoints`: load `homeScore: 10`, play step 0, `addPoints(home, 1)` ×257. Kill the active renderer (K1): the
  receiver shows **267** at step 0. Repeat with the standby disabled (forces the cold path): still **267** (uncapped
  journal). Rev 1's design produces 10 here (review 1's probe).
- Basketball `startClock`: start a 60 s game clock, wait 20 s, K1: the receiver's clock region matches a reference render of
  0:40 ±1 displayed unit; cold path: the instance carries the "elapsed time not restored" flag (the test asserts the flag, not
  a silent 1:00).
- Recovery-probe fixture (coded OGraf graphic in `fixtures/`): renders patches encoding its custom-action counter, a data
  hash, its step and an internal clock; used for every kind K1–K6 with exact expected values.
- More than 50,000 journal entries on one instance; an `ended` instance; a `stepCount: -1` graphic at a dynamic step; a
  graphic whose `load` takes 1.5 s (no false K3a); poison action (K3b) with quarantine message.
- Late join and reconnect, pixel-compared with the authoritative page after settle (M3).
- A Designed graphic (M5) with an internal clock: exact elapsed time on the cold path via its declared checkpoint.

### 2.14 Ordering, authority and results (E-046; review 1 F6)

**Identities.**
- `engineRun`: UUIDv7 per engine process start.
- `inc`: renderer incarnation, a `u64` the engine assigns at every renderer spawn, strictly increasing per channel within an
  engine run; `(engineRun, channel, inc)` names one renderer process. Each renderer also has its `slot` (a/b) and current
  role (active/standby).
- Launch token: 256 random bits per incarnation, passed to the renderer in an environment variable (never argv, never a URL).
  After its program page loads, the renderer hands the token to that page with `Frame::execute_java_script` (present in cef
  156.7.0); the page uses it once, in its registration message, over loopback.
- `(instanceId, generation)` per occupancy (§2.7); `seq` per channel action; `revision` per channel state change.

**Registration.** A page's first message is `Register{channel, role, hostVersion, lastSeq?, inc?, token?}`.
- `role: program` is accepted only if the connection is from loopback, `inc` is the current incarnation of a live renderer of
  that channel, the token matches that incarnation (constant-time compare), and no other page holds that incarnation's
  program registration. Otherwise the socket closes with 4403 and the source address is logged. A role string never grants
  authority; a public browser asking for `program` is refused, not silently downgraded, so the mistake is visible.
- A page of a superseded incarnation (an old renderer reconnecting after it was replaced) is refused with 4410
  "superseded"; if that renderer process is still alive the engine kills it.
- Mirror roles (`web-*`, `preview`, `editor`) register without a token, subject to the user's access scope (§2.12).

**Authority.** Exactly one page per channel is authoritative: the program page of the renderer the engine last made active.
At promotion the engine records a barrier `B` = the last `seq` dispatched. Results for `seq ≤ B` that the old active never
delivered are taken from the new active (it executed the same actions live); results from a superseded incarnation that
arrive later are logged and dropped.

**Result commit.** OGraf allows overlapping calls, so results may resolve out of order (play #1 after play #2). Per instance
the engine commits results **in `seq` order**: the result for `seq n` updates engine state (`currentStep`, phase) only
after every earlier dispatched action on that instance has committed or timed out. An API caller still gets its own action's
result as soon as it arrives; only the state update is ordered. A result for an `(instanceId, generation)` that no longer
exists is logged and discarded; it can never recreate or change an instance. Replay results (cold restore, late join) never
commit; they are only compared with the journal (`replay_divergence`).

**Snapshot-to-live handoff.** Registration is handled inside the channel actor, which is single-threaded, so it is atomic
with respect to new actions: the actor takes the snapshot at revision `R` with last `seq` `S` and subscribes the connection to
the live stream from `S + 1` in the same step. The page applies the snapshot, buffering live actions that arrive meanwhile,
then applies them in `seq` order (with `skipAnimation: true` if their `ts` is more than 500 ms old by then), and sends
`Synced{seq}`. Pages ignore any `seq` they have already applied. No action accepted during a replay is lost or executed
twice.

**Tests (M3/M4):** reversed result completion with a fake graphic; a late result after `dispose`; an old page reconnecting
after its renderer was replaced; a non-loopback and a token-less loopback `program` registration attempt; 200 actions/s
fired during a snapshot replay, after which each page's applied-`seq` list equals the journal; promotion with actions in
flight.

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
| `iceoryx2` | `0.10.1` (2026-11-20), MSRV 1.89 | MIT OR Apache-2.0 | frames (renderer → output paint rings, E-042) | **Version-checked only** (crates.io: version, date, licence, MSRV, rev 2). Its API (publish-subscribe service, loaned samples, overflow behaviour, cleanup of a dead publisher's resources) has **not** been read in source yet; that is M1 task 4 and spike S8, before any code depends on it. |
| `memmap2` | `0.9.12` (2026-10-21), MSRV 1.65 | MIT OR Apache-2.0 | frames (fallback only) | Version-checked only. Fallback if S8 rejects iceoryx2: our own ring over a platform shared-memory object (`shm_open` on Unix, a pagefile-backed `CreateFileMappingW` section on Windows) mapped with memmap2. |

Not used, on purpose: `turbojpeg` (C dependency, E-030), `tauri-runtime-cef` 3.0.0-alpha.7 (alpha, pins cef 154.5.0 against
our 156, and cef-dll-sys `links` forbids two versions in one graph, E-003), `ndi-sdk-sys` (GPL-3.0, no macOS), `ndi-sdk` (no
send/receive). Rev 1 listed iceoryx2/memmap2 here; rev 2 moves full-rate frames between processes (E-040, E-042).

Also read in the cef 156.7.0 source for rev 2: `Frame::execute_java_script` (launch-token hand-off, §2.14),
`BrowserHost::execute_dev_tools_method` and `add_dev_tools_message_observer` (S9 only), `BrowserHost::set_windowless_frame_rate`
(standby rate switch), `RequestHandler::on_render_process_terminated` (K2).

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
| engine ↔ output process **control** | `interprocess` local socket (named pipe / UDS), one per channel | `u32` LE length + postcard, max 1 MiB, versioned | down: `Configure`, `Activate{inc}`, `TestPattern`, `Shutdown`, `Ping`; up: `Hello{epoch}`, `Pong`, `Health`, `Event`, `MonitorFrame` (program) | heartbeat 100 ms; health 1 Hz |
| engine ↔ renderer **control** | local socket, one per renderer (two per channel) | same | down: `Configure`, `Role{active\|standby}`, `Reload`, `Shutdown`, `Ping`; up: `Hello`, `Pong{ui_latency_us}`, `Health`, `Event`, `MonitorFrame` (preview, standby only) | heartbeat 100 ms; health 1 Hz |
| renderer → output process **paints** | shared-memory paint ring per renderer (E-042; iceoryx2, S8) | raw BGRA + `PaintHeader{inc, paint_seq, painted_at_mono_ns, w, h, stride}` | full-rate paints (active), every 6th slot (standby) | up to 497 MB/s per 1080p59.94 channel, no extra copy (§2.4) |
| engine ↔ pages (CEF program/preview, web outputs, editor) | WebSocket `/ws/host/<ch>` | JSON | down: `Snapshot`, `Action`, `TimeSync`; up: `Register`, `Result`, `Beat`, `Synced`, `PageError` | per action; beats 10 Hz |
| engine ↔ external controllers | HTTP + `/ws/events` + RossTalk-compatible TCP 7788 (off by default) | JSON (RossTalk text) | | |

Monitor JPEGs (≈ 0.3 MB/s per monitor; program from the output process, preview from the standby renderer) travel on a
**second, monitor-only local socket** per process with credits (2 credits, newest wins), so a burst of monitor frames can
never delay a heartbeat; the engine relays one frame in flight per WS client, because browser WebSockets have no receive
backpressure. (`MonitorFrame` in the table above means that second socket.)

### 4.2 Engine ↔ output process and renderer protocol
- **Spawn order per channel:** output process first (`noplayout-output --channel <id>`, env `NOPLAYOUT_IPC_NAME`,
  `NOPLAYOUT_IPC_TOKEN`). Its `Hello{proto, version, pid, token, epoch{mono_ns, utc_100ns}}` fixes the channel clock epoch.
  Then both renderers (`noplayout-renderer --channel <id> --slot a|b`, env `NOPLAYOUT_IPC_NAME`, `NOPLAYOUT_IPC_TOKEN`,
  `NOPLAYOUT_HOST_URL`, `NOPLAYOUT_LAUNCH_TOKEN`, `NOPLAYOUT_INC`), each answering `Hello{proto, renderer_version,
  cef_version, pid, token}` within 10 s. A protocol mismatch is a hard error with a plain message (mixed install). The
  first renderer to report its program page `Synced` becomes active; the other becomes standby.
- **Configure** (idempotent): output process `format{w, h, num, den}`, `ndi{senders[{name, kind: fillAlpha|fill|key}],
  fillType}`, `monitor{w: 480, h: 270, quality: 80, every_n}`, `rings{a, b}`; renderer `format`, `epoch`, `drive: A|B`,
  `ring` name, `pages{program_url, preview_url?}`.
- **Heartbeat:** both directions `Ping{seq}` every 100 ms. Renderer `Pong` carries `ui_latency_us` (a task posted to the CEF
  UI thread), `last_paint_seq`; output `Pong` carries `last_slot_k`, per-sender `last_submit_k` and in-flight age. Missing
  Pongs for 500 ms, or UI latency > 500 ms, are failure kinds K4/K6 (§2.13.1).
- **Health (1 Hz):** output: `slots`, `slots_skipped`, `held` (no newer paint), `held_failover`, `superseded`, `pacer_late_us
  p99`, `convert_ms_p50/p99`, per sender `submit_ms_p99`, `submit_blocked`, `pair_partial` (§5.4), `connection_count`;
  renderer: `paints`, `ring_overwrites`, `rss_bytes`.
- **Events:** `NdiRuntimeMissing{searched}`, `SenderStall{name, age_ms}`, `SenderRecreated{name}`, `PaintStall`,
  `PageTerminated{reason}`, `ConsoleError{text}`, `CefInitFailed{reason}`, `ReplicaDiverged{instanceId}`.
- **Orphan rule:** a renderer or output process whose control socket closes exits after 3 s (the output holds the last frame
  meanwhile).

### 4.3 Host protocol (pages; E-047, review 1 F11)

**Server timestamps.** Every `Action` and every journal entry in a `Snapshot` carries
`ts: {utcUs: i64, mono: {run: engineRun, us: u64}, slot?: {k: u64, epoch: <channel epoch id>}}`:
- `utcUs`: the engine's UTC time in microseconds since the Unix epoch, taken from one engine `TimeSource` that reads UTC once at
  engine start and advances it with the monotonic clock, so it never steps backwards within an engine run even if the system
  clock is adjusted (NTP slew or step). Its offset from the system clock is reported in Health.
- `mono.us`: engine monotonic microseconds since `engineRun` started; the unambiguous ordering clock.
- `slot.k`: the channel's output slot whose deadline is the next one after the action was issued, in the output process's
  clock domain (§5.1), which ties the action to NDI timecode `tc(k)`.

The timestamps are **informational, not scheduling**: pages execute actions on receipt, in `seq` order; no page waits for a
timestamp, and independent browsers are still not frame-locked (IDEA §4.2). They are used for: the journal and cold restore
(the `ts` deltas feed declared checkpoints, §2.13.4, and the S9 attempt); the handoff rule that applies stale buffered actions
with `skipAnimation` (§2.14); diagnostics ("action reached the web page 180 ms after the engine issued it"); and the operator
flags that need elapsed times ("startClock 1 min 12 s ago").

**Messages.**
- Up `Register{channel, role, hostVersion, lastSeq?, inc?, token?}` (§2.14) → down `Snapshot{revision, lastSeq, serverNow:
  ts, layers: [{layer, instanceId, generation, graphicUrl, dataAtLoad, renderCharacteristics, journal: [{seq, ts, method,
  params, quarantined?}], checkpoint?}], cued?, flags: [...]}`, then `Action{revision, seq, ts, instanceId, generation,
  method, params}` in `seq` order.
- Down `TimeSync{serverNow: ts}` every 5 s and on request; the page estimates its clock offset from the round trip and
  reports `lagMs` (receipt time − `ts`) in its `Beat`, so Health can show per-page lag.
- Up `Result{seq, instanceId, generation, statusCode, statusMessage?, currentStep?, payload?}`, `Beat{n, rafCount, lagMs,
  lastAppliedSeq}` every 100 ms, `Synced{seq}`, `PageError{text}`.
- Results from non-authoritative pages are never committed. A disagreement with the authoritative page (program says step 2,
  a web page says step 1) is counted as `mirror_divergence` in Health.

**Tests (M3):** every `Action` a browser page receives has `ts` with `utcUs` and `mono.us` non-decreasing in `seq` order; a
system clock step on the test machine does not make `utcUs` go backwards; a late-joining page's `Snapshot` carries the
original `ts` of each journal entry and a `serverNow`; `lagMs` appears in Health for a page throttled by the test.

---

## 5. Frame pacing (E-006, E-048, E-049)

One clock per channel, **in the output process**, in integer arithmetic. The engine never paces frames. Renderers in mode A
compute the same deadlines from the same epoch to time their begin-frames; they never decide what goes to air.

### 5.1 Clock
1. **Rate:** `num/den` from the format (60000/1001, 50/1, 30000/1001, 25/1). `T_k = epoch_mono + floor(k · den · 10⁹ /
   num)` ns, computed from `k` every time in `u128` (never `+= 16.683 ms`; `u64` overflows after about 213 days at 59.94).
   `epoch_mono` is taken when the output process starts; `U0` = UTC at that moment in 100 ns units. Both go to the engine in
   `Hello` and from there to both renderers. The monotonic clocks used (`mach_absolute_time`, `CLOCK_MONOTONIC`, QPC) are
   system-wide on each OS, so every process on the machine computes the same `T_k`.
2. **Sleeping:** an absolute deadline minus 0.5 ms with the OS's precise timer (macOS `mach_wait_until`, Linux
   `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME)`, Windows high-resolution waitable timer
   `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`), then spin to `T_k`. Pacer, sender and begin-frame threads run at elevated
   priority (macOS QoS user-interactive, Windows `THREAD_PRIORITY_TIME_CRITICAL`, Linux `SCHED_FIFO` only if permitted,
   otherwise nice).
3. **Timecode:** `tc(k) = U0 + floor(k · den · 10⁷ / num)` (100 ns units), written explicitly on FILL and KEY. Never
   `Timecode::SYNTHESIZE`, so NDI's synthesizer drift (fixed in 6.4) is irrelevant. `clock_video(false)` on every sender: the
   channel clock is the only pacing authority. A respawned output process (K6) starts a new epoch; the timecode jump is
   logged as an event.

### 5.2 Drive modes
- **Mode A, external begin-frame (preferred):** `external_begin_frame_enabled = 1`. The active renderer posts
  `send_external_begin_frame` at `T_k − lead` for every `k` (lead = one frame to start, tuned by S1); the standby at every
  6th `k`. CEF #4311 (open, last updated 2026-10-23) says external begin-frames carry `Now()`, no deadline and a fixed 60 Hz
  interval, and that a tick arriving while one is pending is dropped. At 59.94 the interval is 0.1 % off, which is harmless.
  At 50/25 it is 20 % off, which can affect CSS/rAF timing, so the 50 Hz formats get their own S1 runs.
- **Mode B, free-running (floor):** `windowless_frame_rate = ceil(fps)` (60 for 59.94, 50, 30 for 29.97, 25). CEF paints at
  its own pace into the ring and the pacer samples (§5.3). At 59.94 and 29.97 this is a **declared rate conversion**
  (CEF 60 → 59.94): one source frame in every 1,000 is not shown, by design. That is not a drop of an output frame, and it is
  reported separately from loss (§5.5).

### 5.3 Paint selection, identity and ring ownership (review 1 F7)
- **Identity.** A paint is named by `(inc, paint_seq)` and stamped `painted_at` (§2.4). An output slot is named by `k` and
  `tc(k)`. The two are independent; the output frame records which paint it carried (`k ↔ paint`), but nothing infers which
  begin-frame produced a paint. Rev 1's "the paint that results is tagged k+1" and "tag it with the requesting tick" are both
  withdrawn.
- **Selection at `T_k`:** the pacer takes the newest `READY` paint of the active ring with `painted_at ≤ T_k − guard`
  (guard 1 ms). In mode A that is normally the paint from the begin-frame posted at `T_k − lead`. In mode B the pacer adds
  hysteresis at the phase crossing (once a crossing has produced a skip, it does not produce a repeat in the next 900 slots;
  it may take a paint that completed up to 1 ms after `T_k − guard`, inside the one-slot latency), so a crossing gives exactly
  one skip, not a repeat/skip
  flutter. S1 validates the selector against the thresholds in §5.5.
- **If no newer paint exists,** the previous snapshot is resent with the new `tc(k)` (`held`; legitimate for a static
  graphic, and acceptance uses the moving oracle fixture so it can tell). If several paints arrived, the newest is sent
  (`superseded += gap`). If the pacer itself wakes after `T_k + period/2`, it skips the slot (`slots_skipped`, shown as
  "Dropped") rather than bursting.
- **Ring (4 slots per renderer, E-042).** Each slot has a header `{state: FREE|WRITING|READY|READING, inc, paint_seq,
  painted_at}`, changed only by atomic compare-and-swap. Writer (`on_paint`): `FREE → WRITING`, copy, header, `→ READY`
  (release). If no slot is FREE it reclaims the **oldest READY** slot (`ring_overwrites++`), **never a READING one**. Reader
  (pacer): chosen slot `READY → READING`, convert into its own buffers, `→ FREE`; every older `READY` slot `→ FREE`. The
  reader holds at most one slot, so the writer never blocks and never waits on the output process. A writer that dies mid-copy
  leaves a `WRITING` slot the reader ignores; the dead incarnation's ring is discarded. A respawned output process resets any
  `READING` slot to `FREE` (it is the only reader).

### 5.4 Senders, pairing, backpressure, progress (review 1 F7)
- The pacer converts the chosen paint **once** into an immutable snapshot `S_k = {k, tc(k), paint, FILL buffer, KEY buffer}`
  (or one Fill+Alpha buffer). FILL and KEY of the same `k` always come from the same paint and carry the same `tc(k)`.
- Each sender thread owns 3 `VideoFrame`s (one inside the SDK, one queued, one being filled); `submit_video` returns the
  displaced one. Each sender has a one-deep queue, newest wins. If a sender is still inside `submit_video` when `S_{k+1}`
  arrives, `S_k` is dropped from that sender only (`submit_blocked++`), and if that leaves a slot with only one plane sent,
  `pair_partial++`. The pacer never calls the SDK, so a blocked sender cannot delay the clock or the other plane.
- **What is and is not guaranteed.** Software pairing is guaranteed and tested: identical `tc(k)` and identical source paint
  on FILL and KEY. Simultaneous presentation at independent receivers is **not** guaranteed by noPlayout; the receiver or
  switcher pairs by timecode or by arrival. The UI's Separate Fill & Key hint says so.
- **Progress monitor** (10 Hz): per sender, time since `submit_video` was entered without returning, and `k − last_submit_k`;
  pacer lateness; ring staleness. `SenderStall` at 250 ms in flight; recreate at 2 s; output-process restart after a second
  stall within 10 s (K5, §2.13.1). These are separate from the control heartbeat, so a process that answers Pings while a
  sender is stuck is still caught.

### 5.5 Acceptance oracle (receiver side; review 1 F4)
**The fixture.** `fixtures/frame-oracle` is a coded OGraf graphic. In each `requestAnimationFrame` callback it increments a
counter `n` and computes `m = round((t − t0) / P)` from the rAF timestamp (`P` from `renderCharacteristics.frameRate`). It draws
two opaque patches (top-left A, bottom-right B), each encoding `n` and `m` as 24-bit values with a parity bit, in 16 × 16 px
black/white cells; a third patch encodes `n` **in alpha only** (cells α = 255 or 0), so it appears in the KEY plane and in the
alpha of Fill+Alpha; plus the 16-step alpha ramp and a moving box. Cells this large survive NDI's compression.

**The decoder** (`noplayout-oracle`) reads **pixels only**: it never reads noPlayout's metadata, counters or logs. It runs in a
separate receiver process (grafton-ndi `Receiver`) and, in M1, on the output process's paint dumps. Per received frame `j`
it reports: `Δn` (from A), tear (A ≠ B), key/alpha mismatch (alpha-patch `n` ≠ A's `n`), `Δm`, timecode step (`tc_j −
tc_{j−1}` ≠ one period ±1 tick), FILL/KEY timecode inequality for the same `n`, arrival gaps > 2 periods, unreadable patches
(parity), and KEY luminance vs the ramp's nominal alpha (±1 LSB for a ≥ 16).

**Thresholds** (per run; `N` = output frames after a declared 120-frame warm-up):

| Mode / rate | Repeats (Δn = 0) | Skips (Δn ≥ 2) | Backwards (Δn < 0) | Tears | Key/alpha mismatch | Timecode errors | Δm ≠ 1 | Unreadable |
|---|---|---|---|---|---|---|---|---|
| A, all eight formats | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| B, 59.94 / 29.97 (declared 1000:1001 conversion) | 0 | ≤ ⌈N/1000⌉ + 1, each exactly Δn = 2, ≥ 900 frames apart | 0 | 0 | 0 | 0 | same as skips | 0 |
| B, 50 / 25 (rates matched) | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

Output-side counters must agree with the oracle (`slots_skipped = 0`, `pair_partial = 0` on reference hardware with healthy
receivers); a disagreement between them fails the run, because one of the two is wrong.

**Injected faults must fail.** A test build of the output process has a `fault-inject` feature. Each injection must trip
exactly its intended detector and make the run fail: (1) resend the previous snapshot once (repeat); (2) skip one source
paint (skip); (3) send a paint three paints old (backwards); (4) send KEY from the previous paint (key mismatch); (5) write
half a frame from the next paint (tear); (6) bump one `tc` by a period (timecode); (7) freeze the fixture's time base for 10
frames while `n` still advances (Δm); (8) block one sender for 300 ms (`SenderStall`, `pair_partial`). These run in CI on
the paint-dump path and locally with NDI.

**What each mode may claim.** Mode A passing the A row: "no dropped or repeated frames". Mode B at 59.94/29.97 passing its
row: "output frames continuous; source rendered at 60 Hz and rate-converted, one source frame in 1,000 not shown". Mode B
is never reported as zero-drop at those rates. IDEA's soak bar ("drop no frames") is read as the **A row**; an OS that only
passes in mode B does not meet it without Caleb's decision (Q3).

### 5.6 Choosing the mode, and when (review 1 F10)
- **S1a, M1, paint level (provisional).** The real output-process clock and selector (no NDI yet) consume the real ring; the
  oracle decodes the selected paints. A and B, per OS, at 59.94 and 50, 10 minutes each. Result: a *provisional* per-OS mode,
  recorded as such in STATUS.md and the ADR draft. Nothing is called receiver-proven.
- **S1b, M2, receiver level (final).** The same runs through NDI to a receiver process, plus all eight formats for 10 minutes
  each in the chosen mode. Ship A on every OS where it passes the A row, B where only B passes its row, and record the choice
  in the final ADR. Switchable in Advanced diagnostics (not in the operator UI). If CEF ships timed begin-frames (#4311's
  `SendExternalBeginFrameWithTiming`), A becomes the default everywhere after a re-run.
- An OS/rate that passes neither row stops that OS's M2 for that rate (§6.1); it is not silently relabelled.

### 5.7 Not frame-locked, and the Mac Studio
Not frame-locked: web outputs (IDEA's stated limitation) and two channels relative to each other (each has its own epoch).
Genlock and cross-channel lock need the NDI Advanced SDK and are out of scope.

Known host issue: the org's Mac Studio currently has a degraded host timer (replayServer finding: about 19 ms p50 late at
59.94 since 2026-11-18). Frame-pacing and recovery-time measurements on that machine are void until it is fixed, so S1, S7 and
the M6 soak run on other machines (R9).

---

## 6. Risks and their failure conditions

| # | Risk | Fails when (measurable) | Detection | Mitigation / fallback |
|---|---|---|---|---|
| R1 | External begin-frame unusable on an OS (CEF #4311 timing, #4166 incomplete frames on macOS x86_64 with the Rust crate) | S1a/S1b mode A misses the A row of §5.5 (any repeat, skip, tear or key mismatch) in a 10-min oracle run | S1 harness: oracle on paint dumps (M1) and at an NDI receiver (M2) | Mode B on that OS if it passes its row (status per §6.1). If neither mode passes at 50 Hz: **stop** for 50 Hz on that OS (§6.1); 50 Hz formats stay required until Caleb decides (Q4). |
| R2 | CPU readback + conversion too slow | p99 of paint copy + convert + submit > 8 ms per 1080p frame, or any `slots_skipped` with 2 channels at 1080p59.94 on reference hardware | Health histograms (S2, M2 bench) | LUT split over threads → SIMD (std::arch), both inside v1. If still too slow: **stop**; the `accelerated_osr` path becomes a v1 task by ADR, not a v1.x deferral, unless Caleb approves a deferral (§6.1). |
| R3 | 2160p formats (33 MB/frame, ≈1 GB/s at 29.97) | 2160p29.97 or 2160p25 30-min oracle run misses the mode's row on reference hardware | M6 format runs | The "needs a strong GPU" label (already in UI.md) is fine only if the run passes. Removing 2160p from the format list is a **Caleb-approved deferral only** (Q4); otherwise M6 is not done. |
| R4 | grafton-ndi single maintainer (615/619 commits in 2026); API breaks | a needed fix is not released within 30 days, or 4.0 breaks the API | Renovate, issue tracker | `noplayout-ndi` trait boundary; own `libloading` wrapper over the MIT headers + `NDIlib_v6_load` (about 1 kLOC). |
| R5 | cef-rs moves fast and runs ~2 Chromium majors behind upstream (156 vs 158) | a bump breaks the build or S1; a Chromium CVE matters for trusted local pages | CI on Renovate PRs | `=` pin, bump PRs gated on S1 + pixel tests; pages restricted to the engine origin (§2.12). |
| R6 | macOS bundling: CEF framework + 5 helper apps inside a Tauri-built `.app`, signing with per-helper entitlements (`allow-jit`) | the renderer helpers fail to launch from inside the app bundle, or notarization rejects | S5 (M0 dev bundle), M6 notarization dry run | Custom xtask bundling for macOS (build the Tauri app, then insert `noplayout-renderer.app` built by `bundle-cef-app` under `Contents/Frameworks/`, sign inside-out). |
| R7 | Linux: WebKitGTK UI quirks (Tauri), CEF OSR without a display | UI unusable on NVIDIA/Wayland, or CEF fails to init without X/Wayland | M0/M4 manual check on Fedora KDE Wayland + an NVIDIA box; CI under Xvfb | `WEBKIT_DISABLE_DMABUF_RENDERER=1` default on Linux; CEF `--ozone-platform=headless` test in S4. A windowed cef-rs UI host (E-003 option c) is a **new shell and packaging implementation**, not a flag: if needed it is a separate ADR, its own cards and Caleb's approval (§6.1). |
| R8 | Web fill/key filter precision or browser differences (OBS's CEF, vMix) | `/key` page differs from the NDI KEY by more than 2 LSB for a ≥ 16 in the CEF pixel test | M3 pixel test | WebGL pass on the layer root. |
| R9 | OS timer jitter; the Mac Studio's degraded timer | S1 timer probe p99 > 1 ms late on a reference machine | timer probe in S1 (sleep to deadline, histogram) | Spin margin raise; run timing gates on healthy hosts only. |
| R10 | Misbehaving graphics (load never resolves, throws) | one bad graphic blocks a layer or the channel | host runtime timeouts (10 s / 5 s), conformance tests with bad fixtures | Instance marked failed with a plain message; layer stays usable; renderer unaffected. A graphic that hangs the page is K3a/K3b (§2.13). |
| R11 | *(superseded by D-013)* OES test data | — | — | D-013: the corpus is ported from `unc-virtual-score-server`; no recording gate. STATUS.md carries D-013's stated limitation. |
| R12 | *(superseded by D-012/D-014)* Basketball package | — | — | Supplied, public, Apache-2.0 with OFL fonts; committed under `fixtures/` in M0. |
| R13 | Hosted CI runners can't run CEF OSR (no GPU, session) on a lane | conformance/pixel jobs crash on that lane after software-GL flags (`--disable-gpu`, SwiftShader) | M1 CI | That lane builds + unit-tests only, and **every milestone** on that platform needs a local `cargo xtask preflight` evidence file at the milestone commit (§8.2). It is never a permanent skip with no replacement run. Linux (Xvfb) and macOS arm64 must run them in CI. |
| R14 | Two renderers + preview × 4 channels exhaust CPU/GPU/RAM | 4 × 1080p59.94 with standbys and previews misses the oracle row on reference hardware | M6 4-channel run | Previews drop to 15 fps or pause when no monitor is watching; standby paint rate drops from 1/6 to 1/12; Health shows the cost. If still failing: Q1 (standby per channel). "Up to 4 channels" stays. |
| R15 | Receivers disagree on NDI alpha (vMix premultiplied option, TriCaster switch, DistroAV) | a supported receiver shows dark or bright edges with the default | M2 interop matrix (NDI Studio Monitor, OBS+DistroAV, vMix if available) | Per-mode fill-type default + hint text; documented receiver settings. |
| R16 | OS grabs Ctrl+Space (macOS input sources, Linux IBus/fcitx) | the typing-safe TAKE alternate never reaches the app | Settings → Hotkeys "Test" box (truth), plus macOS `com.apple.symbolichotkeys` keys 60/61 and a Linux IBus/fcitx process check for the warning line | User remaps; warning shown (answers Usopp's question: the Test box detects it on every OS, the warning can be predicted on macOS and Linux only). |
| R17 | Single engine process is a single point of failure for all channels | supervised engine restart → first restored NDI frame > 5 s | M6 crash test | Keep the engine small and panic-free (`panic = "abort"` is not used; per-task `catch_unwind` around plug-in decoders); restart budget measured. |
| R18 | Unauthenticated LAN control (IDEA default) | someone on the production LAN takes or clears graphics | logged source addresses | Token + scope under Advanced; documented; Caleb may flip the default (E-034). |
| R19 | Shared Anthropic budget runs out mid-milestone | Fable unavailable for review | `hermes usage` | Per model-routing: build on flash-next/Codex, frontier review queued; never silently downgraded. |
| R20 | Warm standby costs too much (a second CEF per channel) | S7/M6: standby adds > 60 % CPU or > 400 MB RSS per channel on reference hardware, or makes 4 channels miss the oracle row | Health per process; S7, M6 4-channel run | Lower standby paint rate (1/12), share the preview browser into it (already), then Q1's option of standby per channel. Not silently turned off. |
| R21 | Poison action hangs both replicas | K3b in conformance or in the field | §2.13.1 detector | Cold restore without the quarantined action; the instance is named on the On-air strip. Cold-path time is not guaranteed ≤ 2 s (Q1). |
| R22 | iceoryx2 0.10.1 does not fit (API, overflow semantics, dead-publisher cleanup on Windows/macOS) | S8 shows extra copies, blocking writers, or leaked shared memory after `kill -9` | S8 in M1 | Own 4-slot ring over memmap2 + platform shared-memory objects (§3); same protocol (§5.3). |
| R23 | Standby replicas diverge (non-deterministic graphics) | `ReplicaDiverged` persists after a rebuild | divergence check (§2.13.4) | Instance labelled non-deterministic in Health; recovery for it is "as the standby shows", stated, not hidden. |

### 6.1 Fallback completion status (review 1 F9)

A fallback never turns a missing requirement into "done". Each one has a status that says what it means for the milestone:
**in scope** (the fallback still meets the requirement), **partial** (the milestone stays open; STATUS.md names the gap and
the next step), **stop** (work on that item halts and Franky reports to Caleb with numbers), or **deferral** (only with
Caleb's written approval, recorded as a D-nnn). Evidence owner = who produces the measurement that decides it.

| Risk | Fallback | Status if used | Evidence owner | Evidence |
|---|---|---|---|---|
| R1 | mode B at 59.94/29.97 | **partial** for the soak bar (Q3), in scope for output continuity | build card for S1 + Franky review | S1b oracle report per OS |
| R1 | both modes fail at 50/25 on an OS | **stop** for those formats on that OS; **deferral** only by Caleb (Q4) | same | S1b report |
| R2 | threads/SIMD | in scope | M2 card | S2 numbers |
| R2 | accelerated OSR path | **stop** until an ADR + cards exist; deferral only by Caleb | Franky | S2 + M6 4-channel numbers |
| R3 | "needs a strong GPU" label | in scope if the 2160p runs pass | M6 card | 30-min oracle runs |
| R3 | drop 2160p from the list | **deferral** only by Caleb (Q4) | Franky | same |
| R4 | own libloading NDI wrapper | in scope | build card | loopback + oracle runs |
| R5 | hold the cef pin | in scope | Renovate PR | S1 + pixel re-run |
| R6 | custom macOS bundling | in scope | M0/M6 card | notarization dry run |
| R7 | windowed cef-rs UI host | **stop** + separate ADR and approval | Franky | M0/M4 Linux manual check |
| R8 | WebGL key pass | in scope | M3 card | pixel test |
| R13 | CEF tests local-only on a lane | **partial** CI coverage, accepted only with a `preflight` evidence file at every milestone commit on that platform | milestone card | `preflight.json` linked in the PR |
| R14 | lower preview/standby rates | in scope | M6 card | 4-channel oracle run |
| R20 | standby per channel optional | **deferral** of "≤ 2 s on every channel" only by Caleb (Q1) | Franky | S7 + M6 numbers |
| R22 | own shm ring | in scope | M1 card | S8 report |

### 6.2 Acceptance coverage (platform × format × test)

| Test | Windows 11 x64 | macOS arm64 | Linux x86_64 | macOS x86_64 |
|---|---|---|---|---|
| Build, clippy, fmt, unit tests | CI | CI | CI | CI |
| CEF conformance + pixel goldens (EBU + Basketball) | CI if R13 allows, else `preflight` | CI | CI (Xvfb) | CI |
| Pacing oracle S1b: all 8 formats, 10 min each, chosen mode | reference PC | reference Mac mini | named Linux box | CI runner, informational only (VM timing is not acceptance) unless Caleb names an Intel Mac (Q6) |
| Recovery S7/M6: K1–K6, 20 runs of K1 | reference PC | reference Mac mini | named Linux box | CI runner, K1 functional only |
| 2 h soak at 1080p59.94 (IDEA) | reference PC | reference Mac mini | — (not required by IDEA; 30-min run) | — |
| 30-min runs: 1080p50, 2160p29.97, 2160p25, 2 channels, 4 channels | reference PC | reference Mac mini | named Linux box | — |
| Installer + M4 show smoke on a clean machine/VM | yes | yes | yes | yes (CI or VM) |

Every milestone report lists, per row, *passed* (with the commit and an evidence link), *skipped* (with the reason) or *not
yet exercised*; skipped and unexercised checks are never counted as passed.

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
   clean shutdown on `Shutdown`. `noplayout-output`: starts, sends `Hello{epoch}`, exits on `Shutdown` (no NDI yet).
5. `noplayout-app`: Tauri v2 window with a Svelte placeholder that spawns or attaches to the engine and shows "Engine:
   running · Renderer: CEF 156.0.31 OK" from `/api/v1/health`.
6. `xtask`: `cargo xtask cef` (export-cef-dir into `target/cef/<ver>/<target>`), `cargo xtask run` (dev-bundles the renderer
   on macOS with `bundle-cef-app`, then runs the app), `cargo xtask preflight` (runs the local CEF/NDI/timing checks a lane
   can't, and writes `target/preflight/<os>-<arch>-<commit>.json` with each check's result: R13, §8.2). **Spike S5:** macOS
   dev bundle layout works from the workspace.
7. CI workflow (§8) on four lanes, plus `cargo deny check`.
8. Fixtures (D-014): commit the Basketball package as `fixtures/basketball-ograf-package/` with a NOTICE line naming Caleb
   as author, `OFL-Barlow.txt` kept with the fonts, and remove the local `.git/info/exclude` entry. Port the OES corpus
   (D-013) into `fixtures/oes/` (protocol data and vectors only; no venue names or addresses).

Done when: all four CI lanes are green on the M0 commit, and on a real Windows 11 x64, macOS arm64 and Linux x86_64 desktop
`cargo xtask run` opens the noPlayout window showing "Engine: running" and the renderer's CEF version (screenshot per OS in
the PR). macOS x86_64 is proven by its CI lane running the same smoke headlessly (`noplayout-engine --self-test` spawns a
renderer and checks `Hello`).

### M1: OGraf host runtime + CEF offscreen renderer + output clock (no NDI) + early spikes
Tasks:
1. Vendor OGraf v1 (schemas, examples) at 945f8bb; `library` import + validation + serving (E-024).
2. `web/host`: `ograf-host.js` per §2.5 (incl. `Beat`, `seq`/generation on results), `renderer.html`, vitest suite with
   misbehaving fake graphics (never-resolving load, throws, out-of-order promise resolution).
3. Host protocol on the engine (`hostbus`) with `ts` on every action (§4.3), journals (§2.13.2), snapshots, registration and
   authority (§2.14); a dev CLI `noplayout-engine ctl load|play|update|stop|custom|kill-renderer|hang` drives it (the rundown
   comes in M4).
4. Renderer: transparent OSR program browser, paint ring writer (§5.3), active/standby roles, mode A begin-frame thread and
   mode B, launch token hand-off. **Spike S8** (iceoryx2 vs own ring, E-042): zero extra copy, writer never blocks, `kill -9`
   of the writer on all three OSes leaves no leaked shared memory after the output process restarts.
5. Output process without NDI: channel clock (§5.1), ring reader and selector (§5.3), `--dump-paints <dir> --frames N`
   (raw dumps with `k`, `tc(k)` and the paint chosen), `fault-inject` feature (§5.5).
6. `noplayout-oracle` + `fixtures/frame-oracle` + `fixtures/recovery-probe`; oracle unit tests on synthetic frames, and the
   eight injected faults (§5.5) each failing on the dump path.
7. **Spike S1a** (paint-level pacing, A vs B, per OS, 59.94 and 50; provisional, §5.6) and **S2** (readback + copy +
   conversion cost at 1080p and 2160p). Provisional mode recorded in STATUS.md and an ADR draft; the ADR is final in M2.
8. Supervisor: active + warm standby per channel, failure kinds K1–K4 and K6 (§2.13.1), warm promotion, standby rebuild, cold
   restore. **Spike S7a** (recovery, paint level): the harness injects K1, K2, K3a and K4 with the **Basketball on-air set**
   (scorebug, ticker cycling, sponsor bug) and the recovery-probe fixture loaded; t1 = the first output slot whose paint
   decodes (oracle + Basketball reference crops) to the expected state. 20 runs per kind per OS, plus the cold path with the
   standby disabled. Includes the `addPoints` ×257 and `startClock` cases (§2.13.6).
9. Conformance harness (`tests/conformance`): every EBU example and every Basketball graphic, unmodified, through load → play
   → each step → update → each custom action → stop → dispose in the real CEF renderer; assert return payloads, zero console
   errors, `currentStep` semantics (O6/O7), `stepCount: -1` handling.
10. Pixel tests: the alpha-ramp fixture and a gradient/shape fixture rendered to PNG and compared with goldens (shape goldens
    are OS-independent within ±2 LSB; text goldens, if any, are per OS); Basketball goldens per OS.

Done when: (a) the conformance suite passes for all nine EBU examples (`bar-chart, headline, l3rd-name, minimal, ograf-logo,
renderer-test, responsive-lower-third, scoreboard, weather`) and the six Basketball graphics on Linux and macOS arm64 in CI and
on Windows by CI or `preflight`, with no console errors; (b) IDEA's M1 test: the Basketball scorebug renders and animates
offscreen at 1080p59.94 and its saved paint sequence shows correct alpha: background alpha 0, antialiased edges between 1 and
254, and the alpha-ramp fixture's 16 patches within ±1 LSB of nominal; (c) the S1a, S2 and S8 reports exist with measured
numbers and the provisional mode; (d) **S7a gate**: on each of Windows 11, macOS arm64 and Linux, the warm path meets its
fail lines (§2.13.5: K1 ≤ 500 ms, K3a/K4 ≤ 1,000 ms) in 20 of 20 runs, both Basketball recovery tests give the expected
result, and the cold-path times are recorded. If (d) fails, M2 does not start and Franky reports to Caleb with the numbers.

### M2: NDI output: Fill+Alpha and Separate Fill & Key, test pattern, health
Tasks:
1. `noplayout-ndi`: trait, grafton-ndi implementation, runtime detection with searched paths and the OS-specific install link,
   "Check again".
2. `noplayout-pixels`: un-premultiply (LUT, threads), shaped/unshaped fill, key, with exhaustive unit tests over all (a, c)
   pairs and a criterion-style bench (`cargo bench -p noplayout-pixels`).
3. Sender threads, snapshot pairing, backpressure, progress monitor and K5 (§5.4), counters, diagnostic metadata,
   `connection_count` polling, in the output process.
4. Test pattern graphic (§2.10.3) and `POST /api/v1/outputs/test-pattern`.
5. Monitor taps (program from the output process, preview from the standby) + monitor sockets + `/ws/monitor` + **Spike S3**
   (IPC and JPEG cost at 4 channels × 2 sources).
6. Minimal operator-facing surfaces from the approved mockups: Program monitor tile and the Health panel's channel block
   (frame rate, dropped, duplicated, receivers), NDI branding (link, ®, About text).
7. Receiver-side oracle harness (`tests/ndi_oracle.rs`, real runtime; prints "skipped: NDI runtime not found (searched …)"
   otherwise): an NDI `Receiver` process decodes FILL+KEY or Fill+Alpha per §5.5; checks resolution, `FrameRate`, timecode
   steps and FILL/KEY equality, oracle counters, and the alpha round trip `premul → straight → re-premul` within 1 LSB for
   a ≥ 16. The eight injected faults run here too.
8. **Spike S1b** (receiver-level pacing, final; §5.6): per OS, both modes at 59.94 and 50 for 10 minutes, then all eight
   formats for 10 minutes each in the chosen mode. Final ADR.
9. **Spike S7b** (recovery at an NDI receiver): S7a's matrix plus K5 and K6, with t1 measured at the receiver (§2.13.1).
10. Interop matrix: NDI Studio Monitor (NDI Tools, not redistributed), OBS + DistroAV, vMix if a machine has it.

Done when: with the NDI 6.4 runtime installed on two machines, NDI Studio Monitor and OBS (DistroAV) on the second machine
show `NOPLAYOUT Main` keyed cleanly over a background, and `NOPLAYOUT Main FILL` / `KEY` line up when keyed (no edge fringe
on the alpha ramp); the receiver-side oracle passes its mode's row of §5.5 for every format on Windows 11, macOS arm64 and
Linux (zero repeats, skips, tears, key mismatches and timecode errors in mode A; mode B only within its declared
rate-conversion bound); every injected fault fails its detector; S7b meets the same fail lines as S7a measured at the
receiver, with no source gap > 100 ms for K1–K4; the test pattern appears on every NDI source; the Health block shows
receiver counts that change when a receiver connects and disconnects. Separately: NDI runtime absent → red light with the
install link, web side unaffected, no crash. Bench check (D-010): on Caleb's switcher, Separate Fill & Key with the default
**Shaped** FILL keys the alpha-ramp and an antialiased-text graphic with no dark or bright fringe at the switcher's normal CG
key setting. If Unshaped looks right instead, the default flips before M2 closes and the result is recorded in the E-009 ADR.
Any OS/rate that passes only mode B, or neither, is reported with its §6.1 status; it is not counted as passed for the soak
bar.

### M3: Web outputs
Tasks:
1. `/out/<ch>/program|fill|key|preview` role pages (§2.10.2), with access scope and token handling.
2. Host protocol completion: revisioned snapshots, reconnect with `lastSeq`, `TimeSync` and per-page lag in Health,
   mirror-divergence counter.
3. Port-in-use detection with "next free port" (UI.md §1.5 wording) and the frame-lock note.
4. Tests: late-join (a second CEF page joins mid-show after play + 2 steps + update + 257 `addPoints`, and after a 2 s settle
   its pixels equal the program page's within tolerance); reconnect (kill the WS, actions continue, the page converges);
   `/key` vs NDI KEY pixel test (R8); shaped/unshaped `/fill` vs NDI FILL; the §4.3 timestamp tests; the §2.14 authority and
   handoff tests; "Send test pattern" reaches every web role.
5. `docs/how-to/obs.md` with a curl script that drives the EBU scoreboard through the API.

Done when: an OBS Browser Source (current OBS release) on `/out/main/program` shows the EBU scoreboard with alpha over another
source, driven only by the documented curl script (load, play, score update, next step, custom action, stop); a browser tab
opened halfway through shows the same state without replaying animations; every action that tab receives carries `ts`; the
late-join, key pixel, timestamp and authority tests pass in CI.

### M4: Playout mode + settings store + Output Setup wizard + OGraf Server API (+ Take IDs)
Tasks:
1. Settings store (§2.11): one owner with `effective_rev` and `If-Match`, the apply transaction (validate → plan → pending →
   apply with undo → commit), migrations framework with backups, last-good as last-successfully-applied, rejected-file
   report. Fault matrix, each asserting effective state, the files on disk and what the next start loads: truncated file;
   invalid value; old version migrated successfully (backup written first); old version whose migration fails validation;
   future version refused and never overwritten; first start on a clean profile creates last-good; crash injected after
   the pending write, after the `settings.json` rename and before the last-good write; two concurrent writers (UI autosave
   and `PUT /settings`) with one getting `409`; channel 1 restarts but channel 2's NDI sender name is taken, rolled back; new
   HTTP port already bound, rolled back with the old listener still serving; read-only directory.
2. Shows + rundown model with Take IDs (E-021): uniqueness (inline error on duplicate, D-010), auto-increment assignment,
   renumber, paste remap, undo journal.
3. Playout engine (§2.7): preload cue, TAKE, NEXT STEP, Take & advance, OUT, CLEAR LAYER/ALL (cut), live update and pending
   changes, custom actions with schema-validated payload forms, generations and ordered result commit (§2.14). Preview
   browser in the standby renderer.
4. API: OGraf Server API (`/ograf/v1`) with a conformance test generated from `server-api.yaml`; `/api/v1` with Take ID routes;
   `/ws/events` with tally; OpenAPI at `/api/docs`.
5. RossTalk-compatible listener (§2.9.3; D-010, off by default), with a test transcript of the commands the Companion
   XPression module and a Carbonite CG-device preset send.
6. UI to Usopp's approved rev 3: shell + top bar lights, Output Setup wizard, Playout (rundown with Take ID column, monitors,
   on-air strip incl. the recovery flags of §2.13.3 as Usopp words them, schema-generated data form, transport with Take ID box
   and its pending/unknown states), Settings (Outputs, Hotkeys incl. Take ID keys and the Ctrl+Space Test box, Remote control
   incl. "Fire by Take ID"), Health (incl. standby state, journal length, per-page lag), restore and rejected-settings
   prompts. Hotkey rules per UI.md §0.6 (paused while typing, typing-safe alternates).
7. Channel restart on format/NDI-mode change with confirmation; on-air journal persistence; restore prompt (Enter = Restore).
8. `docs/test-shows/basketball-show.md`: a 30-step show script (pregame, scorebug on, lower thirds by Take ID, matchup steps,
   ticker, clears), plus `scripts/show-api.sh` that runs the same show through HTTP only.

Done when: from a clean profile, the wizard sets up 2 channels (Web + NDI) and the test pattern runs; the Basketball package
(and EBU examples) import; the 30-step show runs **twice**: once from the UI by keyboard only (Take ID + Enter + Space, →, Esc,
Shift+Esc) and once from `scripts/show-api.sh` alone, with NDI and web outputs showing the right graphic and step at every
checkpoint (the API run also asserts engine state and program-renderer pixels at checkpoints); a Companion button using the
documented HTTP request takes a graphic by Take ID, and with the RossTalk listener switched on, the Companion XPression module
(or `nc` sending the same lines) cues and takes by Take ID; typing a Take ID that another item already uses shows the inline
error and changes nothing; killing the app and engine mid-show (a cold start) and relaunching offers Restore, and Enter
brings the graphics back on their steps with scores changed by `addPoints` intact (killing only the app reattaches with
nothing replayed); the §2.11 fault matrix passes in full; Usopp's review of the built screens against the mockups has no
open blocking findings.

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
7. Fixture (D-013): the OES corpus ported in M0 from `unc-virtual-score-server` (`tests/test_protocol.py`,
   `tests/test_ingestion.py` incl. per-sender UDP framing, and the `simulators/` per-sport streams recorded once into
   `.npcap` captures, each with a golden field timeline generated from the ported vectors). Committed in `fixtures/oes/`
   (Caleb's MIT code, protocol data only). STATUS.md states D-013's limitation: the simulators share the decoder's protocol
   knowledge, so a live console check is recommended (not a gate) at the first game.

Done when: the ported OES corpus, replayed through **each transport** (serial over a pty pair on Linux, TCP dial and listen,
UDP including a two-sender interleave with frames split across datagrams), produces exactly the golden field timelines,
byte-for-byte as D-013 requires; with a capture driving a scorebug, "Main Clock → Game clock" updates the on-air graphic
within 100 ms plus one frame of the message; a manual override survives the next 100 source messages and "Back to live"
restores it; stopping the stream holds every field at its last value and turns the light amber within the Quiet timeout; on
real hardware, pulling the USB serial adapter shows Lost within 1 s and replugging it recovers without a restart; a show
saved on one computer opens on another with its mappings intact and its source marked "Not set up on this computer"; the
OES PR's diff outside `crates/noplayout-decoder-oes/` is limited to the registry line, the feature line, docs and STATUS.md
(the plug-in boundary holds).

Size: about 4 build cards (transports + capture + plug-in contract, OES decoder, mapping/override engine, UI) plus a review
card. A later decoder (Daktronics AS5000 next, D-010) is one card plus its capture.

### M5: Edit mode + OGraf export (builds to UI.md rev 3 §3 and IDEA §5.2; review 1 F3)
The target is Usopp's UI.md rev 3 §3 (`~/OrgWork/projects/noplayout/design/UI.md`, §3.1–§3.5), every control in its §3.2
table, both D (default) and A (Advanced) rows. Nothing in IDEA §5.2 is deferred; a deferral would need Caleb's approval.

Tasks:
1. Designed-graphic document model (versioned JSON): elements (Rectangle, Text, Image, Clock, Group), bindings, the seven
   field kinds, segments In / Step 1…n / Out, per-element tracks and keyframes, update transitions, custom actions.
2. Generated `graphic.mjs`: a small embedded runtime (Web Animations API) with no external dependencies; segments map to
   `playAction` steps and `stopAction`; update transitions; custom actions jump to a segment or set a field; the declared
   recovery checkpoint (`v_noplayout`, §2.13.4) so Designed graphics restore exactly, internal clocks included.
3. Elements and properties (UI §3.2 Properties rows): Rectangle fill, linear gradient, border, corner radius; Text font, size,
   weight, tracking, alignment, shrink to fit; Image file and fit; Clock count up/down, formats M:SS and SS.t, internal or
   data-driven; all elements X/Y/W/H and opacity; Advanced rotation, blend, mask (on Group), exact anchor, text overflow
   rules.
4. Data fields → OGraf `schema` with GDD types: Text, Number, Yes/No, Colour, Image, Choice, List, each with a default and
   bindable to any bindable property (🔗); Advanced field id, GDD type, required, min/max, choices editor (Usopp Δ13 plain
   names in the UI, correct GDD types in the file).
5. Timeline: segment strip with ▶ per segment and `+ Step` (stepCount set automatically); tracks per element, keyframes,
   easing presets (Ease out, Ease in-out, Linear, Snap); scrubber, play/pause, loop segment; Advanced custom easing curve and
   numeric per-keyframe timing, update transition ("On data change…"), custom actions ("Actions…").
6. Canvas and editor (UI §3.1, §3.2, §3.5): frames from the editor's own preview renderer process (E-025, D-010) at editor
   resolution, handles drawn by Svelte; safe guides (action + title), grid + snap, reference image/still at 50 % never
   exported, zoom (fit, 50 %, 100 %); Elements tree with drag to reorder/group, eye and lock; Library with Designed/Coded
   badges, import, duplicate, rename, delete, show file location; preview renderer states Starting / Didn't start /
   Running.
7. Coded graphics (UI §3.3): locked design, name, thumbnail (capture or file), default data, segment play buttons.
8. Fonts: embed with an OS/2 `fsType` licence check; warn and fall back when embedding is not allowed (UI §3.4 wording).
9. Thumbnail rendered by the engine (CEF renderer frame → PNG).
10. Import/export (UI §3.4): import errors and warnings as specified; export validates against the vendored v1 schema, checks
    no external dependencies, renders the thumbnail, zip or folder; saving a graphic used in the rundown leaves on-air
    instances untouched.
11. Third-party check: SuperFly `ograf-server` (MIT) pinned to a commit in a test container, used as a smoke test only.

Acceptance fixtures (each built in the editor with no code, by a scripted UI test driving the Tauri window, exported, and
then played in noPlayout on NDI and web; pixel comparisons are against the editor's own preview frames at the named
timeline points within ±2 LSB for shapes):
- **M5-F1 shapes:** rectangles with flat fill, linear gradient, border and corner radius; a Group with a mask.
- **M5-F2 text:** tracking, the three alignments, shrink to fit with a 40-character value in a 20-character box; text
  overflow rule (Advanced).
- **M5-F3 images:** a fixed image in each fit mode, and an Image field bound to an image element, changed live.
- **M5-F4 clock:** four clocks (count up M:SS, count down M:SS, count down SS.t, data-driven M:SS bound to a Number field
  updated by `updateAction`); the internal clocks checked at 0 s, 10 s and 65 s; the count-down clock also checked after a
  K1 kill and after a forced cold restore (declared checkpoint: exact elapsed time).
- **M5-F5 fields:** one field of each of the seven kinds with defaults, each bound to a property; the exported `schema` is
  checked field by field for the GDD type and default; changing each field through the API changes the right pixels.
- **M5-F6 animation:** In (two elements with different easing presets), Step 1, Step 2, Out; an update transition on a bound
  value; a custom action that jumps to Step 2 and one that sets a field; frames compared at 5 timeline points per segment;
  scrubber position frames equal played frames at the same time.
- **M5-F7 lower third (IDEA's M5 test):** bar + two bound text fields, In / Step 1 / Out, one custom action; also plays in the
  pinned ograf-server via its API (scripted).
- Editor behaviour tests: snap to grid and to safe guides; the reference image absent from the export; lock and hide in the
  Elements tree; a font whose `fsType` forbids embedding shows the warning; a coded graphic's design locked; an invalid
  package import shows the §3.4 error list.

Done when: M5-F1…F7 and the editor behaviour tests pass on Windows 11, macOS arm64 and Linux (CI where R13 allows, otherwise
`preflight`); every export validates against the v1 schema; Usopp's review of the built Edit screens against UI rev 3 §3 has
no open blocking findings.

### M6: Hardening, watchdog, recovery, soak, packaging
Tasks:
1. Recovery as specified (§2.13, §4.2): the full K1–K7 matrix with the recovery-probe fixture and the Basketball show, on
   each reference machine; the §2.13.6 tests; engine restart (R17).
2. Soak harness: 2 h at 1080p59.94, scorebug clock running (data-source replay feeding it) + ticker cycling + the frame-oracle
   fixture on a top layer; records Health every second, RSS of every process, and the receiver-side oracle (§5.5).
3. Format and load runs (§6.2): 30 minutes each of 1080p50, 2160p29.97, 2160p25, 2 channels and 4 channels (with standbys and
   previews) on the reference machines; every format 10 minutes on Linux.
4. Copy diagnostics, log rotation, port-conflict flow, all error texts from UI.md checked.
5. Packaging: Windows NSIS (and MSI if WiX works on CI), macOS `.dmg` with the custom bundling (R6) and documented signing +
   notarization steps (unsigned until Caleb provides credentials, which is an approval item), Linux AppImage + `.deb`
   (`avahi-daemon`/`libavahi-client3` as dependencies for NDI). NDI runtime detected, not bundled (E-011). Licence/NOTICE files
   (NDI EULA flow-down text, Chromium/CEF, IJG, OGraf, Barlow OFL).
6. Release workflow (tag → installers as artifacts). `osv-scanner` + `cargo deny` clean.
7. Reference hardware named by Caleb before the soak (IDEA: an Apple M-series Mac mini and a mid-range Windows PC with a
   discrete GPU; not the Mac Studio while its timer is degraded), plus the Linux box for §6.2 (Q6).

Done when: every IDEA §9 test passes, each reported with its commit and evidence link per §6.2 (skipped and unexercised checks
listed as such, never as passed): the conformance and pixel suites; the receiver-side oracle; the 2-hour soak on both
reference machines with the **mode A row** of §5.5 (zero repeats, skips, tears, key mismatches, timecode errors and
`slots_skipped`) and RSS growth under 5 % after the first 10 minutes for every process; the crash test (kill the active
renderer mid-show) restores output with the same graphics, steps, custom-action effects and running clocks, measured at an NDI
receiver per §2.13.1, within 2 s in 20 out of 20 runs on each reference machine (target ≤ 500 ms); the K2–K6 runs meet their
§2.13.5 fail lines; the cold path's measured time is reported against Caleb's answer to Q1; installers are produced by CI for
Windows x64, macOS arm64 and x86_64, and Linux x86_64, and each installs on a clean machine or VM and passes the M4 show
smoke. A reference machine that meets the soak bar only in mode B does not pass unless Caleb has answered Q3 to accept it.

### Spikes (measure before building on them)
| ID | Question | When | Output |
|---|---|---|---|
| S1a | Paint-level pacing, A vs B, per OS, 59.94 and 50 (oracle on dumps) | M1 | provisional per-OS mode (§5.6) |
| S1b | Receiver-level pacing, final mode, all 8 formats | M2 | final ADR with the §5.5 rows per OS |
| S2 | `on_paint` copy + conversion cost at 1080p/2160p | M1–M2 | numbers vs the 8 ms budget (R2/R3) |
| S3 | Monitor JPEG + IPC cost, 4 channels × 2 sources | M2 | CPU %, latency; go/no-go for raw fallback |
| S4 | Linux CEF OSR under Xvfb, Wayland and `--ozone-platform=headless` | M0–M1 | CI and desktop flags |
| S5 | macOS renderer helper layout from a dev tree and inside the app bundle | M0, M6 | xtask bundling |
| S6 | Ctrl+Space and other OS-grabbed keys per OS, as seen by the Tauri webview | M4 | hotkey warning table (R16) |
| S7a | Recovery at paint level, K1–K4 + cold path, Basketball on-air set, per OS | M1 | per-stage times vs §2.13.5; gate for M2 |
| S7b | Recovery at an NDI receiver, K1–K6 | M2 | same, end to end |
| S8 | iceoryx2 0.10.1 vs own ring for 8 MB paints (copies, blocking, `kill -9` cleanup) per OS | M1 | E-042 final choice (R22) |
| S9 | Virtual-time replay for third-party graphics' clocks (DevTools `Emulation.setVirtualTimePolicy`) | M4 (optional) | adopt only if real time resumes cleanly; otherwise the §2.13.4 limitation stands |

Order inside M1 (breaks rev 1's S1 ↔ M2 loop, F10): renderer ring writer → output clock + selector (no NDI) → oracle →
S1a/S7a. Nothing in M1 depends on an M2 task, and nothing in M1 is called receiver-proven.

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

### 8.2 When the repo goes private: schedule and cost (review 1 F8)
Rates (Robin, E4): Linux $0.006/min, Windows $0.010/min, macOS (arm64 or Intel) $0.062/min, per-job minutes rounded up.
Assumed job length 20 min per lane (rev 1's figure; cef-rs's own jobs run 3–12 min with a warm CEF cache). Per run: Linux
$0.12, Windows $0.20, each macOS lane $1.24. The Free plan's $12/month is the **whole account's** allowance, shared with every
other private repo, so the figures below are gross and nothing is assumed to be free.

**Chosen private schedule (hosted only, until Caleb approves the self-hosted Mac):**
- Triggers: `pull_request` (opened/synchronize) and `push` to `main` only. No `push` trigger on feature branches, so a PR
  update runs once, not twice. `concurrency: {group: ${{ github.workflow }}-${{ github.ref }}, cancel-in-progress: true}`.
  `paths-ignore` for docs-only changes (`docs/**`, `*.md`).
- PR runs: Linux + Windows only. Estimated 30 PR updates/month.
- `main` merges: all four lanes. Estimated 6 merges/month (one per milestone card group).
- Nightly: Linux only, full CEF test set, and only if `main` moved since the last nightly (a first job compares SHAs and
  exits). At most 30/month; with 6 merges usually ≤ 6.
- Milestone tags and release builds: run on demand, approval-gated (tags are Caleb's).
- Caches: rust-cache + CEF cache per lane as in §8.1 (about 1 GB total of the 10 GB free per repo; 7-day idle eviction means a
  cold macOS cache after a quiet week, about +10 min on that run). Artifacts: test reports and dumps with `retention-days: 3`;
  installers only on tags. Artifact storage pricing is not in Robin's report; a question for her before the switch (§9).

| Item | Runs/month | Cost |
|---|---:|---:|
| PR: Linux + Windows | 30 | $9.60 |
| `main` merge: Linux + Windows | 6 | $1.92 |
| `main` merge: macOS arm64 + macOS x86_64 | 6 | $14.88 |
| Nightly Linux (worst case, `main` moved every day) | 30 | $3.60 |
| **Total, expected (nightly at its worst case)** | | **$30.00/month** |
| Sensitivity: 60 PR updates and 12 merges | | $56.40/month |

**What replaces macOS on PRs.** PRs that touch `crates/noplayout-{renderer,output,frames,pixels,ndi}`, `web/host` or the
xtask bundling must carry a `cargo xtask preflight` file from a native Apple Silicon Mac at the PR head (CEF conformance,
pixel goldens, dev-bundle launch) linked in the PR description; the reviewer checks the commit hash in it. Native Intel
macOS coverage is the hosted `macos-15-intel` lane on every `main` merge until GitHub retires it (August 2027); Rosetta smoke
is not counted as native Intel coverage (§8.3). The Mac Studio stays excluded from all timing acceptance (§5.7).

**Optional, Caleb-approved:** a self-hosted runner on the Mac Studio (dedicated macOS user with no org credentials, ephemeral
work dirs, local CEF cache; vegapunk sets it up) would run macOS arm64 on every PR at no compute cost and drop the
expected bill to about $22.56/month (the Intel lane stays hosted). GitHub advises against self-hosted runners on public
repos, so this happens only after the switch.

- The repo must have its Apache-2.0 licence (D-010) and a clean history audit (no NDI SDK, no facility data, no secrets) before the switch.
- A GitHub spending limit for Actions (account billing setting, Caleb's to set) caps surprises; suggested $60/month.

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
- **vegapunk:** nothing until the private-repo switch (§8.2 optional self-hosted runner, Caleb-approved) and any lab NDI
  machines.
- **Usopp (rev 2 additions, for a small UI addendum after Caleb approves this plan):** (6) new operator-facing states that
  need wording and placement: standby state per channel (ready / rebuilding / diverged / off); recovery flags on the On-air
  strip ("elapsed time not restored", "custom actions froze this graphic and it was skipped"); "Preview restarting…" after a
  promotion; journal-length warning ("restoring this graphic after a crash would take about N s"); per-page lag for web
  outputs in Health; settings apply failures that name the failed step (§2.11). The strings in this plan are placeholders.
  (7) Settings writes can now return "changed elsewhere, reload" (`409`).
- **Robin:** no new research needed for M0–M4.5. Before the first public release: re-diff the NDI 6.4 EULA against 6.3.2 (her
  noted gap). Before the private switch: GitHub Actions artifact-storage quota and price for a private repo on the Free plan
  (§8.2).
- **Rayleigh:** `main` (4363981) carries D-001…D-014; it was merged into `design/plan` at 96a24c2 (merge, not rebase, so the
  reviewed 87908b9 stays reachable). Nothing pushed. A second cold review is gated on this revision.

---

## Open questions for Caleb

D-012…D-014 closed rev 1's two questions (Basketball package, OES data). Review 1 exposed places where IDEA's wording can't be
guaranteed for every case. I have not relaxed any of them; the plan keeps IDEA's bar and asks you here. Numbers marked *est.*
are my estimates until the M1/M2 spikes measure them.

**Q1. Does "back within 2 s" have to hold even after a double failure?**
- *What's met:* any single renderer failure (crash, hang, killed process, GPU process crash) is covered by a warm spare
  renderer per channel that has run the same show live. Output never goes black (the last frame holds) and the right picture
  is back in *est.* 0.15 s for a crash and *est.* 0.65 s for a hang. That covers IDEA's crash test with a wide margin.
- *What isn't guaranteed:* when the spare is also gone (a second failure within the few seconds it takes to rebuild it, a
  graphic bug that freezes both copies, an engine restart), the channel rebuilds from scratch: *est.* 0.6–2.5 s with the
  Basketball show, depending on the machine. The picture still holds the last frame during that time.
- *Cost of the spare:* one more browser per channel, *est.* 150–400 MB RAM and 15–30 % more CPU per channel.
- *Options:* (a) 2 s is guaranteed for single failures and a measured best-effort target for double failures;
  (b) 2 s for everything, which needs a third pre-started browser per channel (more RAM, and it still can't beat the
  freeze-both-copies case); (c) drop the spare to save resources and accept about 1–3 s recoveries that also lose a
  graphic's own running clock.
- **Recommend (a).**

**Q2. A third-party graphic's own running clock after a from-scratch rebuild.**
- OGraf gives a host no way to read or set a graphic's internal timer. After a from-scratch rebuild (the Q1 double-failure
  case only), a clock the graphic runs itself (Basketball `startClock`) restarts from where its data says, not from the
  time that actually passed. Clocks fed from data (OES → `gameClock`) and every graphic built in noPlayout's editor come back
  exactly.
- *Options:* (a) accept, show a clear flag on the On-air strip, and try the virtual-time spike S9; (b) require graphics
  used for live clocks to be data-driven or to support noPlayout's declared checkpoint.
- **Recommend (a).** For your own Basketball package, feeding the clock from the OES source avoids it entirely.

**Q3. If an OS can only pace in free-run mode, does that pass the "drop no frames" soak?**
- In free-run mode at 59.94 the browser draws at 60 Hz and noPlayout outputs a perfect 59.94 stream, so one drawn frame in
  every 1,000 is never shown, about once every 16.7 s. Every output frame is present with correct timecode; a scrolling
  ticker moves one frame's worth further at that moment. At 50 and 25 the rates match and nothing is skipped.
- *Options:* (a) accept it on that OS, labelled in STATUS.md and the release notes; (b) mark that OS's 59.94/29.97 formats
  *partial* until CEF adds timed begin-frames (CEF issue #4311, open).
- **Recommend:** decide when S1b shows which OSes need it (M2). My default is (b) for the soak claim with the formats still
  usable, because IDEA says "no frames".

**Q4. 50 Hz and UHD on every OS.**
- IDEA requires all eight formats, and 2160p needs about 1 GB/s of copy and conversion per channel at 29.97. If a supported
  OS or the reference hardware can't do a format cleanly, rev 1 would have quietly labelled it experimental or removed it.
  Rev 2 makes that a stop: the milestone stays open.
- *Options:* (a) keep every format required, and build the GPU-shared-texture path in v1 if needed (more work, ADR first);
  (b) let a specific format/OS pair ship as "not supported on this machine" with your approval, per pair.
- **Recommend (a)** until S2 (M1) and the M6 runs give real numbers; I'll come back with them before any deferral.

**Q5. Private CI, when the repo goes private.** The plan in §8.2 costs about $30/month gross on hosted runners (PRs on
Linux/Windows, all four lanes on merges, Linux nightly), up to about $56 with twice the activity, with a native-Mac
`preflight` file replacing macOS checks on PRs. **Recommend:** approve that schedule and a $60/month Actions spending limit at
the switch; the Mac Studio runner stays optional.

**Q6. Reference hardware for timing tests.** IDEA names an M-series Mac mini and a mid-range Windows PC with a discrete GPU
for the soak. Timing and recovery tests also need a named Linux box, and native Intel-Mac timing needs a real Intel Mac (the
CI runner is a VM). **Recommend:** name the Linux box now (any desktop with a discrete GPU), and accept CI-runner-only coverage
for Intel Macs (functional, not timing) given the 2027 sunset.

## Review 1 disposition

All eleven findings are accepted; none is rebutted. "Section" is where the change lives.

| Finding | Change | Section |
|---|---|---|
| F1 [blocking] 2 s recovery: no origin, kinds or budget | Output process owns the clock and NDI, so output holds through renderer failures; warm standby renderer per channel; failure kinds K1–K7 (nine rows) with detectors and budgets; t0/t1 defined at an independent NDI receiver; per-stage budget table; early S7a (M1) gate with the Basketball on-air set, S7b (M2) at a receiver; heartbeat tightened to 100 ms / 500 ms, inside IDEA's bounds | §2.1, §2.4, §2.4a, §2.13.1, §2.13.5, §4.2, §7 M1/M2/M6, E-040, E-041, E-043, Q1 |
| F2 [blocking] capped replay loses state | Cap and `goto` fallback withdrawn; uncapped persisted journal of every dispatched action, no compaction for third-party graphics; warm standby holds live state incl. internal clocks; explicit guarantee table by state class; operator flags for what cannot be rebuilt; declared checkpoint for Designed graphics; `addPoints` ×257 and `startClock` tests on warm and cold paths | §2.7, §2.13.2–§2.13.4, §2.13.6, §7 M1/M3/M4/M5, E-019 (superseded), E-044, E-045, Q2 |
| F3 [should] M5 implicit | M5 builds to UI rev 3 §3.1–§3.5 control by control and to IDEA §5.2; seven acceptance fixtures (shapes, text, images, clock, all field kinds, animation, lower third) plus editor behaviour tests | §7 M5 |
| F4 [blocking] counted defects pass | Pixel-only receiver-side oracle (counter + time-base + alpha-only patches); numeric thresholds per mode and rate; eight injected faults that must fail; mode B's 1000:1001 conversion stated as conversion, never as zero-drop; soak bar = mode A row | §5.2, §5.5, §6 R1, §7 M2/M6, E-006, E-048, Q3 |
| F5 [should] settings apply not transactional | Single owner with effective revision and `If-Match`; validate → plan → pending → apply with undo → commit; last-good = last applied; migration, future-version and interrupted-apply rules; full fault matrix | §2.9.2, §2.11, §7 M4, E-020, E-050 |
| F6 [should] ordering/authority across incarnations | Instance generation; renderer incarnation + launch token; loopback + token authority binding; promotion barrier; per-instance result commit in `seq` order; atomic snapshot-to-live handoff; tests | §2.5, §2.7, §2.14, §4.3, E-046 |
| F7 [should] paint identity and sender backpressure | Paint identity `(inc, paint_seq)` separate from slot `k`; no begin-frame inference; ring states with CAS, reader never overwritten; per-sender threads, one-deep queues, `pair_partial`; progress monitor and K5 | §2.4, §5.3, §5.4, E-042, E-049 |
| F8 [should] private CI cost | Real schedule (PR Linux/Windows, merges all four, Linux nightly only when `main` moved), dedup/cancel, cache and artifact policy; $30.00 expected, $56.40 sensitivity; native-Mac `preflight` replaces macOS on PRs; hosted Intel lane on merges; Rosetta not counted | §8.2, Q5 |
| F9 [should] fallbacks hide gaps | Fallback status table (in scope / partial / stop / Caleb deferral) with evidence owners; R1/R2/R3/R7/R13 rewritten; platform × format × test coverage table; passed vs skipped vs unexercised reporting | §6, §6.1, §6.2, §7 M6, Q4 |
| F10 [should] S1 ↔ M2 circular | S1a in M1 at paint level (provisional, no NDI needed) and S1b in M2 at a receiver (final); M1 order stated | §5.6, §7 M1/M2, spikes table |
| F11 [blocking] no server timestamp | `ts` with UTC (non-decreasing within a run), engine monotonic and channel slot domain on every action and journal entry; `TimeSync` and per-page lag; stated as informational, not scheduling; tests | §4.3, §2.14, §7 M3, E-047 |
| Gap: web-output test pattern | Test pattern drives every web role; M3 tests it | §2.10.2, §7 M3 |
| Gap: later JSON/HTTP/CSV/Sheets sources | Extension point kept explicit (already in rev 1's §2.8 revision) | §2.8 |
| R11/R12 superseded | Reconciled with D-013/D-014; Basketball committed in M0; OES corpus ported | §6, §7 M0/M4.5 |
| API audit notes (`FrameRate::new`/`connection_count` return `Result`; no borrowed builder into owned submit; backpressure) | Written into the NDI section | §2.10.1 |

## Process note
- Tier: rev 2 written by Franky on the frontier model (card t_63bf63cf), no subagents. Inputs: cold review 1
  (`~/OrgWork/projects/noplayout/reviews/plan-review-1.md`, Codex gpt-6.1-sol), `docs/01-decisions.md` D-001…D-014 (merged
  from `main` at 96a24c2), IDEA.md, UI.md rev 3, Robin's foundations report and addendum.
- Verified myself for rev 2 on 2026-11-26: iceoryx2 0.10.1 and memmap2 0.9.12 version, date, licence and MSRV from crates.io
  (API not yet read in source: M1 task 4 / S8); the OGraf v1 graphics manifest schema at 945f8bb allows `^v_.*` vendor keys
  (basis for `v_noplayout`); in the cef 156.7.0 sources: `execute_java_script`, `execute_dev_tools_method`,
  `add_dev_tools_message_observer`, `set_windowless_frame_rate`, `on_render_process_terminated`; in the Basketball package:
  `addPoints` (defaults `points = 2`), `startClock`/`stopClock`/`setClock`/`resetShotClock`, `showMessage`, the
  `gameClock`/`shotClock`/`clockRunning` data fields, the internal clock driven by `performance.now()`, and its size
  (4.6 MB, 111 files, 75 fonts/images); the eight IDEA formats; CI arithmetic recomputed from Robin's rates.
- Estimates, not measurements: every number marked *est.* and the §2.13.5 stage budget; standby CPU/RAM cost. S7a/S7b, S1a/S1b,
  S2 and S8 replace them with measurements before anything depends on them.
- Not done: no code compiled or run; no build cards (Rayleigh creates them after Caleb approves); nothing pushed; no GitHub
  repo. A second cold review is gated on this revision.

Rev 1 process note (t_95b8a2c9), kept for the record:
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
