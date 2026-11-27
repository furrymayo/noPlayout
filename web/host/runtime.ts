import type { GraphicsAPI } from 'ograf';
import { ServerClock } from './clock';
import type { Action, HostDown, HostUp, LoadParams, Method, Params, Snapshot, SnapshotLayer, Timestamp } from './protocol';

export type Graphic = Pick<GraphicsAPI.Graphic, Method>;
export interface Surface { show(layer: number): void; remove(): void }
export interface CreatedGraphic {
  graphic: Graphic; surface: Surface;
  manifest: { stepCount?: number; v_noplayout?: { recovery?: string } };
}
export interface RuntimeAdapters {
  send(message: HostUp): void;
  create(params: LoadParams): Promise<CreatedGraphic>;
  now(): number;
}
interface Outcome { statusCode: number; statusMessage?: string; currentStep?: number; payload?: unknown }
interface Instance {
  id: string; generation: number; layer: number; cued: boolean; alive: boolean;
  data: Params; ready: Promise<Outcome>; created?: CreatedGraphic;
  pending: Set<number>; unsettled: number; frontier: number;
  checkpointRequested: boolean; checkpointInFlight: boolean;
  currentStep?: number; atEnd: boolean; stateSeq: number; loadFailed: boolean;
}
class CheckpointRestoreError extends Error {
  constructor(readonly layer: SnapshotLayer) { super('Checkpoint restore failed'); }
}
const failure = (statusCode: number, statusMessage: string): Outcome => ({ statusCode, statusMessage });
const ok = (outcome: Outcome) => outcome.statusCode >= 200 && outcome.statusCode < 300;
/** `a` is not earlier than `b`: monotonic within one engine run, the engine's non-stepping UTC across runs (§4.3). */
const notBefore = (a: Timestamp, b: Timestamp) => a.mono.run === b.mono.run ? a.mono.us >= b.mono.us : a.utcUs >= b.utcUs;
function normalize(value: unknown): Outcome {
  if (value === undefined) return { statusCode: 200 };
  if (!value || typeof value !== 'object' || Array.isArray(value)) return failure(500, 'Invalid OGraf return payload');
  const { statusCode, statusMessage, currentStep, result, payload, ...extra } = value as Params;
  if (!Number.isInteger(statusCode) || (statusCode as number) < 100 || (statusCode as number) > 599)
    return failure(500, 'Invalid OGraf statusCode');
  if (currentStep !== undefined && (!Number.isInteger(currentStep) || (currentStep as number) < 0))
    return failure(500, 'Invalid OGraf currentStep');
  return {
    statusCode: statusCode as number,
    ...(typeof statusMessage === 'string' ? { statusMessage } : {}),
    ...(currentStep !== undefined ? { currentStep: currentStep as number } : {}),
    ...(Object.keys(extra).length || (payload !== undefined && result !== undefined)
      ? { payload: { ...extra, ...(result !== undefined ? { result } : {}), ...(payload !== undefined ? { payload } : {}) } }
      : payload !== undefined ? { payload } : result !== undefined ? { payload: result } : {}),
  };
}

/** Ordered starts, overlapping promises, generation-isolated state. The engine owns authority. */
export class HostRuntime {
  readonly clock: ServerClock;
  private instances = new Map<string, Instance>();
  private buffered = new Map<number, Action>();
  private completed = new Set<number>();
  private acceptedSeq = 0;
  private appliedSeq = 0;
  private replaying = false;
  private epoch = 0;
  private lagMs = 0;
  private beatN = 0;
  private rafCount = 0;
  private beatTimer?: ReturnType<typeof setTimeout>;
  private closed = false;

