import type { Timestamp } from './protocol';

/** All advancement uses performance.now(), never the adjustable page wall clock. */
export class ServerClock {
  private anchor?: { ts: Timestamp; localMs: number };
  private last?: Timestamp;
  private roundTripMs = 0;
  constructor(private readonly now: () => number) {}

  // Register -> Snapshot is a measured round trip. Periodic TimeSync has no echoed
  // request id in §4.3; retain the last sample rather than inventing a wire message.
  sync(ts: Timestamp, roundTripMs = this.roundTripMs): void {
    this.roundTripMs = Math.max(0, roundTripMs);
    this.anchor = { ts: structuredClone(ts), localMs: this.now() - this.roundTripMs / 2 };
  }
  serverNow(): Timestamp {
    if (!this.anchor) throw new Error('Server time is not synchronized');
    const { ts, localMs } = this.anchor;
    const elapsedUs = Math.max(0, Math.floor((this.now() - localMs) * 1000));
    let utcUs = ts.utcUs + elapsedUs;
    let us = ts.mono.us + elapsedUs;
    if (this.last?.mono.run === ts.mono.run) {
      utcUs = Math.max(utcUs, this.last.utcUs);
      us = Math.max(us, this.last.mono.us);
    }
    // A slot names the deadline at the original observation, not the current slot.
    this.last = { utcUs, mono: { run: ts.mono.run, us } };
    return structuredClone(this.last);
  }
  lag(ts: Timestamp): number {
    const now = this.serverNow();
    return now.mono.run === ts.mono.run ? Math.max(0, (now.mono.us - ts.mono.us) / 1000) : 0;
  }
}
