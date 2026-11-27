# 0034. Network exposure default as IDEA says (open on the LAN), token optional

- **Status:** accepted (DECISIONS.md: Proposed (flagged))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-034 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-034 (561e4a6), PLAN.md

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Choice.** As IDEA §4.2: the HTTP port (and RossTalk when enabled) binds all interfaces with no token by default, so
  Companion and OBS on other machines work out of the box. Advanced offers localhost-only, LAN-only and a token. Internal
  routes (shutdown, supervisor) are loopback-only with per-launch tokens regardless. Every state change from a non-loopback
  address is logged with its source.
- **Reason and flag.** This matches how broadcast control protocols (RossTalk, XPression) work on production LANs, but it means
  anyone on that LAN can take or clear graphics (R18). It's IDEA's call, so it's not in the open questions; Caleb can flip the
  default to LAN-only + token at no design cost.

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
