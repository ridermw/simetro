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
//! 1. Unknown top-level fields in the SL1 block produce a typed
//!    [`Sl1LoadError::UnknownField`]. The check is programmatic — not a
//!    serde-message heuristic — via a `#[serde(flatten)]` "extra" map
//!    that captures any key not explicitly named on [`RawSl1Scene`].
//! 2. In PR 0, every grammar primitive is still a placeholder. The
//!    primitives are typed as `Vec<serde_json::Value>` so any non-empty
//!    `places`/`links`/`things`/`transforms`/`demand`/`pressure`/
//!    `objectives`/`failure_conditions`/`agents`/`milestones` entry —
//!    well-formed or not — fails load with
//!    [`Sl1LoadError::PrimitiveNotImplemented`]. Each later PR replaces
//!    a primitive's `Vec<Value>` with a strict typed struct and
//!    removes the matching `reject_non_empty!` line.
//!
//! Explicit JSON `null` for the SL1 block is rejected with
//! [`Sl1LoadError::ExpectedObject`] so a scene cannot accidentally
//! bypass SL1 validation by writing `"scenario_language_v1": null`.
//!
//! See `docs/scenario-language-v1.md` and the canonical roadmap spec
//! at `docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`
//! for the full grammar.

use std::collections::BTreeMap;

use serde::Deserialize;
use thiserror::Error;

/// Schema version for the SL1 block itself. Independent of the
/// surrounding scene's `schema_version`, so legacy v1/v2 scenes can
/// adopt SL1 incrementally without bumping their top-level version.
pub const SL1_SCHEMA_VERSION: u32 = 1;
const MAX_SL1_ITEMS_PER_SECTION: usize = 100_000;

// ---------------------------------------------------------------------------
// Raw (post-serde, pre-validation) SL1 scene block.
// ---------------------------------------------------------------------------

/// Raw SL1 scene block. PR 0 deliberately captures every grammar
/// primitive as `Vec<serde_json::Value>` so the validator can reject
/// non-empty entries with [`Sl1LoadError::PrimitiveNotImplemented`]
/// without relying on each placeholder struct's shape. Each later PR
/// replaces the corresponding `Vec<Value>` with a strict typed
/// `Vec<RawSl1Foo>` and removes the matching guard.
///
/// Unknown top-level fields land in [`Self::extra`]; [`validate`]
/// emits a typed [`Sl1LoadError::UnknownField`] for each.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
pub struct RawSl1Scene {
    /// Defaults to [`SL1_SCHEMA_VERSION`] when omitted.
    #[serde(default = "default_sl1_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub places: Vec<serde_json::Value>,
    #[serde(default)]
    pub links: Vec<serde_json::Value>,
    #[serde(default)]
    pub things: Vec<serde_json::Value>,
    #[serde(default)]
    pub transforms: Vec<serde_json::Value>,
    #[serde(default)]
    pub demand: Vec<serde_json::Value>,
    #[serde(default)]
    pub pressure: Vec<serde_json::Value>,
    #[serde(default)]
    pub objectives: Vec<serde_json::Value>,
    #[serde(default)]
    pub failure_conditions: Vec<serde_json::Value>,
    #[serde(default)]
    pub agents: Vec<serde_json::Value>,
    /// Optional `observability` block. PR 0 accepts an omitted block
    /// or an explicit empty object `{}`; an explicit JSON `null` is
    /// treated as equivalent to "omitted" (no observability), matching
    /// the documented example in `docs/scenario-language-v1.md`. Any
    /// non-empty object is rejected with
    /// [`Sl1LoadError::PrimitiveNotImplemented`] until PR 9 adds the
    /// metrics/dashboards/alerts schema; any non-object value is
    /// rejected with [`Sl1LoadError::Parse`].
    #[serde(default)]
    pub observability: Option<serde_json::Value>,
    #[serde(default)]
    pub milestones: Vec<serde_json::Value>,
    /// Permissive catalog/theme/metadata block. Unknown fields here are
    /// allowed because catalog entries are author-facing free-form data
    /// (titles, descriptions, palette notes). Behavior-bearing fields
    /// live outside `catalog`.
    #[serde(default)]
    pub catalog: Option<serde_json::Value>,
    /// Any field not matched above. [`validate`] rejects non-empty
    /// `extra` with [`Sl1LoadError::UnknownField`], giving us a
    /// programmatic strict-schema check that does not depend on
    /// serde's English error text.
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn default_sl1_schema_version() -> u32 {
    SL1_SCHEMA_VERSION
}

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

