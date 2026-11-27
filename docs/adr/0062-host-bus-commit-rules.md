# 0062. Host-bus commit rules: replay never commits, lost renderers resolve as 504, late pages get commands live

- **Status:** accepted
- **Date:** 2026-11-27
- **Deciders:** franky (frontier review t_a3bac44c of PR #2)
- **Related:** PLAN.md §2.13.2, §2.14, §4.3, §5.1; ADR 0059

## Context

PLAN §2.14 says the engine commits each action's result once, in seq order per instance, from
the authoritative program page only, and that replay results (a `Snapshot` journal or a
checkpoint tail) never commit. The M1 build left four gaps the review found:

1. The page did not mark replay results and the engine did not know which seqs it had fed a
   page as replay, so a replayed result could commit.
2. A pending seq sent to an authoritative renderer that died without a warm promotion (cold
   path) stayed pending forever, and every later seq on that instance waited behind it. A page
   that never answered had the same effect.
3. A command issued after the supervisor made an incarnation active but before its program page
   registered was in the page's `Snapshot` journal, so it ran as replay and its result could not
   commit: the caller waited out its timeout and got no result.
4. A standby's `held` results were only drained at promotion, so a long run grew them without
   bound, and promotion counted every already-committed one as `dropped_results`.

## Decision

1. **Replay is decided by both sides.** The page adds `replay: true` to every `Result` for an
   action it ran from a `Snapshot` journal or checkpoint tail. The engine records, per
   connection, the snapshot cut it sent (`replay_upto`); a result counts as replay if the page
   says so or its seq is at or below that cut. Replay results never commit. They are compared
   with the journal's committed result; a status-class mismatch counts as `replay_divergence`,
   reported beside `mirror_divergence` in the channel counters and in Health. A committed result
   the engine made up (item 2) is not compared.
2. **Lost results resolve as a committed failure.** When the authoritative incarnation is
   removed without a promotion, or a promotion makes active a page that saw a pending seq only as
   replay (or not at all), every unanswered pending seq commits as `statusCode 504`,
   `statusMessage "renderer lost before result"`, journalled, in seq order. Each action sent to
   the authoritative page also gets an engine deadline of the page's own timeout plus 2 s (load
   12 s, other methods 7 s); on expiry the same 504 commits. A real result that arrives later is
   counted as `dropped_results`. `ctl` waits 14 s, so it always sees the engine's answer.
3. **Late authoritative pages get commands live.** When an incarnation becomes active with no
   registered program page, the actor records `live_from = last_seq + 1` and keeps a backlog of
   every action from there. At that page's registration its `Snapshot` stops at `live_from − 1`
   (replay) and, in the same actor step, every backlog entry is sent to it as a live `Action` in
   seq order, so their results commit. Mirror pages still get the full channel. The same holds
   when the active page unregisters while its renderer lives.
4. **`held` is pruned on commit.** When the authoritative page commits seq n, n is removed from
   every other connection's `held` map; disposing an instance removes its seqs too.
5. **`ts.slot`** is `{k, epoch}` from the output process's `Hello` (§5.1): `k` is the first slot
   whose deadline `T_k` is after issue (`noplayout_frames::clock`), `epoch` is the output
   process's `epoch_mono_ns` in decimal. Without an output process the field is absent.

## Consequences

- A cold path is now visible to callers as a 504 for the actions in flight, rather than a
  silent stall; the operator can re-fire them. The engine state for those actions is "failed",
  which is the safe reading: the replacement renderer replays them but they did not commit live.
- The 504 is an engine-made status, not the graphic's. It must not be read as a graphic error in
  the eligibility audit (card B builds on this distinction).
- The backlog is bounded by the commands issued while no program page is registered (seconds at
  launch); it is cleared at the next promotion or removal.
