//! `scenario_language_v1` (SL1) grammar.
//!
//! This module establishes the shape of the SL1 grammar — places,
//! links, things, transforms, demand, pressure, objectives,
//! failure_conditions, agents, observability, and milestones. PR 0
//! installed the skeleton with all primitives as placeholders. PR 1
//! landed [`Sl1Place`] — author-declared locations with capacity,
//! storage, accepted/produced thing tags, failure-domain labels, and a
//! strict-predicate operating-state map. PR 2 landed [`Sl1Link`] —
//! declarative typed links between places with direction, capacity,
//! travel ticks, compatibility, backpressure policy, and an optional
//! render hint. Primitives from `things` onward remain placeholders
//! until their dedicated PRs land.
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

/// Maximum length of an SL1 stable identifier (place id, etc.).
/// Matches `loader::MAX_ID_LEN` so SL1 and legacy ids share charset
/// rules and operators can predict a uniform identifier surface.
const MAX_SL1_ID_LEN: usize = 64;

/// Coordinate clamp for SL1 place positions, mirroring
/// `loader::COORD_LIMIT`. Keeps the SL1 world bounded so renderer
/// transforms stay numerically stable.
const SL1_COORD_LIMIT: f32 = 1.0e6;

/// Upper bound for `used_percent`-style predicate thresholds. Percent
/// thresholds beyond 100 cannot fire and almost certainly indicate
/// an author typo.
const SL1_PERCENT_MAX: u8 = 100;

/// Upper bound for `Sl1Link.queue_capacity`. Caps author-supplied queue
/// sizes well below `u64::MAX` so future runtime systems cannot
/// accidentally request pathological allocations from a typo.
pub const MAX_LINK_QUEUE_CAPACITY: u64 = 1_000_000_000;

/// Upper bound for `Sl1Link.travel_ticks`. Same rationale as
/// [`MAX_LINK_QUEUE_CAPACITY`]: keep author-supplied ticks in a
/// human-meaningful range.
pub const MAX_LINK_TRAVEL_TICKS: u64 = 1_000_000_000;

// ---------------------------------------------------------------------------
// Raw (post-serde, pre-validation) SL1 scene block.
// ---------------------------------------------------------------------------

/// Raw SL1 scene block. Each grammar primitive that has not yet had
/// its PR land is typed as `Vec<serde_json::Value>` so the validator
/// can reject non-empty entries with
/// [`Sl1LoadError::PrimitiveNotImplemented`] without relying on a
/// placeholder struct's shape. PR 1 promotes `places` to a typed
/// `Vec<RawSl1Place>`; PRs 2–11 do the same for the remaining
/// primitives in order.
///
/// Unknown top-level fields land in [`Self::extra`]; [`validate`]
/// emits a typed [`Sl1LoadError::UnknownField`] for each.
#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct RawSl1Scene {
    /// Defaults to [`SL1_SCHEMA_VERSION`] when omitted.
    #[serde(default = "default_sl1_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub places: Vec<RawSl1Place>,
    #[serde(default)]
    pub links: Vec<RawSl1Link>,
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

/// Validated SL1 scene. PR 1 populated `places` and PR 2 populated
/// `links`; primitives from `things` onward remain empty placeholders
/// until their dedicated PRs land.
#[derive(Debug, Default, Clone, PartialEq)]
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

// ---------------------------------------------------------------------------
// Place — typed primitive (PR 1).
// ---------------------------------------------------------------------------

/// Raw, post-serde representation of a `places[]` entry. Strict-schema:
/// `#[serde(deny_unknown_fields)]` ensures nested typos do not silently
/// no-op even though top-level SL1 typos are also blocked.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Place {
    pub id: String,
    pub role: String,
    pub pos: [f32; 2],
    /// Optional render hint (e.g. `"hexagon"`, `"circle"`, `"square"`).
    /// Carried opaquely through the protocol so PR 6's frontend can
    /// pick a glyph without re-parsing. Validation is delegated to
    /// the renderer; PR 1 only checks non-emptiness when present.
    #[serde(default)]
    pub shape: Option<String>,
    /// Optional palette index (matches existing `theme.palette[]`
    /// indexing convention). Carried opaquely. PR 1 does not range-
    /// check against palette length — that is the renderer's job in
    /// PR 6, and palette overrides may legitimately introduce new
    /// indices later.
    #[serde(default)]
    pub color: Option<u32>,
    #[serde(default)]
    pub capacity: BTreeMap<String, u64>,
    #[serde(default)]
    pub storage: BTreeMap<String, RawSl1StorageSlot>,
    #[serde(default)]
    pub accepts: Vec<String>,
    #[serde(default)]
    pub produces: Vec<String>,
    #[serde(default)]
    pub failure_domains: Vec<String>,
    /// Map of operating-state name → predicate. Matches the spec map
    /// form (`docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`
    /// §places example), preserving the state name (`strained`,
    /// `overloaded`, `failed`, etc.). The validator translates each
    /// entry into a typed [`Sl1OperatingPredicate`].
    #[serde(default)]
    pub operating_states: BTreeMap<String, RawSl1OperatingState>,
}

