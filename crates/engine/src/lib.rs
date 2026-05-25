//! # simetro-engine
//!
//! Pure simulation core. No IO. No LLM deps. No async runtime.
//!
//! ```text
//!     JSON ──▶ loader ──▶ World ──▶ tick() ──▶ TickOutput
//!                            ▲           │
//!                            │           │
//!                       built-in Agent   ├─▶ snapshot (positions)
//!                       (SpeedTuner)     └─▶ events    (semantic)
//! ```
//!
//! See `docs/architecture.md` and `PLAN.md` §3.

pub mod actions;
pub mod agent;
pub mod agent_log;
pub mod agent_runtime;
pub mod components;
pub mod error;
pub mod events;
pub mod lifecycle;
pub mod llm_agent;
pub mod loader;
pub mod redactor;
pub mod rng;
pub mod scenario_language_v1;
pub mod sl1_agents;
pub mod sl1_objectives;
pub mod sl1_observability;
pub mod sl1_pressure;
pub mod sl1_runtime;
pub mod snapshot;
pub mod state_hash;
pub mod systems;
pub mod tick;
pub mod world;

pub use actions::{apply_action, Outcome};
pub use agent::{Agent, AgentHost, MoverObservation, Observation, SpeedTuner};
pub use agent_log::{
    agent_log_dir, observation_hash, validate_entry, validate_scene_id, AgentLog, AgentLogEntry,
    LlmProvenance, SceneIdError, SchemaError, AGENT_ID_MAX_LEN, PROVENANCE_STR_MAX_LEN,
    RATIONALE_MAX_LEN, RAW_RESPONSE_MAX_BYTES, SCHEMA_VERSION,
};
pub use agent_runtime::{AgentRuntime, EnqueueDecisionOutcome, ExpireOutcome, ProcessReplyOutcome};
pub use components::{
    Consumer, ConsumerId, Mover, MoverId, MoverState, Node, NodeId, NodeShape, Path, PathId,
    Producer, ProducerId, Resource, ResourceId,
};
pub use error::{AgentError, EngineFault, LoadError};
pub use events::{agent_error_to_message, engine_fault_to_payload, load_error_to_fault};
pub use loader::{load_scene_str, AgentSpec, Goal, IdMap, LoadedScene, Theme};
pub use rng::SimRng;
pub use scenario_language_v1::{
    load_str as load_sl1_str, load_value as load_sl1_value, validate as validate_sl1,
    FreshnessState, GameOutcome, RawSl1Scene, Sl1Agent, Sl1AgentAction, Sl1AgentActionKind,
    Sl1AgentKind, Sl1AgentObservationTarget, Sl1AgentRejectionReason, Sl1AgentRuntimeState,
    Sl1Demand, Sl1DemandInstance, Sl1DemandPenalty, Sl1DemandPriority, Sl1DemandRuntime,
    Sl1DemandSchedule, Sl1DemandTarget, Sl1FailureCondition, Sl1FailureConditionKind,
    Sl1FailureConditionParams, Sl1FailureConditionRuntime, Sl1FailurePolicy, Sl1Fault,
    Sl1GamePhase, Sl1Link, Sl1LinkBackpressure, Sl1LinkDirection, Sl1LinkRenderHint, Sl1LoadError,
    Sl1Milestone, Sl1Objective, Sl1ObjectiveKind, Sl1ObjectiveParams, Sl1ObjectiveRuntime,
    Sl1ObjectiveStatus, Sl1Observability, Sl1Place, Sl1Pressure, Sl1RuntimeState, Sl1Scene,
    Sl1Thing, Sl1ThingQualityContract, Sl1ThingRenderHint, Sl1Transform, Sl1TransformIo,
    Sl1TransformState, Sl1VictoryCondition, Sl1VictoryConditionKind, Sl1VictoryConditionParams,
    Sl1VictoryConditionRuntime, Sl1Warning, MAX_DEMAND_OUTSTANDING, MAX_DEMAND_PENALTY_SCORE,
    MAX_DEMAND_REQUIRES, MAX_DEMAND_SCRIPTED_TICKS, MAX_DEMAND_TICKS, MAX_DEMAND_VALUE,
    MAX_OBJECTIVE_BREACH_COUNT, MAX_OBJECTIVE_TICKS, MAX_OBJECTIVE_WEIGHT, MAX_TRANSFORM_AMOUNT,
    MAX_TRANSFORM_CAPACITY_COST, MAX_TRANSFORM_MAX_ATTEMPTS, MAX_TRANSFORM_TICKS,
    SL1_SCHEMA_VERSION,
};
pub use snapshot::{color_batches, encode_snapshot, encode_static, encode_static_parts};
pub use state_hash::{hash_run, hash_world};
pub use tick::{tick_accumulator, tick_once, TickOutput, TickRunner};
pub use world::{RunState, World};
