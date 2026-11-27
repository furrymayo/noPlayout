# 0023. Decoders are gated on real captures; OES only in v1

- **Status:** accepted (DECISIONS.md: Decided (Caleb, D-011); **gate replaced by D-013** for OES)
- **Date:** 2026-11-26
- **Deciders:** Caleb; franky
- **Backfilled:** yes, 2026-11-27, from DECISIONS.md E-023 at 561e4a6 (text copied verbatim, grouped into this template; E-035)
- **Related:** DECISIONS.md E-023 (561e4a6), PLAN.md, docs/01-decisions.md D-010, docs/01-decisions.md D-011, docs/01-decisions.md D-013

## Amendments recorded in DECISIONS.md

> **Rev 2: the OES gate is replaced by D-013.** M4.5 is done when the decoder passes a corpus ported from
> `unc-virtual-score-server` (test vectors plus simulator streams recorded once into captures), byte-for-byte over serial
> loopback, TCP and UDP. No venue recording is required. STATUS.md states D-013's limitation (the simulators share the
> decoder's protocol knowledge; a live console check is recommended at the first game, not a gate). The real-capture gate
> below still applies to later decoders (Daktronics etc.) unless Caleb decides otherwise for them.

## Context

_Not recorded separately in DECISIONS.md; see Decision._

## Options considered

_Not recorded separately in DECISIONS.md; see Decision._

## Decision

- **Reason.** "No fake features": a decoder tested only against bytes we generated proves nothing about the console. OES has
  no checksum, so field-range and length checks are what flag corruption; a real capture is the only way to tune them.

## Consequences

- **Choice (D-011 + gate).** v1 ships one decoder, OES, ported from Caleb's MIT-licensed `unc-virtual-score-server`
  (framing STX / sport type / printable payload / CR; nine sport layouts; 9600 baud). A decoder is listed in the product only
  when a recorded byte stream from a real console decodes to a golden timeline Caleb has checked. Daktronics All Sport 5000
  RTD is next (D-010) and carries the same gate; the `daktronics-allsport-5000` 0.5.0 crate has no tests and skips the
  checksum, so if used, only its field tables are trusted.
