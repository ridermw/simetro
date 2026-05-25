//! # simetro-protocol
//!
//! Versioned wire types shared between the engine, the agent-bridge, the
//! frontend, and any external consumers.
//!
//! ```text
//!   ┌──────────┐  Envelope<SimMessage>   ┌────────────┐
//!   │  engine  │ ───────────────────────▶│  consumer  │
//!   │          │ ◀───────────────────────│ (frontend, │
//!   └──────────┘  Envelope<AgentMessage> │  bridge,   │
//!                                        │  replay)   │
//!                                        └────────────┘
//!
//!   Every envelope carries `schema_version: u32` (Issue 4A).
//!   Receivers MUST reject on mismatch; never silently process.
//! ```
//!
//! # Versioning
//!
//! [`SCHEMA_VERSION`] is the current wire protocol version. When a v2
//! lands, add a migrator in [`version`] and route it before deserializing
//! the typed payload.

use serde::{Deserialize, Serialize};

pub mod capabilities;
pub mod decision_timeline;
pub mod version;
pub mod websocket;

pub use decision_timeline::{
    DecisionEntry, DecisionResponse, DecisionStatus, DecisionTimeline, DecisionTimelineError,
    DecisionTimelineSnapshot, FromJsonError as DecisionTimelineFromJsonError, RawResponseRef,
    SchemaVersionMismatch as DecisionTimelineSchemaMismatch, TimelineId,
    DECISION_TIMELINE_SCHEMA_VERSION, DEFAULT_TIMELINE_CAPACITY,
};
pub use version::SCHEMA_VERSION;

/// Wrapping envelope for every wire message. Consumers MUST check
/// `schema_version == SCHEMA_VERSION` before processing `payload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub schema_version: u32,
    pub seq: u64,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(seq: u64, payload: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            seq,
            payload,
        }
    }

    /// Returns `true` iff this envelope's schema version matches the one
    /// this build was compiled against.
    pub fn is_compatible(&self) -> bool {
        self.schema_version == SCHEMA_VERSION
    }
}

// =====================================================================
//                          Sim  →  consumer
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum SimMessage {
    /// Sent once on connect: theme, ID map, palette, anything constant.
    Static(StaticPayload),
    /// Periodic positional/state delta. Frontend interpolates between snapshots.
    Snapshot(SnapshotPayload),
    /// Semantic events for the tick. Frontend animates from these.
    Events(Vec<SimEvent>),
    /// Result of an agent's `act()` call (rationale, considered, chosen).
    AgentReport(AgentReport),
    /// Sim-fatal condition. Engine has paused.
    Fault(FaultPayload),
    /// Non-fatal degradation. Sim continues.
    Warning(WarningPayload),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StaticPayload {
    pub name: String,
    pub palette: Vec<String>,
    pub background_index: u8,
    /// All nodes in the scene, pre-flattened for the renderer.
    pub nodes: Vec<NodeView>,
    /// All paths in the scene with endpoints baked to positions so the
    /// renderer can group by `color` into one `Path2D` per color and
    /// hit the renderer batching target (~6 draw calls per scene).
    pub paths: Vec<PathView>,
    /// JSON string id → numeric id, segregated by section so two kinds
    /// can share an id space without collision (identifier interning contract).
    pub node_names: std::collections::BTreeMap<u32, String>,
    pub path_names: std::collections::BTreeMap<u32, String>,
    pub mover_names: std::collections::BTreeMap<u32, String>,
    /// `scenario_language_v1` places — author-declared locations with
    /// capacity, storage, accepted/produced thing tags, failure
    /// domains, and an operating-state map. Static metadata only;
    /// per-tick utilization (if/when it lands) goes in [`SnapshotPayload`].
    /// Empty for non-SL1 scenes and for SL1 scenes with no `places`.
    /// Sorted by `id` for deterministic ordering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sl1_places: Vec<Sl1PlaceView>,
    /// `scenario_language_v1` links — author-declared transport edges
    /// between places. Static metadata only; per-tick queue
    /// utilization (PR 4/5) goes in [`SnapshotPayload`]. Empty for
    /// non-SL1 scenes and for SL1 scenes with no `links`. Sorted by
    /// `id` for deterministic ordering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sl1_links: Vec<Sl1LinkView>,
    /// `scenario_language_v1` things — author-declared typed payloads
    /// (jobs, datasets, telemetry, etc.) that flow through places and
    /// links. Static metadata only; per-tick inventory counts and
    /// freshness states go in [`SnapshotPayload::sl1_place_inventories`].
    /// Empty for non-SL1 scenes and for SL1 scenes with no `things`.
    /// Sorted by `id` for deterministic ordering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sl1_things: Vec<Sl1ThingView>,
    /// `scenario_language_v1` transforms — author-declared deterministic
    /// work rules that consume typed inputs, reserve typed capacity, run
    /// for a duration, and produce typed outputs (PR 4). Static metadata
    /// only; per-tick state machine + capacity utilization go in
    /// [`SnapshotPayload::sl1_transform_states`]. Empty for non-SL1
    /// scenes and for SL1 scenes with no `transforms`. Sorted by `id`
    /// for deterministic ordering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sl1_transforms: Vec<Sl1TransformView>,
    /// SL1 demand definitions for this scene. PR 5. Static
    /// metadata only; per-tick outstanding/fulfilled/dropped counts
    /// go in [`SnapshotPayload::sl1_demand_states`]. Empty for non-SL1
    /// scenes and for SL1 scenes with no `demand`. Sorted by `id`
    /// for deterministic ordering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sl1_demand: Vec<Sl1DemandView>,
}

/// Wire-level view of one validated SL1 place. Mirrors
/// `engine::scenario_language_v1::Sl1Place` 1:1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sl1PlaceView {
    pub id: String,
    pub role: String,
    pub pos: [f32; 2],
    /// Optional render hint carried opaquely from the scene JSON. PR 6
    /// is the first frontend consumer; intermediate consumers must
    /// tolerate `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// Optional palette index, carried opaquely. PR 6's renderer is
    /// responsible for range-checking against `theme.palette`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    pub capacity: std::collections::BTreeMap<String, u64>,
    pub storage: std::collections::BTreeMap<String, Sl1StorageSlotView>,
    pub accepts: Vec<String>,
    pub produces: Vec<String>,
    pub failure_domains: Vec<String>,
    pub operating_states: std::collections::BTreeMap<String, Sl1OperatingStateView>,
}