/// Raw storage slot for a [`RawSl1Place`]. Capacity is the slot's max
/// units; `initial` is the pre-loaded amount at scene load and must not
/// exceed `capacity`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1StorageSlot {
    pub capacity: u64,
    #[serde(default)]
    pub initial: u64,
}

/// Raw operating-state declaration. The author writes a single
/// predicate string under `when` plus an optional `grace_ticks` for
/// debounce. The validator parses `when` into a typed
/// [`Sl1OperatingPredicate`]; **there is no expression engine** —
/// strings are pattern-matched against a closed set of supported
/// predicate templates and any deviation is a typed load error.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1OperatingState {
    pub when: String,
    #[serde(default)]
    pub grace_ticks: Option<u64>,
}

/// Validated `place` primitive (PR 1).
///
/// Carries author-declared static metadata only; per-tick utilization
/// and storage updates land alongside the transforms PR.
#[derive(Debug, Clone, PartialEq)]
pub struct Sl1Place {
    pub id: String,
    pub role: String,
    pub pos: [f32; 2],
    /// Optional render hint (PR 6 renderer interprets; PR 1 carries
    /// opaquely). Empty strings are rejected at validation time.
    pub shape: Option<String>,
    /// Optional palette index. Carried opaquely; renderer is
    /// responsible for range checks.
    pub color: Option<u32>,
    /// Named, unitless capacity buckets (e.g. `query_slots`,
    /// `cooling_tons`). Sorted by key so iteration order is stable.
    pub capacity: BTreeMap<String, u64>,
    /// Named buffer slots that hold a typed thing tag (resolution to
    /// `things[]` ids lands in PR 3). Sorted by key.
    pub storage: BTreeMap<String, Sl1StorageSlot>,
    /// Tags this place accepts as input. Sorted + deduplicated so the
    /// determinism hash is independent of declaration order and free
    /// of cosmetic baseline drift.
    pub accepts: Vec<String>,
    /// Tags this place produces. Sorted + deduplicated.
    pub produces: Vec<String>,
    /// Failure-domain labels (e.g. `eastus`, `az1`). Sorted +
    /// deduplicated.
    pub failure_domains: Vec<String>,
    /// Operating-state map, keyed by state name (`strained`,
    /// `overloaded`, `failed`, etc.). The `BTreeMap` key ordering is
    /// what the determinism hash walks.
    pub operating_states: BTreeMap<String, Sl1OperatingState>,
}

/// Validated storage slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sl1StorageSlot {
    pub capacity: u64,
    pub initial: u64,
}

/// Validated operating-state entry: typed predicate plus optional
/// debounce window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1OperatingState {
    pub predicate: Sl1OperatingPredicate,
    pub grace_ticks: Option<u64>,
}

/// Closed set of supported operating-state predicates for PR 1. Each
/// variant maps to one author-facing template (see `parse_predicate`
/// for the exact textual surface). The closed set is deliberate — the
/// spec forbids an arbitrary predicate language
/// (`docs/scenario-language-v1.md` and the canonical roadmap).
///
/// Future PRs add `InventoryGte` (after Things land) and `MetricGte`
/// (after Observability lands) by extending this enum.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sl1OperatingPredicate {
    /// `<metric_id>.used_percent >= <0..=100>`.
    UsedPercentGte { metric: String, threshold: u8 },
    /// `overloaded_ticks > <ticks>`.
    OverloadedTicksGt { ticks: u64 },
}

/// Raw, post-serde representation of a `links[]` entry. Strict-schema:
/// `#[serde(deny_unknown_fields)]` ensures nested typos do not silently
/// no-op. `direction` and `backpressure` are deserialized as
/// `Option<String>` rather than typed enums so the validator can
/// distinguish *missing* values (a hard error in PR 2; future PRs may
/// allow omitted `direction` for dependency-only links) from *unknown*
/// values (typed [`Sl1LoadError::LinkUnknownDirection`] /
/// [`Sl1LoadError::LinkUnknownBackpressure`]) — serde enums would
/// collapse both cases into a generic parse error.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Link {
    pub id: String,
    #[serde(rename = "type")]
    pub link_type: String,
    pub from: String,
    pub to: String,
    /// Required in PR 2. Future PRs may relax omission to mean
    /// "dependency-only link", which is why this is `Option<String>`
    /// (so the validator can emit [`Sl1LoadError::LinkMissingDirection`]
    /// instead of having serde silently apply a default).
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub capacity: BTreeMap<String, u64>,
    pub travel_ticks: u64,
    #[serde(default)]
    pub compatibility: Vec<String>,
    pub queue_capacity: u64,
    /// Required in PR 2. Same `Option<String>` rationale as
    /// [`Self::direction`].
    #[serde(default)]
    pub backpressure: Option<String>,
    #[serde(default)]
    pub render: Option<RawSl1LinkRenderHint>,
}

