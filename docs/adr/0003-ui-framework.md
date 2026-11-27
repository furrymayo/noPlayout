# 0003. UI framework: Tauri v2 (stable) + system webview

- **Status:** accepted (DECISIONS.md: Proposed)
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-003 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-003 (561e4a6), PLAN.md, ADR 0025

## Context

- **Context.** IDEA asks for Tauri or "a CEF-based UI". Robin D3: Tauri+CEF exists only as `tauri-runtime-cef`
  3.0.0-alpha.7 (no stable release, depends on `cef =154.5.0`).

## Options considered

- **Options.** (a) Tauri v2.12.7 with the system webview (WebView2 / WKWebView / WebKitGTK). (b) Tauri 3 alpha with CEF.
  (c) A windowed cef-rs UI host of our own. (d) Native Rust UI (egui, Slint, Iced).

## Decision

- **Choice.** (a).
- **Reason.** Stable and supported on all three OSes. (b) is alpha and cannot be in the same build graph as our renderer:
  `cef-dll-sys` declares `links = "cef_dll_wrapper"`, so Cargo refuses two versions (154 and our 156) in one workspace. (c)
  means writing window management, menus, dialogs and packaging ourselves. (d) would throw away Usopp's HTML/CSS mockups
  and the shared web tooling. The UI never shows graphics pixels directly: monitors are JPEG frames from the renderer and the
  edit canvas comes from CEF (E-025), so webview differences don't affect what goes to air.

## Consequences

- **Risks.** WebKitGTK quirks on Linux (R7), handled with `WEBKIT_DISABLE_DMABUF_RENDERER=1` and a manual check on Fedora KDE
  Wayland and an NVIDIA machine. Fallback is (c).
- **Would change if.** Tauri 3 with CEF goes stable on a CEF version we can share; re-evaluate then.
