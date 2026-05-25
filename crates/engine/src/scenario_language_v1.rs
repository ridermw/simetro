//! `scenario_language_v1` (SL1) skeleton.
//!
//! This module establishes the shape of the SL1 grammar — places,
//! links, things, transforms, demand, pressure, objectives,
//! failure_conditions, agents, observability, and milestones — without
//! yet implementing any behavior. Each subsequent PR replaces one
//! primitive's empty placeholder with concrete fields, validation, and
//! engine systems.
//!
//! The SL1 block is **strict-schema**: every behavior-bearing struct
//! is annotated with `#[serde(deny_unknown_fields)]`, so a typo or a
//! field that a future PR has not yet wired in produces a typed
//! [`Sl1LoadError::UnknownField`] rather than silently no-op-ing.
//!
//! See `docs/scenario-language-v1.md` and the canonical roadmap spec
//! at `docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`
//! for the full grammar.

use serde::Deserialize;
use thiserror::Error;

/// Schema version for the SL1 block itself. Independent of the
/// surrounding scene's `schema_version`, so legacy v1/v2 scenes can
/// adopt SL1 incrementally without bumping their top-level version.
pub const SL1_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Raw (post-serde, pre-validation) SL1 scene block.
// ---------------------------------------------------------------------------

/// Raw SL1 scene block. Strict-schema: unknown fields are a load error.
///
/// Each section defaults to an empty `Vec` so PR 0 can introduce the
/// block without requiring authors to fill in every primitive. Later
/// PRs populate the per-primitive struct definitions.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Scene {
    /// Defaults to [`SL1_SCHEMA_VERSION`] when omitted.
    #[serde(default = "default_sl1_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub places: Vec<RawSl1Place>,
    #[serde(default)]
    pub links: Vec<RawSl1Link>,
    #[serde(default)]
    pub things: Vec<RawSl1Thing>,
    #[serde(default)]
    pub transforms: Vec<RawSl1Transform>,
    #[serde(default)]
    pub demand: Vec<RawSl1Demand>,
    #[serde(default)]
    pub pressure: Vec<RawSl1Pressure>,
    #[serde(default)]
    pub objectives: Vec<RawSl1Objective>,
    #[serde(default)]
    pub failure_conditions: Vec<RawSl1FailureCondition>,
    #[serde(default)]
    pub agents: Vec<RawSl1Agent>,
    #[serde(default)]
    pub observability: Option<RawSl1Observability>,
    #[serde(default)]
    pub milestones: Vec<RawSl1Milestone>,
    /// Permissive catalog/theme/metadata block. Unknown fields here are
    /// allowed because catalog entries are author-facing free-form data
    /// (titles, descriptions, palette notes). Behavior-bearing fields
    /// live outside `catalog`.
    #[serde(default)]
    pub catalog: Option<serde_json::Value>,
}

fn default_sl1_schema_version() -> u32 {
    SL1_SCHEMA_VERSION
}

/// Placeholder for a `place`. PR 1 populates this struct with id, role,
/// position, capacity, storage, accepts, produces, failure_domains, and
/// operating_states.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Place {}

/// Placeholder for a `link`. PR 2 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Link {}

/// Placeholder for a `thing`. PR 3 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Thing {}

/// Placeholder for a `transform`. PR 4 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Transform {}

/// Placeholder for a `demand`. PR 5 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Demand {}

/// Placeholder for a `pressure`. PR 7 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Pressure {}

/// Placeholder for an `objective`. PR 8 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Objective {}

/// Placeholder for a `failure_condition`. PR 8 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1FailureCondition {}

/// Placeholder for an `agent`. PR 10 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Agent {}

/// Placeholder for the `observability` block. PR 9 populates this
/// struct with `metrics`, `dashboards`, and `alerts`.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Observability {}

/// Placeholder for a `milestone`. PR 11 populates this struct.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Milestone {}

// ---------------------------------------------------------------------------
// Loaded (validated, engine-facing) SL1 scene.
// ---------------------------------------------------------------------------

/// Validated SL1 scene. PR 0 carries only the validated `schema_version`
/// plus empty vectors; each later PR populates the corresponding
/// primitive's data, plus stable id maps and engine state where
/// appropriate.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Scene {
    pub schema_version: u32,
    pub places: Vec<Sl1Place>,
    pub links: Vec<Sl1Link>,
    pub things: Vec<Sl1Thing>,
    pub transforms: Vec<Sl1Transform>,
    pub demand: Vec<Sl1Demand>,
    pub pressure: Vec<Sl1Pressure>,
    pub objectives: Vec<Sl1Objective>,
    pub failure_conditions: Vec<Sl1FailureCondition>,
    pub agents: Vec<Sl1Agent>,
    pub observability: Option<Sl1Observability>,
    pub milestones: Vec<Sl1Milestone>,
}

/// Placeholder loaded `place`. Populated in PR 1.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Place;

