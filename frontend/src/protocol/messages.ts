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
  /** Optional direction hint for the renderer. When set, the renderer
   *  draws an arrowhead at the `to_pos` end. `"bidirectional"` adds a
   *  second arrowhead at the `from_pos` end. Undefined (legacy paths)
   *  draws no arrowheads, preserving the existing transit aesthetic. */
  arrow?: "forward" | "bidirectional";
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
  /** When true, the renderer should draw each node's name from
   *  `node_names` as a label next to the node. Default false to
   *  preserve the un-labeled aesthetic of legacy transit/kinetic
   *  scenes that already convey meaning through shape and color.
   *  SL1 scenes set this true via the synth pass so viewers can
   *  identify each place at a glance. */
  show_node_labels?: boolean;
  // --- scenario_language_v1 (SL1) static metadata (PR 12b) ---
  // All optional; non-SL1 scenes simply omit them.
  /** SL1 places — author-declared locations with role, position,
   *  capacity, storage, and accept/produce contracts. Static metadata
   *  only. Empty/omitted for non-SL1 scenes. */
  sl1_places?: Sl1PlaceView[];
  /** SL1 links — author-declared transport edges between places.
   *  Static metadata only. Empty/omitted for non-SL1 scenes. */
  sl1_links?: Sl1LinkView[];
  sl1_objectives?: Sl1ObjectiveView[];
  sl1_failure_conditions?: Sl1FailureConditionView[];
  sl1_victory_conditions?: Sl1VictoryConditionView[];
  sl1_observability_metrics?: Sl1MetricView[];
  sl1_observability_dashboards?: Sl1DashboardView[];
  sl1_observability_alerts?: Sl1AlertView[];
  sl1_milestones?: Sl1MilestoneView[];
}

/** SL1 place — author-declared location in a scenario_language_v1 scene.
 *  Only fields the frontend uses for rendering are typed here; the
 *  Rust side carries additional metadata that the renderer ignores. */
export interface Sl1PlaceView {
  id: string;
  role: string;
  pos: [number, number];
  /** Optional render hint carried opaquely from the scene JSON. */
  shape?: string;
  /** Optional palette index, carried opaquely. */
  color?: number;
}

/** SL1 link — author-declared transport edge between places. */
export interface Sl1LinkView {
  id: string;
  /** Source place id (matches Sl1PlaceView.id). */
  from: string;
  /** Destination place id (matches Sl1PlaceView.id). */
  to: string;
  /** Direction: "forward" or "bidirectional" — mirrors Rust enum. */
  direction: string;
}

export interface SnapshotPayload {
  tick: number;
  /** Only movers ship per snapshot; nodes and paths live in
   *  `StaticPayload` (immutable for the scene's lifetime). */
  movers: MoverState[];
  // --- scenario_language_v1 (SL1) per-tick runtime state (PR 12b) ---
  // All optional; non-SL1 scenes omit them. Sorted by id server-side.
  sl1_game_outcome?: Sl1GameOutcomeView;
  sl1_game_phase?: string;
  sl1_objective_states?: Sl1ObjectiveRuntimeView[];
  sl1_failure_condition_states?: Sl1FailureConditionRuntimeView[];
  sl1_victory_condition_states?: Sl1VictoryConditionRuntimeView[];
  sl1_dashboard_states?: Sl1DashboardStateView[];
  sl1_alert_states?: Sl1AlertStateView[];
  sl1_metric_states?: Sl1MetricStateView[];
}

// ---------------------------------------------------------------------
//  scenario_language_v1 (SL1) wire-protocol mirror (PR 12b)
//
//  These types must stay in lock-step with `crates/protocol/src/lib.rs`.
//  The Rust source uses `#[serde(tag = "kind", rename_all =
//  "snake_case")]` for tagged enums and `#[serde(rename_all =
//  "snake_case")]` for unit-like enums. Field names match exactly.
//
//  All author-supplied strings (milestone labels, alert ids,
//  objective ids, dashboard ids, outcome reasons, etc.) carry
//  untrusted text. The HUD MUST render them via `textContent` (or
//  equivalent safe API), NEVER via `innerHTML`. SL1 reviewers gate
//  this rule on every PR.
// ---------------------------------------------------------------------

export type Sl1ObjectiveStatusTag = "unknown" | "met" | "breached" | "unsupported";
export type Sl1GameOutcomeState = "in_progress" | "won" | "lost";
export type Sl1GamePhase = "winning" | "losing" | "stabilizing" | "spiraling";
export type Sl1DashboardState = "ok" | "stale" | "no_data";
export type Sl1AlertState = "inactive" | "firing";
export type Sl1MetricState = "ok" | "no_data";
export type Sl1AlertSeverity = "info" | "warning" | "critical";

export type Sl1ObjectiveParamsView =
  | { kind: "keep_fresh"; place: string; thing: string; max_stale_ticks: number }
  | { kind: "complete_jobs_before_deadline"; demand: string; max_missed: number }
  | {
      kind: "maintain_utilization";
      place: string;
      capacity: string;
      min_percent: number;
      max_percent: number;
    }
  | { kind: "unsupported_in_this_pr" };

export interface Sl1ObjectiveView {
  id: string;
  /** Wire field name is `type`; rendered text MUST go through textContent. */
  type: string;
  weight: number;
  params: Sl1ObjectiveParamsView;
}

