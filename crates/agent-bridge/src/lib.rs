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
//!                                          Mock         Copilot      future
//!                                                       (stub)
//! ```

pub mod backend;
pub mod backends;
pub mod bridge;
pub mod error;
pub mod error_mapping;
pub mod prompt;
pub mod tools;
pub mod wire;

pub use backend::{Backend, BackendRequest, BackendResponse, ToolCall, ToolSpec};
pub use backends::copilot::{CopilotBackend, CopilotConfig};
pub use backends::mock::{MockBackend, MockTurn};
pub use bridge::{first_action, parse_tool_call, Bridge};
pub use error::LlmError;
pub use error_mapping::llm_error_to_message;
pub use prompt::{REQUIRED_PROMPT_SUBSTRINGS, SYSTEM_PROMPT};
pub use tools::{action_tool_specs, names};
pub use wire::{hello_envelope, read_envelope, shutdown_envelope, write_envelope, BridgeMessage};
