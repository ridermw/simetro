//! P2.A task 4: typed `LlmError` → `SimMessage` mapping.
//!
//! The mapping is **table-driven**: every `LlmError` variant maps to
//! exactly one `Fault` or `Warning` SimMessage. The mapping is
//! authoritative per `docs/superpowers/analysis/p2a-error-map.md`
//! §1 (the rescue-action / user-visible-behavior matrix) and per
//! spec §10.4 (the error-mapping table).
//!
//! Acceptance criterion from spec §3 task 4:
//!
//! > Table-driven mapping with a unit test per variant.
//!
//! This module is the single source of truth for the bridge-side
//! mapping. The engine consumes `SimMessage` produced here and
//! emits them via `TickRunner::messages()` per the standard event
//! stream.
//!
//! ## Why this lives in the bridge crate
//!
//! `LlmError` itself lives in the bridge (`crates/agent-bridge/src/error.rs`)
//! because the engine MUST NOT depend on bridge-side concepts. The
//! mapping FROM `LlmError` TO `SimMessage` is a bridge concern; the
//! engine only sees the resulting `SimMessage`.
//!
//! ## Lag-frames computation
//!
//! For `Timeout` and `RateLimited`, the resulting `Warning::Behind`
//! payload carries a `lag_frames` value. The bridge does not know
//! the engine's tick rate, so the **caller** computes `lag_frames`
//! and passes it in. For `Timeout` the natural value is
//! `(elapsed_ms / world_dt_ms) as u32`; for `RateLimited` it's
//! `(retry_after_ms / world_dt_ms) as u32`. The mapping function
//! accepts a `lag_frames` parameter (unused for non-Behind variants).

use simetro_protocol::{FaultPayload, SimMessage, WarningPayload};

use crate::error::LlmError;