export interface Sl1ObjectiveRuntimeView {
  objective_id: string;
  status: Sl1ObjectiveStatusTag;
  breach_tick_count: number;
}

export type Sl1FailureConditionParamsView =
  | {
      kind: "stale_target";
      place: string;
      thing: string;
      threshold_ticks: number;
      grace_ticks: number;
    }
  | { kind: "place_state"; place: string; state: string; grace_ticks: number }
  | { kind: "objective_breach_count"; objective_id: string; max_count: number };

export interface Sl1FailureConditionView {
  id: string;
  type: string;
  params: Sl1FailureConditionParamsView;
}

export interface Sl1FailureConditionRuntimeView {
  failure_condition_id: string;
  breach_streak_ticks: number;
  fired_at_tick?: number;
}

export type Sl1VictoryConditionParamsView = { kind: "survive_until"; at_tick: number };

export interface Sl1VictoryConditionView {
  id: string;
  type: string;
  params: Sl1VictoryConditionParamsView;
}

export interface Sl1VictoryConditionRuntimeView {
  victory_condition_id: string;
  met_at_tick?: number;
}

export interface Sl1GameOutcomeView {
  state: Sl1GameOutcomeState;
  /** `Some` only when state == "lost". Author-supplied; safe text. */
  reason?: string;
}

export type Sl1MetricSourceView =
  | { kind: "place_capacity_used_percent"; place: string; capacity: string }
  | { kind: "place_inventory_count"; place: string; thing: string }
  | { kind: "dashboard_freshness"; dashboard: string };

export interface Sl1MetricView {
  id: string;
  source: Sl1MetricSourceView;
}

export interface Sl1MetricStateView {
  metric_id: string;
  state: Sl1MetricState;
  value?: number;
}

export interface Sl1DashboardView {
  id: string;
  type: string;
  depends_on: string[];
  freshness_slo_ticks: number;
}

export interface Sl1DashboardStateView {
  dashboard_id: string;
  state: Sl1DashboardState;
  freshness_ticks?: number;
}

export type Sl1AlertPredicateView =
  | { kind: "gt"; threshold: number }
  | { kind: "lt"; threshold: number }
  | { kind: "out_of_range"; min: number; max: number };

export interface Sl1AlertView {
  id: string;
  metric: string;
  predicate: Sl1AlertPredicateView;
  severity: Sl1AlertSeverity;
}

export interface Sl1AlertStateView {
  alert_id: string;
  state: Sl1AlertState;
  fired_at_tick?: number;
}

export type Sl1MilestoneTriggerView =
  | { type: "pressure_activated"; pressure: string }
  | { type: "pressure_deactivated"; pressure: string }
  | {
      type: "metric_threshold";
      metric: string;
      predicate: "gte" | "lte" | "gt" | "lt";
      value: number;
    }
  | { type: "dashboard_state"; dashboard: string; state: Sl1DashboardState };

export interface Sl1MilestoneView {
  id: string;
  /** Author-supplied label; render via textContent only. */
  label: string;
  trigger_kind: string;
  trigger: Sl1MilestoneTriggerView;
  camera_focus?: string[];
  highlight?: string;
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
  | { kind: "agent_decided"; agent_id: string; action: ActionTag }
  // --- scenario_language_v1 (SL1) events (PR 12b) ---
  | {
      kind: "sl1_pressure_lifecycle";
      pressure_id: string;
      pressure_kind: string;
      event: "started" | "ended";
      tick: number;
    }
  | {
      kind: "sl1_objective_state_changed";
      objective_id: string;
      from: Sl1ObjectiveStatusTag;
      to: Sl1ObjectiveStatusTag;
      tick: number;
    }
  | { kind: "sl1_failure_condition_fired"; failure_condition_id: string; tick: number }
  | { kind: "sl1_victory_condition_met"; victory_condition_id: string; tick: number }
  | {
      kind: "sl1_game_outcome_changed";
      from: Sl1GameOutcomeState;
      to: Sl1GameOutcomeState;
      tick: number;
      reason?: string;
    }
  | {
      kind: "sl1_dashboard_state_changed";
      dashboard_id: string;
      from: Sl1DashboardState;
      to: Sl1DashboardState;
      tick: number;
    }
  | {
      kind: "sl1_alert_fired";
      alert_id: string;
      metric_id: string;
      value: number;
      severity: string;
      predicate: string;
      tick: number;
    }
  | {
      kind: "sl1_alert_cleared";
      alert_id: string;
      metric_id: string;
      tick: number;
    }
  | {
      kind: "sl1_agent_action_applied";
      agent_id: string;
      action_kind: string;
      target_id: string;
      cost: number;
      tick: number;
    }
  | {
      kind: "sl1_agent_action_rejected";
      agent_id: string;
      action_kind: string;
      target_id?: string;
      reason: string;
      tick: number;
    }
  | {
      kind: "sl1_agent_llm_disabled";
      agent_id: string;
      tick: number;
    }
  | {
      kind: "sl1_milestone_fired";
      milestone_id: string;
      label: string;
      trigger_kind: string;
      camera_focus?: string[];
      highlight?: string;
      tick: number;
    };

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
