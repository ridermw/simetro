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
    /// Stable string identifier for this variant. Used by the
    /// fixture-suite drift-detection test (P2.A task 11) and the
    /// runbook docs. The match is exhaustive — adding a new variant
    /// without an arm here fails to compile, so the variant catalogue
    /// CANNOT drift from the enum definition.
    #[must_use]
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::NotAuthenticated => "not_authenticated",
            Self::SubprocessDied { .. } => "subprocess_died",
            Self::Refused { .. } => "refused",
            Self::Timeout { .. } => "timeout",
            Self::RateLimited { .. } => "rate_limited",
            Self::MalformedResponse { .. } => "malformed_response",
            Self::Disconnected => "disconnected",
        }
    }

    /// One representative instance of every variant. Pair with
    /// [`Self::variant_name`] for compile-time-checked variant
    /// enumeration. Adding a new variant requires both updating
    /// `variant_name`'s match arm (compile-time check) AND appending
    /// here (test catches the count mismatch via [`Self::ALL_VARIANTS`]).
    ///
    /// The strings in payload fields are intentionally minimal; this
    /// constructor exists only so callers (tests, docs) can iterate
    /// every variant.
    #[must_use]
    pub fn one_of_each() -> Vec<Self> {
        vec![
            Self::NotAuthenticated,
            Self::SubprocessDied { code: None },
            Self::Refused {
                agent_id: String::new(),
                message: String::new(),
            },
            Self::Timeout {
                agent_id: String::new(),
                elapsed_ms: 0,
            },
            Self::RateLimited { retry_after_ms: 0 },
            Self::MalformedResponse {
                agent_id: String::new(),
                raw: String::new(),
            },
            Self::Disconnected,
        ]
    }

    /// Catalogue of every `LlmError` variant identifier, derived from
    /// [`Self::one_of_each`] so the names stay in lock-step with the
    /// enum via the exhaustive match in [`Self::variant_name`]. Used
    /// by the fixture-suite drift test.
    ///
    /// **Adding a variant**: add the arm to `variant_name`, append a
    /// representative to `one_of_each`, and create
    /// `crates/agent-bridge/tests/fixtures/error_modes/<name>.json`.
    /// The test `every_llm_error_variant_has_a_fixture` enforces all
    /// three changes land in the same PR.
    #[must_use]
    pub fn all_variants() -> Vec<&'static str> {
        Self::one_of_each().iter().map(Self::variant_name).collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn variant_name_is_unique_per_variant() {
        let names = LlmError::all_variants();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            names.len(),
            "variant_name collision in LlmError: {names:?}"
        );
    }

    #[test]
    fn one_of_each_covers_every_variant_round_trip() {
        // Sanity: each instance returns a non-empty variant name.
        for err in LlmError::one_of_each() {
            let name = err.variant_name();
            assert!(!name.is_empty(), "empty variant_name for {err:?}");
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "variant_name must be snake_case ascii: {name:?}"
            );
        }
    }
}
