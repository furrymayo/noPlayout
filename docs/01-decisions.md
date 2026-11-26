# noPlayout: project decisions

Later entries override earlier ones; nothing is deleted. Engineering choices live in `DECISIONS.md` (Franky).

| ID | Date | Decider | Decision | Why |
|---|---|---|---|---|
| D-001 | 2026-11-26 | Caleb | Product name is **noPlayout** (not Keyline). Crates `noplayout-core`, `noplayout-renderer`, `noplayout-app`; default NDI source name `NOPLAYOUT <channel name>`. | Caleb's choice. |
| D-002 | 2026-11-26 | Caleb | Run as an org project: Robin researches, Usopp designs the screens, Franky builds milestone by milestone with review gates. | Caleb's choice. |
| D-003 | 2026-11-26 | Caleb | GitHub repo is **public for now**, going private later. | Free CI minutes on Windows/macOS/Linux while public. Consequence: nothing that can't be public goes in the repo (no NDI SDK binaries/docs, no facility data, no secrets); CI design must also have a plan for private later (cost of macOS/Windows minutes). |
| D-004 | 2026-11-26 | Rayleigh (standing org rules) | `IDEA.md` is the source of truth; the build brief is `docs/BUILD-BRIEF.md`. Product-facing defaults stay generic (no venue, team or organisation names). NDI: dynamic loading, MIT headers only, no SDK in repo, NDI branding rules per Robin's 2026-10-30 NDI licence report. | Org rules + prior research. |
| D-005 | 2026-11-26 | Caleb | **Take IDs** (XPression-style): every rundown item has a user-editable number, and the operator can call up a graphic by typing its number (cue on Preview, then TAKE). Also exposed through the control API and hotkeys so Companion/Stream Deck can fire by number. | Caleb: operators call graphics by number, as in XPression. |
| D-006 | 2026-11-26 | Caleb | **Integrated third-party data** must have a home in the UI: a Data Sources area where the operator adds a source (scoreboard over serial, TCP or UDP as the reference examples; later JSON/HTTP poll, CSV/Sheets), sees its live status and values, and maps source fields onto graphic fields (e.g. scoreboard "Main Clock" → scorebug gameClock). The `DataSource` design in IDEA.md §7 must cover serial, TCP (dial-out and listen) and UDP (receive) transports. | Caleb's review of the mockups. |
| D-007 | 2026-11-26 | Caleb | Mockups approved, subject to D-005/D-006 being added. Usopp's open questions answered with Rayleigh's picks: (1) CLEAR LAYER / CLEAR ALL always cut; OUT is the only animated exit. (2) Add always-live hotkey alternates that work inside text fields (e.g. Ctrl+Space = TAKE). (3) Restore prompt: Enter = **Restore**. (4) Preview shows the cued item alone in v1. (5) Wizard: Web and NDI both on by default; NDI only when the runtime is found. (6) Per-channel format kept, under Advanced. (7) Auto-advance after TAKE off by default. | Caleb: "Yes, use Rayleigh's picks." |

## Open (not decided)
- Basketball OGraf Package: referenced by IDEA.md §2 as "provided alongside this file", but not on the install yet. Needed by M1. Also needs a licence check before it goes in a public repo.
- Build scope for scoreboard data sources (IDEA.md §7 says "designed now, built later"): v1 build or after v1? Asked 2026-11-26, not answered. Franky plans the framework so either works and recommends.