/// Optional render hint for a [`RawSl1Link`]. Carried opaquely from
/// scene JSON; PR 6's frontend interprets `style` and `color`. PR 1's
/// rubber-duck review surfaced the same need for places: type render
/// hints now so typos like `"styl"` fail load instead of being
/// silently discarded by an untyped `serde_json::Value` carrier.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1LinkRenderHint {
    pub style: String,
    #[serde(default)]
    pub color: Option<u32>,
}

/// Validated `link` primitive (PR 2). Declarative only — runtime
/// queue mutation lands alongside transforms (PR 4) and demand
/// (PR 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1Link {
    pub id: String,
    pub link_type: String,
    pub from: String,
    pub to: String,
    pub direction: Sl1LinkDirection,
    /// Named, unitless capacity buckets (e.g. `events_per_tick`).
    /// Sorted by key.
    pub capacity: BTreeMap<String, u64>,
    pub travel_ticks: u64,
    /// Sorted + deduplicated. Values reference `things[].id` *or* a
    /// thing tag — PR 3 lands the cross-check against the `things[]`
    /// catalog. PR 2 only canonicalizes.
    pub compatibility: Vec<String>,
    pub queue_capacity: u64,
    pub backpressure: Sl1LinkBackpressure,
    pub render: Option<Sl1LinkRenderHint>,
}

/// Validated render hint for a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1LinkRenderHint {
    pub style: String,
    pub color: Option<u32>,
}

/// Closed set of supported `direction` values for PR 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sl1LinkDirection {
    Forward,
    Bidirectional,
}

