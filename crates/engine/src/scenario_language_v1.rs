//! `scenario_language_v1` (SL1) skeleton.
//!
//! This module establishes the shape of the SL1 grammar — places,
//! links, things, transforms, demand, pressure, objectives,
//! failure_conditions, agents, observability, and milestones — without
//! yet implementing any behavior. Each subsequent PR replaces one
//! primitive's empty placeholder with concrete fields, validation, and
//! engine systems.
//!
//! The SL1 block is **strict-schema** in two complementary ways:
//!
//! 1. Every behavior-bearing struct carries
//!    `#[serde(deny_unknown_fields)]`, so a typo or a field a future
//!    PR has not yet wired in produces [`Sl1LoadError::UnknownField`]
//!    rather than silently no-op-ing.
//! 2. In PR 0, every grammar primitive is still a placeholder. To make
//!    sure proto-SL1 scenes can never load and silently no-op, the
//!    validator rejects non-empty `places`/`links`/`things`/`transforms`
//!    /`demand`/`pressure`/`objectives`/`failure_conditions`/`agents`/
//!    `milestones` with [`Sl1LoadError::PrimitiveNotImplemented`].
//!    Each later PR removes its primitive from that guard as it lands.
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
/// from [`validate`] or [`load_str`].
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

    /// A non-`unknown field` serde failure (type mismatch, malformed
    /// JSON, etc.) inside the SL1 block. Distinct from
    /// [`Self::UnknownField`] so tooling can render the two
    /// differently.
    #[error("scenario_language_v1: parse error: {message}")]
    Parse { message: String },

    /// A behavior-bearing grammar primitive (`places`, `links`,
    /// `things`, `transforms`, `demand`, `pressure`, `objectives`,
    /// `failure_conditions`, `agents`, `milestones`) is present but
    /// the matching PR has not yet implemented its semantics in this
    /// build. Surfaces as a typed load error rather than letting a
    /// proto-SL1 scene silently no-op.
    ///
    /// Each later PR removes its section from this guard.
    #[error(
        "scenario_language_v1.{section}: primitive not yet implemented in this build; \
         the matching PR has not landed"
    )]
    PrimitiveNotImplemented { section: &'static str },
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
    /// construct a sample value. Hidden from rustdoc because no caller
    /// should match on it semantically — real variants land in later
    /// PRs and external matches should use `_`.
    #[doc(hidden)]
    #[error("scenario_language_v1 warning (reserved): {0}")]
    __Reserved(String),
}

/// Fatal SL1 engine faults. Populated in later PRs. PR 0 emits none.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum Sl1Fault {
    /// Reserved placeholder so the enum is inhabited. Hidden from
    /// rustdoc because no caller should match on it semantically —
    /// real variants land in later PRs and external matches should
    /// use `_`.
    #[doc(hidden)]
    #[error("scenario_language_v1 fault (reserved): {0}")]
    __Reserved(String),
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
/// PR 0 enforces:
/// - `schema_version` must equal [`SL1_SCHEMA_VERSION`].
/// - All grammar primitives must be empty —
///   [`Sl1LoadError::PrimitiveNotImplemented`] for any
///   `places`/`links`/`things`/`transforms`/`demand`/`pressure`/
///   `objectives`/`failure_conditions`/`agents`/`milestones` with
///   entries, since this build cannot give them behavior.
/// - The optional `observability` block may be present but empty.
///
/// Strict-schema rejection of unknown fields happens at the serde
/// layer via [`load_str`] / [`load_value`] (or by the surrounding scene
/// loader when this block is embedded in a larger scene).
///
/// # Errors
/// Returns [`Sl1LoadError::UnsupportedSchema`] when the SL1 block's
/// `schema_version` is outside the supported range, or
/// [`Sl1LoadError::PrimitiveNotImplemented`] when a primitive is
/// non-empty.
pub fn validate(raw: RawSl1Scene) -> Result<Sl1Scene, Sl1LoadError> {
    if raw.schema_version != SL1_SCHEMA_VERSION {
        return Err(Sl1LoadError::UnsupportedSchema {
            found: raw.schema_version,
            supported: SL1_SCHEMA_VERSION,
        });
    }

    // PR 0 has no behavior for any primitive. Reject non-empty sections
    // so a proto-SL1 scene can't silently no-op while developers wait
    // for PRs 1–11.
    macro_rules! reject_non_empty {
        ($vec:expr, $name:literal) => {
            if !$vec.is_empty() {
                return Err(Sl1LoadError::PrimitiveNotImplemented { section: $name });
            }
        };
    }
    reject_non_empty!(raw.places, "places");
    reject_non_empty!(raw.links, "links");
    reject_non_empty!(raw.things, "things");
    reject_non_empty!(raw.transforms, "transforms");
    reject_non_empty!(raw.demand, "demand");
    reject_non_empty!(raw.pressure, "pressure");
    reject_non_empty!(raw.objectives, "objectives");
    reject_non_empty!(raw.failure_conditions, "failure_conditions");
    reject_non_empty!(raw.agents, "agents");
    reject_non_empty!(raw.milestones, "milestones");

    Ok(Sl1Scene {
        schema_version: raw.schema_version,
        places: Vec::new(),
        links: Vec::new(),
        things: Vec::new(),
        transforms: Vec::new(),
        demand: Vec::new(),
        pressure: Vec::new(),
        objectives: Vec::new(),
        failure_conditions: Vec::new(),
        agents: Vec::new(),
        observability: raw.observability.map(|_| Sl1Observability),
        milestones: Vec::new(),
    })
}

