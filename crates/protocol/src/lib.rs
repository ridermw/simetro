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
    /// hit the renderer batching and allocation target batching target (~6 draw calls per scene).
    pub paths: Vec<PathView>,
    /// JSON string id → numeric id, segregated by section so two kinds
    /// can share an id space without collision (identifier interning contract).
    pub node_names: std::collections::BTreeMap<u32, String>,
    pub path_names: std::collections::BTreeMap<u32, String>,
    pub mover_names: std::collections::BTreeMap<u32, String>,
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
/// RemovePiece, plus the P2.A task 9 resource/production tools
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

    // ---- Author tools (P2.A task 9) ---------------------------------
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
        };
        let back: StaticPayload = roundtrip(&sp);
        assert_eq!(back.node_names.len(), 2);
        assert_eq!(back.node_names[&0], "a");
        assert_eq!(back.path_names[&0], "ab");
        assert_eq!(back.nodes.len(), 1);
        assert_eq!(back.paths[0].color, 3);
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