/// Closed set of supported `backpressure` policies. Runtime semantics
/// land in PR 4/5 when transforms/demand actually push items through
/// the link queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sl1LinkBackpressure {
    BlockUpstream,
    DropLowPriority,
    SpillToBuffer,
    DegradeQuality,
}

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

    // ---- Place primitive (PR 1) -------------------------------------------
    /// Two `places[]` entries declared the same `id`.
    #[error("scenario_language_v1.places: duplicate id {id:?}")]
    PlaceDuplicateId { id: String },

    /// A place `id` is empty, too long, or contains characters outside
    /// the allowed alphanumeric/`_`/`-` charset.
    #[error("scenario_language_v1.places: invalid id {id:?}")]
    PlaceInvalidId { id: String },

    /// A place `role` is empty.
    #[error("scenario_language_v1.places[{id:?}].role: must be non-empty")]
    PlaceEmptyRole { id: String },

    /// A place `pos` coordinate is non-finite (NaN/inf) or exceeds the
    /// coordinate clamp.
    #[error("scenario_language_v1.places[{id:?}].pos: non-finite or out of bounds")]
    PlaceInvalidPos { id: String },

    /// A `storage[*].initial` value exceeds its slot's `capacity`.
    #[error(
        "scenario_language_v1.places[{id:?}].storage[{slot:?}]: \
         initial {initial} exceeds capacity {capacity}"
    )]
    PlaceStorageInitialExceedsCapacity {
        id: String,
        slot: String,
        initial: u64,
        capacity: u64,
    },

    /// A `storage[*].capacity` is zero. A zero-capacity buffer cannot
    /// hold anything and is always an authoring mistake.
    #[error("scenario_language_v1.places[{id:?}].storage[{slot:?}]: capacity must be > 0")]
    PlaceStorageCapacityZero { id: String, slot: String },

    /// A `capacity`, `storage`, `accepts`, `produces`, or
    /// `failure_domains` key/entry is empty.
    #[error("scenario_language_v1.places[{id:?}].{field}: empty entry not allowed")]
    PlaceEmptyEntry { id: String, field: &'static str },

    /// An `accepts`, `produces`, or `failure_domains` list contains a
    /// duplicate entry.
    #[error("scenario_language_v1.places[{id:?}].{field}: duplicate entry {value:?}")]
    PlaceDuplicateEntry {
        id: String,
        field: &'static str,
        value: String,
    },

    /// An `operating_states[*].when` predicate string did not match
    /// any supported predicate template. The list of supported
    /// templates is enumerated by [`Sl1OperatingPredicate`] variants.
    #[error(
        "scenario_language_v1.places[{id:?}].operating_states[{state:?}].when: \
         unsupported predicate {predicate:?}"
    )]
    PlaceUnsupportedPredicate {
        id: String,
        state: String,
        predicate: String,
    },

    /// A `used_percent` predicate threshold exceeds 100. Percent
    /// thresholds outside `0..=100` cannot fire and are always an
    /// authoring mistake.
    #[error(
        "scenario_language_v1.places[{id:?}].operating_states[{state:?}].when: \
         used_percent threshold {threshold} exceeds {max}"
    )]
    PlacePercentThresholdOutOfRange {
        id: String,
        state: String,
        threshold: u64,
        max: u8,
    },

    /// An operating-state name is empty.
    #[error("scenario_language_v1.places[{id:?}].operating_states: empty state name")]
    PlaceEmptyOperatingStateName { id: String },

    /// `shape` is present but the string is empty/whitespace.
    /// Authors who want the default glyph should omit the field entirely
    /// rather than write `""`.
    #[error("scenario_language_v1.places[{id:?}].shape: empty string (omit field for default)")]
    PlaceEmptyShape { id: String },

    /// A `used_percent` predicate references a metric name that is not
    /// declared in this place's `capacity` map. Without a matching
    /// capacity bucket the predicate has no denominator and would
    /// silently never fire, which is the silent-fail pattern the SL1
    /// strict-schema rule exists to prevent.
    #[error(
        "scenario_language_v1.places[{id:?}].operating_states[{state:?}].when: \
         references unknown capacity metric {metric:?}"
    )]
    PlacePredicateUnknownMetric {
        id: String,
        state: String,
        metric: String,
    },

    // ---- Link primitive (PR 2) --------------------------------------------
    /// Two `links[]` entries declared the same `id`.
    #[error("scenario_language_v1.links: duplicate id {id:?}")]
    LinkDuplicateId { id: String },

    /// A link `id` is empty, too long, or contains characters outside
    /// the allowed alphanumeric/`_`/`-` charset.
    #[error("scenario_language_v1.links: invalid id {id:?}")]
    LinkInvalidId { id: String },

    /// A link `type` is empty.
    #[error("scenario_language_v1.links[{id:?}].type: must be non-empty")]
    LinkEmptyType { id: String },

    /// A link's `from` or `to` does not match any declared place id.
    /// `which` is the literal `"from"` or `"to"` so diagnostics point
    /// at the offending field.
    #[error("scenario_language_v1.links[{id:?}].{which}: unknown place {place:?}")]
    LinkUnknownPlace {
        id: String,
        which: &'static str,
        place: String,
    },

    /// A link's `from` equals its `to`. Self-looping transport links
    /// are almost certainly an authoring mistake. Future PRs may
    /// reintroduce self-references explicitly for dependency-only
    /// link types.
    #[error("scenario_language_v1.links[{id:?}]: self-loop ({place:?} -> {place:?}) not allowed")]
    LinkSelfLoop { id: String, place: String },

    /// `direction` was omitted. Required in PR 2 to keep the omission
    /// slot reserved for future dependency-only link semantics.
    #[error("scenario_language_v1.links[{id:?}].direction: required field missing")]
    LinkMissingDirection { id: String },

    /// `direction` was supplied but did not match a supported value.
    #[error("scenario_language_v1.links[{id:?}].direction: unknown value {value:?}")]
    LinkUnknownDirection { id: String, value: String },

    /// `backpressure` was omitted. Required in PR 2 — the spec
    /// explicitly says backpressure policies must be explicit.
    #[error("scenario_language_v1.links[{id:?}].backpressure: required field missing")]
    LinkMissingBackpressure { id: String },

    /// `backpressure` was supplied but did not match a supported value.
    #[error("scenario_language_v1.links[{id:?}].backpressure: unknown value {value:?}")]
    LinkUnknownBackpressure { id: String, value: String },

    /// A `capacity` or `compatibility` entry is empty.
    #[error("scenario_language_v1.links[{id:?}].{field}: empty entry not allowed")]
    LinkEmptyEntry { id: String, field: &'static str },

    /// A `compatibility` list contains a duplicate entry.
    #[error("scenario_language_v1.links[{id:?}].compatibility: duplicate entry {value:?}")]
    LinkDuplicateCompatibility { id: String, value: String },

    /// `travel_ticks` is zero. PR 2 transport links require >0; future
    /// PRs may reintroduce zero-tick edges as a distinct dependency-only
    /// concept.
    #[error("scenario_language_v1.links[{id:?}].travel_ticks: must be > 0")]
    LinkTravelTicksZero { id: String },

    /// `travel_ticks` exceeds [`MAX_LINK_TRAVEL_TICKS`].
    #[error("scenario_language_v1.links[{id:?}].travel_ticks: {value} exceeds maximum {max}")]
    LinkTravelTicksOutOfRange { id: String, value: u64, max: u64 },

    /// `queue_capacity` is zero. Zero-capacity queues are degenerate;
    /// backpressure policies cannot meaningfully apply.
    #[error("scenario_language_v1.links[{id:?}].queue_capacity: must be > 0")]
    LinkQueueCapacityZero { id: String },

    /// `queue_capacity` exceeds [`MAX_LINK_QUEUE_CAPACITY`].
    #[error("scenario_language_v1.links[{id:?}].queue_capacity: {value} exceeds maximum {max}")]
    LinkQueueCapacityOutOfRange { id: String, value: u64, max: u64 },

    /// `render.style` is empty. Authors who want default rendering
    /// should omit the `render` field entirely.
    #[error(
        "scenario_language_v1.links[{id:?}].render.style: empty string (omit render for default)"
    )]
    LinkEmptyRenderStyle { id: String },
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
/// PR 2 enforces (in addition to PR 1):
/// - `schema_version` must equal [`SL1_SCHEMA_VERSION`].
/// - Unknown top-level fields land in [`RawSl1Scene::extra`] and are
///   rejected with [`Sl1LoadError::UnknownField`].
/// - `places` entries are typed; each is validated for id charset/length
///   uniqueness, finite coords, non-empty role, non-zero storage
///   capacity, `storage[*].initial <= storage[*].capacity`, deduplicated
///   set-like fields, and an operating-state map whose `when` strings
///   parse into a closed set of supported predicates.
/// - `links` entries are typed; each is validated for id charset/length
///   uniqueness, non-empty `type`, `from`/`to` referencing declared
///   places (no self-loops), closed-enum `direction` and `backpressure`
///   (each distinguishing Missing vs Unknown), non-empty capacity keys,
///   deduplicated non-empty compatibility entries, `travel_ticks` in
///   `1..=`[`MAX_LINK_TRAVEL_TICKS`], `queue_capacity` in
///   `1..=`[`MAX_LINK_QUEUE_CAPACITY`], and an optional render hint
///   with a non-empty `style`.
/// - All remaining behavior-bearing primitives must be empty —
///   [`Sl1LoadError::PrimitiveNotImplemented`] for any
///   `things`/`transforms`/`demand`/`pressure`/`objectives`/
///   `failure_conditions`/`agents`/`milestones` with entries.
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

    // Defensive per-section item caps. For PR 1, `places` is fully
    // typed; the cap is still a diagnostic / sanity bound preventing
    // pathological JSON. For not-yet-implemented primitives, the cap
    // runs before the empty-section guard so the diagnostic still
    // surfaces if author code happens to declare a 100k+ list while
    // waiting for the matching PR.
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

    // PRs 2–11 add behavior for each primitive. Reject non-empty
    // sections so a proto-SL1 scene can't silently no-op while
    // developers wait. PR 1 has removed `places` from this guard
    // because the typed `Vec<RawSl1Place>` is now validated below.
    macro_rules! reject_non_empty {
        ($vec:expr, $name:literal) => {
            if !$vec.is_empty() {
                return Err(Sl1LoadError::PrimitiveNotImplemented { section: $name });
            }
        };
    }
    reject_non_empty!(raw.things, "things");
    reject_non_empty!(raw.transforms, "transforms");
    reject_non_empty!(raw.demand, "demand");
    reject_non_empty!(raw.pressure, "pressure");
    reject_non_empty!(raw.objectives, "objectives");
    reject_non_empty!(raw.failure_conditions, "failure_conditions");
    reject_non_empty!(raw.agents, "agents");
    reject_non_empty!(raw.milestones, "milestones");

    // Validate places (PR 1).
    let mut places: Vec<Sl1Place> = Vec::with_capacity(raw.places.len());
    let mut seen_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for raw_place in raw.places {
        let place = validate_place(raw_place)?;
        if !seen_ids.insert(place.id.clone()) {
            return Err(Sl1LoadError::PlaceDuplicateId { id: place.id });
        }
        places.push(place);
    }
    // Sort places by id so engine iteration order is independent of
    // declaration order in the source JSON.
    places.sort_by(|a, b| a.id.cmp(&b.id));

    // Validate links (PR 2). Links may reference any declared place
    // regardless of declaration order, so we collect the full set of
    // valid place ids first.
    let place_ids: std::collections::BTreeSet<String> =
        places.iter().map(|p| p.id.clone()).collect();
    let mut links: Vec<Sl1Link> = Vec::with_capacity(raw.links.len());
    let mut seen_link_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for raw_link in raw.links {
        let link = validate_link(raw_link, &place_ids)?;
        if !seen_link_ids.insert(link.id.clone()) {
            return Err(Sl1LoadError::LinkDuplicateId { id: link.id });
        }
        links.push(link);
    }
    // Sort links by id so engine iteration order is independent of
    // declaration order.
    links.sort_by(|a, b| a.id.cmp(&b.id));

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
        places,
        links,
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