    /// Surfaces an unknown field inside the SL1 block. PR 0 detects
    /// these programmatically via the `extra` map on [`RawSl1Scene`]
    /// (no reliance on serde's English error text).
    #[error("scenario_language_v1: unknown field: {field}")]
    UnknownField { field: String },

    /// The SL1 block exists but is not a JSON object (e.g. explicit
    /// `"scenario_language_v1": null` or `"scenario_language_v1": 42`).
    /// Distinct from omitting the block, which is allowed.
    #[error("scenario_language_v1: expected a JSON object, got {found}")]
    ExpectedObject { found: &'static str },

    /// A serde-level shape error (type mismatch, malformed JSON, etc.)
    /// inside the SL1 block. Distinct from [`Self::UnknownField`] so
    /// tooling can render the two differently.
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

    #[error("scenario_language_v1.{section}: found {count} items, maximum {max}")]
    TooManyItems {
        section: &'static str,
        count: usize,
        max: usize,
    },
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
/// - Unknown top-level fields land in [`RawSl1Scene::extra`] and are
///   rejected with [`Sl1LoadError::UnknownField`].
/// - All grammar primitives must be empty —
///   [`Sl1LoadError::PrimitiveNotImplemented`] for any
///   `places`/`links`/`things`/`transforms`/`demand`/`pressure`/
///   `objectives`/`failure_conditions`/`agents`/`milestones` with
///   entries, since this build cannot give them behavior.
/// - The optional `observability` block may be present but must be
///   an empty object.
///
/// # Errors
/// See variants of [`Sl1LoadError`].
pub fn validate(raw: RawSl1Scene) -> Result<Sl1Scene, Sl1LoadError> {
    if raw.schema_version != SL1_SCHEMA_VERSION {
        return Err(Sl1LoadError::UnsupportedSchema {
            found: raw.schema_version,
            supported: SL1_SCHEMA_VERSION,
        });
    }

    // Programmatic strict-schema check: any field not named on
    // RawSl1Scene flowed into `extra` via #[serde(flatten)]. Reject
    // the first one we find. Sorted iteration via BTreeMap keeps
    // diagnostics deterministic.
    if let Some((name, _)) = raw.extra.iter().next() {
        return Err(Sl1LoadError::UnknownField {
            field: name.clone(),
        });
    }

    // Defensive per-section item caps. Even though PR 0 rejects any
    // non-empty primitive, the cap is the right shape for later PRs
    // when a primitive becomes valid. Note: serde has already
    // allocated `Vec<Value>` by the time this check runs, so the cap
    // is a diagnostic / sanity bound — not a parse-time memory
    // defense against a maliciously huge input. A future loader pass
    // that wants byte-level protection should add streaming or
    // preallocation limits in addition to this check.
    check_section_cap("places", raw.places.len())?;
    check_section_cap("links", raw.links.len())?;
    check_section_cap("things", raw.things.len())?;
    check_section_cap("transforms", raw.transforms.len())?;
    check_section_cap("demand", raw.demand.len())?;
    check_section_cap("pressure", raw.pressure.len())?;
    check_section_cap("objectives", raw.objectives.len())?;
    check_section_cap("failure_conditions", raw.failure_conditions.len())?;
    check_section_cap("agents", raw.agents.len())?;
    check_section_cap("milestones", raw.milestones.len())?;

    // PR 0 has no behavior for any primitive. Reject non-empty sections
    // so a proto-SL1 scene can't silently no-op while developers wait
    // for PRs 1–11. The vecs are Vec<serde_json::Value> in PR 0 so
    // even a well-formed future shape (e.g. `{"id": "p1"}`) reaches
    // this guard instead of bouncing off a per-primitive struct.
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

    // The optional observability block must be an empty object until
    // PR 9 implements its schema.
    let observability = if let Some(value) = raw.observability {
        match value {
            serde_json::Value::Object(map) if map.is_empty() => Some(Sl1Observability),
            serde_json::Value::Object(_) => {
                return Err(Sl1LoadError::PrimitiveNotImplemented {
                    section: "observability",
                });
            }
            other => {
                return Err(Sl1LoadError::Parse {
                    message: format!(
                        "scenario_language_v1.observability must be an object, got {}",
                        json_kind(&other)
                    ),
                });
            }
        }
    } else {
        None
    };

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
        observability,
        milestones: Vec::new(),
    })
}