/// Wire-level storage slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1StorageSlotView {
    pub capacity: u64,
    pub initial: u64,
}

/// Wire-level operating-state entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1OperatingStateView {
    pub predicate: Sl1OperatingPredicateView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_ticks: Option<u64>,
}

/// Wire-level operating-state predicate. Tagged so the TS side can
/// pattern-match without ambiguity. Future predicate kinds are added
/// in their respective PRs (Things → InventoryGte, Observability →
/// MetricGte).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Sl1OperatingPredicateView {
    UsedPercentGte { metric: String, threshold: u8 },
    OverloadedTicksGt { ticks: u64 },
}

/// Wire-level view of one validated SL1 link. Mirrors
/// `engine::scenario_language_v1::Sl1Link` 1:1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1LinkView {
    pub id: String,
    #[serde(rename = "type")]
    pub link_type: String,
    pub from: String,
    pub to: String,
    pub direction: Sl1LinkDirectionView,
    pub capacity: std::collections::BTreeMap<String, u64>,
    pub travel_ticks: u64,
    pub compatibility: Vec<String>,
    pub queue_capacity: u64,
    pub backpressure: Sl1LinkBackpressureView,
    /// Optional render hint carried opaquely from the scene JSON. PR 6
    /// is the first frontend consumer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render: Option<Sl1LinkRenderHintView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sl1LinkDirectionView {
    Forward,
    Bidirectional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sl1LinkBackpressureView {
    BlockUpstream,
    DropLowPriority,
    SpillToBuffer,
    DegradeQuality,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1LinkRenderHintView {
    pub style: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
}

/// Wire-level view of one validated SL1 thing. Mirrors
/// `engine::scenario_language_v1::Sl1Thing` 1:1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sl1ThingView {
    pub id: String,
    pub kind: String,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_budget_ticks: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_contract: Option<Sl1ThingQualityContractView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render: Option<Sl1ThingRenderHintView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sl1ThingQualityContractView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_drop_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_late_ticks: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1ThingRenderHintView {
    pub glyph: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
}

/// Wire-level freshness state for a (place, thing) inventory slot.
/// Mirrors `engine::scenario_language_v1::FreshnessState`. All five
/// variants are defined now even though PR 3 only reaches the first
/// three (`Degraded`/`Invalid` arrive with PR 8 quality contracts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FreshnessStateView {
    NoData,
    Ok { last_set_tick: u64 },
    Stale { last_set_tick: u64 },
    Degraded,
    Invalid,
}

