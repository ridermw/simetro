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
pub mod bridge;
pub mod error;
pub mod prompt;
pub mod tools;

pub use backend::{Backend, BackendRequest, BackendResponse, ToolCall, ToolSpec};
pub use backends::copilot::{CopilotBackend, CopilotConfig};
pub use backends::mock::{MockBackend, MockTurn};
pub use bridge::{first_action, parse_tool_call, Bridge};
pub use error::LlmError;
pub use prompt::{REQUIRED_PROMPT_SUBSTRINGS, SYSTEM_PROMPT};
pub use tools::{action_tool_specs, names};
