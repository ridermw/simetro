//! Mappings from engine error types into wire `FaultPayload` and
//! `WarningPayload` variants (PLAN §11.2).
//!
//! Every fallible code path in the engine is responsible for surfacing
//! a typed message — never a silent log. The conversions live here so
//! callers can pattern-match without duplicating string formatting.

use simetro_protocol::{FaultPayload, SimMessage, WarningPayload};

use crate::error::{AgentError, EngineFault, LoadError};

/// Convert a [`LoadError`] into a wire `Fault::LoadError` payload.
/// `Parse` carries explicit line/col; other variants stringify with
/// `Display`.
#[must_use]
pub fn load_error_to_fault(err: &LoadError) -> FaultPayload {
    match err {
        LoadError::Parse { line, col, message } => FaultPayload::LoadError {
            message: message.clone(),
            line: Some(*line),
            col: Some(*col),
        },
        other => FaultPayload::LoadError {
            message: other.to_string(),
            line: None,
            col: None,
        },
    }
}

/// Convert an [`AgentError`] into either a `Fault` (panic — sim pauses)
/// or a `Warning` (invalid action or timeout — sim continues).
///
/// PLAN §11.2 specifies AgentError::Panicked → Fault::AgentCrashed,
/// AgentError::InvalidAction / Timeout → Warning::InvalidAction.
#[must_use]
pub fn agent_error_to_message(err: &AgentError) -> SimMessage {
    match err {
        AgentError::Panicked { agent_id, message } => {
            SimMessage::Fault(FaultPayload::AgentCrashed {
                agent_id: agent_id.clone(),
                message: message.clone(),
            })
        }
        AgentError::InvalidAction { agent_id, reason } => {
            SimMessage::Warning(WarningPayload::InvalidAction {
                agent_id: agent_id.clone(),
                reason: reason.clone(),
            })
        }
        AgentError::Timeout {
            agent_id,
            budget_ms,
        } => SimMessage::Warning(WarningPayload::InvalidAction {
            agent_id: agent_id.clone(),
            reason: format!("timeout after {budget_ms}ms"),
        }),
    }
}

/// Convert an [`EngineFault`] into the matching wire `FaultPayload`.
#[must_use]
pub fn engine_fault_to_payload(fault: &EngineFault) -> FaultPayload {
    match fault {
        EngineFault::NumericDrift { tick, mover: _ } => FaultPayload::NumericDrift { tick: *tick },
        EngineFault::BaselineHashMismatch { expected, found } => {
            FaultPayload::BaselineHashMismatch {
                expected: expected.clone(),
                found: found.clone(),
            }
        }
        EngineFault::ChannelSaturated { lag_frames: _ } => FaultPayload::EngineFault {
            message: fault.to_string(),
        },
        EngineFault::SystemPanic { system, message } => FaultPayload::EngineFault {
            message: format!("system '{system}' panicked: {message}"),
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_preserves_line_col() {
        let e = LoadError::Parse {
            line: 12,
            col: 3,
            message: "unexpected token".into(),
        };
        match load_error_to_fault(&e) {
            FaultPayload::LoadError {
                line: Some(l),
                col: Some(c),
                message,
            } => {
                assert_eq!(l, 12);
                assert_eq!(c, 3);
                assert!(message.contains("unexpected"));
            }
            other => panic!("unexpected fault: {other:?}"),
        }
    }

    #[test]
    fn non_parse_load_error_has_no_position() {
        let e = LoadError::UnsupportedVersion {
            found: 2,
            supported: 1,
        };
        match load_error_to_fault(&e) {
            FaultPayload::LoadError {
                line: None,
                col: None,
                message,
            } => {
                assert!(message.contains("unsupported"));
            }
            other => panic!("unexpected fault: {other:?}"),
        }
    }

    #[test]
    fn agent_panic_maps_to_fault() {
        let e = AgentError::Panicked {
            agent_id: "speed_tuner_0".into(),
            message: "boom".into(),
        };
        match agent_error_to_message(&e) {
            SimMessage::Fault(FaultPayload::AgentCrashed { agent_id, .. }) => {
                assert_eq!(agent_id, "speed_tuner_0");
            }
            other => panic!("expected Fault::AgentCrashed, got {other:?}"),
        }
    }

    #[test]
    fn invalid_action_maps_to_warning() {
        let e = AgentError::InvalidAction {
            agent_id: "a".into(),
            reason: "unknown mover".into(),
        };
        match agent_error_to_message(&e) {
            SimMessage::Warning(WarningPayload::InvalidAction { reason, .. }) => {
                assert!(reason.contains("unknown"));
            }
            other => panic!("expected Warning::InvalidAction, got {other:?}"),
        }
    }

    #[test]
    fn timeout_maps_to_warning() {
        let e = AgentError::Timeout {
            agent_id: "a".into(),
            budget_ms: 250,
        };
        match agent_error_to_message(&e) {
            SimMessage::Warning(WarningPayload::InvalidAction { reason, .. }) => {
                assert!(reason.contains("timeout") && reason.contains("250"));
            }
            other => panic!("expected Warning, got {other:?}"),
        }
    }

    #[test]
    fn engine_fault_numeric_drift_maps() {
        let f = EngineFault::NumericDrift { tick: 42, mover: 1 };
        match engine_fault_to_payload(&f) {
            FaultPayload::NumericDrift { tick } => assert_eq!(tick, 42),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn engine_fault_system_panic_carries_context() {
        let f = EngineFault::SystemPanic {
            system: "movement",
            message: "index out of range".into(),
        };
        match engine_fault_to_payload(&f) {
            FaultPayload::EngineFault { message } => {
                assert!(message.contains("movement"));
                assert!(message.contains("index"));
            }
            other => panic!("{other:?}"),
        }
    }
}