// ---------------------------------------------------------------------------
// Place validation helpers (PR 1).
// ---------------------------------------------------------------------------

fn validate_place(raw: RawSl1Place) -> Result<Sl1Place, Sl1LoadError> {
    validate_sl1_id(&raw.id)?;
    if raw.role.trim().is_empty() {
        return Err(Sl1LoadError::PlaceEmptyRole { id: raw.id });
    }
    if !raw.pos[0].is_finite()
        || !raw.pos[1].is_finite()
        || raw.pos[0].abs() > SL1_COORD_LIMIT
        || raw.pos[1].abs() > SL1_COORD_LIMIT
    {
        return Err(Sl1LoadError::PlaceInvalidPos { id: raw.id });
    }
    if let Some(shape) = raw.shape.as_ref() {
        if shape.trim().is_empty() {
            return Err(Sl1LoadError::PlaceEmptyShape { id: raw.id });
        }
    }
    // Capacity entries: reject empty keys. Zero values are allowed —
    // the spec example `query_slots: 0` is a valid "declared but
    // currently unavailable" capacity bucket.
    for key in raw.capacity.keys() {
        if key.trim().is_empty() {
            return Err(Sl1LoadError::PlaceEmptyEntry {
                id: raw.id,
                field: "capacity",
            });
        }
    }
    // Storage slots: each key must be non-empty, capacity must be > 0,
    // and initial must not exceed capacity.
    for (slot, slot_def) in &raw.storage {
        if slot.trim().is_empty() {
            return Err(Sl1LoadError::PlaceEmptyEntry {
                id: raw.id,
                field: "storage",
            });
        }
        if slot_def.capacity == 0 {
            return Err(Sl1LoadError::PlaceStorageCapacityZero {
                id: raw.id,
                slot: slot.clone(),
            });
        }
        if slot_def.initial > slot_def.capacity {
            return Err(Sl1LoadError::PlaceStorageInitialExceedsCapacity {
                id: raw.id,
                slot: slot.clone(),
                initial: slot_def.initial,
                capacity: slot_def.capacity,
            });
        }
    }
    let storage: BTreeMap<String, Sl1StorageSlot> = raw
        .storage
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                Sl1StorageSlot {
                    capacity: v.capacity,
                    initial: v.initial,
                },
            )
        })
        .collect();

    let accepts = canonicalize_set(&raw.id, "accepts", raw.accepts)?;
    let produces = canonicalize_set(&raw.id, "produces", raw.produces)?;
    let failure_domains = canonicalize_set(&raw.id, "failure_domains", raw.failure_domains)?;

    let mut operating_states: BTreeMap<String, Sl1OperatingState> = BTreeMap::new();
    for (state, raw_state) in raw.operating_states {
        if state.trim().is_empty() {
            return Err(Sl1LoadError::PlaceEmptyOperatingStateName { id: raw.id });
        }
        let predicate = parse_predicate(&raw.id, &state, &raw_state.when)?;
        // Strict-schema: a `used_percent` predicate must reference a
        // capacity bucket that actually exists on this place. Otherwise
        // the predicate has no denominator and would silently never
        // fire — the exact silent-fail pattern SL1 is designed to
        // prevent.
        if let Sl1OperatingPredicate::UsedPercentGte { metric, .. } = &predicate {
            if !raw.capacity.contains_key(metric) {
                return Err(Sl1LoadError::PlacePredicateUnknownMetric {
                    id: raw.id,
                    state,
                    metric: metric.clone(),
                });
            }
        }
        operating_states.insert(
            state,
            Sl1OperatingState {
                predicate,
                grace_ticks: raw_state.grace_ticks,
            },
        );
    }

    Ok(Sl1Place {
        id: raw.id,
        role: raw.role,
        pos: raw.pos,
        shape: raw.shape,
        color: raw.color,
        capacity: raw.capacity,
        storage,
        accepts,
        produces,
        failure_domains,
        operating_states,
    })
}

