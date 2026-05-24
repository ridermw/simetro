//! Typed errors for LLM backends. Lives in the bridge, never in core.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("backend not authenticated")]
    NotAuthenticated,

    #[error("backend subprocess died (code: {code:?})")]
    SubprocessDied { code: Option<i32> },

    #[error("agent {agent_id} refused: {message}")]
    Refused { agent_id: String, message: String },

    #[error("agent {agent_id} timed out after {elapsed_ms}ms")]
    Timeout { agent_id: String, elapsed_ms: u32 },

    #[error("rate limited; retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u32 },

    #[error("agent {agent_id} returned malformed response")]
    MalformedResponse { agent_id: String, raw: String },

    #[error("backend disconnected")]
    Disconnected,
}