/// Placeholder loaded `link`. Populated in PR 2.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Link;

/// Placeholder loaded `thing`. Populated in PR 3.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Thing;

/// Placeholder loaded `transform`. Populated in PR 4.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Transform;

/// Placeholder loaded `demand`. Populated in PR 5.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Demand;

/// Placeholder loaded `pressure`. Populated in PR 7.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Pressure;

/// Placeholder loaded `objective`. Populated in PR 8.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Objective;

/// Placeholder loaded `failure_condition`. Populated in PR 8.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1FailureCondition;

/// Placeholder loaded `agent`. Populated in PR 10.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Agent;

/// Placeholder loaded `observability`. Populated in PR 9.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Observability;

/// Placeholder loaded `milestone`. Populated in PR 11.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Milestone;

// ---------------------------------------------------------------------------
// Error / warning / fault / outcome taxonomy.
// ---------------------------------------------------------------------------

/// Errors raised while loading the SL1 block. Each variant is reachable
/// from [`validate`].
///
/// `#[non_exhaustive]` because later PRs add variants as each primitive
/// gains real validation rules; downstream pattern matches must use
/// `_` to remain forward-compatible.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum Sl1LoadError {
    #[error("scenario_language_v1.schema_version: found {found}, supported {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },

    /// Surfaces a serde `unknown field` error from any strict-schema
    /// SL1 struct. `field` carries the original serde message verbatim
    /// so the renderer can point at the offending location.
    #[error("scenario_language_v1: unknown field: {field}")]
    UnknownField { field: String },
}

/// Non-fatal SL1 conditions surfaced to the UI. Populated in later PRs
/// (transform starved/blocked/late, demand dropped, dashboard stale,
/// invalid agent action, etc.). PR 0 emits none.
///
/// `#[non_exhaustive]` because later PRs add variants.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum Sl1Warning {
    /// Reserved placeholder so the enum is inhabited and so tests can
    /// construct a sample value. Real variants land in later PRs.
    #[error("scenario_language_v1 warning (reserved): {0}")]
    Reserved(String),
}

/// Fatal SL1 engine faults. Populated in later PRs. PR 0 emits none.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum Sl1Fault {
    /// Reserved placeholder so the enum is inhabited. Real variants
    /// land in later PRs.
    #[error("scenario_language_v1 fault (reserved): {0}")]
    Reserved(String),
}

/// Terminal outcome of an SL1 scenario. Real evaluation lands in PR 8;
/// PR 0 always reports [`GameOutcome::InProgress`].
///
/// Once a scenario transitions to [`GameOutcome::Won`] or
/// [`GameOutcome::Lost`], the outcome is sticky for the remainder of
/// the run.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum GameOutcome {
    #[default]
    InProgress,
    Won,
    Lost {
        reason: String,
    },
}

impl GameOutcome {
    /// True if the outcome is terminal (`Won` or `Lost`).
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, GameOutcome::Won | GameOutcome::Lost { .. })
    }
}

// ---------------------------------------------------------------------------
// Validation.
// ---------------------------------------------------------------------------

/// Validate a parsed [`RawSl1Scene`] into a [`Sl1Scene`].
///
/// PR 0 only enforces `schema_version`; later PRs add per-primitive
/// rules. The strict-schema rejection of unknown fields happens at the
/// serde layer via [`load_str`] (or by the surrounding scene loader
/// when this block is embedded in a larger scene).
///
/// # Errors
/// Returns [`Sl1LoadError::UnsupportedSchema`] when the SL1 block's
/// `schema_version` is outside the supported range.
pub fn validate(raw: RawSl1Scene) -> Result<Sl1Scene, Sl1LoadError> {
    if raw.schema_version != SL1_SCHEMA_VERSION {
        return Err(Sl1LoadError::UnsupportedSchema {
            found: raw.schema_version,
            supported: SL1_SCHEMA_VERSION,
        });
    }

    // Future PRs populate these conversions; PR 0 carries empty Vecs.
    Ok(Sl1Scene {
        schema_version: raw.schema_version,
        places: raw.places.into_iter().map(|_| Sl1Place).collect(),
        links: raw.links.into_iter().map(|_| Sl1Link).collect(),
        things: raw.things.into_iter().map(|_| Sl1Thing).collect(),
        transforms: raw.transforms.into_iter().map(|_| Sl1Transform).collect(),
        demand: raw.demand.into_iter().map(|_| Sl1Demand).collect(),
        pressure: raw.pressure.into_iter().map(|_| Sl1Pressure).collect(),
        objectives: raw.objectives.into_iter().map(|_| Sl1Objective).collect(),
        failure_conditions: raw
            .failure_conditions
            .into_iter()
            .map(|_| Sl1FailureCondition)
            .collect(),
        agents: raw.agents.into_iter().map(|_| Sl1Agent).collect(),
        observability: raw.observability.map(|_| Sl1Observability),
        milestones: raw.milestones.into_iter().map(|_| Sl1Milestone).collect(),
    })
}

