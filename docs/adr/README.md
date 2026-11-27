# Architecture Decision Records

Standard: GeneralDocuments/project-adr-template.md. Newest at the bottom.

ADRs 0001–0059 were backfilled in M0 from `DECISIONS.md` E-001…E-059 at 561e4a6 (E-035): ADR NNNN is entry E-NNN, its text copied verbatim. Product decisions (D-nnn) stay in `docs/01-decisions.md`. From M0 on, a design choice gets its ADR here in the same PR.

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [0001](0001-process-model.md) | Three process kinds: app, headless engine, one renderer per channel | accepted | 2026-11-26 |
| [0002](0002-workspace-layout.md) | Cargo workspace split; renderer does not link the engine core | accepted | 2026-11-26 |
| [0003](0003-ui-framework.md) | UI framework: Tauri v2 (stable) + system webview | accepted | 2026-11-26 |
| [0004](0004-ui-front-end.md) | UI front end: Svelte 5 + TypeScript | accepted | 2026-11-26 |
| [0005](0005-cef-paint-mode.md) | CEF paint mode: CPU `on_paint`, shared texture off in v1 | accepted | 2026-11-26 |
| [0006](0006-frame-pacing.md) | Frame pacing: integer per-channel clock; drive mode chosen per OS by S1 | accepted | 2026-11-26 |
| [0007](0007-ndi-crate.md) | NDI crate: grafton-ndi 3.0.0 behind our own trait | accepted | 2026-11-26 |
| [0008](0008-where-ndi-is-sent.md) | NDI sending happens inside the renderer process | superseded by 0040 | 2026-11-26 |
| [0009](0009-shaped-vs-unshaped-defaults.md) | Shaped vs unshaped defaults, per output mode | accepted | 2026-11-26 |
| [0010](0010-ndi-wire-details.md) | NDI wire format BGRA/BGRX, explicit timecode, SDK clocking off | accepted | 2026-11-26 |
| [0011](0011-ndi-runtime.md) | NDI runtime detected and linked, never bundled | accepted | 2026-11-26 |
| [0012](0012-ipc-transport-and-encoding.md) | IPC: local sockets + postcard; no shared memory in v1 | accepted; superseded in part by 0042 | 2026-11-26 |
| [0013](0013-monitor-frames.md) | Monitor frames: downscaled JPEG, credit flow control | accepted | 2026-11-26 |
| [0014](0014-one-cef-browser-process-per-channel.md) | One CEF browser process per channel | accepted | 2026-11-26 |
| [0015](0015-cue-preloads.md) | Cue preloads the graphic into a hidden slot | accepted | 2026-11-26 |
| [0016](0016-web-fill-and-key.md) | Web fill/key pages made from the same scene with SVG colour-matrix filters | accepted | 2026-11-26 |
| [0017](0017-one-host-runtime-program-authoritative.md) | One OGraf host runtime for every page; the CEF program page is authoritative | accepted | 2026-11-26 |
| [0018](0018-how-the-engine-drives-pages.md) | Pages are driven over a WebSocket host protocol, not CEF process messages | accepted | 2026-11-26 |
| [0019](0019-crash-recovery.md) | Crash recovery by replaying each instance's action log with `skipAnimation` | superseded by 0041, 0043, 0044, 0045 | 2026-11-26 |
| [0020](0020-settings-store.md) | Settings: JSON, atomic write, last-good, validation in Rust | accepted | 2026-11-26 |
| [0021](0021-take-ids.md) | Take IDs: u16, unique per show, enforced in the engine | accepted | 2026-11-26 |
| [0022](0022-decoder-plug-ins.md) | Decoder plug-ins: compiled in, one crate per decoder | accepted | 2026-11-26 |
| [0023](0023-real-captures-gate-decoders-oes-only-in-v1.md) | Decoders are gated on real captures; OES only in v1 | accepted | 2026-11-26 |
| [0024](0024-ograf-spec-vendoring-and-offline-validation.md) | OGraf spec vendored at a commit; manifest validation never uses the network | accepted | 2026-11-26 |
| [0025](0025-edit-mode-preview-through-cef.md) | Edit-mode preview rendered by a CEF renderer | accepted | 2026-11-26 |
| [0026](0026-ograf-server-api-mapping.md) | OGraf Server API mapping: renderer = channel, RenderTarget = `{layer}` | accepted | 2026-11-26 |
| [0027](0027-openapi-from-code.md) | OpenAPI generated from code with utoipa | accepted | 2026-11-26 |
| [0028](0028-intel-macos.md) | Intel macOS: tested until GitHub retires the runner, then build-only | accepted | 2026-11-26 |
| [0029](0029-toolchain.md) | Toolchain pinned to Rust 1.100.0, edition 2024 | accepted | 2026-11-26 |
| [0030](0030-jpeg-encoder.md) | Pure-Rust JPEG encoder | accepted | 2026-11-26 |
| [0031](0031-serialport-mpl-2-0.md) | Accept `serialport` (MPL-2.0) | accepted | 2026-11-26 |
| [0032](0032-licence-gate.md) | cargo-deny licence allowlist; nothing copyleft in the renderer | accepted | 2026-11-26 |
| [0033](0033-engine-death-and-restore.md) | Engine death: renderers and output processes exit; supervised restart restores, cold start asks | accepted | 2026-11-26 |
| [0034](0034-network-exposure.md) | Network exposure default as IDEA says (open on the LAN), token optional | accepted | 2026-11-26 |
| [0035](0035-adrs.md) | ADRs in `docs/adr/`, one per accepted entry | accepted | 2026-11-26 |
| [0036](0036-repo-licence.md) | Repo licence Apache-2.0 | accepted | 2026-11-26 |
| [0037](0037-rosstalk-compatible-listener.md) | RossTalk-compatible listener in v1, off by default | accepted | 2026-11-26 |
| [0038](0038-data-source-storage-and-loss-behaviour.md) | Data-source connections per computer, mappings per show; hold on loss | accepted | 2026-11-26 |
| [0039](0039-take-onto-an-occupied-layer.md) | TAKE onto an occupied layer replaces it with a cut | accepted | 2026-11-26 |
| [0040](0040-per-channel-output-process-owns-the-clock-and-ndi.md) | Per-channel output process owns the clock and the NDI senders | accepted | 2026-11-26 |
| [0041](0041-warm-standby-renderer-per-channel.md) | Warm standby renderer per channel, live-mirrored | accepted | 2026-11-26 |
| [0042](0042-shared-memory-paint-ring.md) | Shared-memory paint ring per renderer (iceoryx2 or own ring) | accepted; superseded in part by 0063 | 2026-11-26 |
| [0043](0043-failure-kinds-and-the-recovery-measurement.md) | Failure kinds, detectors and the recovery measurement | accepted | 2026-11-26 |
| [0044](0044-uncapped-on-air-journal.md) | Uncapped on-air journal; no compaction for third-party graphics | accepted | 2026-11-26 |
| [0045](0045-time-dependent-state.md) | Time-dependent state: declared checkpoints, flags, no silent loss | accepted | 2026-11-26 |
| [0046](0046-identities-authority-and-result-order.md) | Identities, authority binding and ordered result commit | accepted | 2026-11-26 |
| [0047](0047-server-timestamps.md) | Server timestamps on the host protocol | accepted | 2026-11-26 |
| [0048](0048-pacing-acceptance-by-a-receiver-side-oracle.md) | Pacing acceptance by a pixel-only receiver-side oracle | accepted | 2026-11-26 |
| [0049](0049-paint-identity-one-conversion-per-sender-threads.md) | Paint identity, single conversion, per-sender threads | accepted | 2026-11-26 |
| [0050](0050-settings-apply-transaction.md) | Settings apply transaction with one owner and an effective revision | accepted | 2026-11-26 |
| [0051](0051-fallback-completion-status.md) | Fallback completion status; required formats and platforms stay required | accepted | 2026-11-26 |
| [0052](0052-recovery-fidelity-per-replica-copy-and-standby-eligibility.md) | Recovery fidelity per replica copy; promotion eligibility; what the 2 s same-state guarantee covers | accepted | 2026-11-26 |
| [0053](0053-pacing-oracle-time-base-in-the-source-domain.md) | Pacing oracle time base in the source domain, plus a lag band | accepted | 2026-11-26 |
| [0054](0054-settings-commit-point-and-post-apply-failures.md) | Settings: one commit point, defined outcome for every persistence and rollback failure | accepted | 2026-11-26 |
| [0055](0055-a-non-returning-ndi-submit-recycles-the-output-process.md) | A non-returning NDI submit recycles the output process; slow submits keep the sender | accepted | 2026-11-26 |
| [0056](0056-declared-checkpoint-protocol-and-the-designed-time-model.md) | Declared checkpoint protocol: quiescent cut, frontier seq, timed tail inside restore | accepted | 2026-11-26 |
| [0057](0057-paintstall-detected-by-a-forced-repaint-probe.md) | PaintStall detected by a forced-repaint probe, not by rAF | accepted | 2026-11-26 |
| [0058](0058-time-declarations-list-every-timer.md) | Time declarations list every timer with its entry points; exposure and flags per timer; CI declaration audit | accepted | 2026-11-26 |
| [0059](0059-load-frontier-for-checkpoint-declaring-graphics.md) | Load frontier: exact timestamped full replay before the first checkpoint and as the last fallback (`replayed-timed`) | accepted | 2026-11-26 |
| [0060](0060-immutable-ograf-package-library.md) | Immutable OGraf package snapshots and offline import boundaries | proposed | 2026-11-27 |
| [0061](0061-provisional-drive-mode.md) | Provisional drive mode per OS (S1a): macOS A, Windows/Linux pending | draft | 2026-11-27 |
| [0062](0062-host-bus-commit-rules.md) | Host-bus commit rules: replay never commits, lost renderers resolve as 504, late pages get commands live | accepted | 2026-11-27 |
| [0063](0063-own-paint-ring-transport.md) | Paint ring transport: own ring over libc `mmap` / `CreateFileMappingW`; iceoryx2 not evaluated (supersedes 0042 in part) | accepted | 2026-11-27 |
| [0064](0064-pixel-golden-budgets.md) | Pixel goldens: per-OS shape goldens, premultiplied colour compare, fixed budgets (0 px shape, ≤ 32 px text) and a changed-score negative control | accepted | 2026-11-27 |