/// Wire-level snapshot of one inventory slot. PR 3 emits one entry
/// per declared `storage[thing_id]` slot on every snapshot, even when
/// `count == 0`, so the frontend can render an empty slot rather than
/// silently dropping it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1PlaceInventoryView {
    pub place_id: String,
    pub thing_id: String,
    pub count: u64,
    pub freshness: FreshnessStateView,
}

/// Wire-level view of one validated SL1 transform. Mirrors
/// `engine::scenario_language_v1::Sl1Transform` 1:1. PR 4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1TransformView {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub runs_on: String,
    pub inputs: Vec<Sl1TransformIoView>,
    pub outputs: Vec<Sl1TransformIoView>,
    pub cadence_ticks: u64,
    pub duration_ticks: u64,
    pub deadline_ticks: u64,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub capacity_cost: std::collections::BTreeMap<String, u64>,
    pub failure_policy: Sl1FailurePolicyView,
    pub max_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1TransformIoView {
    pub thing_id: String,
    pub amount: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sl1FailurePolicyView {
    RetryThenWarn,
    Drop,
}

/// Wire-level snapshot of one transform's runtime state for a tick.
/// Mirrors `engine::scenario_language_v1::Sl1TransformState`. Emitted
/// once per declared transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Sl1TransformStateView {
    Idle,
    Running {
        scheduled_at: u64,
        started_at: u64,
        attempt: u32,
    },
    Starved {
        scheduled_at: u64,
        since: u64,
        attempts: u32,
    },
    Blocked {
        scheduled_at: u64,
        since: u64,
        attempts: u32,
    },
    Late {
        scheduled_at: u64,
        attempt: u32,
        since: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1TransformRuntimeView {
    pub transform_id: String,
    #[serde(flatten)]
    pub state: Sl1TransformStateView,
}

/// Wire-level view of one validated SL1 demand definition. Mirrors
/// `engine::scenario_language_v1::Sl1Demand` 1:1. PR 5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1DemandView {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub target: Sl1DemandTargetView,
    pub requires: Vec<String>,
    pub spawn_schedule: Sl1DemandScheduleView,
    pub deadline_ticks: u64,
    pub priority: Sl1DemandPriorityView,
    pub value: u64,
    pub penalty: Sl1DemandPenaltyView,
}

/// Wire-level demand target. PR 5 supports `place` only; future PRs
/// add other variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Sl1DemandTargetView {
    Place { id: String },
}

