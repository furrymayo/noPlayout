import { describe, expect, it } from 'vitest';
import { ServerClock } from './clock';
import { parseHostMessage } from './validation';
const ts = { utcUs: 1000000, mono: { run: 'run', us: 1000 } };

describe('time synchronization', () => {
  it('estimates one-way transit from a measured round trip and retains it for unsolicited syncs', () => {
    let ms = 0; const clock = new ServerClock(() => ms);
    clock.sync(ts, 80);
    expect(clock.serverNow()).toEqual({ utcUs: 1040000, mono: { run: 'run', us: 41000 } });
    ms = 100; clock.sync({ utcUs: 1100000, mono: { run: 'run', us: 101000 } });
    expect(clock.serverNow()).toEqual({ utcUs: 1140000, mono: { run: 'run', us: 141000 } });
  });
});

describe('wire validation', () => {
  const a = { type: 'Action', seq: 2, revision: 3, instanceId: 'i', generation: 1, method: 'playAction', params: {}, ts };
  it('accepts exactly the action identity and timestamp envelope', () => {
    expect(parseHostMessage(JSON.stringify(a))).toEqual(a);
  });
  it.each([
    { ...a, method: 'constructor' }, { ...a, generation: -1 }, { ...a, seq: 1.1 },
    { ...a, ts: { utcUs: 1 } }, { ...a, params: null }, { ...a, type: 'Register' },
    { type: 'Snapshot', lastSeq: 1, revision: 1, serverNow: ts, flags: [], layers: [{}] },
  ])('rejects malformed/unrecognised control input before calling graphics: %j', (value) => {
    expect(() => parseHostMessage(JSON.stringify(value))).toThrow();
  });
});
