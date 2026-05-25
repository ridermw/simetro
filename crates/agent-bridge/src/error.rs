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

    /// Catalogue of every `LlmError` variant identifier.
    ///
    /// **Compile-time guarantee.** Each entry in the returned vec is
    /// produced by calling `name_for(&Self::SomeVariant {...})`, so
    /// the name returned ALWAYS comes from the exhaustive match in
    /// `name_for`. Arm-vs-name drift is structurally impossible:
    /// adding a new variant requires (1) an arm in `name_for`
    /// (compile error if missing) and (2) a `name_for(&Self::New {})`
    /// call below (no compile-time check, but visible adjacent edit).
    ///
    /// The list is in enum declaration order so test output is
    /// stable.
    #[must_use]
    pub fn all_variants() -> Vec<&'static str> {
        // EXHAUSTIVE match. Adding a variant without an arm here
        // fails to compile.
        fn name_for(e: &LlmError) -> &'static str {
            match e {
                LlmError::NotAuthenticated => "not_authenticated",
                LlmError::SubprocessDied { .. } => "subprocess_died",
                LlmError::Refused { .. } => "refused",
                LlmError::Timeout { .. } => "timeout",
                LlmError::RateLimited { .. } => "rate_limited",
                LlmError::MalformedResponse { .. } => "malformed_response",
                LlmError::Disconnected => "disconnected",
            }
        }
        vec![
            name_for(&LlmError::NotAuthenticated),
            name_for(&LlmError::SubprocessDied { code: None }),
            name_for(&LlmError::Refused {
                agent_id: String::new(),
                message: String::new(),
            }),
            name_for(&LlmError::Timeout {
                agent_id: String::new(),
                elapsed_ms: 0,
            }),
            name_for(&LlmError::RateLimited { retry_after_ms: 0 }),
            name_for(&LlmError::MalformedResponse {
                agent_id: String::new(),
                raw: String::new(),
            }),
            name_for(&LlmError::Disconnected),
        ]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Variant-name uniqueness: catches accidental duplicate arm
    /// names (e.g. typo where two arms return the same string).
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

    /// Variant-name shape: all snake_case ascii.
    #[test]
    fn all_variant_names_are_snake_case_ascii() {
        for name in LlmError::all_variants() {
            assert!(!name.is_empty(), "empty variant name");
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "variant name must be snake_case ascii: {name:?}"
            );
        }
    }

    /// `variant_name(instance)` MUST agree with `all_variants()` for
    /// the same variant. Catches drift between the public
    /// `variant_name` match and the internal `_exhaustive_check`
    /// match in `all_variants`.
    #[test]
    fn variant_name_matches_all_variants_list() {
        let instances: Vec<(LlmError, &'static str)> = vec![
            (LlmError::NotAuthenticated, "not_authenticated"),
            (LlmError::SubprocessDied { code: None }, "subprocess_died"),
            (
                LlmError::Refused {
                    agent_id: String::new(),
                    message: String::new(),
                },
                "refused",
            ),
            (
                LlmError::Timeout {
                    agent_id: String::new(),
                    elapsed_ms: 0,
                },
                "timeout",
            ),
            (LlmError::RateLimited { retry_after_ms: 0 }, "rate_limited"),
            (
                LlmError::MalformedResponse {
                    agent_id: String::new(),
                    raw: String::new(),
                },
                "malformed_response",
            ),
            (LlmError::Disconnected, "disconnected"),
        ];
        let all = LlmError::all_variants();
        for (instance, expected_name) in &instances {
            assert_eq!(
                instance.variant_name(),
                *expected_name,
                "variant_name disagrees for {instance:?}"
            );
            assert!(
                all.contains(expected_name),
                "all_variants() missing {expected_name:?}"
            );
        }
        // Both lists must be the same length — otherwise a variant
        // was added in one place but not the other.
        assert_eq!(
            instances.len(),
            all.len(),
            "instance count ({}) != all_variants count ({}); some variant is missing from \
             either `_exhaustive_check` or this test's `instances` list",
            instances.len(),
            all.len()
        );
    }
}
