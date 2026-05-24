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
//!   Receivers reject on mismatch; never silently process.
//! ```

use serde::{Deserialize, Serialize};

pub mod version;

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
}

/// Sim → consumer messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SimMessage {
    Static(StaticPayload),
    Snapshot(SnapshotPayload),
    Events { events: Vec<SimEvent> },
    AgentReport(AgentReport),
    Fault(FaultPayload),
    Warning(WarningPayload),
}

/// Bridge/agent → engine messages. Live in Phase 2; declared here so
/// the protocol surface is stable from Phase 1.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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

// ---------- payloads (stubs filled in across Steps 4–11) ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StaticPayload {
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotPayload {
    pub tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SimEvent {
    Tick { tick: u64 },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentReport {
    pub tick: u64,
    pub agent_id: String,
    pub rationale: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FaultPayload {
    LoadError { message: String },
    AgentCrashed { agent_id: String, message: String },
    NumericDrift { tick: u64 },
    EngineFault { message: String },
    BaselineHashMismatch { expected: String, found: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WarningPayload {
    InvalidAction { agent_id: String, reason: String },
    Behind { lag_frames: u32 },
    TickOverBudget { ms: f32 },
    AgentLogSlow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    NoOp,
    SetSpeed { mover: u32, speed: f32 },
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn envelope_carries_schema_version() {
        let env = Envelope::new(0, SimMessage::Events { events: vec![] });
        assert_eq!(env.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn envelope_roundtrips_json() {
        let env = Envelope::new(
            42,
            SimMessage::Events {
                events: vec![SimEvent::Tick { tick: 7 }],
            },
        );
        let s = serde_json::to_string(&env).expect("encode");
        let back: Envelope<SimMessage> = serde_json::from_str(&s).expect("decode");
        assert_eq!(back.seq, 42);
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        match back.payload {
            SimMessage::Events { events } => assert_eq!(events.len(), 1),
            _ => panic!("expected Events variant"),
        }
    }
}
