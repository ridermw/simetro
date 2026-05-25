// frontend/src/protocol/messages.ts
//
// wire-protocol contract — Wire-protocol type mirror. These types MUST stay in lock-
// step with `crates/protocol/src/lib.rs`. The Rust source uses
// `#[serde(tag = "kind", content = "payload", rename_all =
// "snake_case")]` on `SimMessage` and `AgentMessage`, and
// `#[serde(tag = "kind", rename_all = "snake_case")]` (flattened) on
// `SimEvent`, `Action`, `FaultPayload`, `WarningPayload`. The shapes
// here mirror that exactly — variants and enum-like fields are
// snake_case strings, struct field names match Rust field names.
//
// Drift is caught two ways:
//   1. The Envelope.schema_version check on every inbound message; a
//      mismatch raises a Fault::SchemaMismatch banner and freezes the
//      renderer (protocol compatibility).
//   2. These are hand-mirrored today. The matching Rust side has
//      roundtrip JSON tests (see crates/protocol/src/lib.rs).
//
// Numeric IDs (u32 in Rust) become `number` here — JS safely represents
// integers up to 2^53, well above our piece-count cap of 100_000.

export const SCHEMA_VERSION = 1;

export interface Envelope<T> {
  schema_version: number;
  seq: number;
  payload: T;
}

// ---------------------------------------------------------------------
//  Engine  →  consumers
// ---------------------------------------------------------------------

export type NodeShapeTag = "circle" | "square" | "triangle" | "diamond" | "hexagon";

export interface NodeView {
  id: number;
  pos: [number, number];
  shape: NodeShapeTag;
  /** Palette index. */
  color: number;
}

export interface PathView {
  id: number;
  /** Endpoints baked into the view so the renderer can group all
   *  paths of one color into a single `Path2D` (renderer batching target). */
  from_pos: [number, number];
  to_pos: [number, number];
  color: number;
}

export interface MoverState {
  id: number;
  pos: [number, number];
  speed: number;
  /** Path id the mover is currently on; 0 when waiting at a node. */
  on_path: number;
}

export interface StaticPayload {
  name: string;
  palette: string[];
  background_index: number;
  nodes: NodeView[];
  paths: PathView[];
  /** Numeric id → JSON id, segregated by section so two pieces in
   *  different sections may share a JSON id without colliding. */
  node_names: Record<number, string>;
  path_names: Record<number, string>;
  mover_names: Record<number, string>;
}

export interface SnapshotPayload {
  tick: number;
  /** Only movers ship per snapshot; nodes and paths live in
   *  `StaticPayload` (immutable for the scene's lifetime). */
  movers: MoverState[];
}

// ---------------------------------------------------------------------
//  Semantic events (event protocol contract). Flat tagged union: `kind` + fields.
// ---------------------------------------------------------------------

export type HighlightReason = "agent_focus" | "bottleneck" | "goal_reached";

export type ActionTag = "no_op" | "set_speed" | "place_piece" | "connect_pieces" | "remove_piece";

export type SimEvent =
  | { kind: "tick"; tick: number }
  | { kind: "mover_departed"; mover: number; from_node: number; path: number }
  | { kind: "mover_arrived"; mover: number; at_node: number; path: number }
  | { kind: "mover_speed_change"; mover: number; old: number; new: number }
  | { kind: "node_highlighted"; node: number; reason: HighlightReason }
  | { kind: "path_pulsed"; path: number }
  | { kind: "agent_decided"; agent_id: string; action: ActionTag };

export type SimEventKind = SimEvent["kind"];

// ---------------------------------------------------------------------
//  Actions (engine ⇆ agent)
// ---------------------------------------------------------------------

export type Action =
  | { kind: "no_op" }
  | { kind: "set_speed"; mover: number; speed: number }
  | { kind: "place_piece"; piece_kind: string; pos: [number, number] }
  | { kind: "connect_pieces"; from: number; to: number }
  | { kind: "remove_piece"; id: number };

export interface ConsideredAction {
  action: Action;
  confidence: number;
}

export interface AgentReport {
  tick: number;
  agent_id: string;
  considered: ConsideredAction[];
  chosen: Action | null;
  rationale: string;
  confidence: number;
}

// ---------------------------------------------------------------------
//  Faults & warnings
// ---------------------------------------------------------------------

export type FaultPayload =
  | { kind: "load_error"; message: string; line: number | null; col: number | null }
  | { kind: "agent_crashed"; agent_id: string; message: string }
  | { kind: "numeric_drift"; tick: number }
  | { kind: "engine_fault"; message: string }
  | { kind: "baseline_hash_mismatch"; expected: string; found: string }
  | { kind: "schema_mismatch"; expected: number; found: number }
  | { kind: "transport_lost" };

export type WarningPayload =
  | { kind: "invalid_action"; agent_id: string; reason: string }
  // `agent_id` is set when the engine attributes lag to a specific
  // agent (e.g. a live LLM bridge that missed its reply deadline).
  // Omitted for engine-wide pacing issues. Optional + back-compat
  // with v1 payloads that never carried this field.
  | { kind: "behind"; lag_frames: number; agent_id?: string }
  | { kind: "tick_over_budget"; ms: number }
  | { kind: "agent_log_slow" };

// ---------------------------------------------------------------------
//  Top-level message tagged with kind + payload
// ---------------------------------------------------------------------

export type SimMessage =
  | { kind: "static"; payload: StaticPayload }
  | { kind: "snapshot"; payload: SnapshotPayload }
  | { kind: "events"; payload: SimEvent[] }
  | { kind: "agent_report"; payload: AgentReport }
  | { kind: "fault"; payload: FaultPayload }
  | { kind: "warning"; payload: WarningPayload };

// ---------------------------------------------------------------------
//  Agent  →  engine (future)
// ---------------------------------------------------------------------

export type AgentMessage =
  | { kind: "connect"; payload: { agent_id: string; capabilities: string[] } }
  | { kind: "action"; payload: Action }
  | { kind: "heartbeat"; payload: null }
  | { kind: "disconnect"; payload: { reason: string } };

// ---------------------------------------------------------------------
//  Helpers
// ---------------------------------------------------------------------

/** Type guard used by transport implementations to enforce schema compatibility on every
 *  inbound message. A bad envelope is fatal — we do not try to recover. */
export function isCurrentSchema(env: Envelope<unknown>): boolean {
  return env.schema_version === SCHEMA_VERSION;
}

/** Render an Action as a human-readable one-liner. Used by the
 *  Inspector; safe for textContent. */
export function formatAction(a: Action | null): string {
  if (a === null) return "(none)";
  switch (a.kind) {
    case "no_op":
      return "NoOp";
    case "set_speed":
      return `SetSpeed(mover=${a.mover}, speed=${a.speed.toFixed(2)})`;
    case "place_piece":
      return `PlacePiece(${a.piece_kind}, [${a.pos[0]}, ${a.pos[1]}])`;
    case "connect_pieces":
      return `ConnectPieces(${a.from} → ${a.to})`;
    case "remove_piece":
      return `RemovePiece(${a.id})`;
  }
}

/** Render an ActionTag (the snake_case discriminant carried by
 *  AgentDecided events) as a display label. */
export function formatActionTag(t: ActionTag): string {
  switch (t) {
    case "no_op":
      return "NoOp";
    case "set_speed":
      return "SetSpeed";
    case "place_piece":
      return "PlacePiece";
    case "connect_pieces":
      return "ConnectPieces";
    case "remove_piece":
      return "RemovePiece";
  }
}
