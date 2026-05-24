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

pub mod agent;
pub mod components;
pub mod error;
pub mod loader;
pub mod rng;
pub mod snapshot;
pub mod systems;
pub mod tick;
pub mod world;

pub use agent::{Agent, AgentHost, MoverObservation, Observation, SpeedTuner};
pub use components::{Mover, MoverId, MoverState, Node, NodeId, NodeShape, Path, PathId};
pub use error::{AgentError, EngineFault, LoadError};
pub use loader::{load_scene_str, AgentSpec, Goal, IdMap, LoadedScene, Theme};
pub use rng::SimRng;
pub use snapshot::{color_batches, encode_snapshot, encode_static};
pub use tick::{tick_accumulator, tick_once, TickOutput, TickRunner};
pub use world::{RunState, World};