/// Parse + validate a standalone SL1 block from a `serde_json::Value`.
///
/// Used by the surrounding scene loader so that unknown-field rejection
/// from the SL1 block becomes a typed [`Sl1LoadError::UnknownField`]
/// instead of being swallowed into the outer scene's parse error.
///
/// # Errors
/// Returns [`Sl1LoadError::UnknownField`] for any unknown
/// behavior-bearing field, [`Sl1LoadError::Parse`] for a type-mismatch
/// or malformed-shape error, or [`Sl1LoadError::UnsupportedSchema`] for
/// an out-of-range `schema_version`.
pub fn load_value(value: serde_json::Value) -> Result<Sl1Scene, Sl1LoadError> {
    let raw: RawSl1Scene = serde_json::from_value(value).map_err(serde_error_to_sl1)?;
    validate(raw)
}

/// Parse + validate a standalone SL1 block from JSON.
///
/// Used by tests and tooling that want to load an SL1 fragment outside
/// of a full simetro scene. The surrounding scene loader wires the SL1
/// block into the larger `RawScene` flow inside `crate::loader` via
/// [`load_value`].
///
/// # Errors
/// Same as [`load_value`].
pub fn load_str(json: &str) -> Result<Sl1Scene, Sl1LoadError> {
    let raw: RawSl1Scene = serde_json::from_str(json).map_err(serde_error_to_sl1)?;
    validate(raw)
}

/// Classify a serde error from an SL1 block into a typed
/// [`Sl1LoadError`]. Unknown-field rejections become
/// [`Sl1LoadError::UnknownField`] so the renderer can highlight the
/// offending key; everything else becomes [`Sl1LoadError::Parse`].
fn serde_error_to_sl1(e: serde_json::Error) -> Sl1LoadError {
    let message = e.to_string();
    if message.starts_with("unknown field") {
        Sl1LoadError::UnknownField { field: message }
    } else {
        Sl1LoadError::Parse { message }
    }
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
    fn non_empty_primitive_rejected_until_pr_lands() {
        // PR 0 has no behavior for any grammar primitive — even a
        // perfectly-shaped (empty) entry must fail load, otherwise a
        // proto-SL1 scene would silently no-op.
        for (json, expected_section) in [
            (r#"{"places": [{}]}"#, "places"),
            (r#"{"links": [{}]}"#, "links"),
            (r#"{"things": [{}]}"#, "things"),
            (r#"{"transforms": [{}]}"#, "transforms"),
            (r#"{"demand": [{}]}"#, "demand"),
            (r#"{"pressure": [{}]}"#, "pressure"),
            (r#"{"objectives": [{}]}"#, "objectives"),
            (r#"{"failure_conditions": [{}]}"#, "failure_conditions"),
            (r#"{"agents": [{}]}"#, "agents"),
            (r#"{"milestones": [{}]}"#, "milestones"),
        ] {
            let err = load_str(json).unwrap_err();
            match err {
                Sl1LoadError::PrimitiveNotImplemented { section } => {
                    assert_eq!(section, expected_section, "json was {json}");
                }
                other => panic!("expected PrimitiveNotImplemented for {json}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_error_distinguished_from_unknown_field() {
        // A type mismatch (places must be a Vec, not a string) is a
        // Parse error, not an UnknownField.
        let err = load_str(r#"{"places": "not a list"}"#).unwrap_err();
        match err {
            Sl1LoadError::Parse { message } => {
                assert!(
                    !message.starts_with("unknown field"),
                    "type mismatch should not be classified as unknown field: {message}"
                );
            }
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn load_value_typed_unknown_field_error() {
        let v: serde_json::Value = serde_json::from_str(r#"{"mystery": 1}"#).unwrap();
        let err = load_value(v).unwrap_err();
        assert!(matches!(err, Sl1LoadError::UnknownField { .. }));
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