fn check_section_cap(section: &'static str, count: usize) -> Result<(), Sl1LoadError> {
    if count > MAX_SL1_ITEMS_PER_SECTION {
        return Err(Sl1LoadError::TooManyItems {
            section,
            count,
            max: MAX_SL1_ITEMS_PER_SECTION,
        });
    }
    Ok(())
}

/// Parse + validate a standalone SL1 block from a `serde_json::Value`.
///
/// Used by the surrounding scene loader so the SL1 block's strict
/// validation runs through this typed path, producing a typed
/// [`Sl1LoadError`] instead of being swallowed into the outer scene's
/// parse error.
///
/// # Errors
/// - [`Sl1LoadError::ExpectedObject`] if the value is null, a number,
///   string, bool, or array — anything other than a JSON object.
/// - [`Sl1LoadError::UnknownField`], [`Sl1LoadError::Parse`],
///   [`Sl1LoadError::PrimitiveNotImplemented`], or
///   [`Sl1LoadError::UnsupportedSchema`] as documented on [`validate`].
pub fn load_value(value: serde_json::Value) -> Result<Sl1Scene, Sl1LoadError> {
    if !value.is_object() {
        return Err(Sl1LoadError::ExpectedObject {
            found: json_kind(&value),
        });
    }
    let raw: RawSl1Scene = serde_json::from_value(value).map_err(|e| Sl1LoadError::Parse {
        message: e.to_string(),
    })?;
    validate(raw)
}

/// Parse + validate a standalone SL1 block from JSON.
///
/// Convenience around [`load_value`] for tests and tooling that want
/// to load an SL1 fragment outside of a full simetro scene.
///
/// # Errors
/// Same as [`load_value`].
pub fn load_str(json: &str) -> Result<Sl1Scene, Sl1LoadError> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| Sl1LoadError::Parse {
        message: e.to_string(),
    })?;
    load_value(value)
}

