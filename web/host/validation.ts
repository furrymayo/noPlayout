import type { HostDown } from './protocol';

const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const integer = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;
const methods = ['load', 'dispose', 'playAction', 'stopAction', 'updateAction', 'customAction'];
function timestamp(v: unknown): boolean {
  return object(v) && Number.isSafeInteger(v.utcUs) && object(v.mono) && typeof v.mono.run === 'string' && integer(v.mono.us)
    && (v.slot === undefined || (object(v.slot) && integer(v.slot.k) && typeof v.slot.epoch === 'string'));
}
function identity(v: Record<string, unknown>): boolean {
  return typeof v.instanceId === 'string' && !!v.instanceId && integer(v.generation);
}
function format(v: unknown): boolean {
  return object(v) && typeof v.frameRate === 'number' && Number.isFinite(v.frameRate) && v.frameRate > 0
    && object(v.resolution) && integer(v.resolution.width) && v.resolution.width > 0 && integer(v.resolution.height) && v.resolution.height > 0;
}
function load(v: unknown): boolean {
  return object(v) && typeof v.graphicUrl === 'string' && !!v.graphicUrl && integer(v.layer) && v.layer >= 1
    && object(v.data) && format(v.renderCharacteristics) && (v.cued === undefined || typeof v.cued === 'boolean');
}
function entry(v: unknown): boolean {
  return object(v) && integer(v.seq) && v.seq > 0 && timestamp(v.ts) && methods.includes(String(v.method)) && object(v.params)
    && (v.method !== 'load' || load(v.params)) && (v.quarantined === undefined || typeof v.quarantined === 'boolean');
}
function layer(v: unknown): boolean {
  return object(v) && identity(v) && integer(v.layer) && v.layer >= 1 && typeof v.graphicUrl === 'string'
    && object(v.dataAtLoad) && format(v.renderCharacteristics) && timestamp(v.loadTs)
    && Array.isArray(v.journal) && v.journal.every(entry)
    && (v.checkpoint === undefined || (object(v.checkpoint) && 'state' in v.checkpoint && integer(v.checkpoint.frontierSeq) && timestamp(v.checkpoint.cutTs)));
}
/** Fail closed on malformed control messages before any dynamic method or import runs. */
export function parseHostMessage(json: string): HostDown {
  const v: unknown = JSON.parse(json);
  if (!object(v)) throw new Error('Expected host message');
  const valid = v.type === 'TimeSync' ? timestamp(v.serverNow)
    : v.type === 'Checkpoint' ? identity(v)
    : v.type === 'Action' ? integer(v.revision) && identity(v) && entry(v)
    : v.type === 'Snapshot' ? integer(v.revision) && integer(v.lastSeq) && timestamp(v.serverNow)
      && Array.isArray(v.layers) && v.layers.every(layer) && (v.cued === undefined || layer(v.cued)) && Array.isArray(v.flags)
    : false;
  if (!valid) throw new Error('Invalid host protocol message');
  return v as unknown as HostDown;
}
