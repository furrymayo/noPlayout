# 0012. IPC: local sockets + postcard; no shared memory in v1

- **Status:** accepted; superseded in part by 0042 (DECISIONS.md: **Superseded in part by E-042** (local sockets + postcard stay for control))
- **Date:** 2026-11-26
- **Deciders:** franky (proposed); Caleb (approved the plan, D-016)
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-012 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-012 (561e4a6), PLAN.md, ADR 0008, ADR 0040, ADR 0042

## Amendments recorded in DECISIONS.md

> **Rev 2, superseded in part by E-042.** Control (heartbeat, configure, health, events) stays on `interprocess` local sockets
> with postcard, now one per process (output process and each renderer), with heartbeats at 100 ms. Monitor JPEGs keep a
> separate socket. Full-rate paints from renderer to output process go over the shared-memory ring of E-042, because NDI
> left the renderer (E-040).

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

- **Options.** gRPC, JSON over local sockets, postcard over local sockets, shared memory (iceoryx2 0.10.1), CEF process
  messages.

## Decision

- **Choice.** `interprocess` 2.4.10 local sockets (named pipes on Windows, Unix domain sockets elsewhere), two per channel
  (control and frames), `u32` length prefix + postcard, protocol version in `Hello`.
- **Reason.** Small messages at low rates (heartbeat 500 ms, health 1 Hz, JPEGs ≈15 fps). No code generator and no extra
  runtime; postcard is compact and serde-native. Separate sockets keep a burst of monitor frames from delaying a heartbeat.
  Shared memory is unnecessary once full-rate pixels stay in the renderer (E-008).

## Consequences

_Not recorded separately in DECISIONS.md; see Decision._