fn is_valid_sl1_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_SL1_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validate_sl1_id(id: &str) -> Result<(), Sl1LoadError> {
    if !is_valid_sl1_id(id) {
        return Err(Sl1LoadError::PlaceInvalidId { id: id.to_string() });
    }
    Ok(())
}

/// Reject empty entries and duplicates, then sort lexicographically.
/// Used for set-like fields (`accepts`, `produces`, `failure_domains`)
/// where author declaration order has no semantic meaning, so a stable
/// canonical order eliminates cosmetic hash baseline drift.
fn canonicalize_set(
    place_id: &str,
    field: &'static str,
    raw: Vec<String>,
) -> Result<Vec<String>, Sl1LoadError> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in &raw {
        if entry.trim().is_empty() {
            return Err(Sl1LoadError::PlaceEmptyEntry {
                id: place_id.to_string(),
                field,
            });
        }
        if !seen.insert(entry.clone()) {
            return Err(Sl1LoadError::PlaceDuplicateEntry {
                id: place_id.to_string(),
                field,
                value: entry.clone(),
            });
        }
    }
    Ok(seen.into_iter().collect())
}

// ---------------------------------------------------------------------------
// Link validation helpers (PR 2).
// ---------------------------------------------------------------------------

fn validate_link(
    raw: RawSl1Link,
    place_ids: &std::collections::BTreeSet<String>,
) -> Result<Sl1Link, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::LinkInvalidId { id: raw.id });
    }
    if raw.link_type.trim().is_empty() {
        return Err(Sl1LoadError::LinkEmptyType { id: raw.id });
    }
    if !place_ids.contains(&raw.from) {
        return Err(Sl1LoadError::LinkUnknownPlace {
            id: raw.id,
            which: "from",
            place: raw.from,
        });
    }
    if !place_ids.contains(&raw.to) {
        return Err(Sl1LoadError::LinkUnknownPlace {
            id: raw.id,
            which: "to",
            place: raw.to,
        });
    }
    if raw.from == raw.to {
        let place = raw.from.clone();
        return Err(Sl1LoadError::LinkSelfLoop { id: raw.id, place });
    }

    let direction = match raw.direction.as_deref() {
        None => return Err(Sl1LoadError::LinkMissingDirection { id: raw.id }),
        Some("forward") => Sl1LinkDirection::Forward,
        Some("bidirectional") => Sl1LinkDirection::Bidirectional,
        Some(other) => {
            return Err(Sl1LoadError::LinkUnknownDirection {
                id: raw.id,
                value: other.to_string(),
            });
        }
    };

    let backpressure = match raw.backpressure.as_deref() {
        None => return Err(Sl1LoadError::LinkMissingBackpressure { id: raw.id }),
        Some("block_upstream") => Sl1LinkBackpressure::BlockUpstream,
        Some("drop_low_priority") => Sl1LinkBackpressure::DropLowPriority,
        Some("spill_to_buffer") => Sl1LinkBackpressure::SpillToBuffer,
        Some("degrade_quality") => Sl1LinkBackpressure::DegradeQuality,
        Some(other) => {
            return Err(Sl1LoadError::LinkUnknownBackpressure {
                id: raw.id,
                value: other.to_string(),
            });
        }
    };

    for key in raw.capacity.keys() {
        if key.trim().is_empty() {
            return Err(Sl1LoadError::LinkEmptyEntry {
                id: raw.id,
                field: "capacity",
            });
        }
    }

    if raw.travel_ticks == 0 {
        return Err(Sl1LoadError::LinkTravelTicksZero { id: raw.id });
    }
    if raw.travel_ticks > MAX_LINK_TRAVEL_TICKS {
        return Err(Sl1LoadError::LinkTravelTicksOutOfRange {
            id: raw.id,
            value: raw.travel_ticks,
            max: MAX_LINK_TRAVEL_TICKS,
        });
    }

    if raw.queue_capacity == 0 {
        return Err(Sl1LoadError::LinkQueueCapacityZero { id: raw.id });
    }
    if raw.queue_capacity > MAX_LINK_QUEUE_CAPACITY {
        return Err(Sl1LoadError::LinkQueueCapacityOutOfRange {
            id: raw.id,
            value: raw.queue_capacity,
            max: MAX_LINK_QUEUE_CAPACITY,
        });
    }

    let compatibility = canonicalize_link_compatibility(&raw.id, raw.compatibility)?;

    let render = if let Some(r) = raw.render {
        if r.style.trim().is_empty() {
            return Err(Sl1LoadError::LinkEmptyRenderStyle { id: raw.id });
        }
        Some(Sl1LinkRenderHint {
            style: r.style,
            color: r.color,
        })
    } else {
        None
    };

    Ok(Sl1Link {
        id: raw.id,
        link_type: raw.link_type,
        from: raw.from,
        to: raw.to,
        direction,
        capacity: raw.capacity,
        travel_ticks: raw.travel_ticks,
        compatibility,
        queue_capacity: raw.queue_capacity,
        backpressure,
        render,
    })
}