/// Stable, English-only one-word kind tag for a `serde_json::Value`,
/// used in [`Sl1LoadError::ExpectedObject`] and observability shape
/// diagnostics. Kept tiny and locale-free on purpose.
fn json_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
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
                assert_eq!(field, "mystery");
            }
            other => panic!("expected UnknownField, got {other:?}"),
        }
    }

    #[test]
    fn explicit_null_block_rejected() {
        let err = load_value(serde_json::Value::Null).unwrap_err();
        match err {
            Sl1LoadError::ExpectedObject { found } => {
                assert_eq!(found, "null");
            }
            other => panic!("expected ExpectedObject, got {other:?}"),
        }
    }

    #[test]
    fn non_object_block_rejected() {
        for (json, expected_kind) in [
            ("[]", "array"),
            ("42", "number"),
            (r#""hello""#, "string"),
            ("true", "bool"),
            ("null", "null"),
        ] {
            let err = load_str(json).unwrap_err();
            match err {
                Sl1LoadError::ExpectedObject { found } => {
                    assert_eq!(found, expected_kind, "json was {json}");
                }
                other => panic!("expected ExpectedObject for {json}, got {other:?}"),
            }
        }
    }

    #[test]
    fn non_empty_place_with_real_shape_hits_primitive_guard() {
        // Even a well-formed future-PR shape inside `places` must hit
        // PrimitiveNotImplemented in PR 0, not bounce off a per-struct
        // deny_unknown_fields rule. This is the ergonomic the rubber
        // duck flagged in round 2.
        let err = load_str(r#"{"places": [{"id": "p1", "role": "node"}]}"#).unwrap_err();
        match err {
            Sl1LoadError::PrimitiveNotImplemented { section } => {
                assert_eq!(section, "places");
            }
            other => panic!("expected PrimitiveNotImplemented, got {other:?}"),
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
    fn section_caps_are_checked_before_placeholder_rejection() {
        let raw = RawSl1Scene {
            schema_version: SL1_SCHEMA_VERSION,
            places: (0..=MAX_SL1_ITEMS_PER_SECTION)
                .map(|_| serde_json::Value::Object(serde_json::Map::new()))
                .collect(),
            ..RawSl1Scene::default()
        };
        let err = validate(raw).unwrap_err();
        assert_eq!(
            err,
            Sl1LoadError::TooManyItems {
                section: "places",
                count: MAX_SL1_ITEMS_PER_SECTION + 1,
                max: MAX_SL1_ITEMS_PER_SECTION,
            }
        );
    }

    #[test]
    fn parse_error_distinguished_from_unknown_field() {
        // A type mismatch (places must be a Vec, not a string) is a
        // Parse error, not an UnknownField.
        let err = load_str(r#"{"places": "not a list"}"#).unwrap_err();
        match err {
            Sl1LoadError::Parse { message } => {
                assert!(
                    !message.is_empty(),
                    "Parse error should carry a non-empty message"
                );
            }
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn load_value_typed_unknown_field_error() {
        let v: serde_json::Value = serde_json::from_str(r#"{"mystery": 1}"#).unwrap();
        let err = load_value(v).unwrap_err();
        match err {
            Sl1LoadError::UnknownField { field } => assert_eq!(field, "mystery"),
            other => panic!("expected UnknownField, got {other:?}"),
        }
    }

    #[test]
    fn unknown_field_alongside_primitive_is_unknown_field() {
        // Unknown-field detection happens BEFORE primitive guards, so a
        // typo at the top level is surfaced even if the author also
        // populated a primitive that would have hit
        // PrimitiveNotImplemented.
        let err = load_str(r#"{"mystery": 1, "places": [{}]}"#).unwrap_err();
        match err {
            Sl1LoadError::UnknownField { field } => assert_eq!(field, "mystery"),
            other => panic!("expected UnknownField first, got {other:?}"),
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
    fn non_empty_observability_hits_primitive_guard_until_pr9() {
        // Until PR 9 introduces typed observability fields, any
        // populated observability block is treated as an
        // unimplemented primitive — not introspected for unknown
        // fields. That keeps PR 0 from silently no-op'ing on
        // proto-observability content.
        let err = load_str(r#"{"observability": {"alerts": []}}"#).unwrap_err();
        match err {
            Sl1LoadError::PrimitiveNotImplemented { section } => {
                assert_eq!(section, "observability");
            }
            other => panic!("expected PrimitiveNotImplemented, got {other:?}"),
        }
    }

    #[test]
    fn observability_must_be_object() {
        let err = load_str(r#"{"observability": []}"#).unwrap_err();
        match err {
            Sl1LoadError::Parse { message } => {
                assert!(message.contains("observability"));
                assert!(message.contains("array"));
            }
            other => panic!("expected Parse, got {other:?}"),
        }
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