/// Parse + validate a standalone SL1 block from JSON.
///
/// Used by tests and tooling that want to load an SL1 fragment outside
/// of a full simetro scene. The surrounding scene loader wires the SL1
/// block into the larger `RawScene` flow inside `crate::loader`.
///
/// # Errors
/// Returns [`Sl1LoadError::UnknownField`] for any unknown
/// behavior-bearing field, or [`Sl1LoadError::UnsupportedSchema`] for
/// an out-of-range `schema_version`.
pub fn load_str(json: &str) -> Result<Sl1Scene, Sl1LoadError> {
    let raw: RawSl1Scene = serde_json::from_str(json).map_err(|e| {
        // Translate serde's "unknown field" into a typed Sl1LoadError;
        // surface other parse errors verbatim under the same variant
        // so callers see a single failure surface.
        Sl1LoadError::UnknownField {
            field: e.to_string(),
        }
    })?;
    validate(raw)
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn empty_block_loads() {
        let scene = load_str("{}").expect("empty SL1 block should load");
        assert_eq!(scene.schema_version, SL1_SCHEMA_VERSION);
        assert!(scene.places.is_empty());
        assert!(scene.links.is_empty());
        assert!(scene.things.is_empty());
        assert!(scene.transforms.is_empty());
        assert!(scene.demand.is_empty());
        assert!(scene.pressure.is_empty());
        assert!(scene.objectives.is_empty());
        assert!(scene.failure_conditions.is_empty());
        assert!(scene.agents.is_empty());
        assert!(scene.observability.is_none());
        assert!(scene.milestones.is_empty());
    }

    #[test]
    fn explicit_schema_version_one_loads() {
        let scene = load_str(r#"{"schema_version": 1}"#).expect("v1 SL1 block should load");
        assert_eq!(scene.schema_version, 1);
    }

    #[test]
    fn unsupported_schema_version_rejected() {
        let err = load_str(r#"{"schema_version": 99}"#).unwrap_err();
        match err {
            Sl1LoadError::UnsupportedSchema { found, supported } => {
                assert_eq!(found, 99);
                assert_eq!(supported, SL1_SCHEMA_VERSION);
            }
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
    }

    #[test]
    fn unknown_top_level_field_rejected() {
        let err = load_str(r#"{"mystery": 42}"#).unwrap_err();
        match err {
            Sl1LoadError::UnknownField { field } => {
                assert!(
                    field.contains("mystery"),
                    "expected serde message to name the field, got {field}"
                );
            }
            other => panic!("expected UnknownField, got {other:?}"),
        }
    }

    #[test]
    fn unknown_field_inside_place_rejected() {
        let err = load_str(r#"{"places": [{"id": "p1"}]}"#).unwrap_err();
        match err {
            Sl1LoadError::UnknownField { field } => {
                assert!(field.contains("id"), "got {field}");
            }
            other => panic!("expected UnknownField, got {other:?}"),
        }
    }

    #[test]
    fn known_empty_sections_load() {
        let json = r#"{
            "schema_version": 1,
            "places": [],
            "links": [],
            "things": [],
            "transforms": [],
            "demand": [],
            "pressure": [],
            "objectives": [],
            "failure_conditions": [],
            "agents": [],
            "milestones": [],
            "catalog": {"title": "demo"}
        }"#;
        let scene = load_str(json).expect("explicit empties should load");
        assert!(scene.places.is_empty());
        assert!(scene.observability.is_none());
    }

    #[test]
    fn catalog_is_permissive_and_ignored() {
        // catalog accepts arbitrary metadata without exploding.
        let json = r#"{
            "catalog": {"any": "thing", "nested": {"x": 1}}
        }"#;
        let scene = load_str(json).expect("catalog should accept free-form data");
        assert_eq!(scene.schema_version, SL1_SCHEMA_VERSION);
    }

    #[test]
    fn empty_observability_loads() {
        let json = r#"{"observability": {}}"#;
        let scene = load_str(json).expect("empty observability should load");
        assert!(scene.observability.is_some());
    }

    #[test]
    fn unknown_field_inside_observability_rejected() {
        let err = load_str(r#"{"observability": {"alerts": []}}"#).unwrap_err();
        assert!(matches!(err, Sl1LoadError::UnknownField { .. }));
    }

    #[test]
    fn game_outcome_default_is_in_progress() {
        let g = GameOutcome::default();
        assert_eq!(g, GameOutcome::InProgress);
        assert!(!g.is_terminal());
    }

    #[test]
    fn game_outcome_terminal_states_are_sticky_signal() {
        assert!(GameOutcome::Won.is_terminal());
        assert!(GameOutcome::Lost {
            reason: "test".to_string()
        }
        .is_terminal());
    }
}
