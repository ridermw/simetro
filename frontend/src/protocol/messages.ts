// frontend/src/protocol/messages.ts
//
// PLAN §6 — Wire-protocol type mirror. These types MUST stay in lock-
// step with `crates/protocol/src/lib.rs`. Cross-language drift here is
// caught two ways:
//
//   1. The Envelope.schema_version check on every inbound message; a
//      mismatch raises a Fault::SchemaMismatch banner and freezes the
//      renderer (PLAN §6, §11.2).
//   2. P2 will codegen these from a shared schema; for now they are
//      hand-mirrored and the matching Rust side has a roundtrip JSON
//      test (see crates/protocol/tests).
//
// Numeric IDs (u32 in Rust) become `number` here — JS safely represents
// integers up to 2^53, well above our piece-count cap of 100_000.

export const SCHEMA_VERSION = 1;

export interface Envelope<T> {
  schema_version: number;
  seq: number;
  payload: T;
}

export interface StaticPayload {
  scene_name: string;
  theme: ThemePayload;
  // Maps numeric handle -> human-readable JSON id (for the Inspector).
  id_map: Record<number, string>;
}

export interface ThemePayload {
  palette: string[];
  background_index: number;
  font: string;
}

export interface SnapshotPayload {
  tick: number;
  nodes: NodeSnapshot[];
  paths: PathSnapshot[];
  movers: MoverSnapshot[];
}

export interface NodeSnapshot {
  id: number;
  pos: [number, number];
  shape: "circle" | "square" | "triangle" | "diamond";
  color: number;
}

export interface PathSnapshot {
  id: number;
  from: number;
  to: number;
  color: number;
}

export interface MoverSnapshot {
  id: number;
  pos: [number, number];
  on_path: number;
  speed: number;
}

export type SimEvent =
  | { tag: "MoverDeparted"; mover: number; from_node: number; path: number }
  | { tag: "MoverArrived"; mover: number; at_node: number; path: number }
  | { tag: "MoverSpeedChange"; mover: number; old: number; new: number }
  | { tag: "NodeHighlighted"; node: number; reason: string }
  | { tag: "PathPulsed"; path: number }
  | { tag: "AgentDecided"; agent_id: number; action: string }
  | { tag: "Tick"; tick: number };

export interface AgentReport {
  agent_id: number;
  tick: number;
  considered: { action: string; confidence: number; chosen: boolean }[];
  rationale: string;
  confidence: number;
}

export type EngineFault =
  | { kind: "LoadError"; field: string; message: string }
  | { kind: "AgentCrashed"; agent_id: number; message: string }
  | { kind: "NumericDrift"; tick: number; mover: number }
  | { kind: "ChannelSaturated"; lag_frames: number }
  | { kind: "SystemPanic"; system: string; message: string }
  | { kind: "SchemaMismatch"; found: number; supported: number };

export type EngineWarning =
  | { kind: "InvalidAction"; agent_id: number; reason: string }
  | { kind: "Behind"; lag_frames: number }
  | { kind: "TickOverBudget"; ms: number }
  | { kind: "AgentLogSlow" };

export type SimMessage =
  | { type: "Static"; payload: StaticPayload }
  | { type: "Snapshot"; payload: SnapshotPayload }
  | { type: "Events"; payload: SimEvent[] }
  | { type: "AgentReport"; payload: AgentReport }
  | { type: "Fault"; payload: EngineFault }
  | { type: "Warning"; payload: EngineWarning };

// Type guard used by transport implementations to enforce §6 on every
// inbound message. A bad envelope is fatal — we do not try to recover.
export function isCurrentSchema(env: Envelope<unknown>): boolean {
  return env.schema_version === SCHEMA_VERSION;
}
