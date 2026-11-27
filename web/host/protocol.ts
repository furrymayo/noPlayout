import type { RenderCharacteristics } from 'ograf';

// PLAN §4.3. `type` discriminates JSON messages; role is never an authority credential.
export type Role = 'program' | 'preview' | 'web-program' | 'web-fill' | 'web-key' | 'editor';
export interface Timestamp {
  utcUs: number;
  mono: { run: string; us: number };
  slot?: { k: number; epoch: string };
}
export type Method = 'load' | 'playAction' | 'stopAction' | 'updateAction' | 'customAction' | 'dispose';
export type Params = Record<string, unknown>;
export interface JournalEntry {
  seq: number; ts: Timestamp; method: Method; params: Params; quarantined?: boolean; replay?: true;
}
export interface Action extends JournalEntry {
  type: 'Action'; revision: number; instanceId: string; generation: number;
}
export interface Checkpoint {
  state: unknown; frontierSeq: number; cutTs: Timestamp;
}
export interface SnapshotLayer {
  layer: number; instanceId: string; generation: number; graphicUrl: string;
  dataAtLoad: Params; renderCharacteristics: RenderCharacteristics; loadTs: Timestamp;
  journal: JournalEntry[]; checkpoint?: Checkpoint;
}
export interface Snapshot {
  type: 'Snapshot'; revision: number; lastSeq: number; serverNow: Timestamp;
  layers: SnapshotLayer[]; cued?: SnapshotLayer; flags: unknown[];
}
export type HostDown = Snapshot | Action | { type: 'TimeSync'; serverNow: Timestamp }
  | { type: 'Checkpoint'; instanceId: string; generation: number };
export type HostUp =
  | { type: 'Register'; channel: string; role: Role; hostVersion: string; lastSeq?: number; inc?: number; token?: string }
  | { type: 'Result'; seq: number; instanceId: string; generation: number; statusCode: number; statusMessage?: string; currentStep?: number; payload?: unknown }
  | { type: 'Beat'; n: number; rafCount: number; lagMs: number; lastAppliedSeq: number }
  | { type: 'Synced'; seq: number }
  | { type: 'PageError'; text: string }
  | { type: 'CheckpointResult'; instanceId: string; generation: number; frontierSeq: number; cutTs: Timestamp; state: unknown }
  | { type: 'CheckpointDeferred'; instanceId: string };
export interface LoadParams extends Params {
  graphicUrl: string; layer: number; data: Params; renderCharacteristics: RenderCharacteristics; cued?: boolean;
}