fn canonicalize_link_compatibility(
    link_id: &str,
    raw: Vec<String>,
) -> Result<Vec<String>, Sl1LoadError> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in &raw {
        if entry.trim().is_empty() {
            return Err(Sl1LoadError::LinkEmptyEntry {
                id: link_id.to_string(),
                field: "compatibility",
            });
        }
        if !seen.insert(entry.clone()) {
            return Err(Sl1LoadError::LinkDuplicateCompatibility {
                id: link_id.to_string(),
                value: entry.clone(),
            });
        }
    }
    Ok(seen.into_iter().collect())
}

/// Parse an operating-state `when` string into a typed
/// [`Sl1OperatingPredicate`]. Matches a closed set of templates only;
/// **no expression engine** — anything outside the templates is a
/// typed load error. Surface for PR 1:
///
/// - `<metric_id>.used_percent >= <0..=100>`
/// - `overloaded_ticks > <ticks>`
///
/// PRs 3 and 9 add `InventoryGte` and `MetricGte` templates.
fn parse_predicate(
    place_id: &str,
    state: &str,
    when: &str,
) -> Result<Sl1OperatingPredicate, Sl1LoadError> {
    let trimmed = when.trim();

    // Try `<metric_id>.used_percent >= <threshold>` first because the
    // dot disambiguates against the bare `overloaded_ticks` template.
    if let Some((lhs, rhs)) = trimmed.split_once(">=") {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        if let Some(metric) = lhs.strip_suffix(".used_percent") {
            let metric = metric.trim();
            if validate_sl1_id(metric).is_ok() {
                let threshold: u64 =
                    rhs.parse()
                        .map_err(|_| Sl1LoadError::PlaceUnsupportedPredicate {
                            id: place_id.to_string(),
                            state: state.to_string(),
                            predicate: when.to_string(),
                        })?;
                if threshold > u64::from(SL1_PERCENT_MAX) {
                    return Err(Sl1LoadError::PlacePercentThresholdOutOfRange {
                        id: place_id.to_string(),
                        state: state.to_string(),
                        threshold,
                        max: SL1_PERCENT_MAX,
                    });
                }
                return Ok(Sl1OperatingPredicate::UsedPercentGte {
                    metric: metric.to_string(),
                    #[allow(clippy::cast_possible_truncation)]
                    threshold: threshold as u8,
                });
            }
        }
    }

    // `overloaded_ticks > <ticks>`. Use `>` only (not `>=`) per spec
    // example.
    if let Some((lhs, rhs)) = trimmed.split_once('>') {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        // Avoid catching `>=` here: if the next char of the original
        // operator was `=` then split_once('>') would have produced
        // `rhs` starting with `=`; reject that to keep `>` strict.
        if lhs == "overloaded_ticks" && !rhs.starts_with('=') {
            let ticks: u64 = rhs
                .parse()
                .map_err(|_| Sl1LoadError::PlaceUnsupportedPredicate {
                    id: place_id.to_string(),
                    state: state.to_string(),
                    predicate: when.to_string(),
                })?;
            return Ok(Sl1OperatingPredicate::OverloadedTicksGt { ticks });
        }
    }

    Err(Sl1LoadError::PlaceUnsupportedPredicate {
        id: place_id.to_string(),
        state: state.to_string(),
        predicate: when.to_string(),
    })
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
    fn non_empty_place_missing_id_hits_typed_parse_error() {
        // Now that `places` is a typed `Vec<RawSl1Place>` with strict
        // `deny_unknown_fields`, a place missing required fields like
        // `pos` no longer hits `PrimitiveNotImplemented` — it hits a
        // typed parse error from serde. Documents the post-PR-1
        // behavior change.
        let err = load_str(r#"{"places": [{"id": "p1", "role": "node"}]}"#).unwrap_err();
        match err {
            Sl1LoadError::Parse { message } => {
                assert!(message.contains("pos") || message.contains("missing field"));
            }
            other => panic!("expected Parse for missing pos field, got {other:?}"),
        }
    }

    #[test]
    fn non_empty_primitive_rejected_until_pr_lands() {
        // PRs 3-11 have no behavior for their primitive — even a
        // perfectly-shaped (empty) entry must fail load, otherwise a
        // proto-SL1 scene would silently no-op. PR 1 removed `places`
        // and PR 2 removed `links` from this guard because both are
        // now typed and validated.
        for (json, expected_section) in [
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
        // Build 100_001 valid typed RawSl1Place values to trigger the
        // section cap before any later per-entry validation runs.
        let raw = RawSl1Scene {
            schema_version: SL1_SCHEMA_VERSION,
            places: (0..=MAX_SL1_ITEMS_PER_SECTION)
                .map(|i| RawSl1Place {
                    id: format!("p{i}"),
                    role: "filler".to_string(),
                    pos: [0.0, 0.0],
                    shape: None,
                    color: None,
                    capacity: BTreeMap::new(),
                    storage: BTreeMap::new(),
                    accepts: Vec::new(),
                    produces: Vec::new(),
                    failure_domains: Vec::new(),
                    operating_states: BTreeMap::new(),
                })
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
        let err = load_str(r#"{"mystery": 1, "things": [{}]}"#).unwrap_err();
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