/// Wire-level demand schedule. PR 5 supports `fixed` and `scripted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Sl1DemandScheduleView {
    Fixed { every_ticks: u64, start_tick: u64 },
    Scripted { ticks: Vec<u64> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sl1DemandPriorityView {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1DemandPenaltyView {
    pub score: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Wire-level snapshot of one demand's runtime state for a tick. PR 5.
/// `outstanding` = current Pending instances. `fulfilled_count` and
/// `dropped_count` are cumulative monotonic counters since scene
/// start. `next_sequence` is the next sequence number to assign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sl1DemandRuntimeView {
    pub demand_id: String,
    pub outstanding: u32,
    pub fulfilled_count: u64,
    pub dropped_count: u64,
    pub next_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeView {
    pub id: u32,
    pub pos: [f32; 2],
    pub shape: NodeShapeTag,
    /// Palette index.
    pub color: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathView {
    pub id: u32,
    pub from_pos: [f32; 2],
    pub to_pos: [f32; 2],
    /// Palette index. Renderer groups by this to one `Path2D` per color.
    pub color: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeShapeTag {
    Circle,
    Square,
    Triangle,
    Diamond,
    Hexagon,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotPayload {
    pub tick: u64,
    pub movers: Vec<MoverState>,
    /// SL1 per-(place, thing) inventory state for this tick. Empty for
    /// non-SL1 scenes and for SL1 scenes with no `places[].storage[]`
    /// slots. Sorted by `(place_id, thing_id)` for deterministic
    /// ordering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sl1_place_inventories: Vec<Sl1PlaceInventoryView>,
    /// SL1 per-transform runtime state for this tick. Emitted once
    /// per declared transform (sorted by `transform_id`). Empty for
    /// non-SL1 scenes and for SL1 scenes with no `transforms`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sl1_transform_states: Vec<Sl1TransformRuntimeView>,
    /// SL1 per-demand runtime state for this tick (PR 5). Emitted once
    /// per declared demand (sorted by `demand_id`). Empty for non-SL1
    /// scenes and for SL1 scenes with no `demand`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sl1_demand_states: Vec<Sl1DemandRuntimeView>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MoverState {
    pub id: u32,
    pub pos: [f32; 2],
    pub speed: f32,
    pub on_path: u32,
}

// ---------------------------------------------------------------------
//  Semantic events (event protocol contract). Renamed from `Event` per Issue 8A to
//  avoid clash with DOM/Tauri/channel `Event` types.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SimEvent {
    Tick {
        tick: u64,
    },
    MoverDeparted {
        mover: u32,
        from_node: u32,
        path: u32,
    },
    MoverArrived {
        mover: u32,
        at_node: u32,
        path: u32,
    },
    MoverSpeedChange {
        mover: u32,
        old: f32,
        new: f32,
    },
    NodeHighlighted {
        node: u32,
        reason: HighlightReason,
    },
    PathPulsed {
        path: u32,
    },
    AgentDecided {
        /// Stable agent identifier as registered with the engine
        /// (matches [`AgentReport::agent_id`]). Was `u32` in v0; now a
        /// `String` so multi-agent runs can distinguish decisions and
        /// downstream consumers can correlate with [`AgentReport`].
        agent_id: String,
        action: ActionTag,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HighlightReason {
    AgentFocus,
    Bottleneck,
    GoalReached,
}

/// Lightweight discriminant of an [`Action`] for inclusion in [`SimEvent`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionTag {
    NoOp,
    SetSpeed,
    PlacePiece,
    ConnectPieces,
    RemovePiece,
    DefineResource,
    AddProducer,
    AddConsumer,
    SetGoal,
}

impl Action {
    pub fn tag(&self) -> ActionTag {
        match self {
            Action::NoOp => ActionTag::NoOp,
            Action::SetSpeed { .. } => ActionTag::SetSpeed,
            Action::PlacePiece { .. } => ActionTag::PlacePiece,
            Action::ConnectPieces { .. } => ActionTag::ConnectPieces,
            Action::RemovePiece { .. } => ActionTag::RemovePiece,
            Action::DefineResource { .. } => ActionTag::DefineResource,
            Action::AddProducer { .. } => ActionTag::AddProducer,
            Action::AddConsumer { .. } => ActionTag::AddConsumer,
            Action::SetGoal { .. } => ActionTag::SetGoal,
        }
    }
}

// ---------------------------------------------------------------------
//  AgentReport — surfaces in the Inspector panel.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentReport {
    pub tick: u64,
    pub agent_id: String,
    pub considered: Vec<ConsideredAction>,
    pub chosen: Option<Action>,
    pub rationale: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsideredAction {
    pub action: Action,
    pub confidence: f32,
}

// ---------------------------------------------------------------------
//  Faults & warnings.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FaultPayload {
    LoadError {
        message: String,
        line: Option<u32>,
        col: Option<u32>,
    },
    AgentCrashed {
        agent_id: String,
        message: String,
    },
    NumericDrift {
        tick: u64,
    },
    EngineFault {
        message: String,
    },
    BaselineHashMismatch {
        expected: String,
        found: String,
    },
    SchemaMismatch {
        expected: u32,
        found: u32,
    },
    TransportLost,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WarningPayload {
    InvalidAction {
        agent_id: String,
        reason: String,
    },
    /// Engine fell behind real-time by `lag_frames` ticks.
    /// `agent_id` is set when the lag is attributable to a specific
    /// agent (e.g. a live LLM bridge that missed its reply deadline);
    /// `None` for engine-wide pacing issues.
    Behind {
        lag_frames: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
    },
    TickOverBudget {
        ms: f32,
    },
    AgentLogSlow,
    /// SL1 transform observability warning (PR 4). Emitted once per
    /// state-entry / significant transition. `event` is the canonical
    /// transition (`starved`, `blocked`, `late`, `failed`,
    /// `slot_missed`); details (attempt, scheduled_at, since) are
    /// available via the snapshot's `sl1_transform_states`.
    Sl1Transform {
        transform_id: String,
        event: Sl1TransformWarningKind,
        tick: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attempt: Option<u32>,
    },
    /// SL1 demand observability warning (PR 5). `sequence` identifies
    /// the Dropped instance; `BacklogOverflow` leaves it `None`.
    /// `value`, `penalty_score`, and `penalty_warning` are surfaced on
    /// Dropped so PR 8 can wire score arithmetic and severity routing
    /// without a protocol change. `penalty_warning` is the
    /// author-supplied severity tag from `demand[].penalty.warning`.
    Sl1Demand {
        demand_id: String,
        event: Sl1DemandWarningKind,
        tick: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        penalty_score: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        penalty_warning: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Sl1TransformWarningKind {
    Starved,
    Blocked,
    Late,
    Failed,
    SlotMissed,
}

/// SL1 demand observability warning (PR 5). Emitted once per
/// terminal demand transition (Dropped) and once per backlog
/// overflow entry. `sequence` identifies the demand instance for
/// Dropped events; backlog overflow leaves it `None`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Sl1DemandWarningKind {
    Dropped,
    BacklogOverflow,
}

// =====================================================================
//                          Agent  →  engine
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum AgentMessage {
    Connect {
        agent_id: String,
        capabilities: Vec<String>,
    },
    Action(Action),
    Heartbeat,
    Disconnect {
        reason: String,
    },
}

/// Actions an agent may take. Author actions (PlacePiece/ConnectPieces/
/// RemovePiece, plus the resource/production tools
/// DefineResource/AddProducer/AddConsumer/SetGoal) mutate the world
/// when valid; malformed or unsafe requests are rejected with a typed
/// [`WarningPayload::InvalidAction`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    NoOp,
    SetSpeed {
        mover: u32,
        speed: f32,
    },
    PlacePiece {
        piece_kind: String,
        pos: [f32; 2],
    },
    ConnectPieces {
        from: u32,
        to: u32,
    },
    RemovePiece {
        id: u32,
    },

    // ---- Author tools  ---------------------------------
    /// Create a new resource kind addressable by `name`. `color` is a
    /// palette index validated against the loaded theme.
    DefineResource {
        name: String,
        color: u8,
    },
    /// Add a producer that emits `amount` of `resource` (by name)
    /// every `interval_ticks`.
    AddProducer {
        resource: String,
        amount: u64,
        interval_ticks: u32,
    },
    /// Add a consumer that drains `amount` of `resource` (by name)
    /// every `interval_ticks` when inventory is sufficient.
    AddConsumer {
        resource: String,
        amount: u64,
        interval_ticks: u32,
    },
    /// Set the scene's win/end condition. Today only `"loop_forever"`
    /// is supported; future variants will be added here.
    SetGoal {
        goal: String,
    },
}

// =====================================================================
//                                Tests
// =====================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de>>(v: &T) -> T {
        let s = serde_json::to_string(v).expect("encode");
        serde_json::from_str(&s).expect("decode")
    }

    // ---- 1. Envelope basics ----------------------------------------

    #[test]
    fn envelope_new_uses_current_schema_version() {
        let env = Envelope::new(0, SimMessage::Events(vec![]));
        assert_eq!(env.schema_version, SCHEMA_VERSION);
        assert!(env.is_compatible());
    }

    #[test]
    fn envelope_detects_schema_mismatch() {
        let mut env = Envelope::new(1, SimMessage::Events(vec![]));
        env.schema_version = SCHEMA_VERSION + 1;
        assert!(!env.is_compatible());
    }

    #[test]
    fn envelope_roundtrips_through_json() {
        let env = Envelope::new(42, SimMessage::Events(vec![SimEvent::Tick { tick: 7 }]));
        let back: Envelope<SimMessage> = roundtrip(&env);
        assert_eq!(back.seq, 42);
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        match back.payload {
            SimMessage::Events(events) => assert_eq!(events.len(), 1),
            _ => panic!("expected Events variant"),
        }
    }

    // ---- 2. SimEvent variants encode/decode ------------------------

    #[test]
    fn sim_event_mover_departed_roundtrips() {
        let e = SimEvent::MoverDeparted {
            mover: 3,
            from_node: 1,
            path: 2,
        };
        assert_eq!(roundtrip(&e), e);
    }

    #[test]
    fn sim_event_mover_arrived_roundtrips() {
        let e = SimEvent::MoverArrived {
            mover: 3,
            at_node: 5,
            path: 2,
        };
        assert_eq!(roundtrip(&e), e);
    }

    #[test]
    fn sim_event_node_highlighted_carries_reason() {
        let e = SimEvent::NodeHighlighted {
            node: 9,
            reason: HighlightReason::Bottleneck,
        };
        let back: SimEvent = roundtrip(&e);
        assert_eq!(back, e);
    }

    #[test]
    fn sim_event_agent_decided_uses_action_tag() {
        let e = SimEvent::AgentDecided {
            agent_id: "speed_tuner_0".into(),
            action: ActionTag::SetSpeed,
        };
        let s = serde_json::to_string(&e).expect("encode");
        // ActionTag is a plain string in JSON, not a tagged object.
        assert!(s.contains("\"action\":\"set_speed\""));
    }

    // ---- 3. Action variants + Action::tag() ------------------------

    #[test]
    fn action_tag_matches_variant() {
        assert_eq!(Action::NoOp.tag(), ActionTag::NoOp);
        assert_eq!(
            Action::SetSpeed {
                mover: 1,
                speed: 1.0
            }
            .tag(),
            ActionTag::SetSpeed
        );
        assert_eq!(
            Action::PlacePiece {
                piece_kind: "node".into(),
                pos: [0.0, 0.0]
            }
            .tag(),
            ActionTag::PlacePiece,
        );
        assert_eq!(
            Action::ConnectPieces { from: 1, to: 2 }.tag(),
            ActionTag::ConnectPieces
        );
        assert_eq!(Action::RemovePiece { id: 3 }.tag(), ActionTag::RemovePiece);
    }

    #[test]
    fn action_set_speed_roundtrips() {
        let a = Action::SetSpeed {
            mover: 1,
            speed: 1.5,
        };
        assert_eq!(roundtrip(&a), a);
    }

    // ---- 4. AgentMessage roundtrips --------------------------------

    #[test]
    fn agent_message_action_roundtrips() {
        let msg = AgentMessage::Action(Action::SetSpeed {
            mover: 2,
            speed: 2.0,
        });
        let s = serde_json::to_string(&msg).expect("encode");
        let _back: AgentMessage = serde_json::from_str(&s).expect("decode");
        assert!(s.contains("\"kind\":\"action\""));
    }

    // ---- 5. Faults & warnings --------------------------------------

    #[test]
    fn fault_load_error_carries_position() {
        let f = FaultPayload::LoadError {
            message: "expected ','".into(),
            line: Some(12),
            col: Some(4),
        };
        let s = serde_json::to_string(&f).expect("encode");
        assert!(s.contains("\"line\":12"));
        assert!(s.contains("\"col\":4"));
    }

    #[test]
    fn fault_schema_mismatch_roundtrips() {
        let f = FaultPayload::SchemaMismatch {
            expected: 1,
            found: 999,
        };
        let s = serde_json::to_string(&f).expect("encode");
        let _back: FaultPayload = serde_json::from_str(&s).expect("decode");
        assert!(s.contains("schema_mismatch"));
    }

    #[test]
    fn warning_behind_roundtrips() {
        // Engine-pacing (no agent_id) — default form.
        let w = WarningPayload::Behind {
            lag_frames: 7,
            agent_id: None,
        };
        let s = serde_json::to_string(&w).expect("encode");
        let back: WarningPayload = serde_json::from_str(&s).expect("decode");
        assert_eq!(back, w);
        assert!(s.contains("\"lag_frames\":7"));
        // agent_id: None must NOT appear in serialized form (backward
        // compat: old consumers that don't know about the field still
        // see the v1 shape).
        assert!(!s.contains("agent_id"));
    }

    #[test]
    fn warning_behind_with_agent_id_roundtrips() {
        // Live LLM agent attribution case.
        let w = WarningPayload::Behind {
            lag_frames: 3,
            agent_id: Some("metro-pulse-llm".to_string()),
        };
        let s = serde_json::to_string(&w).expect("encode");
        let back: WarningPayload = serde_json::from_str(&s).expect("decode");
        assert_eq!(back, w);
        assert!(s.contains("\"lag_frames\":3"));
        assert!(s.contains("\"agent_id\":\"metro-pulse-llm\""));
    }

    #[test]
    fn warning_behind_legacy_payload_decodes() {
        // A v1-era payload without agent_id must still deserialize.
        let legacy = r#"{"kind":"behind","lag_frames":5}"#;
        let w: WarningPayload = serde_json::from_str(legacy).expect("decode legacy");
        assert_eq!(
            w,
            WarningPayload::Behind {
                lag_frames: 5,
                agent_id: None
            }
        );
    }

    // ---- 6. Snapshot + Static --------------------------------------

    #[test]
    fn snapshot_with_movers_roundtrips() {
        let snap = SnapshotPayload {
            tick: 100,
            movers: vec![MoverState {
                id: 1,
                pos: [10.0, 20.0],
                speed: 1.5,
                on_path: 2,
            }],
            sl1_place_inventories: Vec::new(),
            sl1_transform_states: Vec::new(),
            sl1_demand_states: Vec::new(),
        };
        let back: SnapshotPayload = roundtrip(&snap);
        assert_eq!(back.tick, 100);
        assert_eq!(back.movers.len(), 1);
        assert_eq!(back.movers[0].speed, 1.5);
    }

    #[test]
    fn static_payload_id_map_roundtrips() {
        let mut node_names = std::collections::BTreeMap::new();
        node_names.insert(0u32, "a".to_string());
        node_names.insert(1u32, "b".to_string());
        let mut path_names = std::collections::BTreeMap::new();
        path_names.insert(0u32, "ab".to_string());
        let sp = StaticPayload {
            name: "demo".into(),
            palette: vec!["#000".into()],
            background_index: 0,
            nodes: vec![NodeView {
                id: 0,
                pos: [10.0, 20.0],
                shape: NodeShapeTag::Circle,
                color: 2,
            }],
            paths: vec![PathView {
                id: 0,
                from_pos: [10.0, 20.0],
                to_pos: [30.0, 40.0],
                color: 3,
            }],
            node_names,
            path_names,
            mover_names: std::collections::BTreeMap::new(),
            sl1_places: Vec::new(),
            sl1_links: Vec::new(),
            sl1_things: Vec::new(),
            sl1_transforms: Vec::new(),
            sl1_demand: Vec::new(),
        };
        let back: StaticPayload = roundtrip(&sp);
        assert_eq!(back.node_names.len(), 2);
        assert_eq!(back.node_names[&0], "a");
        assert_eq!(back.path_names[&0], "ab");
        assert_eq!(back.nodes.len(), 1);
        assert_eq!(back.paths[0].color, 3);
    }

    #[test]
    fn sl1_places_round_trip_through_static_payload() {
        // Covers both predicate variants and both with-/without-
        // grace_ticks, plus shape/color presence and absence. Goal:
        // detect any silent serde shape change in PR 1's protocol
        // mirror.
        let mut capacity = std::collections::BTreeMap::new();
        capacity.insert("query_slots".to_string(), 64u64);
        let mut storage = std::collections::BTreeMap::new();
        storage.insert(
            "hot_cache".to_string(),
            Sl1StorageSlotView {
                capacity: 1024,
                initial: 256,
            },
        );
        let mut ops = std::collections::BTreeMap::new();
        ops.insert(
            "strained".to_string(),
            Sl1OperatingStateView {
                predicate: Sl1OperatingPredicateView::UsedPercentGte {
                    metric: "query_slots".into(),
                    threshold: 80,
                },
                grace_ticks: None,
            },
        );
        ops.insert(
            "overloaded".to_string(),
            Sl1OperatingStateView {
                predicate: Sl1OperatingPredicateView::UsedPercentGte {
                    metric: "query_slots".into(),
                    threshold: 95,
                },
                grace_ticks: Some(120),
            },
        );
        ops.insert(
            "failed".to_string(),
            Sl1OperatingStateView {
                predicate: Sl1OperatingPredicateView::OverloadedTicksGt { ticks: 600 },
                grace_ticks: None,
            },
        );
        let sp = StaticPayload {
            name: "demo".into(),
            palette: vec!["#000".into()],
            background_index: 0,
            nodes: vec![],
            paths: vec![],
            node_names: std::collections::BTreeMap::new(),
            path_names: std::collections::BTreeMap::new(),
            mover_names: std::collections::BTreeMap::new(),
            sl1_places: vec![
                Sl1PlaceView {
                    id: "kusto-cluster".into(),
                    role: "compute".into(),
                    pos: [120.0, 80.0],
                    shape: Some("hexagon".into()),
                    color: Some(2),
                    capacity,
                    storage,
                    accepts: vec!["query".into()],
                    produces: vec!["result".into()],
                    failure_domains: vec!["az1".into()],
                    operating_states: ops,
                },
                Sl1PlaceView {
                    id: "dashboard".into(),
                    role: "observability".into(),
                    pos: [0.0, 0.0],
                    shape: None,
                    color: None,
                    capacity: std::collections::BTreeMap::new(),
                    storage: std::collections::BTreeMap::new(),
                    accepts: vec![],
                    produces: vec![],
                    failure_domains: vec![],
                    operating_states: std::collections::BTreeMap::new(),
                },
            ],
            sl1_links: Vec::new(),
            sl1_things: Vec::new(),
            sl1_transforms: Vec::new(),
            sl1_demand: Vec::new(),
        };
        let back: StaticPayload = roundtrip(&sp);
        assert_eq!(back.sl1_places, sp.sl1_places);

        let json = serde_json::to_value(&sp).unwrap();
        // Predicate shape is internally tagged.
        let preds = &json["sl1_places"][0]["operating_states"];
        assert_eq!(preds["strained"]["predicate"]["kind"], "used_percent_gte");
        assert_eq!(preds["overloaded"]["predicate"]["kind"], "used_percent_gte");
        assert_eq!(preds["failed"]["predicate"]["kind"], "overloaded_ticks_gt");
        // Absent grace_ticks must NOT appear (skip_serializing_if).
        assert!(preds["strained"].get("grace_ticks").is_none());
        // Absent shape/color must NOT appear (skip_serializing_if).
        let dashboard = &json["sl1_places"][1];
        assert!(dashboard.get("shape").is_none());
        assert!(dashboard.get("color").is_none());

        // Non-SL1 scenes must NOT include the field at all.
        let bare = StaticPayload {
            name: "legacy".into(),
            palette: vec![],
            background_index: 0,
            nodes: vec![],
            paths: vec![],
            node_names: std::collections::BTreeMap::new(),
            path_names: std::collections::BTreeMap::new(),
            mover_names: std::collections::BTreeMap::new(),
            sl1_places: vec![],
            sl1_links: vec![],
            sl1_things: vec![],
            sl1_transforms: vec![],
            sl1_demand: vec![],
        };
        let bare_json = serde_json::to_value(&bare).unwrap();
        assert!(bare_json.get("sl1_places").is_none());
        assert!(bare_json.get("sl1_links").is_none());
    }

    #[test]
    fn sl1_links_round_trip_through_static_payload() {
        // Covers both directions, every backpressure, render
        // presence/absence + color presence/absence, sorted
        // compatibility, and skip-if-empty semantics.
        let mut cap = std::collections::BTreeMap::new();
        cap.insert("events_per_tick".to_string(), 120u64);
        let links = vec![
            Sl1LinkView {
                id: "telemetry-to-normalizer".into(),
                link_type: "data_stream".into(),
                from: "src".into(),
                to: "dst".into(),
                direction: Sl1LinkDirectionView::Forward,
                capacity: cap.clone(),
                travel_ticks: 1,
                compatibility: vec!["gpu_heartbeat".into()],
                queue_capacity: 1000,
                backpressure: Sl1LinkBackpressureView::BlockUpstream,
                render: Some(Sl1LinkRenderHintView {
                    style: "flow".into(),
                    color: Some(3),
                }),
            },
            Sl1LinkView {
                id: "two-way".into(),
                link_type: "bus".into(),
                from: "a".into(),
                to: "b".into(),
                direction: Sl1LinkDirectionView::Bidirectional,
                capacity: std::collections::BTreeMap::new(),
                travel_ticks: 5,
                compatibility: vec![],
                queue_capacity: 1,
                backpressure: Sl1LinkBackpressureView::DropLowPriority,
                render: None,
            },
        ];
        let sp = StaticPayload {
            name: "demo".into(),
            palette: vec!["#000".into()],
            background_index: 0,
            nodes: vec![],
            paths: vec![],
            node_names: std::collections::BTreeMap::new(),
            path_names: std::collections::BTreeMap::new(),
            mover_names: std::collections::BTreeMap::new(),
            sl1_places: vec![],
            sl1_links: links.clone(),
            sl1_things: vec![],
            sl1_transforms: vec![],
            sl1_demand: vec![],
        };
        let back: StaticPayload = roundtrip(&sp);
        assert_eq!(back.sl1_links, links);

        let json = serde_json::to_value(&sp).unwrap();
        assert_eq!(json["sl1_links"][0]["type"], "data_stream");
        assert_eq!(json["sl1_links"][0]["direction"], "forward");
        assert_eq!(json["sl1_links"][0]["backpressure"], "block_upstream");
        assert_eq!(json["sl1_links"][1]["direction"], "bidirectional");
        assert_eq!(json["sl1_links"][1]["backpressure"], "drop_low_priority");
        // Render absent -> field skipped.
        assert!(json["sl1_links"][1].get("render").is_none());
    }

    // ---- 7. AgentReport --------------------------------------------

    #[test]
    fn agent_report_with_considered_roundtrips() {
        let rep = AgentReport {
            tick: 100,
            agent_id: "speed_tuner_0".into(),
            considered: vec![
                ConsideredAction {
                    action: Action::SetSpeed {
                        mover: 1,
                        speed: 1.5,
                    },
                    confidence: 0.82,
                },
                ConsideredAction {
                    action: Action::SetSpeed {
                        mover: 1,
                        speed: 1.0,
                    },
                    confidence: 0.61,
                },
            ],
            chosen: Some(Action::SetSpeed {
                mover: 1,
                speed: 1.5,
            }),
            rationale: "m1 has been waiting".into(),
            confidence: 0.82,
        };
        let back: AgentReport = roundtrip(&rep);
        assert_eq!(back.considered.len(), 2);
        assert!(back.chosen.is_some());
    }

    // ---- 8. Schema version constant --------------------------------

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
