import { HostRuntime } from './runtime';
import { browserLoader } from './loader';
import { HostConnection, type LaunchIdentity } from './connection';
import type { Role } from './protocol';

export { HostRuntime } from './runtime';
export { GraphicLoader } from './loader';
export { HostConnection } from './connection';
export { ServerClock } from './clock';
export type * from './protocol';

declare global {
  interface Window {
    noplayoutLaunch: (identity: LaunchIdentity) => void;
    v_noplayout: { serverNow: HostRuntime['serverNow'] };
  }
}
const search = new URLSearchParams(location.search);
const roles: Role[] = ['program', 'preview', 'web-program', 'web-fill', 'web-key', 'editor'];
const role = search.get('role') as Role | null;
const channel = search.get('channel');
if (!role || !roles.includes(role) || !channel) throw new Error('Renderer requires channel and role');
document.documentElement.dataset.role = role;
// TODO(M3): fill/key presentation filters and public /out routes; see STATUS.md.
const stage = document.getElementById('layers')!;
const loader = browserLoader(stage);
let runtime: HostRuntime | undefined;
const connection = new HostConnection({ channel, role, origin: location.origin,
  onOpen() {
    runtime?.close();
    runtime = new HostRuntime({ send: (m) => connection.send(m), create: (p) => loader.create(p), now: () => performance.now() });
    runtime.startBeats();
  },
  onMessage(message, roundTripMs) { void runtime?.receive(message, roundTripMs).catch(() => connection.send({ type: 'PageError', text: 'Host action dispatch failed' })); },
  onDisconnect() { runtime?.close(); runtime = undefined; },
});
// CEF invokes this hook with execute_java_script after the page loads. Never read URL
// token/inc fields, and never use the public role string as an authority credential.
window.noplayoutLaunch = (identity) => connection.launch(identity);
window.v_noplayout = { serverNow: () => {
  if (!runtime) throw new Error('Host is disconnected');
  return runtime.serverNow();
} };
let raf = 0;
function frame() { runtime?.animationFrame(); raf = requestAnimationFrame(frame); }
raf = requestAnimationFrame(frame);
window.addEventListener('error', () => connection.send({ type: 'PageError', text: 'Graphic page error' }));
window.addEventListener('unhandledrejection', () => connection.send({ type: 'PageError', text: 'Unhandled graphic promise rejection' }));
window.addEventListener('pagehide', () => { cancelAnimationFrame(raf); runtime?.close(); connection.close(); }, { once: true });