  constructor(private readonly adapters: RuntimeAdapters) { this.clock = new ServerClock(adapters.now); }
  get lastAppliedSeq(): number { return this.appliedSeq; }
  serverNow = (): Timestamp => this.clock.serverNow();
  inspect(id: string, generation: number) {
    const i = this.instances.get(id);
    return i?.generation === generation ? {
      data: structuredClone(i.data), currentStep: i.currentStep, atEnd: i.atEnd,
      stepCount: i.created?.manifest.stepCount ?? 1, cued: i.cued, layer: i.layer,
    } : undefined;
  }
  animationFrame(): void { this.rafCount++; }
  startBeats(): void {
    if (this.beatTimer !== undefined || this.closed) return;
    const tick = () => {
      this.adapters.send({ type: 'Beat', n: ++this.beatN, rafCount: this.rafCount, lagMs: this.lagMs, lastAppliedSeq: this.appliedSeq });
      this.beatTimer = setTimeout(tick, 100);
    };
    this.beatTimer = setTimeout(tick, 100);
  }
  close(): void {
    this.closed = true; this.epoch++;
    clearTimeout(this.beatTimer); this.beatTimer = undefined;
    for (const i of this.instances.values()) this.retire(i);
    this.instances.clear(); this.buffered.clear();
  }
  receive(message: HostDown, roundTripMs?: number): Promise<void> {
    if (this.closed) return Promise.resolve();
    switch (message.type) {
      case 'TimeSync': this.clock.sync(message.serverNow, roundTripMs); return Promise.resolve();
      case 'Snapshot': return this.snapshot(message, roundTripMs);
      case 'Checkpoint': {
        const i = this.instances.get(message.instanceId);
        if (i?.alive && i.generation === message.generation) {
          i.checkpointRequested = true; this.checkpoint(i);
        }
        return Promise.resolve();
      }
      case 'Action': {
        if (message.seq <= this.acceptedSeq || this.buffered.has(message.seq)) return Promise.resolve();
        this.buffered.set(message.seq, message);
        if (!this.replaying) this.drain();
        return Promise.resolve();
      }
    }
  }
  private drain(): Promise<Outcome>[] {
    const work: Promise<Outcome>[] = [];
    while (this.buffered.has(this.acceptedSeq + 1)) {
      const a = this.buffered.get(++this.acceptedSeq)!;
      this.buffered.delete(a.seq);
      let stale = false;
      try { this.lagMs = this.clock.lag(a.ts); stale = this.lagMs > 500; } catch { /* load can precede a snapshot in unit/embed use */ }
      work.push(this.dispatch(a, stale));
    }
    return work;
  }
  private complete(seq: number): void {
    if (seq <= this.appliedSeq) return;
    this.completed.add(seq);
    while (this.completed.delete(this.appliedSeq + 1)) this.appliedSeq++;
  }
  private result(a: Action, outcome: Outcome): void {
    this.adapters.send({ type: 'Result', seq: a.seq, instanceId: a.instanceId, generation: a.generation, ...outcome });
  }
  private make(id: string, generation: number, p: LoadParams, ts: Timestamp, replay: boolean): Instance {
    const prior = this.instances.get(id);
    if (prior) this.retire(prior);
    if (!p.cued) {
      for (const i of this.instances.values()) if (i.alive && !i.cued && i.layer === p.layer) this.retire(i);
    } else {
      for (const i of this.instances.values()) if (i.alive && i.cued) this.retire(i);
    }
    const i: Instance = {
      id, generation, layer: p.layer, cued: !!p.cued, alive: true, data: structuredClone(p.data ?? {}),
      ready: Promise.resolve({ statusCode: 200 }), pending: new Set(), unsettled: 0, frontier: 0,
      checkpointRequested: false, checkpointInFlight: false, atEnd: false, stateSeq: 0, loadFailed: false,
    };
    this.instances.set(id, i);
    i.ready = this.bounded(i, async () => {
      const created = await this.adapters.create(p);
      i.created = created;
      if (!i.alive || i.loadFailed) { created.surface.remove(); return failure(410, 'Instance disposed'); }
      const value = await created.graphic.load({ data: structuredClone(i.data), renderType: 'realtime', renderCharacteristics: p.renderCharacteristics,
        v_noplayout: { ts, replay, serverNow: this.serverNow } });
      if (i.loadFailed) void this.invoke(i, 'dispose', {});
      return value;
    }, 10000).then((outcome) => {
      if (!ok(outcome)) { i.loadFailed = true; i.created?.surface.remove(); }
      return outcome;
    });
    return i;
  }
  private dispatch(a: Action, stale = false, replay = false): Promise<Outcome> {
    const epoch = this.epoch;
    let i = this.instances.get(a.instanceId);
    if (a.method === 'load') i = this.make(a.instanceId, a.generation, a.params as LoadParams, a.ts, replay);
    if (!i?.alive || i.generation !== a.generation) {
      const outcome = failure(410, 'Instance generation is no longer present');
      this.result(a, outcome); this.complete(a.seq); return Promise.resolve(outcome);
    }
    const instance = i;
    instance.pending.add(a.seq);
    const operation = a.method === 'load' ? instance.ready : instance.ready.then((loaded) => {
      if (!instance.alive) return failure(410, 'Instance disposed');
      if (!ok(loaded)) {
        if (a.method === 'dispose') this.retire(instance, false);
        return failure(424, 'Graphic load did not succeed');
      }
      if (a.method === 'playAction' && instance.cued) this.promote(instance, Number(a.params.layer ?? instance.layer));
      if (a.method === 'updateAction') instance.data = { ...instance.data, ...(a.params.data as Params ?? {}) };
      try { stale ||= this.clock.lag(a.ts) > 500; } catch { /* direct embed without TimeSync */ }
      const params = { ...a.params, ...((stale || replay) ? { skipAnimation: true } : {}), v_noplayout: { ts: a.ts, replay, serverNow: this.serverNow } };
      const result = this.invoke(instance, a.method, params);
      if (a.method === 'dispose') this.retire(instance, false);
      return result;
    });
    return operation.then((outcome) => {
      instance.pending.delete(a.seq);
      instance.frontier = Math.max(instance.frontier, a.seq);
      if (epoch !== this.epoch) return outcome;
      this.complete(a.seq);
      if ((instance.alive && this.instances.get(instance.id) === instance) || a.method === 'dispose') {
        if (ok(outcome) && a.seq >= instance.stateSeq && (a.method === 'playAction' || a.method === 'stopAction')) {
          instance.stateSeq = a.seq;
          if (a.method === 'playAction') { instance.currentStep = outcome.currentStep; instance.atEnd = outcome.currentStep === undefined; }
          if (a.method === 'stopAction') { instance.currentStep = undefined; instance.atEnd = true; }
        }
        this.result(a, outcome);
        this.checkpoint(instance);
      }
      return outcome;
    });
  }
  private promote(i: Instance, layer: number): void {
    for (const other of this.instances.values()) if (other !== i && other.alive && !other.cued && other.layer === layer) this.retire(other);
    i.layer = layer; i.cued = false; i.created?.surface.show(layer);
  }
  private invoke(i: Instance, method: Method, params: Params): Promise<Outcome> {
    return this.bounded(i, () => {
      const graphic = i.created!.graphic;
      // Runtime validation precedes this call; OGraf's typed signatures differ per method.
      const fn = graphic[method] as (p: Params) => Promise<unknown>;
      if (typeof fn !== 'function') throw new Error('Graphic method is missing');
      return fn.call(graphic, params);
    }, 5000);
  }
  private bounded(i: Instance, call: () => unknown, ms: number): Promise<Outcome> {
    i.unsettled++;
    return new Promise((resolve) => {
      const timer = setTimeout(() => resolve(failure(504, `Graphic method timed out after ${ms} ms`)), ms);
      let promise: Promise<unknown>;
      try { promise = Promise.resolve(call()); } catch { promise = Promise.reject(new Error('Graphic method threw')); }
      promise.then((value) => resolve(normalize(value)), () => resolve(failure(500, 'Graphic method failed'))).finally(() => {
        clearTimeout(timer); i.unsettled--;
        // A timeout is not proof the graphic settled: checkpoint stays deferred until here.
        queueMicrotask(() => this.checkpoint(i));
      });
    });
  }
  private retire(i: Instance, dispose = true): void {
    if (!i.alive) return;
    i.alive = false; i.checkpointRequested = false; i.created?.surface.remove();
    if (this.instances.get(i.id) === i) this.instances.delete(i.id);
    if (dispose) void i.ready.then((loaded) => {
      if (ok(loaded) && i.created) return this.invoke(i, 'dispose', {});
    });
  }
  private checkpoint(i: Instance): void {
    if (!i.alive || !i.checkpointRequested || i.checkpointInFlight) return;
    if (this.replaying || i.pending.size || i.unsettled || i.frontier === 0) {
      this.adapters.send({ type: 'CheckpointDeferred', instanceId: i.id }); return;
    }
    // The engine resolves effective declarations (manifest OR per-package sidecar) and
    // sends Checkpoint only for declared instances. Role strings cannot request a cut.
    const frontierSeq = i.frontier; const cutTs = this.serverNow();
    i.checkpointRequested = false; i.checkpointInFlight = true;
    // invoke starts synchronously: the graphic captures state before its first await.
    void this.invoke(i, 'customAction', { id: 'v_noplayout_checkpoint', payload: {}, v_noplayout: { ts: cutTs, replay: false, serverNow: this.serverNow } }).then((outcome) => {
      i.checkpointInFlight = false;
      if (!i.alive) return;
      if (ok(outcome) && outcome.payload !== undefined)
        this.adapters.send({ type: 'CheckpointResult', instanceId: i.id, generation: i.generation, frontierSeq, cutTs, state: outcome.payload });
      else this.adapters.send({ type: 'PageError', text: 'Checkpoint capture failed' });
    });
  }
  private async snapshot(s: Snapshot, roundTripMs?: number): Promise<void> {
    const epoch = ++this.epoch;
    this.replaying = true; this.clock.sync(s.serverNow, roundTripMs);
    for (const i of this.instances.values()) this.retire(i);
    this.instances.clear(); this.completed.clear(); this.appliedSeq = 0; this.acceptedSeq = s.lastSeq;
    for (const seq of this.buffered.keys()) if (seq <= s.lastSeq) this.buffered.delete(seq);
    try {
      const layers = [...s.layers, ...(s.cued ? [s.cued] : [])];
      await Promise.all(layers.map((l) => this.restore(l, s, l === s.cued)));
      if (epoch !== this.epoch || this.closed) return;
      const replay = layers.flatMap((l) => l.journal.filter((a) => !a.quarantined).map((entry): Action => ({
        ...entry, type: 'Action', revision: s.revision, instanceId: l.instanceId, generation: l.generation,
      }))).sort((a, b) => a.seq - b.seq);
      await Promise.all(replay.map((a) => this.dispatch(a, true, true)));
      if (epoch !== this.epoch || this.closed) return;
      this.appliedSeq = s.lastSeq; this.completed.clear();
      // The subscription starts at snapshot lastSeq + 1. Drain batches until caught up;
      // duplicate packets are discarded by seq, not by the instance's last result.
      while (true) {
        const work = this.drain();
        if (!work.length) break;
        await Promise.all(work);
        if (epoch !== this.epoch || this.closed) return;
      }
      this.replaying = false;
      this.adapters.send({ type: 'Synced', seq: this.appliedSeq });
      for (const i of this.instances.values()) this.checkpoint(i);
    } catch (error) {
      if (epoch !== this.epoch || this.closed) return;
      for (const i of this.instances.values()) this.retire(i);
      // Engine owns the stored head/older checkpoints; a tail-only page cannot invent them.
      // Remain in rebuild (no Synced) until the engine sends the older checkpoint/load frontier.
      const text = error instanceof CheckpointRestoreError
        ? `checkpoint_discarded: ${JSON.stringify({ instanceId: error.layer.instanceId, generation: error.layer.generation,
          frontierSeq: error.layer.checkpoint?.frontierSeq })}; resend older checkpoint or load frontier`
        : 'snapshot_restore_failed: graphic load or replay failed';
      this.adapters.send({ type: 'PageError', text });
    }
  }
  private async restore(l: SnapshotLayer, s: Snapshot, cued: boolean): Promise<void> {
    const i = this.make(l.instanceId, l.generation, { graphicUrl: l.graphicUrl, data: l.dataAtLoad,
      renderCharacteristics: l.renderCharacteristics, layer: l.layer, cued }, l.loadTs, true);
    if (!ok(await i.ready)) throw new Error('Snapshot load failed');
    if (!i.alive || this.instances.get(i.id) !== i) throw new Error('Snapshot superseded');
    const cp = l.checkpoint;
    if (cp) {
      if (!Number.isSafeInteger(cp.frontierSeq) || cp.frontierSeq < 1 || cp.frontierSeq > s.lastSeq
          || l.journal.some((a) => a.seq <= cp.frontierSeq) || !notBefore(cp.cutTs, l.loadTs))
        throw new CheckpointRestoreError(l);
      const restored = await this.invoke(i, 'customAction', { id: 'v_noplayout_restore', payload: { state: cp.state, cutTs: cp.cutTs, nowTs: this.serverNow() },
        skipAnimation: true, v_noplayout: { ts: cp.cutTs, replay: true, serverNow: this.serverNow } });
      if (!ok(restored)) { this.retire(i); throw new CheckpointRestoreError(l); }
      i.frontier = cp.frontierSeq;
    }
  }
}
