import type { HostDown, HostUp, Role } from './protocol';
import { parseHostMessage } from './validation';

export interface LaunchIdentity { inc: number; token: string }
interface Socket {
  readonly readyState: number;
  onopen: (() => void) | null;
  onmessage: ((event: { data: string }) => void) | null;
  onclose: (() => void) | null;
  send(value: string): void; close(): void;
}
interface ConnectionOptions {
  channel: string; role: Role; origin: string;
  connect?: (url: string) => Socket;
  now?: () => number;
  onOpen?: () => void; onMessage?: (m: HostDown, roundTripMs?: number) => void; onDisconnect?: () => void;
}
/** Page transport only. A role never establishes authority; the engine checks Register. */
export class HostConnection {
  private socket?: Socket;
  private launched = false;
  private closed = false;
  private retry?: ReturnType<typeof setTimeout>;
  constructor(private readonly options: ConnectionOptions) {
    if (options.role !== 'program') this.open();
  }
  launch(identity: LaunchIdentity): void {
    if (this.options.role !== 'program' || this.launched || this.closed) throw new Error('Launch identity already consumed or role is not program');
    if (!Number.isSafeInteger(identity.inc) || identity.inc < 0 || !identity.token) throw new Error('Invalid launch identity');
    this.launched = true;
    this.open({ inc: identity.inc, token: identity.token });
  }
  send(message: HostUp): void {
    if (!this.closed && this.socket?.readyState === 1) this.socket.send(JSON.stringify(message));
  }
  close(): void {
    this.closed = true; clearTimeout(this.retry); this.socket?.close();
  }
  private open(identity?: LaunchIdentity): void {
    const options = this.options;
    const url = new URL(`/ws/host/${encodeURIComponent(options.channel)}`, options.origin);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    const socket = (options.connect ?? ((url) => new WebSocket(url) as unknown as Socket))(url.href);
    this.socket = socket;
    const now = options.now ?? (() => performance.now());
    let registeredAt: number | undefined;
    socket.onopen = () => {
      registeredAt = now();
      // The identity is not taken from location/search/hash, logged or retained after Register.
      this.send({ type: 'Register', channel: options.channel, role: options.role, hostVersion: '0.0.1', ...identity });
      identity = undefined;
      options.onOpen?.();
    };
    socket.onmessage = ({ data }) => {
      try {
        const message = parseHostMessage(data);
        const roundTripMs = message.type === 'Snapshot' && registeredAt !== undefined ? Math.max(0, now() - registeredAt) : undefined;
        if (message.type === 'Snapshot') registeredAt = undefined;
        options.onMessage?.(message, roundTripMs);
      } catch { this.send({ type: 'PageError', text: 'Invalid host protocol message' }); }
    };
    socket.onclose = () => {
      options.onDisconnect?.();
      // A CEF program page must be reloaded/re-launched by its supervisor, not downgrade
      // into an uncredentialled mirror. Mirrors rebuild from a fresh Snapshot on reconnect.
      if (!this.closed && options.role !== 'program') this.retry = setTimeout(() => this.open(), 1000);
    };
  }
}
