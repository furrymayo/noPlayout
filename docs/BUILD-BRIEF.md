# Agent prompt: build "noPlayout", an OGraf graphics playout engine

> **Org note (Rayleigh, 2026-11-26):** this brief was written for a single coding agent. In this org the work is split, and the split overrides the brief where they differ:
> - Research (OGraf spec, crate APIs, NDI, CEF, CI runners) is done by **Robin**; Franky reads her report and still verifies exact crate APIs on docs.rs/source for the versions he pins.
> - `UI.md` and the screen mockups are written by **Usopp**; Caleb approves the mockup PDF before any UI code.
> - `PLAN.md`, `DECISIONS.md`, code, CI and milestones are **Franky's**. PLAN.md gets a cold review (Codex gpt-6.1-sol) and Caleb's approval before M0 build starts.
> - The spec file is `IDEA.md` (this brief calls it `idea.md`).
> - Decisions log: `docs/01-decisions.md`. `DECISIONS.md` at the repo root is Franky's engineering decision record.


You are a senior Rust and broadcast-software engineer. Build **noPlayout**, a cross-platform (Windows / macOS / Linux) broadcast graphics engine built on the **EBU OGraf v1** standard. The full specification is in **`idea.md`** in this repository. Read all of it before writing any code. It is the source of truth. If this prompt and `idea.md` disagree, `idea.md` wins.

## What success looks like
A user installs noPlayout on Windows, macOS or Linux and goes through these steps:
1. On first launch they configure outputs in a short wizard: video format, channels, Web output and/or NDI output, and key/fill mode.
2. They import the provided **Basketball OGraf Package** (in `reference/basketball-ograf-package/`).
3. In **Playout** mode they build a rundown and run a game, taking graphics on and off air.
4. The graphics come out with correct key/fill both as **web output URLs** (for OBS / vMix browser sources) and as **NDI sources** (Fill+Alpha, or separate FILL and KEY sources).
5. In **Edit** mode they design a simple lower third and export it as a valid OGraf package.

The UI must stay clean and uncluttered: the main screens hold only what an operator needs during a show, and advanced options sit behind "Advanced" toggles. Settings must be robust: validated, versioned, written atomically, and recoverable.

## Hard constraints
- **Language:** Rust for the core, the renderer host, outputs, the control API and settings. Web tech (TypeScript + Svelte, unless you record a better choice) is allowed only for the UI frontend and for graphic content.
- **Platforms:** Windows x86_64, macOS arm64 and x86_64, and Linux x86_64. The same codebase must build on all of them. CI must prove it.
- **Standard:**
  - Use OGraf v1 graphics exactly as specified (https://github.com/ebu/ograf, `v1/specification`).
  - Validate manifests against the official JSON schemas.
  - Implement the OGraf Server API for external control where it applies.
- **HTML rendering for outputs:** use CEF offscreen rendering through the `cef` crate (tauri-apps/cef-rs). Do not use system webviews to produce output frames.
- **NDI:** use the NDI 6 SDK through a Rust binding, loading the runtime dynamically. Missing NDI must be handled gracefully. Follow the NDI SDK licence and distribution rules.
- **Reference package:** the Basketball OGraf Package must work unmodified. Treat any failure as an engine bug, not a package bug, unless you can show the package violates the spec. In that case, report it.

## How to work
1. **Research first, then plan.**
   - Clone the OGraf spec repo and read the graphics spec, the Server API spec and the JSON schemas.
   - Read the current docs for `cef` (cef-rs), your chosen NDI crate, Tauri v2, `axum` (or your chosen HTTP server) and `serialport`.
   - Do **not** guess crate APIs from memory. Check docs.rs or the source for the exact versions you pin.
   - Then write `PLAN.md`: the architecture as you will actually build it, the crate list with pinned versions, the IPC design, the risks, and a task list per milestone.
2. **Record decisions.** Keep `DECISIONS.md`. Every non-obvious choice gets an entry with the options considered, the choice, and the reason. Examples: which NDI crate, shaped vs unshaped default, frame-pacing method, UI framework.
3. **Build milestone by milestone** (M0 → M6 in `idea.md` §11). Each milestone must:
   - compile and pass `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` on all three OSes in CI;
   - end with something runnable, and a short "How to try it" section added to `README.md`;
   - be committed with a clear message, with a git tag per milestone (`m0`, `m1`, …).
4. **No fake features.**
   - Don't stub something and call it done. If a feature is partial, label it `// TODO(Mx):` with a matching entry in `STATUS.md`.
   - Don't simulate NDI or CEF output in tests and claim they work. Real-output tests may skip with a clear message when the runtime is unavailable in CI, but they must exist and run locally.
5. **Test as you go.** Use the tests in `idea.md` §9: OGraf conformance on the EBU examples and the Basketball package, golden-image pixel tests with an alpha ramp, NDI loopback when available, crash recovery and a soak test script.
6. **When blocked**, for example if a platform can't build CEF OSR or a crate is broken:
   - try at most two reasonable alternatives;
   - write up the problem in `STATUS.md` with what you tried;
   - continue with the other milestones. Do not silently drop scope.
7. **Keep the UI calm.** Before writing UI code, write `UI.md` with low-fidelity layouts for these screens: Output Setup wizard, Playout, Edit, Settings, Health. Show which controls are default and which are under Advanced. The layouts in `idea.md` §5 are the baseline.

## Priorities if you run out of time
Deliver in this order and stop at a clean milestone boundary rather than leaving everything half-done:
1. **M0–M2:** offscreen OGraf rendering and NDI key/fill.
2. **M3:** web outputs.
3. **M4:** Playout mode, settings and API.
4. **M5:** Edit mode.
5. **M6:** hardening and installers.

## Final report (write to `STATUS.md` and summarise in your last message)
- Each milestone's status: done / partial / not started, with evidence (test names, screenshots or commands).
- Exact steps to build and run on each OS, including the prerequisites: NDI runtime, Clang/LLVM for bindgen, and how CEF binaries are fetched.
- Known issues and limitations, especially frame pacing, key/fill sync, and macOS signing.
- Every assumption you made that the user should confirm.