/// Convert a typed [`LlmError`] into a [`SimMessage`] that the
/// engine can surface via `TickRunner::messages()`.
///
/// `fallback_agent_id` is used for variants that don't carry an
/// `agent_id` (e.g. `NotAuthenticated`, `SubprocessDied`,
/// `RateLimited`, `Disconnected`). For variants that DO carry their
/// own `agent_id`, the carried value wins.
///
/// `lag_frames` is the engine-tick-count equivalent of the error's
/// time field; computed by the caller from the engine's `world.dt`.
/// Only consulted for `Timeout` and `RateLimited`; ignored otherwise.
/// For `Timeout` and `RateLimited`, `lag_frames` is **clamped to a
/// minimum of 1** because `Warning::Behind { lag_frames: 0 }` would
/// semantically mean "on time" — which contradicts the error type.
/// Callers should still pass the true elapsed value; the clamp
/// catches edge cases where the timeout fires in less than one
/// engine frame (`elapsed_ms < world_dt_ms`).
#[must_use]
pub fn llm_error_to_message(
    err: &LlmError,
    fallback_agent_id: &str,
    lag_frames: u32,
) -> SimMessage {
    match err {
        // ---- Faults (engine pauses; no recovery via Warning) ----
        LlmError::NotAuthenticated => SimMessage::Fault(FaultPayload::AgentCrashed {
            agent_id: fallback_agent_id.to_string(),
            message: "not authenticated".to_string(),
        }),

        LlmError::SubprocessDied { code } => SimMessage::Fault(FaultPayload::AgentCrashed {
            agent_id: fallback_agent_id.to_string(),
            message: match code {
                Some(c) => format!("subprocess died (code: {c})"),
                None => "subprocess died (no exit code)".to_string(),
            },
        }),

        LlmError::Disconnected => SimMessage::Fault(FaultPayload::AgentCrashed {
            agent_id: fallback_agent_id.to_string(),
            message: "ACP stdio disconnected".to_string(),
        }),

        // ---- Warnings (engine continues; agent specific) ----
        LlmError::Refused { agent_id, message } => {
            SimMessage::Warning(WarningPayload::InvalidAction {
                agent_id: agent_id.clone(),
                reason: format!("refused: {message}"),
            })
        }

        LlmError::Timeout {
            agent_id,
            elapsed_ms: _,
        } => SimMessage::Warning(WarningPayload::Behind {
            lag_frames: lag_frames.max(1),
            agent_id: Some(agent_id.clone()),
        }),

        LlmError::RateLimited { retry_after_ms: _ } => {
            // RateLimited is engine-pacing (the bridge / model is the
            // pacer, not a specific agent), so agent_id falls back to
            // the caller-supplied identifier. The retry_after_ms is
            // already encoded in the lag_frames the caller computed.
            SimMessage::Warning(WarningPayload::Behind {
                lag_frames: lag_frames.max(1),
                agent_id: Some(fallback_agent_id.to_string()),
            })
        }

        LlmError::MalformedResponse { agent_id, raw: _ } => {
            SimMessage::Warning(WarningPayload::InvalidAction {
                agent_id: agent_id.clone(),
                reason: "malformed response".to_string(),
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Catches the "added an LlmError variant without updating the
    /// mapping" regression at compile time. Each arm here must map
    /// to a variant. If a new variant is added to LlmError without
    /// a corresponding arm here, compilation fails on this match.
    fn variant_name(err: &LlmError) -> &'static str {
        match err {
            LlmError::NotAuthenticated => "NotAuthenticated",
            LlmError::SubprocessDied { .. } => "SubprocessDied",
            LlmError::Refused { .. } => "Refused",
            LlmError::Timeout { .. } => "Timeout",
            LlmError::RateLimited { .. } => "RateLimited",
            LlmError::MalformedResponse { .. } => "MalformedResponse",
            LlmError::Disconnected => "Disconnected",
        }
    }

    /// All seven LlmError variants enumerated with canonical sample
    /// values. The `MUST equal LLM_ERROR_VARIANT_COUNT` assertion in
    /// `variant_catalogue_has_every_variant` ensures forgetting to
    /// add a variant here is caught at runtime.
    const LLM_ERROR_VARIANT_COUNT: usize = 7;

    fn every_llm_error() -> Vec<LlmError> {
        vec![
            LlmError::NotAuthenticated,
            LlmError::SubprocessDied { code: Some(137) },
            LlmError::Refused {
                agent_id: "agent-a".into(),
                message: "i cannot do that dave".into(),
            },
            LlmError::Timeout {
                agent_id: "agent-b".into(),
                elapsed_ms: 60_000,
            },
            LlmError::RateLimited {
                retry_after_ms: 5_000,
            },
            LlmError::MalformedResponse {
                agent_id: "agent-c".into(),
                raw: r#"{"this is": "not valid"}"#.into(),
            },
            LlmError::Disconnected,
        ]
    }

    #[test]
    fn variant_catalogue_has_every_variant() {
        let v = every_llm_error();
        assert_eq!(
            v.len(),
            LLM_ERROR_VARIANT_COUNT,
            "every_llm_error() has {} entries but LLM_ERROR_VARIANT_COUNT = {}. \
             If you added an LlmError variant, update BOTH the constant AND every_llm_error() \
             AND add an arm to llm_error_to_message.",
            v.len(),
            LLM_ERROR_VARIANT_COUNT,
        );
        // Defensive: no duplicates.
        let mut seen: Vec<&'static str> = Vec::new();
        for e in &v {
            let n = variant_name(e);
            assert!(!seen.contains(&n), "duplicate variant in catalogue: {n}");
            seen.push(n);
        }
    }

    #[test]
    fn all_variants_have_a_message_mapping() {
        // Just calling llm_error_to_message for every variant verifies
        // the mapping doesn't panic. The per-variant tests below check
        // the actual mapped value.
        for err in every_llm_error() {
            let _msg = llm_error_to_message(&err, "fallback-agent", 0);
        }
    }

    // ---- Per-variant assertion tests ----

    #[test]
    fn not_authenticated_maps_to_fault_agent_crashed() {
        let msg = llm_error_to_message(&LlmError::NotAuthenticated, "fallback-agent", 0);
        match msg {
            SimMessage::Fault(FaultPayload::AgentCrashed { agent_id, message }) => {
                assert_eq!(agent_id, "fallback-agent");
                assert!(
                    message.contains("not authenticated"),
                    "message should describe the auth failure; got: {message}"
                );
            }
            other => panic!("expected Fault::AgentCrashed, got {other:?}"),
        }
    }

    #[test]
    fn subprocess_died_with_code_includes_code_in_message() {
        let msg = llm_error_to_message(
            &LlmError::SubprocessDied { code: Some(137) },
            "fallback-agent",
            0,
        );
        match msg {
            SimMessage::Fault(FaultPayload::AgentCrashed { agent_id, message }) => {
                assert_eq!(agent_id, "fallback-agent");
                assert!(
                    message.contains("137"),
                    "message should include exit code; got: {message}"
                );
                assert!(
                    message.contains("subprocess died"),
                    "message should mention subprocess; got: {message}"
                );
            }
            other => panic!("expected Fault::AgentCrashed, got {other:?}"),
        }
    }

    #[test]
    fn subprocess_died_without_code_handles_none() {
        let msg = llm_error_to_message(
            &LlmError::SubprocessDied { code: None },
            "fallback-agent",
            0,
        );
        match msg {
            SimMessage::Fault(FaultPayload::AgentCrashed { agent_id, message }) => {
                assert_eq!(agent_id, "fallback-agent");
                assert!(
                    message.contains("no exit code") || message.contains("subprocess died"),
                    "message should handle no-code case; got: {message}"
                );
            }
            other => panic!("expected Fault::AgentCrashed, got {other:?}"),
        }
    }

    #[test]
    fn refused_maps_to_warning_invalid_action_with_agent_id() {
        let err = LlmError::Refused {
            agent_id: "metro-pulse-llm".into(),
            message: "i cannot perform that action".into(),
        };
        let msg = llm_error_to_message(&err, "fallback-agent", 0);
        match msg {
            SimMessage::Warning(WarningPayload::InvalidAction { agent_id, reason }) => {
                // Refused carries its own agent_id; fallback NOT used.
                assert_eq!(agent_id, "metro-pulse-llm");
                assert!(
                    reason.starts_with("refused:"),
                    "reason should be prefixed with refused:; got: {reason}"
                );
                assert!(
                    reason.contains("cannot perform"),
                    "reason should include the original message; got: {reason}"
                );
            }
            other => panic!("expected Warning::InvalidAction, got {other:?}"),
        }
    }

    #[test]
    fn timeout_maps_to_warning_behind_with_agent_id_and_lag_frames() {
        let err = LlmError::Timeout {
            agent_id: "metro-pulse-llm".into(),
            elapsed_ms: 60_000,
        };
        let msg = llm_error_to_message(&err, "fallback-agent", 12);
        match msg {
            SimMessage::Warning(WarningPayload::Behind {
                lag_frames,
                agent_id,
            }) => {
                assert_eq!(lag_frames, 12);
                assert_eq!(agent_id, Some("metro-pulse-llm".to_string()));
            }
            other => panic!("expected Warning::Behind, got {other:?}"),
        }
    }

    #[test]
    fn rate_limited_maps_to_warning_behind_with_fallback_agent_id() {
        let err = LlmError::RateLimited {
            retry_after_ms: 5_000,
        };
        let msg = llm_error_to_message(&err, "metro-pulse-llm", 3);
        match msg {
            SimMessage::Warning(WarningPayload::Behind {
                lag_frames,
                agent_id,
            }) => {
                assert_eq!(lag_frames, 3);
                // RateLimited doesn't carry agent_id, so fallback is used.
                assert_eq!(agent_id, Some("metro-pulse-llm".to_string()));
            }
            other => panic!("expected Warning::Behind, got {other:?}"),
        }
    }

    #[test]
    fn malformed_response_maps_to_warning_invalid_action_without_leaking_raw() {
        let raw_with_secret = r#"{"chosen": "ghp_realtokenpattern1234567890123456789012345"}"#;
        let err = LlmError::MalformedResponse {
            agent_id: "metro-pulse-llm".into(),
            raw: raw_with_secret.into(),
        };
        let msg = llm_error_to_message(&err, "fallback-agent", 0);
        match msg {
            SimMessage::Warning(WarningPayload::InvalidAction { agent_id, reason }) => {
                assert_eq!(agent_id, "metro-pulse-llm");
                assert!(
                    reason.contains("malformed response"),
                    "reason should mention malformed; got: {reason}"
                );
                // Critical: do NOT leak the raw response body into the
                // user-visible Warning. The raw bytes should only flow
                // to the AgentLog write-path, which has redaction.
                assert!(
                    !reason.contains("ghp_"),
                    "reason MUST NOT include the raw response body \
                     (raw can carry secrets per p2a-security-threat-model.md §5.3); \
                     got reason: {reason}"
                );
            }
            other => panic!("expected Warning::InvalidAction, got {other:?}"),
        }
    }

    #[test]
    fn disconnected_maps_to_fault_agent_crashed_with_stdio_message() {
        let msg = llm_error_to_message(&LlmError::Disconnected, "metro-pulse-llm", 0);
        match msg {
            SimMessage::Fault(FaultPayload::AgentCrashed { agent_id, message }) => {
                assert_eq!(agent_id, "metro-pulse-llm");
                assert!(
                    message.to_lowercase().contains("disconnect") || message.contains("stdio"),
                    "message should mention disconnection/stdio; got: {message}"
                );
            }
            other => panic!("expected Fault::AgentCrashed, got {other:?}"),
        }
    }

    // ---- Cross-cutting properties ----

    #[test]
    fn no_mapping_panics_on_any_variant() {
        for err in every_llm_error() {
            let msg = llm_error_to_message(&err, "fallback", 1);
            // Sanity check: serialize the resulting SimMessage to JSON
            // to ensure it round-trips through serde without panicking.
            let json = serde_json::to_string(&msg).expect("SimMessage must serialize");
            assert!(!json.is_empty(), "serialized message empty for {err:?}");
        }
    }

    #[test]
    fn fault_variants_never_use_a_warning_payload() {
        for err in [
            LlmError::NotAuthenticated,
            LlmError::SubprocessDied { code: Some(1) },
            LlmError::Disconnected,
        ] {
            let msg = llm_error_to_message(&err, "x", 0);
            assert!(
                matches!(msg, SimMessage::Fault(_)),
                "variant {err:?} should produce a Fault, not {msg:?}"
            );
        }
    }

    #[test]
    fn warning_variants_never_use_a_fault_payload() {
        for err in [
            LlmError::Refused {
                agent_id: "a".into(),
                message: "x".into(),
            },
            LlmError::Timeout {
                agent_id: "a".into(),
                elapsed_ms: 1,
            },
            LlmError::RateLimited { retry_after_ms: 1 },
            LlmError::MalformedResponse {
                agent_id: "a".into(),
                raw: "x".into(),
            },
        ] {
            let msg = llm_error_to_message(&err, "x", 0);
            assert!(
                matches!(msg, SimMessage::Warning(_)),
                "variant {err:?} should produce a Warning, not {msg:?}"
            );
        }
    }

    /// Closes PR #11 R1 MEDIUM: `Warning::Behind { lag_frames: 0 }`
    /// semantically means "on time", which contradicts the error.
    /// Verify the mapping clamps to a minimum of 1 for Timeout +
    /// RateLimited so the resulting Warning is never self-contradictory.
    #[test]
    fn timeout_clamps_lag_frames_to_minimum_of_one() {
        let err = LlmError::Timeout {
            agent_id: "agent".into(),
            elapsed_ms: 100,
        };
        let msg = llm_error_to_message(&err, "fallback", 0);
        match msg {
            SimMessage::Warning(WarningPayload::Behind { lag_frames, .. }) => {
                assert!(
                    lag_frames >= 1,
                    "Timeout must produce Behind with lag_frames >= 1, got {lag_frames}"
                );
            }
            other => panic!("expected Warning::Behind, got {other:?}"),
        }
    }

    #[test]
    fn rate_limited_clamps_lag_frames_to_minimum_of_one() {
        let err = LlmError::RateLimited { retry_after_ms: 50 };
        let msg = llm_error_to_message(&err, "agent", 0);
        match msg {
            SimMessage::Warning(WarningPayload::Behind { lag_frames, .. }) => {
                assert!(
                    lag_frames >= 1,
                    "RateLimited must produce Behind with lag_frames >= 1, got {lag_frames}"
                );
            }
            other => panic!("expected Warning::Behind, got {other:?}"),
        }
    }

    #[test]
    fn timeout_preserves_lag_frames_when_already_nonzero() {
        let err = LlmError::Timeout {
            agent_id: "agent".into(),
            elapsed_ms: 60_000,
        };
        let msg = llm_error_to_message(&err, "fallback", 42);
        match msg {
            SimMessage::Warning(WarningPayload::Behind { lag_frames, .. }) => {
                assert_eq!(
                    lag_frames, 42,
                    "non-zero caller-supplied lag_frames must pass through unmodified"
                );
            }
            other => panic!("expected Warning::Behind, got {other:?}"),
        }
    }
}
