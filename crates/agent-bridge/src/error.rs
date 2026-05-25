//! Typed errors for LLM backends. Lives in the bridge, never in core.

use thiserror::Error;

#[derive(Debug, Clone, Error)]
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

impl LlmError {
    /// Catalogue of every `LlmError` variant identifier. Used by
    /// drift-detection tests (P2.A task 11 fixture suite) and the
    /// runbook docs to enumerate the error surface.
    ///
    /// **Adding a variant**: append a new entry here AND create a
    /// matching fixture under
    /// `crates/agent-bridge/tests/fixtures/error_modes/<name>.json`.
    /// The test `every_llm_error_variant_has_a_fixture` enforces both
    /// changes land in the same PR.
    pub const ALL_VARIANTS: &'static [&'static str] = &[
        "not_authenticated",
        "subprocess_died",
        "refused",
        "timeout",
        "rate_limited",
        "malformed_response",
        "disconnected",
    ];
}
