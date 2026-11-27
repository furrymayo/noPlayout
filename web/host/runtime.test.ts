import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HostRuntime, type Graphic, type RuntimeAdapters } from './runtime';
import type { Action, HostUp, Snapshot, Timestamp } from './protocol';

const ts = (us = 0): Timestamp => ({ utcUs: 1000000 + us, mono: { run: 'run', us } });
const format = { resolution: { width: 1920, height: 1080 }, frameRate: 60000 / 1001 };
const action = (seq: number, method: Action['method'], params = {}, generation = 1): Action =>
  ({ type: 'Action', revision: seq, seq, ts: ts(seq), instanceId: 'i', generation, method, params });
const load = (seq = 1, generation = 1) => action(seq, 'load', {
  graphicUrl: '/graphics/test/manifest.ograf.json', data: { a: 1, b: 2 }, renderCharacteristics: format, layer: 2,
}, generation);
const deferred = <T = unknown>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => { resolve = r; });
  return { promise, resolve };
};
const flush = async () => { for (let i = 0; i < 30; i++) await Promise.resolve(); };
function setup(overrides: Record<string, unknown> = {}, manifest = { stepCount: -1, v_noplayout: { recovery: 'checkpoint' } }) {
  const graphic = {
    load: vi.fn(async () => ({ statusCode: 200 })),
    playAction: vi.fn(async (_params: Parameters<Graphic['playAction']>[0]) => ({ statusCode: 200, currentStep: 0 })),
    updateAction: vi.fn(async () => undefined),
    stopAction: vi.fn(async () => undefined),
    customAction: vi.fn(async () => ({ statusCode: 200 })),
    dispose: vi.fn(async () => undefined), ...overrides,
  };
  const sent: HostUp[] = [];
  const surface = { show: vi.fn(), remove: vi.fn() };
  const create = vi.fn<RuntimeAdapters['create']>(async () => ({ graphic, surface, manifest }));
  const host = new HostRuntime({ send: (m: HostUp) => sent.push(m), create, now: () => performance.now() });
  return { host, graphic, sent, create, surface };
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('OGraf dispatch', () => {
  it('gates all actions on load, then starts overlapping methods in seq order', async () => {
    const loading = deferred(); const playing = deferred();
    const { host, graphic, sent } = setup({ load: vi.fn(() => loading.promise), playAction: vi.fn(() => playing.promise) });
    host.receive(load()); host.receive(action(2, 'playAction', { skipAnimation: true }));
    host.receive(action(3, 'updateAction', { data: { a: 3 }, skipAnimation: true }));
    await flush(); expect(graphic.playAction).not.toHaveBeenCalled();
    loading.resolve({ statusCode: 200 }); await flush();
    expect(graphic.playAction).toHaveBeenCalledTimes(1); expect(graphic.updateAction).toHaveBeenCalledTimes(1);
    expect(graphic.playAction.mock.invocationCallOrder[0]).toBeLessThan(graphic.updateAction.mock.invocationCallOrder[0]);
    expect(graphic.updateAction).toHaveBeenCalledWith(expect.objectContaining({ data: { a: 3 }, skipAnimation: true }));
    expect(host.inspect('i', 1)?.data).toEqual({ a: 3, b: 2 });
    expect(sent.filter((m) => m.type === 'Result').map((m) => m.seq)).toEqual([1, 3]);
    expect(host.lastAppliedSeq).toBe(1);
    playing.resolve({ statusCode: 200, currentStep: undefined, result: { ended: true } }); await flush();
    expect(sent.at(-1)).toEqual({ type: 'Result', seq: 2, instanceId: 'i', generation: 1, statusCode: 200, payload: { ended: true } });
    expect(host.lastAppliedSeq).toBe(3); expect(host.inspect('i', 1)?.atEnd).toBe(true);
  });
  it('reports load timeout without ever calling a queued method', async () => {
    const { host, graphic, sent } = setup({ load: vi.fn(() => new Promise(() => {})) });
    host.receive(load()); host.receive(action(2, 'playAction')); await flush();
    await vi.advanceTimersByTimeAsync(10000); await flush();
    expect(graphic.playAction).not.toHaveBeenCalled();
    expect(sent).toContainEqual(expect.objectContaining({ type: 'Result', seq: 1, statusCode: 504 }));
    expect(sent).toContainEqual(expect.objectContaining({ type: 'Result', seq: 2, statusCode: 424 }));
    expect(host.lastAppliedSeq).toBe(2);
  });
  it('bounds actions, catches throws, normalizes void and preserves all result identities', async () => {
    const { host, sent } = setup({ playAction: vi.fn(() => { throw new Error('broken'); }), customAction: vi.fn(() => new Promise(() => {})) });
    host.receive(load()); await flush();
    host.receive(action(2, 'playAction')); host.receive(action(3, 'customAction', { id: 'hang' }));
    host.receive(action(4, 'stopAction', { skipAnimation: true })); await flush();
    await vi.advanceTimersByTimeAsync(5000);
    expect(sent).toContainEqual(expect.objectContaining({ type: 'Result', seq: 2, generation: 1, statusCode: 500 }));
    expect(sent).toContainEqual({ type: 'Result', seq: 4, instanceId: 'i', generation: 1, statusCode: 200 });
    expect(sent).toContainEqual(expect.objectContaining({ type: 'Result', seq: 3, statusCode: 504 }));
  });
  it('never lets disposed-generation late results touch or report for a successor', async () => {
    const first = deferred();
    const { host, graphic, sent } = setup({ playAction: vi.fn(() => first.promise) });
    host.receive(load()); await flush(); host.receive(action(2, 'playAction')); await flush();
    host.receive(action(3, 'dispose')); await flush();
    host.receive(load(4, 2)); await flush();
    expect(graphic.dispose).toHaveBeenCalledTimes(1);
    first.resolve({ statusCode: 200, currentStep: 99 }); await flush();
    expect(sent.filter((m) => m.type === 'Result' && m.seq === 2)).toEqual([]);
    expect(host.inspect('i', 2)?.currentStep).toBeUndefined();
    expect(host.lastAppliedSeq).toBe(4);
  });
  it('cleans up a late successful load after the 10 s timeout', async () => {
    const loading = deferred();
    const { host, graphic, surface } = setup({ load: vi.fn(() => loading.promise) });
    host.receive(load()); await flush(); await vi.advanceTimersByTimeAsync(10000);
    expect(surface.remove).toHaveBeenCalled();
    loading.resolve({ statusCode: 200 }); await flush();
    expect(graphic.dispose).toHaveBeenCalledTimes(1);
  });
  it('rechecks animation age after a slow load and preserves vendor result fields', async () => {
    const loading = deferred();
    const { host, graphic, sent } = setup({ load: vi.fn(() => loading.promise),
      customAction: vi.fn(async () => ({ statusCode: 200, statusMessage: 'done', result: { score: 1 }, v_extra: 7 })) });
    host.receive({ type: 'TimeSync', serverNow: ts() }); host.receive(load());
    host.receive(action(2, 'playAction', { skipAnimation: false })); await flush();
    await vi.advanceTimersByTimeAsync(600); loading.resolve({ statusCode: 200 }); await flush();
    expect(graphic.playAction).toHaveBeenCalledWith(expect.objectContaining({ skipAnimation: true }));
    host.receive(action(3, 'customAction', { id: 'result' })); await flush();
    expect(sent.at(-1)).toMatchObject({ type: 'Result', seq: 3, payload: { result: { score: 1 }, v_extra: 7 } });
  });
  it('promotes a cue with visibility only, and never clips a dynamic step count', async () => {
    const { host, graphic, surface } = setup();
    const cue = load(); cue.params.cued = true;
    host.receive(cue); await flush(); expect(surface.show).not.toHaveBeenCalled();
    for (let seq = 2; seq < 8; seq++) host.receive(action(seq, 'playAction', { delta: 1, skipAnimation: true }));
    await flush();
    expect(surface.show).toHaveBeenCalledExactlyOnceWith(2);
    expect(graphic.playAction).toHaveBeenCalledTimes(6);
    expect(host.inspect('i', 1)?.stepCount).toBe(-1);
  });
});

const snapshot = (journal: Snapshot['layers'][number]['journal'] = []): Snapshot => ({
  type: 'Snapshot', revision: 4, lastSeq: 4, serverNow: ts(1000000), flags: [],
  layers: [{ layer: 2, instanceId: 'i', generation: 1, graphicUrl: '/graphics/test/manifest.ograf.json',
    dataAtLoad: { a: 1, b: 2 }, renderCharacteristics: format, loadTs: ts(10), journal }],
});

describe('snapshot handoff and time', () => {
  it('replays original timestamps, buffers 200 live actions, deduplicates and catches up before Synced', async () => {
    const loading = deferred();
    const { host, graphic, sent } = setup({ load: vi.fn(() => loading.promise) });
    const s = snapshot([{ seq: 2, method: 'playAction', ts: ts(20), params: {} },
      { seq: 3, method: 'customAction', ts: ts(30), params: { id: 'poison' }, quarantined: true },
      { seq: 4, method: 'updateAction', ts: ts(40), params: { data: { a: 4 } } }]);
    const restoring = host.receive(s); await flush();
    for (let seq = 5; seq < 205; seq++) {
      host.receive(action(seq, 'customAction', { id: 'increment' }));
      host.receive(action(seq, 'customAction', { id: 'increment' }));
    }
    expect(sent.some((m) => m.type === 'Synced')).toBe(false);
    loading.resolve({ statusCode: 200 }); await restoring;
    expect(graphic.load).toHaveBeenCalledWith(expect.objectContaining({ renderType: 'realtime', renderCharacteristics: format,
      v_noplayout: expect.objectContaining({ ts: ts(10), replay: true }) }));
    expect(graphic.playAction).toHaveBeenCalledWith(expect.objectContaining({ skipAnimation: true,
      v_noplayout: expect.objectContaining({ ts: ts(20), replay: true }) }));
    expect(graphic.customAction).toHaveBeenCalledTimes(200);
    expect(graphic.customAction).toHaveBeenLastCalledWith(expect.objectContaining({ skipAnimation: true,
      v_noplayout: expect.objectContaining({ ts: ts(204), replay: false }) }));
    expect(sent.at(-1)).toEqual({ type: 'Synced', seq: 204 });
    host.receive(action(204, 'customAction')); await flush(); expect(graphic.customAction).toHaveBeenCalledTimes(200);
    // Snapshot journal results carry replay: true; live results never do (§2.14).
    const results = sent.filter((m) => m.type === 'Result');
    expect(results.filter((m) => m.replay).map((m) => m.seq)).toEqual([2, 4]);
    expect(results.filter((m) => m.seq >= 5).every((m) => m.replay === undefined)).toBe(true);
  });
  it('starts cross-layer snapshot journal calls in channel seq order, regardless of load completion order', async () => {
    const slow = deferred<{ statusCode: number }>(); const started: number[] = [];
    const { host, create } = setup();
    create.mockImplementation(async (p: unknown) => {
      const layer = (p as { layer: number }).layer;
      return { graphic: { load: vi.fn(() => layer === 2 ? slow.promise : Promise.resolve({ statusCode: 200 })),
        dispose: vi.fn(async () => undefined), stopAction: vi.fn(async () => undefined), updateAction: vi.fn(async () => undefined),
        customAction: vi.fn(async () => ({ statusCode: 200 })), playAction: vi.fn(async () => { started.push(layer); return { statusCode: 200, currentStep: 0 }; }) },
        surface: { show: vi.fn(), remove: vi.fn() }, manifest: { stepCount: -1, v_noplayout: { recovery: 'checkpoint' } } };
    });
    const s = snapshot([{ seq: 3, method: 'playAction', ts: ts(3), params: {} }]);
    s.layers.push({ ...s.layers[0], layer: 4, instanceId: 'j', generation: 2,
      journal: [{ seq: 4, method: 'playAction', ts: ts(4), params: {} }] });
    const restoring = host.receive(s); await flush();
    expect(started).toEqual([]);
    slow.resolve({ statusCode: 200 }); await restoring;
    expect(started).toEqual([2, 4]);
  });
  it('does not schedule future timestamps and skips animation only beyond 500 ms', async () => {
    const { host, graphic } = setup(); await host.receive({ ...snapshot(), lastSeq: 1 });
    host.receive({ ...action(2, 'playAction', { skipAnimation: false }), ts: ts(500000) });
    host.receive({ ...action(3, 'playAction', { skipAnimation: false }), ts: ts(499999) });
    host.receive({ ...action(4, 'playAction', { skipAnimation: false }), ts: ts(9000000) });
    await flush();
    expect(graphic.playAction.mock.calls.map((call) => (call[0] as { skipAnimation: boolean }).skipAnimation)).toEqual([false, true, false]);
  });
  it('sends 100 ms beats without rAF, reports contiguous application and monotonic server time', async () => {
    const { host, sent } = setup(); host.receive({ type: 'TimeSync', serverNow: ts(1000) }); host.startBeats();
    host.animationFrame(); await vi.advanceTimersByTimeAsync(100);
    expect(sent.at(-1)).toEqual({ type: 'Beat', n: 1, rafCount: 1, lagMs: 0, lastAppliedSeq: 0 });
    const before = host.serverNow();
    host.receive({ type: 'TimeSync', serverNow: ts(0) });
    expect(host.serverNow().mono.us).toBeGreaterThanOrEqual(before.mono.us);
    await vi.advanceTimersByTimeAsync(100); expect(sent.at(-1)).toMatchObject({ n: 2, rafCount: 1 });
    host.close(); const n = sent.length; await vi.advanceTimersByTimeAsync(1000); expect(sent).toHaveLength(n);
  });
});

describe('quiescent checkpoints', () => {
  it('defers after B resolves before A, then cuts at highest frontier with neither effect in the tail', async () => {
    const a = deferred(); const b = deferred(); let effects = 0;
    const custom = vi.fn((p: Record<string, unknown>) => {
      if (p.id === 'A') { effects++; return a.promise; }
      if (p.id === 'B') { effects++; return b.promise; }
      return Promise.resolve({ statusCode: 200, result: { effects } });
    });
    const { host, sent } = setup({ customAction: custom });
    await host.receive({ ...snapshot(), lastSeq: 1 });
    host.receive(action(2, 'customAction', { id: 'A' })); host.receive(action(3, 'customAction', { id: 'B' })); await flush();
    host.receive({ type: 'Checkpoint', instanceId: 'i', generation: 1 });
    expect(sent.at(-1)).toEqual({ type: 'CheckpointDeferred', instanceId: 'i' });
    b.resolve({ statusCode: 200 }); await flush();
    expect(custom.mock.calls.some(([p]) => p.id === 'v_noplayout_checkpoint')).toBe(false);
    a.resolve({ statusCode: 200 }); await flush(); await vi.advanceTimersByTimeAsync(0); await flush();
    expect(sent).toContainEqual({ type: 'CheckpointResult', instanceId: 'i', generation: 1, frontierSeq: 3,
      cutTs: ts(1000000), state: { effects: 2 } });
  });
  it('does not block new on-air calls while an asynchronously returning checkpoint is in flight', async () => {
    const capture = deferred(); const { host, sent, graphic } = setup({ customAction: vi.fn((p: { id: string }) =>
      p.id === 'v_noplayout_checkpoint' ? capture.promise : Promise.resolve({ statusCode: 200 })) });
    await host.receive({ ...snapshot(), lastSeq: 1 });
    host.receive(action(2, 'playAction')); await flush();
    host.receive({ type: 'Checkpoint', instanceId: 'i', generation: 1 }); await flush();
    host.receive(action(3, 'playAction')); await flush();
    expect(graphic.playAction).toHaveBeenCalledTimes(2);
    capture.resolve({ statusCode: 200, result: { captured: 2 } }); await flush();
    expect(sent).toContainEqual(expect.objectContaining({ type: 'CheckpointResult', frontierSeq: 2, state: { captured: 2 } }));
  });
  it('includes a third action arriving while A is held in the eventual quiescent frontier', async () => {
    const a = deferred(); const { host, sent } = setup({ customAction: vi.fn((p: { id: string }) =>
      p.id === 'A' ? a.promise : Promise.resolve({ statusCode: 200, result: { captured: true } })) });
    await host.receive({ ...snapshot(), lastSeq: 1 });
    host.receive(action(2, 'customAction', { id: 'A' })); host.receive(action(3, 'customAction', { id: 'B' })); await flush();
    host.receive({ type: 'Checkpoint', instanceId: 'i', generation: 1 });
    host.receive(action(4, 'customAction', { id: 'C' })); await flush();
    expect(sent.some((m) => m.type === 'CheckpointResult')).toBe(false);
    a.resolve({ statusCode: 200 }); await flush(); await vi.advanceTimersByTimeAsync(0); await flush();
    expect(sent).toContainEqual(expect.objectContaining({ type: 'CheckpointResult', frontierSeq: 4 }));
  });
  it('recovers a declared clock exactly from the timed load frontier before the first checkpoint', async () => {
    let valueAtTs = 0; let startedAt = 0; let serverNow: (() => Timestamp) | undefined;
    const { host } = setup({
      load: vi.fn(async (p: { v_noplayout: { ts: Timestamp; serverNow: () => Timestamp } }) => {
        valueAtTs = 60000000; startedAt = p.v_noplayout.ts.mono.us; serverNow = p.v_noplayout.serverNow;
        return { statusCode: 200 };
      }),
      customAction: vi.fn(async (p: { id: string; v_noplayout: { ts: Timestamp } }) => {
        if (p.id === 'startClock') startedAt = p.v_noplayout.ts.mono.us;
        return { statusCode: 200 };
      }),
    });
    const s = snapshot([{ seq: 2, ts: ts(1000000), method: 'customAction', params: { id: 'startClock' } }]);
    s.serverNow = ts(4000000); s.layers[0].loadTs = ts(0);
    await host.receive(s);
    expect(valueAtTs - (serverNow!().mono.us - startedAt)).toBe(57000000);
    await vi.advanceTimersByTimeAsync(20000);
    expect(valueAtTs - (serverNow!().mono.us - startedAt)).toBe(37000000);
  });
  it('discards a failed restore, accepts an older checkpoint, and finally accepts the whole timed journal', async () => {
    const { host, sent, graphic } = setup({ customAction: vi.fn(async (p: { id: string; payload?: { state?: { invalid?: boolean } } }) =>
      ({ statusCode: p.id === 'v_noplayout_restore' && p.payload?.state?.invalid ? 500 : 200 })) });
    const s = snapshot([{ seq: 4, method: 'playAction', ts: ts(40), params: {} }]);
    s.layers[0].checkpoint = { state: { invalid: true }, frontierSeq: 3, cutTs: ts(30) };
    await host.receive(s); expect(sent.at(-1)).toMatchObject({ type: 'PageError', text: expect.stringContaining('checkpoint_discarded') });
    expect(graphic.playAction).not.toHaveBeenCalled();
    s.layers[0].checkpoint = { state: {}, frontierSeq: 2, cutTs: ts(20) };
    await host.receive(s); expect(sent.at(-1)).toEqual({ type: 'Synced', seq: 4 });
    delete s.layers[0].checkpoint;
    s.layers[0].journal.unshift({ seq: 2, method: 'customAction', ts: ts(20), params: { id: 'startClock' } });
    await host.receive(s);
    expect(graphic.customAction).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'startClock',
      v_noplayout: expect.objectContaining({ ts: ts(20), replay: true }) }));
    expect(sent.at(-1)).toEqual({ type: 'Synced', seq: 4 });
  });
  it('accepts a checkpoint taken after an engine restart while preserving the original load timestamp', async () => {
    const { host, sent } = setup(); const s = snapshot();
    s.layers[0].checkpoint = { state: {}, frontierSeq: 3, cutTs: { utcUs: 2000000, mono: { run: 'new-run', us: 10 } } };
    s.serverNow = { utcUs: 3000000, mono: { run: 'new-run', us: 1000010 } };
    await host.receive(s);
    expect(sent.at(-1)).toEqual({ type: 'Synced', seq: 4 });
  });
  it('does not claim quiescence when a method times out but its promise remains unsettled', async () => {
    const { host, sent } = setup({ customAction: vi.fn(() => new Promise(() => {})) });
    await host.receive({ ...snapshot(), lastSeq: 1 }); host.receive(action(2, 'customAction', { id: 'hang' })); await flush();
    await vi.advanceTimersByTimeAsync(5000);
    host.receive({ type: 'Checkpoint', instanceId: 'i', generation: 1 });
    expect(sent.at(-1)).toEqual({ type: 'CheckpointDeferred', instanceId: 'i' });
    expect(sent.some((m) => m.type === 'CheckpointResult')).toBe(false);
  });
  it('restores checkpoint/timed tail and signals an invalid frontier instead of falsely syncing', async () => {
    const { host, graphic, sent } = setup();
    const s = snapshot([{ seq: 4, method: 'customAction', ts: ts(40), params: { id: 'C' } }]);
    s.layers[0].checkpoint = { state: { effects: 2 }, frontierSeq: 3, cutTs: ts(35) };
    await host.receive(s);
    expect(graphic.customAction).toHaveBeenNthCalledWith(1, expect.objectContaining({ id: 'v_noplayout_restore',
      payload: { state: { effects: 2 }, cutTs: ts(35), nowTs: ts(1000000) } }));
    expect(graphic.customAction).toHaveBeenNthCalledWith(2, expect.objectContaining({ id: 'C', skipAnimation: true,
      v_noplayout: expect.objectContaining({ replay: true, ts: ts(40) }) }));
    s.layers[0].checkpoint.frontierSeq = 999;
    sent.length = 0; await host.receive(s);
    expect(sent).toContainEqual(expect.objectContaining({ type: 'PageError', text: expect.stringContaining('"instanceId":"i"') }));
    expect(sent.some((m) => m.type === 'Synced')).toBe(false);
    expect(host.inspect('i', 1)).toBeUndefined();
  });
});
