import { describe, expect, it, vi } from 'vitest';
import { GraphicLoader } from './loader';
import { HostConnection } from './connection';
import type { HostUp } from './protocol';

class Element {
  style: Record<string, string> = {}; dataset: Record<string, string> = {};
  children: Element[] = []; parent?: Element;
  appendChild(e: Element) { e.parent = this; this.children.push(e); return e; }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter((e) => e !== this); }
}
class FakeGraphic extends Element {
  load = vi.fn(async () => undefined); dispose = vi.fn(async () => undefined);
  playAction = vi.fn(); stopAction = vi.fn(); updateAction = vi.fn(); customAction = vi.fn();
}
function dom() {
  const tags = new Map<string, typeof FakeGraphic>();
  const registry = { get: (name: string) => tags.get(name), define: vi.fn((name: string, ctor: typeof FakeGraphic) => { tags.set(name, ctor); }) };
  const document = { createElement: vi.fn((name: string) => { const ctor = tags.get(name); return ctor ? new ctor() : new Element(); }) };
  return { registry, document, stage: new Element() };
}
const params = { graphicUrl: '/graphics/pkg/hash/test.ograf.json', layer: 4, data: {},
  renderCharacteristics: { resolution: { width: 1920, height: 1080 }, frameRate: 50 } };

describe('browser loader adapters (no simulated rendering claims)', () => {
  it('fetches the manifest and registers its real default-export custom element once', async () => {
    const d = dom(); const fetchManifest = vi.fn(async () => ({ main: './graphic.mjs', supportsRealTime: true, stepCount: -1 }));
    const importModule = vi.fn(async () => ({ default: FakeGraphic }));
    const loader = new GraphicLoader({ ...d, baseUrl: 'https://engine.invalid/host/renderer.html', fetchManifest, importModule } as never);
    const first = await loader.create(params); const second = await loader.create(params);
    expect(fetchManifest).toHaveBeenCalledWith('https://engine.invalid/graphics/pkg/hash/test.ograf.json');
    expect(importModule).toHaveBeenCalledWith('https://engine.invalid/graphics/pkg/hash/graphic.mjs');
    expect(d.registry.define).toHaveBeenCalledTimes(1);
    expect(first.graphic).toBeInstanceOf(FakeGraphic); expect(second.graphic).not.toBe(first.graphic);
    expect(d.stage.children).toHaveLength(1); // one stable layer container
  });
  it('keeps hidden cues sized, and promotes without a DOM move or reconnect', async () => {
    const d = dom(); const loader = new GraphicLoader({ ...d, baseUrl: 'https://engine.invalid/',
      fetchManifest: async () => ({ main: 'graphic.mjs', supportsRealTime: true }), importModule: async () => ({ default: FakeGraphic }) } as never);
    const created = await loader.create({ ...params, cued: true });
    const slot = d.stage.children[0]; const parent = slot.parent;
    expect(slot.style.visibility).toBe('hidden'); expect(slot.style.display).not.toBe('none');
    expect(slot.style.width).toBe('1920px'); expect(slot.style.height).toBe('1080px');
    created.surface.show(4);
    expect(slot.parent).toBe(parent); expect(slot.style.visibility).toBe('visible'); expect(slot.style.zIndex).toBe('4');
    created.surface.remove(); expect(d.stage.children).toHaveLength(0);
  });
  it('rejects a non-realtime or malformed manifest before importing', async () => {
    const d = dom(); const importModule = vi.fn();
    const loader = new GraphicLoader({ ...d, baseUrl: 'https://engine.invalid/',
      fetchManifest: async () => ({ main: 'graphic.mjs', supportsRealTime: false }), importModule } as never);
    await expect(loader.create(params)).rejects.toThrow('realtime'); expect(importModule).not.toHaveBeenCalled();
  });
});

class Socket {
  readyState = 0; sent: string[] = [];
  onopen?: () => void; onmessage?: (e: { data: string }) => void; onclose?: () => void;
  send(value: string) { this.sent.push(value); } close() { this.readyState = 3; this.onclose?.(); }
  open() { this.readyState = 1; this.onopen?.(); }
}
describe('host socket registration', () => {
  it('uses /ws/host/channel and consumes program identity once from the hook, never from a URL', () => {
    const socket = new Socket(); const connect = vi.fn(() => socket);
    const connection = new HostConnection({ channel: 'channel', role: 'program', origin: 'https://engine.invalid', connect } as never);
    expect(connect).not.toHaveBeenCalled();
    connection.launch({ inc: 8, token: 'test-only-launch-identity' }); socket.open();
    expect(connect).toHaveBeenCalledWith('wss://engine.invalid/ws/host/channel');
    expect(JSON.parse(socket.sent[0])).toEqual({ type: 'Register', channel: 'channel', role: 'program', hostVersion: '0.0.1', inc: 8, token: 'test-only-launch-identity' });
    expect(() => connection.launch({ inc: 9, token: 'second' })).toThrow();
    connection.close();
  });
  it('measures the registration-to-snapshot round trip without extending the wire schema', () => {
    let now = 100; const socket = new Socket(); const onMessage = vi.fn();
    const connection = new HostConnection({ channel: 'ch', role: 'preview', origin: 'https://engine.invalid',
      now: () => now, connect: () => socket, onMessage } as never);
    socket.open(); now = 180;
    const message = { type: 'Snapshot', revision: 0, lastSeq: 0, serverNow: { utcUs: 1000, mono: { run: 'run', us: 0 } }, layers: [], flags: [] };
    socket.onmessage?.({ data: JSON.stringify(message) });
    expect(onMessage).toHaveBeenCalledWith(message, 80);
    connection.close();
  });
  it('mirror registration carries no launch credential and forwards exactly shaped messages', () => {
    const socket = new Socket(); const connection = new HostConnection({ channel: 'ch', role: 'web-program', origin: 'http://engine.invalid', connect: () => socket } as never);
    socket.open();
    expect(JSON.parse(socket.sent[0])).toEqual({ type: 'Register', channel: 'ch', role: 'web-program', hostVersion: '0.0.1' });
    const m: HostUp = { type: 'Result', seq: 1, instanceId: 'i', generation: 2, statusCode: 200 };
    connection.send(m); expect(JSON.parse(socket.sent[1])).toEqual(m);
    connection.close();
  });
});
