// The engine's health document (noplayout-core health.rs) and the M0 status line built from it.

export type ChildState = 'starting' | 'ok' | 'missing' | 'failed' | 'exited';

export interface ChildHealth {
  state: ChildState;
  pid: number | null;
  version: string | null;
  cef_version: string | null;
  chromium_version: string | null;
  paints: number | null;
  detail: string | null;
}

export interface Health {
  engine: { state: string; version: string; pid: number; uptime_ms: number };
  renderer: ChildHealth;
  output: ChildHealth;
}

/** What the app shell reports (noplayout-app `engine_status`). */
export interface EngineStatus {
  /** `spawned`: this app started the engine; `attached`: it was already running. */
  mode: 'spawned' | 'attached' | 'none';
  port: number;
  health: Health | null;
  error: string | null;
}

const childWords: Record<ChildState, string> = {
  starting: 'starting',
  ok: 'OK',
  missing: 'not found',
  failed: 'failed',
  exited: 'stopped',
};

/** "Renderer: CEF 156.0.31 OK", "Renderer: starting", "Renderer: failed". */
export function rendererLine(r: ChildHealth): string {
  const word = childWords[r.state];
  if (r.state === 'ok' && r.cef_version) {
    return `Renderer: CEF ${r.cef_version} ${word}`;
  }
  return `Renderer: ${word}`;
}

/** The M0 status line: "Engine: running · Renderer: CEF 156.0.31 OK". */
export function statusLine(s: EngineStatus): string {
  if (!s.health) {
    return s.mode === 'none' ? 'Engine: not running' : 'Engine: starting';
  }
  return `Engine: ${s.health.engine.state} · ${rendererLine(s.health.renderer)}`;
}
