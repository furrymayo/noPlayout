import { describe, expect, it } from 'vitest';
import { type ChildHealth, type EngineStatus, rendererLine, statusLine } from './health.ts';

const child = (over: Partial<ChildHealth> = {}): ChildHealth => ({
  state: 'starting',
  pid: null,
  version: null,
  cef_version: null,
  chromium_version: null,
  paints: null,
  detail: null,
  ...over,
});

const status = (renderer: ChildHealth): EngineStatus => ({
  mode: 'spawned',
  port: 8686,
  error: null,
  health: {
    engine: { state: 'running', version: '0.0.1', pid: 1, uptime_ms: 10 },
    renderer,
    output: child({ state: 'ok' }),
  },
});

describe('statusLine', () => {
  it('shows the M0 line when the renderer is up', () => {
    const s = status(child({ state: 'ok', cef_version: '156.0.31' }));
    expect(statusLine(s)).toBe('Engine: running · Renderer: CEF 156.0.31 OK');
  });

  it('shows renderer progress and failure in plain words', () => {
    expect(rendererLine(child())).toBe('Renderer: starting');
    expect(rendererLine(child({ state: 'missing' }))).toBe('Renderer: not found');
    expect(rendererLine(child({ state: 'failed' }))).toBe('Renderer: failed');
  });

  it('covers an engine that is not answering', () => {
    expect(statusLine({ mode: 'none', port: 8686, health: null, error: 'x' })).toBe(
      'Engine: not running',
    );
    expect(statusLine({ mode: 'spawned', port: 8686, health: null, error: null })).toBe(
      'Engine: starting',
    );
  });
});
