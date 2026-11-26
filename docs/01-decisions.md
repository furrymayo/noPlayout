# noPlayout: project decisions

Later entries override earlier ones; nothing is deleted. Engineering choices live in `DECISIONS.md` (Franky).

| ID | Date | Decider | Decision | Why |
|---|---|---|---|---|
| D-001 | 2026-11-26 | Caleb | Product name is **noPlayout** (not Keyline). Crates `noplayout-core`, `noplayout-renderer`, `noplayout-app`; default NDI source name `NOPLAYOUT <channel name>`. | Caleb's choice. |
| D-002 | 2026-11-26 | Caleb | Run as an org project: Robin researches, Usopp designs the screens, Franky builds milestone by milestone with review gates. | Caleb's choice. |
| D-003 | 2026-11-26 | Caleb | GitHub repo is **public for now**, going private later. | Free CI minutes on Windows/macOS/Linux while public. Consequence: nothing that can't be public goes in the repo (no NDI SDK binaries/docs, no facility data, no secrets); CI design must also have a plan for private later (cost of macOS/Windows minutes). |
| D-004 | 2026-11-26 | Rayleigh (standing org rules) | `IDEA.md` is the source of truth; the build brief is `docs/BUILD-BRIEF.md`. Product-facing defaults stay generic (no venue, team or organisation names). NDI: dynamic loading, MIT headers only, no SDK in repo, NDI branding rules per Robin's 2026-10-30 NDI licence report. | Org rules + prior research. |

## Open (not decided)
- Basketball OGraf Package: referenced by IDEA.md §2 as "provided alongside this file", but not on the install yet. Needed by M1. Also needs a licence check before it goes in a public repo.
