//! # simetro-agent-bridge
//!
//! Separate process/binary that bridges the engine's wire protocol to
//! whichever LLM backend the user has configured. Pluggable; the engine
//! never imports an LLM crate.
//!
//! ```text
//!     engine ◀──── protocol (stdio/WS) ────▶ bridge ──▶ Backend trait
//!                                                          │
//!                                            ┌─────────────┼──────────────┐
//!                                            ▼             ▼              ▼
//!                                          Mock         Copilot      OpenAI(P2)
//!                                                       (P1 stub)
//! ```

pub mod backend;
pub mod backends;
pub mod error;

pub use backend::{Backend, BackendRequest, BackendResponse, ToolCall, ToolSpec};
pub use error::LlmError;
