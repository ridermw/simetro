//! `Backend` trait: the boundary every LLM provider implements.
//!
//! ```text
//!     ┌──────────────┐  invoke(req)   ┌────────────────┐
//!     │  Bridge      │ ──────────────▶│ Backend (trait)│
//!     │  harness     │ ◀──────────────│  Mock | Copilot│
//!     └──────────────┘  Result        └────────────────┘
//!                          │
//!                          ▼
//!                   parse tool call
//!                          │
//!                          ▼
//!                     Action  → engine
//! ```

use crate::error::LlmError;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct BackendRequest {
    /// User-visible identifier of the agent making the request.
    /// Surfaces in `LlmError::Refused { agent_id, .. }`.
    pub agent_id: String,
    /// The full prompt the backend should see. The bridge builds
    /// this from the observation + tool specs.
    pub prompt: String,
    /// The Action tool schemas this turn permits the model to call.
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, Default)]
pub struct BackendResponse {
    /// The model's raw text response (used by AgentLog).
    pub raw: String,
    /// Tool calls the model emitted, in order. Bridge picks the first
    /// valid one (PLAN §11.2 InvalidAction warning for malformed).
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    /// JSON Schema describing the tool's argument shape. Bridges send
    /// this to the model verbatim.
    pub json_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
    /// JSON-encoded arguments object. Bridge parses into an
    /// [`simetro_protocol::Action`].
    pub arguments_json: String,
}

#[async_trait]
pub trait Backend: Send + Sync {
    /// Stable identifier surfaced in tracing and config.
    fn name(&self) -> &'static str;

    /// Issue one call to the model.
    ///
    /// # Errors
    /// One of the [`LlmError`] variants per PLAN §11.1.
    async fn invoke(&self, req: BackendRequest) -> Result<BackendResponse, LlmError>;
}
