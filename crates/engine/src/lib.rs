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

pub mod components;
pub mod error;
pub mod loader;
pub mod rng;
pub mod systems;
pub mod tick;
pub mod world;

pub use components::{Mover, MoverId, MoverState, Node, NodeId, NodeShape, Path, PathId};
pub use error::{AgentError, EngineFault, LoadError};
pub use loader::{load_scene_str, AgentSpec, Goal, IdMap, LoadedScene, Theme};
pub use rng::SimRng;
pub use tick::{tick_accumulator, tick_once, TickOutput};
pub use world::{RunState, World};
