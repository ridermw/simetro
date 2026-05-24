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
pub mod components;
pub mod error;
pub mod events;
pub mod loader;
pub mod rng;
pub mod snapshot;
pub mod state_hash;
pub mod systems;
pub mod tick;
pub mod world;

pub use actions::{apply_action, Outcome};
pub use agent::{Agent, AgentHost, MoverObservation, Observation, SpeedTuner};
pub use agent_log::{observation_hash, AgentLog, AgentLogEntry};
pub use components::{Mover, MoverId, MoverState, Node, NodeId, NodeShape, Path, PathId};
pub use error::{AgentError, EngineFault, LoadError};
pub use events::{agent_error_to_message, engine_fault_to_payload, load_error_to_fault};
pub use loader::{load_scene_str, AgentSpec, Goal, IdMap, LoadedScene, Theme};
pub use rng::SimRng;
pub use snapshot::{color_batches, encode_snapshot, encode_static};
pub use state_hash::{hash_run, hash_world};
pub use tick::{tick_accumulator, tick_once, TickOutput, TickRunner};
pub use world::{RunState, World};
