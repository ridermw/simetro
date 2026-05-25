//! Typed error enums for engine subsystems.
//!
//! Per plan Issue 3A: no `anyhow` in core. Every fallible path returns
//! a specific error type so callers can pattern-match and surface
//! typed events to the UI.

use thiserror::Error;

/// Errors produced by the JSON scene loader. scene loader contract + §11.1.
///
/// Every variant has a typed field-path or section/id so the renderer
/// can place the error in the canvas overlay at the right spot.
#[derive(Debug, Error, PartialEq)]
pub enum LoadError {
    #[error("JSON parse error at line {line}, col {col}: {message}")]
    Parse {
        line: u32,
        col: u32,
        message: String,
    },

    #[error("unsupported schema_version: found {found}, supported {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },

    #[error("invalid scene name: {reason}")]
    InvalidName { reason: String },

    #[error("theme.palette has {size} entries (max {max})")]
    PaletteTooLarge { size: usize, max: usize },

    #[error("invalid color at {field}: {value}")]
    InvalidColor { field: String, value: String },

    #[error("{field} = {index} but palette has {max} entries")]
    PaletteIndexOOB {
        field: String,
        index: usize,
        max: usize,
    },

    #[error("{section} has {count} entries (max {max})")]
    TooManyPieces {
        section: &'static str,
        count: usize,
        max: usize,
    },

    #[error("duplicate id `{id}` in {section}")]
    DuplicateId { section: &'static str, id: String },

    #[error("invalid id `{id}` in {section}: ids must match [A-Za-z0-9_-]+ and be ≤64 chars")]
    InvalidId { section: &'static str, id: String },

    #[error("coordinate for `{id}` is non-finite or out of range")]
    NonFiniteCoord { id: String },

    #[error("speed for mover `{id}` = {value} is out of range (0..=100)")]
    SpeedOutOfRange { id: String, value: f32 },

    #[error("{section}[{index}].interval_ticks = {value} is out of range (1..=10_000)")]
    IntervalOOB {
        section: &'static str,
        index: usize,
        value: u32,
    },

    #[error(
        "{field} = {value} is out of range (0..=1_000_000 for inventory, 1..=1_000_000 otherwise)"
    )]
    AmountOOB { field: &'static str, value: u64 },

    #[error("unknown reference: `{from}` -> `{to}`")]
    UnknownReference { from: String, to: String },

    #[error("missing required field `{field}`")]
    MissingField { field: &'static str },

    #[error("unknown agent kind `{kind}` at agents[{index}]")]
    UnknownAgentKind { index: usize, kind: String },

    #[error(
        "agent kind `{kind}` at agents[{index}] requires building with the `llm-live` feature"
    )]
    AgentKindRequiresFeature { index: usize, kind: String },
}

#[derive(Debug, Error, PartialEq)]
pub enum AgentError {
    #[error("agent {agent_id} panicked: {message}")]
    Panicked { agent_id: String, message: String },

    #[error("agent {agent_id} returned invalid action: {reason}")]
    InvalidAction { agent_id: String, reason: String },

    #[error("agent {agent_id} exceeded budget {budget_ms}ms")]
    Timeout { agent_id: String, budget_ms: u32 },
}

#[derive(Debug, Error, PartialEq)]
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
