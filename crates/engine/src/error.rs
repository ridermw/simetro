//! Typed error enums for engine subsystems.
//!
//! Per plan Issue 3A: no `anyhow` in core. Every fallible path returns
//! a specific error type so callers can pattern-match and surface
//! typed events to the UI.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("JSON parse error at line {line}, col {col}: {message}")]
    Parse { line: u32, col: u32, message: String },

    #[error("unsupported schema_version: found {found}, supported {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    // Full variant set is filled in at Step 6.
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("agent {agent_id} panicked: {message}")]
    Panicked { agent_id: String, message: String },

    #[error("agent {agent_id} returned invalid action: {reason}")]
    InvalidAction { agent_id: String, reason: String },

    #[error("agent {agent_id} exceeded budget {budget_ms}ms")]
    Timeout { agent_id: String, budget_ms: u32 },
}

#[derive(Debug, Error)]
pub enum EngineFault {
    #[error("numeric drift at tick {tick} (mover {mover})")]
    NumericDrift { tick: u64, mover: u32 },

    #[error("baseline hash mismatch: expected {expected}, found {found}")]
    BaselineHashMismatch { expected: String, found: String },

    #[error("snapshot channel saturated; behind {lag_frames} frames")]
    ChannelSaturated { lag_frames: u32 },

    #[error("system '{system}' panicked: {message}")]
    SystemPanic {
        system: &'static str,
        message: String,
    },
}
