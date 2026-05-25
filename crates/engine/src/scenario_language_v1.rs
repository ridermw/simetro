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

/// Upper bound for `Sl1Thing.freshness_budget_ticks`. Same rationale
/// as the link bounds: keep author-supplied tick budgets in a
/// human-meaningful range so a stray typo cannot disable freshness
/// tracking entirely.
pub const MAX_THING_FRESHNESS_BUDGET: u64 = 1_000_000_000;

/// Upper bound for transform tick-fields (cadence, duration, deadline)
/// and amount/capacity-cost values. Same human-meaningful range as the
/// other SL1 bounds.
pub const MAX_TRANSFORM_TICKS: u64 = 1_000_000_000;
pub const MAX_TRANSFORM_AMOUNT: u64 = 1_000_000_000;
pub const MAX_TRANSFORM_CAPACITY_COST: u64 = 1_000_000_000;
pub const MAX_TRANSFORM_MAX_ATTEMPTS: u32 = 1_000;

/// Upper bound for `Sl1ThingQualityContract.max_late_ticks`.
pub const MAX_THING_LATE_TICKS: u64 = 1_000_000_000;

/// Upper bound for `Sl1Demand.deadline_ticks` and the Fixed schedule's
/// `every_ticks` / `start_tick` and Scripted schedule ticks. Same
/// human-meaningful range as the other SL1 bounds.
pub const MAX_DEMAND_TICKS: u64 = 1_000_000_000;
/// Upper bound for `Sl1Demand.value`. Same rationale.
pub const MAX_DEMAND_VALUE: u64 = 1_000_000_000;
/// Upper bound for the absolute value of `Sl1DemandPenalty.score`.
/// Penalties are stored as signed but bounded to keep the future score
/// arithmetic in PR 8 inside `i64` without overflow.
pub const MAX_DEMAND_PENALTY_SCORE: i64 = 1_000_000_000;
/// Maximum number of entries in a Scripted spawn schedule.
pub const MAX_DEMAND_SCRIPTED_TICKS: usize = 100_000;
/// Maximum size of `Sl1Demand.requires`. Authors should keep demands
/// focused; long requires lists almost certainly indicate a modelling
/// mistake (mixing many unrelated facts into one demand).
pub const MAX_DEMAND_REQUIRES: usize = 64;
/// Maximum number of in-flight (Pending) demand instances per demand
/// before spawning is paused. See [`Sl1Warning::DemandBacklogOverflow`].
pub const MAX_DEMAND_OUTSTANDING: usize = 10_000;

// ---- Pressure bounds (PR 7) -----------------------------------------------

/// Upper bound for `Sl1Pressure.at_tick` and `duration_ticks`. Same
/// human-meaningful range as the other SL1 tick bounds.
pub const MAX_PRESSURE_TICKS: u64 = 1_000_000_000;
/// Upper bound for an `Sl1Pressure::SourceMultiplier.multiplier`. We
/// store the multiplier scaled to milli-units to keep the runtime
/// deterministic without floating-point arithmetic. `1.0x` is
/// represented as `1_000`. Bound matches `MAX_PRESSURE_TICKS` to keep
/// per-tick injection rates well within `u64`.
pub const MAX_PRESSURE_MULTIPLIER_MILLI: u64 = 1_000_000;
/// Upper bound for an `Sl1Pressure::DemandGrowth.spawn_multiplier`.
/// `2..=64` is the human-meaningful range; the cap is well past any
/// sensible authoring choice. A multiplier of `1` is rejected at load
/// because it would be a no-op pressure.
pub const MAX_PRESSURE_SPAWN_MULTIPLIER: u32 = 64;
/// Upper bound for an `Sl1Pressure::QuotaReduction.reduction_percent`.
/// `100` means "reduce capacity to zero". `0` is rejected at load
/// because it would be a no-op pressure.
pub const MAX_PRESSURE_REDUCTION_PERCENT: u8 = 100;

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
    /// PR 8 victory conditions (`survive_until`). Empty in earlier
    /// schemas. Optional; absent or empty leaves the scene without an
    /// explicit win mechanism, in which case `GameOutcome::Won` is
    /// unreachable (the scene is endless or loss-only).
    #[serde(default)]
    pub victory_conditions: Vec<RawSl1VictoryCondition>,
    #[serde(default)]
    pub agents: Vec<RawSl1Agent>,
    /// Optional `observability` block (PR 9). Carries declarative
    /// metric/dashboard/alert definitions. Omitted block / explicit
    /// `null` / explicit empty object `{}` all yield an empty
    /// observability. Non-object payloads (arrays, scalars) are
    /// rejected by the custom deserializer as a parse error.
    /// Strict-schema: unknown fields inside
    /// `observability`, `observability.metrics[*]`,
    /// `observability.dashboards[*]`, or `observability.alerts[*]`
    /// are rejected via serde's `deny_unknown_fields`, surfacing as
    /// [`Sl1LoadError::Parse`].
    #[serde(default, deserialize_with = "deserialize_observability")]
    pub observability: Option<RawSl1Observability>,
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
    /// PR 8 victory conditions. Empty if the scene does not declare a
    /// win mechanism (only failure conditions can end the run).
    pub victory_conditions: Vec<Sl1VictoryCondition>,
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

/// Raw, post-serde representation of a `things[]` entry. Strict-schema:
/// `#[serde(deny_unknown_fields)]` ensures nested typos do not silently
/// no-op. `schema_version` and `freshness_budget_ticks` are optional —
/// non-data things omit `schema_version`; non-budgeted things omit
/// `freshness_budget_ticks` and never transition to `Stale` purely on
/// elapsed ticks.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Thing {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub schema_version: Option<u32>,
    #[serde(default)]
    pub freshness_budget_ticks: Option<u64>,
    #[serde(default)]
    pub quality_contract: Option<RawSl1ThingQualityContract>,
    #[serde(default)]
    pub render: Option<RawSl1ThingRenderHint>,
}

/// Raw quality-contract block on a [`RawSl1Thing`]. All fields are
/// individually optional, but the block as a whole is opt-in. PR 3
/// only validates the shape; objective/failure-condition evaluation
/// lands in PR 8.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1ThingQualityContract {
    #[serde(default)]
    pub max_drop_percent: Option<f64>,
    #[serde(default)]
    pub max_late_ticks: Option<u64>,
    #[serde(default)]
    pub required_fields: Vec<String>,
}

/// Optional render hint for a [`RawSl1Thing`]. Mirrors the link
/// render hint shape: typed so typos fail load.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1ThingRenderHint {
    pub glyph: String,
    #[serde(default)]
    pub color: Option<u32>,
}

/// Validated `thing` primitive (PR 3). Declarative only — runtime
/// inventory mutation lands with transforms (PR 4) and demand
/// (PR 5).
#[derive(Debug, Clone, PartialEq)]
pub struct Sl1Thing {
    pub id: String,
    pub kind: String,
    /// Sorted + deduplicated.
    pub tags: Vec<String>,
    pub schema_version: Option<u32>,
    /// Absent means the thing is *not* time-budgeted: inventory with
    /// initial > 0 stays [`FreshnessState::Ok`] forever (until later
    /// PRs add quality-contract evaluation). Initial 0 stays
    /// [`FreshnessState::NoData`].
    pub freshness_budget_ticks: Option<u64>,
    pub quality_contract: Option<Sl1ThingQualityContract>,
    pub render: Option<Sl1ThingRenderHint>,
}

/// Validated quality contract. PR 3 carries the values opaquely;
/// PR 8 (objectives + failure conditions) evaluates them.
#[derive(Debug, Clone, PartialEq)]
pub struct Sl1ThingQualityContract {
    /// `0.0..=1.0`, finite, normalized so `-0.0` becomes `0.0`.
    pub max_drop_percent: Option<f64>,
    pub max_late_ticks: Option<u64>,
    /// Sorted + deduplicated.
    pub required_fields: Vec<String>,
}

/// Validated render hint for a [`Sl1Thing`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1ThingRenderHint {
    pub glyph: String,
    pub color: Option<u32>,
}

/// Raw, opt-in `transform` entry inside the SL1 block. PR 4 adds the
/// first deterministic transform system: cadence-driven scheduled
/// instances that consume typed inputs, reserve capacity, run for a
/// configured duration, and produce typed outputs.
///
/// The transform structure is strict (`deny_unknown_fields`); unknown
/// nested keys fail load with [`Sl1LoadError::Parse`].
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Transform {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub runs_on: String,
    #[serde(default)]
    pub inputs: Vec<RawSl1TransformIo>,
    #[serde(default)]
    pub outputs: Vec<RawSl1TransformIo>,
    pub cadence_ticks: u64,
    pub duration_ticks: u64,
    pub deadline_ticks: u64,
    #[serde(default)]
    pub capacity_cost: BTreeMap<String, u64>,
    pub failure_policy: String,
    #[serde(default = "default_transform_max_attempts")]
    pub max_attempts: u32,
}

/// Raw `inputs[]` / `outputs[]` entry for a transform. `thing` must
/// reference a declared thing id (NOT a tag — amounts are typed).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1TransformIo {
    pub thing: String,
    pub amount: u64,
}

fn default_transform_max_attempts() -> u32 {
    1
}

/// Validated transform input/output. Stable order by `thing_id` after
/// per-transform canonicalization so the deterministic hash is
/// declaration-order independent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1TransformIo {
    pub thing_id: String,
    pub amount: u64,
}

/// Validated failure policy. PR 4 ships `RetryThenWarn` and `Drop`;
/// `DegradeQuality` is rejected at load until PR 8 quality contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sl1FailurePolicy {
    /// Each cadence-scheduled instance retries up to `max_attempts`
    /// times after each `Late` outcome. After exhaustion emit one
    /// [`Sl1Warning::TransformFailed`] and reset to `Idle` for the
    /// next cadence slot.
    RetryThenWarn,
    /// Each cadence-scheduled instance gets one attempt. A `Late`
    /// outcome immediately emits [`Sl1Warning::TransformFailed`] and
    /// resets to `Idle`.
    Drop,
}

/// Validated `transform` primitive (PR 4). Deterministic work rule
/// that consumes typed inputs from `runs_on`, reserves typed capacity
/// on that place, runs for `duration_ticks`, and produces typed
/// outputs back onto the same place. Cadence is measured against
/// simulation time (`world.tick % cadence_ticks == 0`); the deadline
/// is measured from the scheduled cadence tick so blocked/starved
/// delays count against it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1Transform {
    pub id: String,
    pub kind: String,
    pub runs_on: String,
    pub inputs: Vec<Sl1TransformIo>,
    pub outputs: Vec<Sl1TransformIo>,
    pub cadence_ticks: u64,
    pub duration_ticks: u64,
    pub deadline_ticks: u64,
    pub capacity_cost: BTreeMap<String, u64>,
    pub failure_policy: Sl1FailurePolicy,
    pub max_attempts: u32,
}

/// Raw `demand[]` entry (PR 5). Strict-schema:
/// `#[serde(deny_unknown_fields)]` ensures nested typos do not silently
/// no-op even though top-level SL1 typos are also blocked.
///
/// The `spawn_schedule` is intentionally loosely typed at deserialize
/// time and dispatched in demand validation so that unsupported
/// schedule types (e.g. `"wave"` until PR 8) can return a typed
/// [`Sl1LoadError::DemandScheduleNotImplemented`] instead of serde's
/// generic "unknown variant" message. The inner
/// `#[serde(deny_unknown_fields)]` on the raw schedule still catches
/// typos like `"every-ticks"`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Demand {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub target: RawSl1DemandTarget,
    #[serde(default)]
    pub requires: Vec<String>,
    pub spawn_schedule: RawSl1DemandSchedule,
    pub deadline_ticks: u64,
    pub priority: String,
    pub value: u64,
    pub penalty: RawSl1DemandPenalty,
}

/// Raw demand target. Explicit `type` discriminator (vs. catalog
/// inference) so future PRs (8/9) can add `dashboard`, `virtual_sink`,
/// etc. without ambiguity or schema migration.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1DemandTarget {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
}

/// Raw demand spawn schedule. Each variant is checked + populated in
/// demand validation. The `kind` discriminator stays a free string
/// so unknown values can be reported with a typed
/// [`Sl1LoadError::DemandUnknownScheduleType`] instead of a generic
/// serde "unknown variant" message.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1DemandSchedule {
    #[serde(rename = "type")]
    pub kind: String,
    /// Required for `fixed`.
    #[serde(default)]
    pub every_ticks: Option<u64>,
    /// Required for `fixed`.
    #[serde(default)]
    pub start_tick: Option<u64>,
    /// Required for `scripted`. Must be strictly increasing,
    /// all > 0, length capped by [`MAX_DEMAND_SCRIPTED_TICKS`].
    #[serde(default)]
    pub ticks: Option<Vec<u64>>,
}

/// Raw demand penalty. Score is signed but bounded to
/// `-MAX_DEMAND_PENALTY_SCORE ..= 0` to make late/dropped demands
/// expensive without enabling pathological score arithmetic.
/// `warning` is an optional author-supplied severity tag carried
/// opaquely in the runtime warning payload; PR 5 does not interpret
/// it. PR 8/9 will map authored tags to a typed severity enum.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1DemandPenalty {
    pub score: i64,
    #[serde(default)]
    pub warning: Option<String>,
}

/// Validated demand target (PR 5). Only [`Self::Place`] is honored at
/// runtime; other declared kinds (`transform`, `dashboard`,
/// `virtual_sink`) are accepted by the discriminator vocabulary but
/// rejected at load with
/// [`Sl1LoadError::DemandTargetKindNotImplemented`] until PR 8/9.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sl1DemandTarget {
    Place(String),
}

/// Validated demand spawn schedule (PR 5). Wave is rejected at load
/// with [`Sl1LoadError::DemandScheduleNotImplemented`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sl1DemandSchedule {
    /// Spawn at `tick == start_tick` and every `every_ticks` thereafter.
    Fixed { every_ticks: u64, start_tick: u64 },
    /// Spawn exactly at each listed tick. Ticks are pre-sorted +
    /// strictly increasing (validator rejects duplicates and out-of-
    /// order entries) so the runtime can use a deterministic cursor.
    Scripted { ticks: Vec<u64> },
}

/// Validated demand priority (PR 5). Closed enum; unknown strings are
/// load errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1DemandPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Validated demand penalty (PR 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1DemandPenalty {
    pub score: i64,
    pub warning: Option<String>,
}

/// Validated `demand` primitive (PR 5). Deterministic spawn rule with
/// typed target, requires list, schedule, deadline, priority, value,
/// and penalty. Fulfillment semantics in PR 5 are **observation only**:
/// when all `requires` thing ids are present (count ≥ 1) at the target
/// place's inventory, the oldest Pending instance transitions to
/// Fulfilled (no inventory is decremented). This matches the spec's
/// `report_refresh` example where dashboards observe facts; future
/// PRs may introduce a consuming variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1Demand {
    pub id: String,
    pub kind: String,
    pub target: Sl1DemandTarget,
    /// Sorted + deduplicated so hash/protocol are declaration-order
    /// independent.
    pub requires: Vec<String>,
    pub spawn_schedule: Sl1DemandSchedule,
    pub deadline_ticks: u64,
    pub priority: Sl1DemandPriority,
    pub value: u64,
    pub penalty: Sl1DemandPenalty,
}

/// Raw, post-serde representation of a `pressure[]` entry. Strict
/// schema (`#[serde(deny_unknown_fields)]`) at this layer rejects any
/// nested typo, then per-variant parameter validation rejects bad
/// multiplier ranges, zero durations, unknown targets, etc. The
/// discriminator is a free string so unknown `type` values surface as
/// a typed [`Sl1LoadError::PressureUnknownType`] instead of serde's
/// generic "unknown variant" error.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Pressure {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub at_tick: u64,
    pub duration_ticks: u64,
    pub target: String,
    /// Required for `source_multiplier`. Names the `thing` id whose
    /// inventory is injected at the target place during the active
    /// window.
    #[serde(default)]
    pub thing: Option<String>,
    /// Required for `source_multiplier`. Authoring uses `f64` for
    /// human readability (e.g. `4.0`, `0.5`); validation converts to
    /// milli-units (`4000`, `500`) and stores integers for
    /// deterministic runtime arithmetic.
    #[serde(default)]
    pub multiplier: Option<f64>,
    /// Required for `demand_growth`. How many additional demand
    /// instances to enqueue per scheduled spawn while active.
    /// Effective spawn count = `1 + spawn_multiplier`; the raw author
    /// number is the multiplier itself (1 = double, 2 = triple, …).
    #[serde(default)]
    pub spawn_multiplier: Option<u32>,
    /// Required for `quota_reduction`. Names the place's capacity
    /// bucket key (matches `places[*].capacity` key set).
    #[serde(default)]
    pub capacity: Option<String>,
    /// Required for `quota_reduction`. Percent reduction in `1..=100`.
    #[serde(default)]
    pub reduction_percent: Option<u8>,
}

/// Closed enum of all pressure variants accepted in PR 7. The
/// discriminator is open-string at the raw layer so unknown types
/// surface as a typed load error; here we lock the variants to ensure
/// downstream systems can pattern-match exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1PressureKind {
    SourceMultiplier,
    DemandGrowth,
    QuotaReduction,
    PathOutage,
    // The following five variants are recognized by the schema in PR 7
    // but do not have runtime effects yet. They emit a typed
    // [`Sl1Warning::PressureUnsupportedInThisPr`] at activation so
    // authors are never misled into thinking they are silently active.
    SchemaDrift,
    DashboardStorm,
    SpotEvictionWave,
    StorageMetadataStorm,
    CoolingDegradation,
}

impl Sl1PressureKind {
    /// Canonical string form used in the loader, protocol, and hash.
    /// Stable across builds; do not rename without bumping the hash
    /// baseline.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceMultiplier => "source_multiplier",
            Self::DemandGrowth => "demand_growth",
            Self::QuotaReduction => "quota_reduction",
            Self::PathOutage => "path_outage",
            Self::SchemaDrift => "schema_drift",
            Self::DashboardStorm => "dashboard_storm",
            Self::SpotEvictionWave => "spot_eviction_wave",
            Self::StorageMetadataStorm => "storage_metadata_storm",
            Self::CoolingDegradation => "cooling_degradation",
        }
    }

    /// Whether PR 7 wires a runtime effect for this kind. The four
    /// "supported" kinds (`source_multiplier`, `demand_growth`,
    /// `quota_reduction`, `path_outage`) drive overlay state read by
    /// downstream systems; the rest emit
    /// [`Sl1Warning::PressureUnsupportedInThisPr`] at activation.
    #[must_use]
    pub const fn has_runtime_effect_in_pr7(self) -> bool {
        matches!(
            self,
            Self::SourceMultiplier | Self::DemandGrowth | Self::QuotaReduction | Self::PathOutage,
        )
    }
}

/// Per-variant typed parameters. Each variant only carries the fields
/// that variant actually uses, so downstream pattern matches cannot
/// observe stale/nonsensical parameters from other variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sl1PressureParams {
    /// Inject inventory into `target` place's storage of `thing` while
    /// active. `multiplier_milli` is the author's `multiplier` value
    /// scaled by 1000 (e.g. `4.0` → `4000`). The runtime computes
    /// per-tick inflow as `multiplier_milli / 1000` rounded toward
    /// zero (so `2.5x` → 2 per tick), clamped by storage capacity.
    SourceMultiplier {
        thing: String,
        multiplier_milli: u64,
    },
    /// While active, multiply the demand's per-spawn count by
    /// `1 + spawn_multiplier`. `target` is the demand id.
    DemandGrowth { spawn_multiplier: u32 },
    /// While active, reduce the named capacity bucket on `target`
    /// place by `reduction_percent`. Effective capacity is
    /// `floor(base * (100 - reduction_percent) / 100)`.
    QuotaReduction {
        capacity: String,
        reduction_percent: u8,
    },
    /// While active, mark `target` link as outaged. Link transport is
    /// not yet implemented in PRs 0–6, so PR 7's runtime only records
    /// the outage in overlay state and surfaces it via the snapshot;
    /// transport gating lands when links transport inventory.
    PathOutage,
    /// Recognized but not yet wired to a runtime effect. Activation
    /// emits [`Sl1Warning::PressureUnsupportedInThisPr`] once per
    /// pressure id, then deactivation proceeds normally.
    UnsupportedInThisPr,
}

/// Validated `pressure` primitive (PR 7). Common fields plus
/// variant-specific [`Sl1PressureParams`]. Scenes carry a stable,
/// id-sorted `Vec<Sl1Pressure>` so the runtime can iterate
/// deterministically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1Pressure {
    pub id: String,
    pub kind: Sl1PressureKind,
    pub at_tick: u64,
    pub duration_ticks: u64,
    pub target: String,
    pub params: Sl1PressureParams,
}

impl Sl1Pressure {
    /// Exclusive end of the pressure's active window.
    /// The active interval is `[at_tick, end_tick())`, i.e. the pressure
    /// is active while `at_tick <= now < end_tick()` and deactivates the
    /// first tick `now >= end_tick()`. The inclusive last active tick is
    /// therefore `end_tick() - 1` (always well-defined because
    /// `duration_ticks > 0` is enforced at load).
    #[must_use]
    pub fn end_tick(&self) -> u64 {
        self.at_tick.saturating_add(self.duration_ticks)
    }
}

// ---------------------------------------------------------------------------
// Objective — typed primitive (PR 8).
// ---------------------------------------------------------------------------

/// Maximum allowed `max_stale_ticks` / `max_missed` / `at_tick` for any
/// objective/failure/victory condition. Matches the pressure cap so an
/// authored scene cannot accidentally schedule a condition millions of
/// ticks into the future.
pub const MAX_OBJECTIVE_TICKS: u64 = 1_000_000;

/// Maximum allowed `weight` on an `objective`. Keeps deterministic
/// `weight * status` integer arithmetic well below `u32::MAX` so future
/// score aggregations cannot silently overflow.
pub const MAX_OBJECTIVE_WEIGHT: u32 = 10_000;

/// Maximum allowed `max_count` on `objective_breach_count` failure
/// conditions. Beyond this the FC is effectively un-fireable for any
/// realistic scene length, so we reject it at load to catch typos.
pub const MAX_OBJECTIVE_BREACH_COUNT: u64 = 1_000_000;

/// Maximum number of observability items per list
/// (`observability.metrics`, `observability.dashboards`,
/// `observability.alerts`). Beyond this the per-tick derivation loop
/// becomes a wall-clock latency concern even with strictly-bounded
/// inner work. Far below the generic per-section cap because each
/// observability item touches metric state every tick.
pub const MAX_OBSERVABILITY_ITEMS: usize = 1_000;

/// Maximum allowed `freshness_slo_ticks` on a dashboard. Equal to
/// [`MAX_OBJECTIVE_TICKS`] so dashboard SLOs can match the longest
/// objective window. Zero is rejected with
/// [`Sl1LoadError::DashboardFreshnessSloZero`].
pub const MAX_DASHBOARD_FRESHNESS_SLO_TICKS: u64 = 1_000_000;

/// Raw `objectives[]` entry. Strict-schema; unknown fields rejected.
/// The discriminator is a free string so unknown `type` surfaces as
/// [`Sl1LoadError::ObjectiveUnknownType`] instead of serde's generic
/// "unknown variant" error.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Objective {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// Optional weight for future scoring (PR 13 policy search).
    /// Default 1. Must be in `1..=MAX_OBJECTIVE_WEIGHT`.
    #[serde(default)]
    pub weight: Option<u32>,
    // KeepFresh / StaleTarget-style fields.
    #[serde(default)]
    pub place: Option<String>,
    #[serde(default)]
    pub thing: Option<String>,
    #[serde(default)]
    pub max_stale_ticks: Option<u64>,
    // CompleteJobsBeforeDeadline.
    #[serde(default)]
    pub demand: Option<String>,
    #[serde(default)]
    pub max_missed: Option<u64>,
    // MaintainUtilization.
    #[serde(default)]
    pub capacity: Option<String>,
    #[serde(default)]
    pub min_percent: Option<u8>,
    #[serde(default)]
    pub max_percent: Option<u8>,
    // CostBudget (recognized-but-unsupported in PR 8).
    #[serde(default)]
    pub max_cost: Option<u64>,
    // DataQuality (recognized-but-unsupported).
    #[serde(default)]
    pub max_contract_violations: Option<u64>,
    // QueryLatency (recognized-but-unsupported).
    #[serde(default)]
    pub p95_max_ticks: Option<u64>,
    /// Common free-text target used by `data_quality` /
    /// `query_latency` / `cost_budget` (unsupported in PR 8). Still
    /// validated to resolve to a declared id where possible.
    #[serde(default)]
    pub target: Option<String>,
}

/// Closed enum of objective kinds. The discriminator is open at the
/// raw layer so unknowns surface as a typed load error; here we lock
/// it so downstream pattern matches can be exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1ObjectiveKind {
    /// Inventory of `thing` on `place` must stay within
    /// `max_stale_ticks` of its `freshness_budget_ticks`.
    KeepFresh,
    /// At most `max_missed` instances of `demand` may be dropped or
    /// late since scene start.
    CompleteJobsBeforeDeadline,
    /// The named capacity bucket on `place` must stay within the
    /// `[min_percent, max_percent]` utilization range.
    MaintainUtilization,
    /// Recognized-but-unsupported in PR 8. Emits
    /// [`Sl1Warning::ObjectiveUnsupportedInThisPr`] once on first
    /// evaluation; status stays `Unsupported` for the rest of the run.
    CostBudget,
    /// Recognized-but-unsupported in PR 8.
    DataQuality,
    /// Recognized-but-unsupported in PR 8.
    QueryLatency,
}

impl Sl1ObjectiveKind {
    /// Canonical wire string. Stable; do not rename without bumping
    /// affected hash baselines.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KeepFresh => "keep_fresh",
            Self::CompleteJobsBeforeDeadline => "complete_jobs_before_deadline",
            Self::MaintainUtilization => "maintain_utilization",
            Self::CostBudget => "cost_budget",
            Self::DataQuality => "data_quality",
            Self::QueryLatency => "query_latency",
        }
    }

    /// Whether PR 8 actually evaluates the variant.
    #[must_use]
    pub const fn has_runtime_effect_in_pr8(self) -> bool {
        matches!(
            self,
            Self::KeepFresh | Self::CompleteJobsBeforeDeadline | Self::MaintainUtilization,
        )
    }
}

/// Per-variant typed parameters. Each variant only carries the fields
/// it actually consumes; downstream pattern matches cannot observe
/// stale parameters from a sibling variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sl1ObjectiveParams {
    KeepFresh {
        place: String,
        thing: String,
        max_stale_ticks: u64,
    },
    CompleteJobsBeforeDeadline {
        demand: String,
        max_missed: u64,
    },
    MaintainUtilization {
        place: String,
        capacity: String,
        min_percent: u8,
        max_percent: u8,
    },
    /// Variant accepted at load but not evaluated. The objective
    /// surfaces as `status == Unsupported` for the whole run.
    UnsupportedInThisPr,
}

/// Validated `objective` primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1Objective {
    pub id: String,
    pub kind: Sl1ObjectiveKind,
    pub weight: u32,
    pub params: Sl1ObjectiveParams,
}

// ---------------------------------------------------------------------------
// FailureCondition — typed primitive (PR 8).
// ---------------------------------------------------------------------------

/// Raw `failure_conditions[]` entry. Strict-schema.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1FailureCondition {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    // StaleTarget.
    #[serde(default)]
    pub place: Option<String>,
    #[serde(default)]
    pub thing: Option<String>,
    #[serde(default)]
    pub threshold_ticks: Option<u64>,
    #[serde(default)]
    pub grace_ticks: Option<u64>,
    // PlaceState.
    #[serde(default)]
    pub state: Option<String>,
    // ObjectiveBreachCount.
    #[serde(default)]
    pub objective_id: Option<String>,
    #[serde(default)]
    pub max_count: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1FailureConditionKind {
    StaleTarget,
    PlaceState,
    ObjectiveBreachCount,
}

impl Sl1FailureConditionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleTarget => "stale_target",
            Self::PlaceState => "place_state",
            Self::ObjectiveBreachCount => "objective_breach_count",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sl1FailureConditionParams {
    StaleTarget {
        place: String,
        thing: String,
        threshold_ticks: u64,
        grace_ticks: u64,
    },
    PlaceState {
        place: String,
        state: String,
        grace_ticks: u64,
    },
    ObjectiveBreachCount {
        objective_id: String,
        max_count: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1FailureCondition {
    pub id: String,
    pub kind: Sl1FailureConditionKind,
    pub params: Sl1FailureConditionParams,
}

// ---------------------------------------------------------------------------
// VictoryCondition — typed primitive (PR 8).
// ---------------------------------------------------------------------------

/// Raw `victory_conditions[]` entry. PR 8 supports the single variant
/// `survive_until { at_tick }`; the enum is left open at the
/// discriminator for future variants.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1VictoryCondition {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// Required for `survive_until`.
    #[serde(default)]
    pub at_tick: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1VictoryConditionKind {
    SurviveUntil,
}

impl Sl1VictoryConditionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SurviveUntil => "survive_until",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sl1VictoryConditionParams {
    /// Met the first tick `now >= at_tick` with no failure condition
    /// previously fired.
    SurviveUntil { at_tick: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1VictoryCondition {
    pub id: String,
    pub kind: Sl1VictoryConditionKind,
    pub params: Sl1VictoryConditionParams,
}

// ---------------------------------------------------------------------------
// Agent — typed primitive (PR 10).
//
// SL1 agents are declarative actors that observe scoped runtime state at
// fixed intervals and propose typed actions. PR 10 supports one real
// action (`throttle_demand`) and ships scripted/mock backends for
// deterministic tests. The `llm` backend is reserved: declarations load
// successfully but the backend produces no decisions in CI and emits a
// one-shot `LlmBackendDisabled` event so authors can tell the difference
// between "agent paused itself" and "live LLM not wired up".
//
// Strict-schema (`deny_unknown_fields`) on `agents[*]` and `agents[*]
// .budgets`. `kind`, `observation_scope[*]`, and `allowed_actions[*]`
// are open-string at the raw layer so unknown variants surface as a
// typed `Sl1LoadError` rather than serde's English text.
// ---------------------------------------------------------------------------

/// Raw `agents[*]` entry. Strict-schema.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Agent {
    pub id: String,
    pub kind: String,
    pub role: String,
    pub interval_ticks: u64,
    #[serde(default)]
    pub observation_scope: Vec<String>,
    #[serde(default)]
    pub allowed_actions: Vec<String>,
    pub budgets: RawSl1AgentBudgets,
    /// Per-objective weights in `[0, 1]`. Used by future built-in
    /// agents to prioritize objectives; PR 10 stores them but the
    /// scripted backend ignores them.
    #[serde(default)]
    pub objective_weights: BTreeMap<String, f64>,
}

/// Raw `agents[*].budgets`. Strict-schema.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1AgentBudgets {
    pub max_cost_per_decision: u64,
    pub cooldown_ticks: u64,
}

/// Validated `agents[*]` entry. PR 10.
#[derive(Debug, Clone, PartialEq)]
pub struct Sl1Agent {
    pub id: String,
    pub kind: Sl1AgentKind,
    pub role: String,
    pub interval_ticks: u64,
    pub observation_scope: Vec<Sl1AgentObservationTarget>,
    pub allowed_actions: Vec<Sl1AgentActionKind>,
    pub max_cost_per_decision: u64,
    pub cooldown_ticks: u64,
    pub objective_weights: BTreeMap<String, f64>,
}

/// Closed enum of agent backend kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1AgentKind {
    /// No-op backend. Never produces a decision. Used in CI baselines.
    Mock,
    /// Deterministic rule-based backend. PR 10 ships this variant as
    /// "no decision" until a later PR wires concrete heuristics.
    Builtin,
    /// Live LLM backend. PR 10 always returns `None` and emits a
    /// one-shot `SimEvent::Sl1AgentLlmDisabled` so authors can
    /// distinguish "feature-gated off" from "agent chose not to act".
    Llm,
}

impl Sl1AgentKind {
    /// Canonical wire string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Builtin => "builtin",
            Self::Llm => "llm",
        }
    }
}

/// Targets an agent may observe at each decision tick. Authors declare
/// scope entries as `"<kind>:<id>"` strings; the loader parses them
/// into typed targets and validates each id against the matching
/// scene-level catalog.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum Sl1AgentObservationTarget {
    Place(String),
    Transform(String),
    Demand(String),
    Dashboard(String),
    Metric(String),
}

impl Sl1AgentObservationTarget {
    /// Canonical wire prefix.
    #[must_use]
    pub const fn kind_str(&self) -> &'static str {
        match self {
            Self::Place(_) => "place",
            Self::Transform(_) => "transform",
            Self::Demand(_) => "demand",
            Self::Dashboard(_) => "dashboard",
            Self::Metric(_) => "metric",
        }
    }
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Place(s)
            | Self::Transform(s)
            | Self::Demand(s)
            | Self::Dashboard(s)
            | Self::Metric(s) => s.as_str(),
        }
    }
}

/// Closed enum of agent action kinds. PR 10 only implements
/// `ThrottleDemand` at runtime; the other variants are accepted in
/// `allowed_actions` declarations so authors can prepare for future
/// PRs without churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum Sl1AgentActionKind {
    SetJobPriority,
    ThrottleDemand,
    ScalePlaceCapacity,
    WarmCache,
    PrioritizeTransform,
    PauseReportRefresh,
}

impl Sl1AgentActionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SetJobPriority => "set_job_priority",
            Self::ThrottleDemand => "throttle_demand",
            Self::ScalePlaceCapacity => "scale_place_capacity",
            Self::WarmCache => "warm_cache",
            Self::PrioritizeTransform => "prioritize_transform",
            Self::PauseReportRefresh => "pause_report_refresh",
        }
    }
}

/// A typed action proposed by an agent backend. PR 10 only implements
/// [`Sl1AgentAction::ThrottleDemand`]; future PRs add the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sl1AgentAction {
    /// Pause spawn of the named demand for `pause_ticks` ticks
    /// starting on the next tick after the agent decides.
    ThrottleDemand { demand_id: String, pause_ticks: u64 },
}

impl Sl1AgentAction {
    #[must_use]
    pub const fn kind(&self) -> Sl1AgentActionKind {
        match self {
            Self::ThrottleDemand { .. } => Sl1AgentActionKind::ThrottleDemand,
        }
    }
    /// Unit cost in PR 10. Future PRs may vary cost by variant.
    #[must_use]
    pub const fn cost(&self) -> u64 {
        1
    }
}

/// Per-agent runtime state (PR 10). Tracks the last tick the agent
/// fired, the tick its cooldown expires at, whether the one-shot
/// "live LLM disabled" event has been emitted, and any agent-owned
/// demand pauses keyed by demand id.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1AgentRuntimeState {
    /// Last tick on which the agent's `interval_ticks` cadence fired.
    /// `None` means the agent has not yet fired.
    pub last_decision_tick: Option<u64>,
    /// Cooldown deadline tick (exclusive). No new actions may apply
    /// while `now < cooldown_until_tick`.
    pub cooldown_until_tick: Option<u64>,
    /// True once `Sl1AgentLlmDisabled` has been emitted for this
    /// agent, so the warning fires exactly once per scene run.
    pub llm_disabled_emitted: bool,
}

/// Reason an agent action was rejected. Carried on
/// `SimEvent::Sl1AgentActionRejected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1AgentRejectionReason {
    /// The proposed action's kind is not in this agent's
    /// `allowed_actions`.
    ActionNotAllowed,
    /// The action's target id is not in this agent's
    /// `observation_scope`.
    ActionTargetOutOfScope,
    /// The action's target id does not resolve to a declared scene
    /// element.
    ActionTargetUnknown,
    /// `action.cost() > agent.max_cost_per_decision`.
    CostExceedsBudget,
    /// The agent is currently cooling down from a previous action.
    Cooldown,
    /// The runtime understood the action but PR 10 does not implement
    /// its effect. Reserved for future variants of
    /// [`Sl1AgentAction`].
    EffectUnsupportedInThisPr,
    /// Action carried an out-of-range or nonsensical parameter (e.g.
    /// `pause_ticks == 0`).
    InvalidActionParameter,
}

impl Sl1AgentRejectionReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActionNotAllowed => "action_not_allowed",
            Self::ActionTargetOutOfScope => "action_target_out_of_scope",
            Self::ActionTargetUnknown => "action_target_unknown",
            Self::CostExceedsBudget => "cost_exceeds_budget",
            Self::Cooldown => "cooldown",
            Self::EffectUnsupportedInThisPr => "effect_unsupported_in_this_pr",
            Self::InvalidActionParameter => "invalid_action_parameter",
        }
    }
}

/// PR 10 bound on per-agent `interval_ticks` to avoid pathological
/// schedules. Same shape as observability caps.
pub const SL1_AGENT_MAX_INTERVAL_TICKS: u64 = 1_000_000;
/// PR 10 bound on per-agent `cooldown_ticks` and per-action
/// `pause_ticks`. Same shape as observability caps.
pub const SL1_AGENT_MAX_COOLDOWN_TICKS: u64 = 1_000_000;
/// Maximum entries in `observation_scope` and `allowed_actions`.
pub const SL1_AGENT_MAX_LIST_LEN: usize = 256;
/// Maximum entries in `objective_weights`.
pub const SL1_AGENT_MAX_OBJECTIVE_WEIGHTS: usize = 256;

// ---------------------------------------------------------------------------
// Observability — typed primitive (PR 9).
//
// Three sub-primitives:
//   1. `metrics`     — point-in-time scalar values derived from runtime
//                      state (place capacity utilization, inventory
//                      counts, dashboard freshness ticks).
//   2. `dashboards`  — aggregated views over a set of `things`; their
//                      state (`ok`/`stale`/`no_data`) becomes a demand
//                      target signal in PR 10.
//   3. `alerts`      — edge-triggered predicate evaluations against a
//                      metric; emit `Sl1AlertFired` + `Sl1AlertCleared`
//                      events on transitions.
//
// Each primitive is strict-schema (`deny_unknown_fields`). Metric
// `source` is an open-string discriminator at the raw layer (so unknown
// kinds surface as a typed `Sl1LoadError` rather than serde's English
// text), then parsed programmatically into a closed enum.
// ---------------------------------------------------------------------------

/// Raw `observability` block. Strict-schema.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Observability {
    #[serde(default)]
    pub metrics: Vec<RawSl1Metric>,
    #[serde(default)]
    pub dashboards: Vec<RawSl1Dashboard>,
    #[serde(default)]
    pub alerts: Vec<RawSl1Alert>,
}

/// Custom deserializer for the top-level `observability` field.
///
/// The plain `#[serde(default)] Option<RawSl1Observability>` derive
/// accepts `"observability": []` as an empty struct because every
/// inner field is `#[serde(default)]`. That silently swallows
/// malformed authored scenes that pass an array (or other non-object
/// shape) and weakens the strict-schema guarantee.
///
/// This function inspects the raw JSON value first and rejects
/// anything that isn't `null` or an object with a typed parse error
/// matching [`Sl1LoadError::Parse`] semantics.
fn deserialize_observability<'de, D>(d: D) -> Result<Option<RawSl1Observability>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value: Option<serde_json::Value> = Option::deserialize(d)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(v @ serde_json::Value::Object(_)) => serde_json::from_value(v)
            .map(Some)
            .map_err(D::Error::custom),
        Some(other) => Err(D::Error::custom(format!(
            "observability must be a JSON object, got {}",
            match other {
                serde_json::Value::Array(_) => "array",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::Bool(_) => "boolean",
                _ => "non-object",
            }
        ))),
    }
}

/// Raw `observability.metrics[*]` entry. Strict-schema.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Metric {
    pub id: String,
    /// Discriminator. Open string at the raw layer; parsed into
    /// [`Sl1MetricSourceKind`] by the loader so unknowns become a
    /// typed [`Sl1LoadError::MetricUnsupportedSource`].
    pub source: String,
    /// Required when `source = "place_capacity_used_percent"` or
    /// `source = "place_inventory_count"`. Names a declared place id.
    #[serde(default)]
    pub place: Option<String>,
    /// Required when `source = "place_capacity_used_percent"`. Names a
    /// capacity-bucket key on the named place.
    #[serde(default)]
    pub capacity: Option<String>,
    /// Required when `source = "place_inventory_count"`. Names a
    /// declared thing id stored on the named place.
    #[serde(default)]
    pub thing: Option<String>,
    /// Required when `source = "dashboard_freshness"`. Names a
    /// declared dashboard id in this `observability.dashboards[]`.
    #[serde(default)]
    pub dashboard: Option<String>,
}

/// Raw `observability.dashboards[*]` entry. Strict-schema.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Dashboard {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// Things whose freshness rolls up into this dashboard. Order in
    /// the source is preserved for human-readable output, but freshness
    /// aggregation is order-independent (max age).
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Maximum age (in ticks) before the dashboard transitions to
    /// `Stale`. Must be `> 0` (zero would mean "stale immediately on
    /// the same tick the data was set", which is always an authoring
    /// mistake).
    pub freshness_slo_ticks: u64,
}

/// Raw `observability.alerts[*]` entry. Strict-schema.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawSl1Alert {
    pub id: String,
    /// Names a metric in this `observability.metrics[]`.
    pub metric: String,
    pub predicate: RawSl1AlertPredicate,
    /// Severity hint for the renderer/agent prompts. Open string at
    /// raw layer; parsed into [`Sl1AlertSeverity`].
    pub severity: String,
}

/// Raw alert predicate. Strict-schema. The closed enum
/// [`Sl1AlertPredicate`] mirrors this with validated bounds.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawSl1AlertPredicate {
    Gt {
        threshold: u64,
    },
    Lt {
        threshold: u64,
    },
    /// Inclusive on both bounds: fires when `value < min` or
    /// `value > max`. Note: this is "out of range" semantics — the
    /// alert fires when the metric leaves the band. PR 9 uses this
    /// flavor because alerts represent abnormal conditions; staying
    /// inside the band is the healthy state.
    OutOfRange {
        min: u64,
        max: u64,
    },
}

/// Validated `observability` primitive.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1Observability {
    pub metrics: Vec<Sl1Metric>,
    pub dashboards: Vec<Sl1Dashboard>,
    pub alerts: Vec<Sl1Alert>,
}

/// Validated `metric` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1Metric {
    pub id: String,
    pub source: Sl1MetricSource,
}

/// Closed enum of metric sources. The discriminator is open-string at
/// the raw layer so unknown kinds surface as a typed load error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1MetricSourceKind {
    PlaceCapacityUsedPercent,
    PlaceInventoryCount,
    DashboardFreshness,
}

impl Sl1MetricSourceKind {
    /// Canonical wire string. Stable across builds; do not rename
    /// without bumping affected hash baselines.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlaceCapacityUsedPercent => "place_capacity_used_percent",
            Self::PlaceInventoryCount => "place_inventory_count",
            Self::DashboardFreshness => "dashboard_freshness",
        }
    }
}

/// Per-variant typed metric source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sl1MetricSource {
    /// `(used / capacity) * 100` for the named bucket on the named
    /// place. `value` is `0..=100`. If the capacity bucket value is
    /// zero (e.g. capacity dropped to 0 by `quota_reduction`), the
    /// derived percent is `0` rather than triggering a divide-by-zero
    /// silent skip.
    PlaceCapacityUsedPercent { place: String, capacity: String },
    /// Current inventory count for the named `(place, thing)` pair.
    PlaceInventoryCount { place: String, thing: String },
    /// Freshness age (in ticks) of the named dashboard. `value` is
    /// `0` when the dashboard is freshly populated this tick, and
    /// grows monotonically until the dashboard's `depends_on` storage
    /// is refreshed. Stays `Ok` past the dashboard's
    /// `freshness_slo_ticks` so threshold alerts on freshness can
    /// fire — the dashboard's own state separately transitions to
    /// [`Sl1DashboardState::Stale`].
    DashboardFreshness { dashboard: String },
}

impl Sl1MetricSource {
    #[must_use]
    pub const fn kind(&self) -> Sl1MetricSourceKind {
        match self {
            Self::PlaceCapacityUsedPercent { .. } => Sl1MetricSourceKind::PlaceCapacityUsedPercent,
            Self::PlaceInventoryCount { .. } => Sl1MetricSourceKind::PlaceInventoryCount,
            Self::DashboardFreshness { .. } => Sl1MetricSourceKind::DashboardFreshness,
        }
    }
}

/// Validated `dashboard` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1Dashboard {
    pub id: String,
    pub kind: Sl1DashboardKind,
    pub depends_on: Vec<String>,
    pub freshness_slo_ticks: u64,
}

/// Closed enum of dashboard kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1DashboardKind {
    /// Periodic refresh report (e.g. Power BI overnight refresh).
    Report,
    /// Streaming dashboard (e.g. Kusto live tile).
    Live,
    /// Author-run ad-hoc query view.
    AdHoc,
}

impl Sl1DashboardKind {
    /// Canonical wire string. Stable.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Report => "report",
            Self::Live => "live",
            Self::AdHoc => "ad_hoc",
        }
    }
}

/// Validated `alert` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sl1Alert {
    pub id: String,
    pub metric: String,
    pub predicate: Sl1AlertPredicate,
    pub severity: Sl1AlertSeverity,
}

/// Validated alert predicate. Bounds are inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sl1AlertPredicate {
    Gt {
        threshold: u64,
    },
    Lt {
        threshold: u64,
    },
    /// Fires when `value < min` or `value > max` (out-of-band). Loader
    /// enforces `min <= max`.
    OutOfRange {
        min: u64,
        max: u64,
    },
}

impl Sl1AlertPredicate {
    /// Evaluate against a metric value. Returns true if the alert
    /// should be firing for this value.
    #[must_use]
    pub const fn fires(self, value: u64) -> bool {
        match self {
            Self::Gt { threshold } => value > threshold,
            Self::Lt { threshold } => value < threshold,
            Self::OutOfRange { min, max } => value < min || value > max,
        }
    }

    /// Stable wire-friendly summary for events/logs.
    #[must_use]
    pub fn summary(self) -> String {
        match self {
            Self::Gt { threshold } => format!("> {threshold}"),
            Self::Lt { threshold } => format!("< {threshold}"),
            Self::OutOfRange { min, max } => format!("out_of_range [{min}, {max}]"),
        }
    }
}

/// Severity classification for an alert. Renderer maps to color/icon;
/// agent prompts (PR 10) treat `Critical` as highest priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl Sl1AlertSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

/// Per-metric runtime state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Sl1MetricState {
    /// The metric resolved to a value this tick.
    Ok { value: u64 },
    /// The metric's source returned no data (e.g. all `depends_on`
    /// things have never been populated for a `dashboard_freshness`
    /// metric). Distinct from `Ok { value: 0 }` so alert predicates
    /// can ignore "no data" without misinterpreting it as healthy.
    #[default]
    NoData,
}

impl Sl1MetricState {
    /// Canonical wire string of the discriminant.
    #[must_use]
    pub const fn discriminant_str(&self) -> &'static str {
        match self {
            Self::Ok { .. } => "ok",
            Self::NoData => "no_data",
        }
    }
}

/// Per-dashboard runtime state. `Stale` carries the current freshness
/// age so the HUD can show how stale the dashboard is.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Sl1DashboardState {
    Ok,
    /// At least one of the dashboard's `depends_on` things has aged
    /// past `freshness_slo_ticks`. `freshness_ticks` is the maximum
    /// age across all `depends_on` things.
    Stale {
        freshness_ticks: u64,
    },
    /// At least one of the `depends_on` things has never been
    /// populated anywhere (no `FreshnessState::Ok` for it). Distinct
    /// from `Stale` so the HUD can warn that a feed is missing
    /// entirely rather than just lagging.
    #[default]
    NoData,
}

impl Sl1DashboardState {
    #[must_use]
    pub const fn discriminant_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Stale { .. } => "stale",
            Self::NoData => "no_data",
        }
    }
}

/// Per-alert runtime state. Edge-triggered: transitions from
/// `Inactive` → `Firing` emit `Sl1AlertFired`, and `Firing` →
/// `Inactive` emit `Sl1AlertCleared`. Same-state ticks do not emit
/// events.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Sl1AlertState {
    #[default]
    Inactive,
    /// The alert predicate is currently firing. `fired_at_tick` is
    /// the tick of the most recent `Inactive` → `Firing` transition.
    Firing { fired_at_tick: u64 },
}

impl Sl1AlertState {
    #[must_use]
    pub const fn discriminant_str(&self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Firing { .. } => "firing",
        }
    }

    #[must_use]
    pub const fn is_firing(&self) -> bool {
        matches!(self, Self::Firing { .. })
    }
}

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
#[derive(Debug, Error, PartialEq, Clone)]
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

    /// A `links[].compatibility` entry references neither a declared
    /// `things[].id` nor a declared `things[].tag`. PR 3 adds this
    /// cross-check now that things are typed.
    #[error(
        "scenario_language_v1.links[{id:?}].compatibility: \
         {value:?} is not a declared thing id or tag"
    )]
    LinkCompatibilityUnknownReference { id: String, value: String },

    // ---- Thing primitive (PR 3) -------------------------------------------
    /// Two `things[]` entries declared the same `id`.
    #[error("scenario_language_v1.things: duplicate id {id:?}")]
    ThingDuplicateId { id: String },

    /// A thing `id` is empty, too long, or contains characters outside
    /// the allowed alphanumeric/`_`/`-` charset.
    #[error("scenario_language_v1.things: invalid id {id:?}")]
    ThingInvalidId { id: String },

    /// A thing `kind` is empty.
    #[error("scenario_language_v1.things[{id:?}].kind: must be non-empty")]
    ThingEmptyKind { id: String },

    /// A `tags` entry is empty.
    #[error("scenario_language_v1.things[{id:?}].tags: empty entry not allowed")]
    ThingEmptyTag { id: String },

    /// A `tags` list contains a duplicate entry.
    #[error("scenario_language_v1.things[{id:?}].tags: duplicate entry {value:?}")]
    ThingDuplicateTag { id: String, value: String },

    /// `schema_version` was supplied but is zero. Zero-version data is
    /// almost certainly an authoring mistake; valid schemas start at 1.
    #[error("scenario_language_v1.things[{id:?}].schema_version: must be >= 1 if present")]
    ThingSchemaVersionZero { id: String },

    /// `freshness_budget_ticks` was supplied but is zero. Omit the
    /// field entirely to declare a thing as non-time-budgeted.
    #[error(
        "scenario_language_v1.things[{id:?}].freshness_budget_ticks: \
         must be > 0 if present (omit field for non-budgeted things)"
    )]
    ThingFreshnessBudgetZero { id: String },

    /// `freshness_budget_ticks` exceeds [`MAX_THING_FRESHNESS_BUDGET`].
    #[error(
        "scenario_language_v1.things[{id:?}].freshness_budget_ticks: \
         {value} exceeds maximum {max}"
    )]
    ThingFreshnessBudgetOutOfRange { id: String, value: u64, max: u64 },

    /// `quality_contract.max_drop_percent` is non-finite or outside
    /// `0.0..=1.0`.
    #[error(
        "scenario_language_v1.things[{id:?}].quality_contract.max_drop_percent: \
         {value} not finite or outside 0.0..=1.0"
    )]
    ThingQualityMaxDropPercentOutOfRange { id: String, value: f64 },

    /// `quality_contract.max_late_ticks` is zero. Omit the field
    /// entirely to declare no lateness budget.
    #[error(
        "scenario_language_v1.things[{id:?}].quality_contract.max_late_ticks: \
         must be > 0 if present"
    )]
    ThingQualityMaxLateTicksZero { id: String },

    /// `quality_contract.max_late_ticks` exceeds [`MAX_THING_LATE_TICKS`].
    #[error(
        "scenario_language_v1.things[{id:?}].quality_contract.max_late_ticks: \
         {value} exceeds maximum {max}"
    )]
    ThingQualityMaxLateTicksOutOfRange { id: String, value: u64, max: u64 },

    /// A `quality_contract.required_fields` entry is empty.
    #[error(
        "scenario_language_v1.things[{id:?}].quality_contract.required_fields: \
         empty entry not allowed"
    )]
    ThingQualityRequiredFieldEmpty { id: String },

    /// A `quality_contract.required_fields` list contains a duplicate
    /// entry.
    #[error(
        "scenario_language_v1.things[{id:?}].quality_contract.required_fields: \
         duplicate entry {value:?}"
    )]
    ThingQualityRequiredFieldDuplicate { id: String, value: String },

    /// `render.glyph` is empty.
    #[error(
        "scenario_language_v1.things[{id:?}].render.glyph: \
         empty string (omit render for default)"
    )]
    ThingEmptyRenderGlyph { id: String },

    /// A `places[*].storage[*]` key references a thing id that is not
    /// declared in `things[]`. PR 3 adds this cross-check now that
    /// things are typed; PR 1 fixtures using untyped storage keys must
    /// either declare the matching thing or remove the storage entry.
    #[error(
        "scenario_language_v1.places[{place_id:?}].storage: \
         {thing_id:?} is not a declared thing id"
    )]
    PlaceStorageUnknownThing { place_id: String, thing_id: String },

    /// A `places[*].accepts` or `places[*].produces` entry references
    /// neither a declared `things[].id` nor a declared `things[].tag`.
    /// `field` is `"accepts"` or `"produces"`.
    #[error(
        "scenario_language_v1.places[{place_id:?}].{field}: \
         {value:?} is not a declared thing id or tag"
    )]
    PlaceUnknownThingReference {
        place_id: String,
        field: &'static str,
        value: String,
    },

    /// `transforms[].id` does not satisfy `is_valid_sl1_id`.
    #[error("scenario_language_v1.transforms[{id:?}].id: invalid identifier")]
    TransformInvalidId { id: String },

    /// `transforms[].id` collides with another transform.
    #[error("scenario_language_v1.transforms: duplicate id {id:?}")]
    TransformDuplicateId { id: String },

    /// `transforms[].type` is empty after trimming.
    #[error("scenario_language_v1.transforms[{id:?}].type: must be non-empty")]
    TransformEmptyType { id: String },

    /// `transforms[].runs_on` does not match any declared place id.
    #[error(
        "scenario_language_v1.transforms[{id:?}].runs_on: \
         {place:?} is not a declared place"
    )]
    TransformUnknownPlace { id: String, place: String },

    /// `transforms[].outputs` is empty. Every transform must declare
    /// at least one output (a "side-effect only" rule should be
    /// modelled as a typed observability event in PR 9).
    #[error("scenario_language_v1.transforms[{id:?}].outputs: must declare at least one output")]
    TransformEmptyOutputs { id: String },

    /// `transforms[].inputs[i].thing` (or `outputs[i].thing`) does
    /// not match any declared thing id. Tags are not accepted here.
    /// `field` is `"inputs"` or `"outputs"`.
    #[error(
        "scenario_language_v1.transforms[{id:?}].{field}: \
         {value:?} is not a declared thing id"
    )]
    TransformUnknownThing {
        id: String,
        field: &'static str,
        value: String,
    },

    /// Two `transforms[].inputs[]` (or `outputs[]`) entries name the
    /// same thing. Sum amounts in JSON instead of repeating.
    #[error(
        "scenario_language_v1.transforms[{id:?}].{field}: \
         duplicate thing entry {value:?}"
    )]
    TransformDuplicateIo {
        id: String,
        field: &'static str,
        value: String,
    },

    /// `transforms[].inputs[i].amount` or `outputs[i].amount` is zero.
    /// Zero-amount IO is meaningless and almost certainly a typo.
    #[error(
        "scenario_language_v1.transforms[{id:?}].{field}[{thing:?}].amount: \
         must be > 0"
    )]
    TransformIoAmountZero {
        id: String,
        field: &'static str,
        thing: String,
    },

    /// `amount` exceeds [`MAX_TRANSFORM_AMOUNT`].
    #[error(
        "scenario_language_v1.transforms[{id:?}].{field}[{thing:?}].amount: \
         {value} exceeds maximum {max}"
    )]
    TransformIoAmountOutOfRange {
        id: String,
        field: &'static str,
        thing: String,
        value: u64,
        max: u64,
    },

    /// `transforms[].cadence_ticks` is zero.
    #[error("scenario_language_v1.transforms[{id:?}].cadence_ticks: must be > 0")]
    TransformCadenceZero { id: String },

    /// `transforms[].duration_ticks` is zero.
    #[error("scenario_language_v1.transforms[{id:?}].duration_ticks: must be > 0")]
    TransformDurationZero { id: String },

    /// `transforms[].deadline_ticks` is zero.
    #[error("scenario_language_v1.transforms[{id:?}].deadline_ticks: must be > 0")]
    TransformDeadlineZero { id: String },

    /// `deadline_ticks < duration_ticks` — the transform can never
    /// complete inside its deadline.
    #[error(
        "scenario_language_v1.transforms[{id:?}].deadline_ticks: \
         {deadline} is less than duration_ticks {duration}"
    )]
    TransformDeadlineLessThanDuration {
        id: String,
        deadline: u64,
        duration: u64,
    },

    /// Any of `cadence_ticks`, `duration_ticks`, `deadline_ticks`
    /// exceed [`MAX_TRANSFORM_TICKS`].
    #[error(
        "scenario_language_v1.transforms[{id:?}].{field}: \
         {value} exceeds maximum {max}"
    )]
    TransformTicksOutOfRange {
        id: String,
        field: &'static str,
        value: u64,
        max: u64,
    },

    /// `capacity_cost` has an empty key.
    #[error("scenario_language_v1.transforms[{id:?}].capacity_cost: empty key not allowed")]
    TransformCapacityCostEmptyKey { id: String },

    /// `capacity_cost.<key>` is zero.
    #[error(
        "scenario_language_v1.transforms[{id:?}].capacity_cost[{key:?}]: \
         must be > 0"
    )]
    TransformCapacityCostZero { id: String, key: String },

    /// `capacity_cost.<key>` exceeds [`MAX_TRANSFORM_CAPACITY_COST`].
    #[error(
        "scenario_language_v1.transforms[{id:?}].capacity_cost[{key:?}]: \
         {value} exceeds maximum {max}"
    )]
    TransformCapacityCostOutOfRange {
        id: String,
        key: String,
        value: u64,
        max: u64,
    },

    /// `capacity_cost.<key>` is not declared on `places[runs_on].capacity`.
    #[error(
        "scenario_language_v1.transforms[{id:?}].capacity_cost: \
         {key:?} is not a declared capacity key on place {place:?}"
    )]
    TransformUnknownCapacityKey {
        id: String,
        key: String,
        place: String,
    },

    /// `failure_policy` is not one of the supported variants. PR 4
    /// accepts `"retry_then_warn"` and `"drop"`; `"degrade_quality"`
    /// is reserved for PR 8 and rejected at load.
    #[error(
        "scenario_language_v1.transforms[{id:?}].failure_policy: \
         {policy:?} is not a supported failure policy"
    )]
    TransformInvalidFailurePolicy { id: String, policy: String },

    /// `max_attempts` is zero. Use `1` for "single attempt".
    #[error("scenario_language_v1.transforms[{id:?}].max_attempts: must be >= 1")]
    TransformMaxAttemptsZero { id: String },

    /// `max_attempts` exceeds [`MAX_TRANSFORM_MAX_ATTEMPTS`].
    #[error(
        "scenario_language_v1.transforms[{id:?}].max_attempts: \
         {value} exceeds maximum {max}"
    )]
    TransformMaxAttemptsOutOfRange { id: String, value: u32, max: u32 },

    // ---- Demand (PR 5) ----------------------------------------------
    /// `demand[].id` does not satisfy `is_valid_sl1_id`.
    #[error("scenario_language_v1.demand[{id:?}].id: invalid identifier")]
    DemandInvalidId { id: String },

    /// `demand[].id` collides with another demand.
    #[error("scenario_language_v1.demand: duplicate id {id:?}")]
    DemandDuplicateId { id: String },

    /// `demand[].type` is empty after trimming. Free-form but must be
    /// non-empty for observability + protocol output.
    #[error("scenario_language_v1.demand[{id:?}].type: must be non-empty")]
    DemandEmptyType { id: String },

    /// `demand[].target.type` is not a recognized vocabulary entry.
    /// PR 5 recognizes `place | transform | dashboard | virtual_sink`;
    /// only `place` is implemented (see
    /// [`Self::DemandTargetKindNotImplemented`]). Any other value here
    /// is treated as a typo.
    #[error(
        "scenario_language_v1.demand[{id:?}].target.type: \
         {kind:?} is not a recognized target kind"
    )]
    DemandUnknownTargetKind { id: String, kind: String },

    /// `demand[].target.type` is recognized but not yet implemented.
    /// PR 5 implements `place` only; `transform`, `dashboard`, and
    /// `virtual_sink` land with PR 8/9.
    #[error(
        "scenario_language_v1.demand[{id:?}].target.type: \
         {kind:?} is not implemented in PR 5 (place only)"
    )]
    DemandTargetKindNotImplemented { id: String, kind: &'static str },

    /// `demand[].target.id` does not match any declared place id.
    #[error(
        "scenario_language_v1.demand[{id:?}].target.id: \
         {target:?} is not a declared place"
    )]
    DemandUnknownTarget { id: String, target: String },

    /// `demand[].requires` is empty. An always-fulfilled-at-spawn
    /// demand is almost certainly a modelling mistake.
    #[error("scenario_language_v1.demand[{id:?}].requires: must declare at least one thing")]
    DemandRequiresEmpty { id: String },

    /// `demand[].requires` exceeds [`MAX_DEMAND_REQUIRES`].
    #[error(
        "scenario_language_v1.demand[{id:?}].requires: \
         {count} entries exceeds maximum {max}"
    )]
    DemandRequiresTooMany {
        id: String,
        count: usize,
        max: usize,
    },

    /// A `demand[].requires` entry references neither a declared
    /// thing id. Tags are NOT accepted because fulfillment is keyed
    /// by exact id.
    #[error(
        "scenario_language_v1.demand[{id:?}].requires: \
         {value:?} is not a declared thing id"
    )]
    DemandUnknownRequires { id: String, value: String },

    /// Two `demand[].requires` entries name the same thing.
    #[error(
        "scenario_language_v1.demand[{id:?}].requires: \
         duplicate entry {value:?}"
    )]
    DemandDuplicateRequires { id: String, value: String },

    /// `demand[].spawn_schedule.type` is recognized in the spec
    /// vocabulary but not yet implemented. PR 5 ships
    /// `fixed | scripted`; `wave` lands with PR 8.
    #[error(
        "scenario_language_v1.demand[{id:?}].spawn_schedule.type: \
         {kind:?} is not implemented in PR 5 (fixed | scripted only)"
    )]
    DemandScheduleNotImplemented { id: String, kind: &'static str },

    /// `demand[].spawn_schedule.type` is not a recognized vocabulary
    /// entry. Typos like `"feixed"` land here instead of silently
    /// becoming a no-op.
    #[error(
        "scenario_language_v1.demand[{id:?}].spawn_schedule.type: \
         {kind:?} is not a recognized schedule kind"
    )]
    DemandUnknownScheduleType { id: String, kind: String },

    /// Required schedule field is missing for the declared type.
    /// E.g. `type: "fixed"` requires `every_ticks` and `start_tick`.
    #[error(
        "scenario_language_v1.demand[{id:?}].spawn_schedule: \
         {field:?} is required for schedule type {kind:?}"
    )]
    DemandScheduleMissingField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// Schedule numeric field is zero where positive is required
    /// (`every_ticks`, `start_tick`).
    #[error("scenario_language_v1.demand[{id:?}].spawn_schedule.{field}: must be > 0")]
    DemandScheduleFieldZero { id: String, field: &'static str },

    /// Schedule numeric field exceeds [`MAX_DEMAND_TICKS`].
    #[error(
        "scenario_language_v1.demand[{id:?}].spawn_schedule.{field}: \
         {value} exceeds maximum {max}"
    )]
    DemandScheduleFieldOutOfRange {
        id: String,
        field: &'static str,
        value: u64,
        max: u64,
    },

    /// `spawn_schedule.ticks` (Scripted) is empty.
    #[error("scenario_language_v1.demand[{id:?}].spawn_schedule.ticks: must be non-empty")]
    DemandScheduleScriptedEmpty { id: String },

    /// `spawn_schedule.ticks` is not strictly increasing (a duplicate
    /// or out-of-order entry was found). Coalescing would silently
    /// change spawn counts so this is a hard error.
    #[error(
        "scenario_language_v1.demand[{id:?}].spawn_schedule.ticks: \
         entries must be strictly increasing (offending tick {tick})"
    )]
    DemandScheduleScriptedNotIncreasing { id: String, tick: u64 },

    /// `spawn_schedule.ticks` length exceeds
    /// [`MAX_DEMAND_SCRIPTED_TICKS`].
    #[error(
        "scenario_language_v1.demand[{id:?}].spawn_schedule.ticks: \
         {count} entries exceeds maximum {max}"
    )]
    DemandScheduleScriptedTooMany {
        id: String,
        count: usize,
        max: usize,
    },

    /// `spawn_schedule.ticks` contains a 0 entry. Tick 0 is reserved
    /// for the pre-tick world and would never fire.
    #[error("scenario_language_v1.demand[{id:?}].spawn_schedule.ticks: tick 0 is reserved")]
    DemandScheduleScriptedTickZero { id: String },

    /// A schedule field was provided that does not apply to the
    /// declared schedule type (e.g. `ticks` on a `fixed` schedule, or
    /// `every_ticks` on a `scripted` schedule). Silently ignoring the
    /// field would let typos look like behavior changes.
    #[error(
        "scenario_language_v1.demand[{id:?}].spawn_schedule: \
         field {field:?} is not valid for schedule type {kind:?}"
    )]
    DemandScheduleUnexpectedField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// `deadline_ticks` is zero. Use ≥ 1.
    #[error("scenario_language_v1.demand[{id:?}].deadline_ticks: must be > 0")]
    DemandDeadlineZero { id: String },

    /// `deadline_ticks` exceeds [`MAX_DEMAND_TICKS`].
    #[error(
        "scenario_language_v1.demand[{id:?}].deadline_ticks: \
         {value} exceeds maximum {max}"
    )]
    DemandDeadlineOutOfRange { id: String, value: u64, max: u64 },

    /// `priority` is not one of `low | normal | high | critical`.
    #[error(
        "scenario_language_v1.demand[{id:?}].priority: \
         {priority:?} is not a recognized priority \
         (expected one of: low, normal, high, critical)"
    )]
    DemandInvalidPriority { id: String, priority: String },

    /// `value` exceeds [`MAX_DEMAND_VALUE`].
    #[error(
        "scenario_language_v1.demand[{id:?}].value: \
         {value} exceeds maximum {max}"
    )]
    DemandValueOutOfRange { id: String, value: u64, max: u64 },

    /// `penalty.score` is positive. Penalty must be ≤ 0 — positive
    /// scores belong on `value` (the reward for fulfillment).
    #[error(
        "scenario_language_v1.demand[{id:?}].penalty.score: \
         {score} must be <= 0 (use `value` for rewards)"
    )]
    DemandPenaltyScorePositive { id: String, score: i64 },

    /// `penalty.score` exceeds [`MAX_DEMAND_PENALTY_SCORE`] in
    /// absolute value.
    #[error(
        "scenario_language_v1.demand[{id:?}].penalty.score: \
         absolute value {abs} exceeds maximum {max}"
    )]
    DemandPenaltyScoreOutOfRange { id: String, abs: i64, max: i64 },

    /// `penalty.warning` is present but empty after trimming.
    #[error(
        "scenario_language_v1.demand[{id:?}].penalty.warning: \
         must be non-empty when present"
    )]
    DemandPenaltyWarningEmpty { id: String },

    // ---- Pressure primitive (PR 7) ----------------------------------------
    /// A pressure `id` is empty, too long, or contains characters outside
    /// the allowed alphanumeric/`_`/`-` charset.
    #[error("scenario_language_v1.pressure: invalid id {id:?}")]
    PressureInvalidId { id: String },

    /// Two `pressure[]` entries declared the same `id`.
    #[error("scenario_language_v1.pressure: duplicate id {id:?}")]
    PressureDuplicateId { id: String },

    /// The `type` discriminator did not match a known pressure kind.
    /// Lists the known kinds via [`Sl1PressureKind::as_str`] in the
    /// error message ordering so authors can copy/paste a valid value.
    #[error("scenario_language_v1.pressure[{id:?}].type: unknown pressure type {kind:?}")]
    PressureUnknownType { id: String, kind: String },

    /// A required type-specific field was missing for the chosen
    /// pressure type. `field` names the missing key; `kind` names the
    /// pressure type for which the field is required.
    #[error("scenario_language_v1.pressure[{id:?}]: type {kind:?} requires field {field:?}")]
    PressureMissingField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// A type-specific field was supplied that the chosen pressure
    /// type does not consume. Surfaced rather than silently ignored so
    /// authors are not misled into thinking the parameter takes effect.
    #[error(
        "scenario_language_v1.pressure[{id:?}]: type {kind:?} does not accept field {field:?}"
    )]
    PressureUnexpectedField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// `duration_ticks` was zero. A zero-duration pressure would
    /// activate and immediately deactivate on the same tick, which is
    /// almost certainly an authoring mistake.
    #[error("scenario_language_v1.pressure[{id:?}].duration_ticks: must be > 0")]
    PressureDurationZero { id: String },

    /// `at_tick` exceeded the SL1 tick clamp.
    #[error(
        "scenario_language_v1.pressure[{id:?}].at_tick: \
         {value} exceeds max {max}"
    )]
    PressureAtTickOutOfRange { id: String, value: u64, max: u64 },

    /// `duration_ticks` exceeded the SL1 tick clamp.
    #[error(
        "scenario_language_v1.pressure[{id:?}].duration_ticks: \
         {value} exceeds max {max}"
    )]
    PressureDurationOutOfRange { id: String, value: u64, max: u64 },

    /// `at_tick + duration_ticks` overflowed `u64` or exceeded
    /// `MAX_PRESSURE_TICKS`. Either way the pressure cannot be
    /// scheduled deterministically.
    #[error(
        "scenario_language_v1.pressure[{id:?}]: \
         at_tick {at_tick} + duration_ticks {duration_ticks} overflows max {max}"
    )]
    PressureEndOverflow {
        id: String,
        at_tick: u64,
        duration_ticks: u64,
        max: u64,
    },

    /// The `target` string did not match any declared id of the kind
    /// expected by this pressure type (place, demand, or link).
    #[error(
        "scenario_language_v1.pressure[{id:?}].target: \
         unknown {expected} {target:?}"
    )]
    PressureUnknownTarget {
        id: String,
        expected: &'static str,
        target: String,
    },

    /// `source_multiplier.thing` named an undeclared thing id.
    #[error("scenario_language_v1.pressure[{id:?}].thing: unknown thing {thing:?}")]
    PressureUnknownThing { id: String, thing: String },

    /// `source_multiplier.thing` was not declared in the target
    /// place's `storage` map. Without a matching storage slot the
    /// runtime has nowhere to inject inventory.
    #[error(
        "scenario_language_v1.pressure[{id:?}]: \
         target place {place:?} has no storage slot for thing {thing:?}"
    )]
    PressureNoStorageSlot {
        id: String,
        place: String,
        thing: String,
    },

    /// `quota_reduction.capacity` named a capacity bucket that is not
    /// declared in the target place's `capacity` map.
    #[error(
        "scenario_language_v1.pressure[{id:?}]: \
         target place {place:?} has no capacity bucket {capacity:?}"
    )]
    PressureUnknownCapacityBucket {
        id: String,
        place: String,
        capacity: String,
    },

    /// `multiplier` is non-finite (NaN/inf), zero, negative, or
    /// exceeds the milli-unit cap.
    #[error(
        "scenario_language_v1.pressure[{id:?}].multiplier: \
         {value} out of allowed range (0, {max_milli} milli-units]"
    )]
    PressureMultiplierOutOfRange {
        id: String,
        value: String,
        max_milli: u64,
    },

    /// `spawn_multiplier` is zero or exceeds the spawn-multiplier cap.
    #[error(
        "scenario_language_v1.pressure[{id:?}].spawn_multiplier: \
         {value} out of allowed range [1, {max}]"
    )]
    PressureSpawnMultiplierOutOfRange { id: String, value: u32, max: u32 },

    /// `reduction_percent` is zero or exceeds 100.
    #[error(
        "scenario_language_v1.pressure[{id:?}].reduction_percent: \
         {value} out of allowed range [1, {max}]"
    )]
    PressureReductionPercentOutOfRange { id: String, value: u8, max: u8 },

    // ---- PR 8 — objectives / failure_conditions / victory_conditions ----
    /// Empty / charset / length violation on an `objective.id`.
    #[error("scenario_language_v1.objectives: invalid id {id:?}")]
    ObjectiveInvalidId { id: String },

    /// Two objectives share an id.
    #[error("scenario_language_v1.objectives: duplicate id {id:?}")]
    ObjectiveDuplicateId { id: String },

    /// `type` does not match any known objective kind.
    #[error("scenario_language_v1.objectives[{id:?}].type: unknown kind {kind:?}")]
    ObjectiveUnknownType { id: String, kind: String },

    /// A required per-variant parameter is missing.
    #[error("scenario_language_v1.objectives[{id:?}] ({kind}): missing required field {field}")]
    ObjectiveMissingField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// A field that does not belong to this variant was set.
    #[error("scenario_language_v1.objectives[{id:?}] ({kind}): unexpected field {field}")]
    ObjectiveUnexpectedField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// `weight` is zero or exceeds [`MAX_OBJECTIVE_WEIGHT`].
    #[error(
        "scenario_language_v1.objectives[{id:?}].weight: {value} out of allowed range [1, {max}]"
    )]
    ObjectiveWeightOutOfRange { id: String, value: u32, max: u32 },

    /// Target id does not resolve in the expected catalog.
    #[error(
        "scenario_language_v1.objectives[{id:?}]: target {target:?} does not resolve to a declared {expected} id"
    )]
    ObjectiveUnknownTarget {
        id: String,
        expected: &'static str,
        target: String,
    },

    /// `keep_fresh` references a `(place, thing)` pair that has no
    /// declared storage slot on that place.
    #[error(
        "scenario_language_v1.objectives[{id:?}] (keep_fresh): \
         place {place:?} has no storage slot for thing {thing:?}"
    )]
    ObjectiveNoStorageSlot {
        id: String,
        place: String,
        thing: String,
    },

    /// `max_stale_ticks`/`max_missed` exceeds [`MAX_OBJECTIVE_TICKS`].
    #[error(
        "scenario_language_v1.objectives[{id:?}].{field}: {value} out of allowed range [1, {max}]"
    )]
    ObjectiveValueOutOfRange {
        id: String,
        field: &'static str,
        value: u64,
        max: u64,
    },

    /// `maintain_utilization.min_percent` > `max_percent`, or either
    /// exceeds 100.
    #[error(
        "scenario_language_v1.objectives[{id:?}] (maintain_utilization): \
         invalid percent range [{min}, {max}] (both must be 0..=100 with min <= max)"
    )]
    ObjectiveInvalidPercentRange { id: String, min: u8, max: u8 },

    /// `maintain_utilization` references a capacity bucket that doesn't
    /// exist on the target place.
    #[error(
        "scenario_language_v1.objectives[{id:?}] (maintain_utilization): \
         place {place:?} has no capacity bucket {capacity:?}"
    )]
    ObjectiveUnknownCapacityBucket {
        id: String,
        place: String,
        capacity: String,
    },

    /// Empty / charset / length violation on a `failure_condition.id`.
    #[error("scenario_language_v1.failure_conditions: invalid id {id:?}")]
    FailureConditionInvalidId { id: String },

    /// Two failure conditions share an id.
    #[error("scenario_language_v1.failure_conditions: duplicate id {id:?}")]
    FailureConditionDuplicateId { id: String },

    /// `type` does not match any known failure-condition kind.
    #[error("scenario_language_v1.failure_conditions[{id:?}].type: unknown kind {kind:?}")]
    FailureConditionUnknownType { id: String, kind: String },

    /// A required per-variant parameter is missing.
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}] ({kind}): missing required field {field}"
    )]
    FailureConditionMissingField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// A field that does not belong to this variant was set.
    #[error("scenario_language_v1.failure_conditions[{id:?}] ({kind}): unexpected field {field}")]
    FailureConditionUnexpectedField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// `threshold_ticks` is zero or exceeds [`MAX_OBJECTIVE_TICKS`].
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}].threshold_ticks: \
         {value} out of allowed range [1, {max}]"
    )]
    FailureConditionThresholdOutOfRange { id: String, value: u64, max: u64 },

    /// `grace_ticks` exceeds [`MAX_OBJECTIVE_TICKS`].
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}].grace_ticks: \
         {value} out of allowed range [0, {max}]"
    )]
    FailureConditionGraceOutOfRange { id: String, value: u64, max: u64 },

    /// `max_count` is zero (would fire on tick 1 with no breach) or
    /// exceeds [`MAX_OBJECTIVE_BREACH_COUNT`].
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}].max_count: \
         {value} out of allowed range [1, {max}]"
    )]
    FailureConditionMaxCountOutOfRange { id: String, value: u64, max: u64 },

    /// `stale_target` references a `(place, thing)` pair that has no
    /// declared storage slot on that place.
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}] (stale_target): \
         place {place:?} has no storage slot for thing {thing:?}"
    )]
    FailureConditionNoStorageSlot {
        id: String,
        place: String,
        thing: String,
    },

    /// Target id does not resolve in the expected catalog.
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}]: target {target:?} does not resolve to a declared {expected} id"
    )]
    FailureConditionUnknownTarget {
        id: String,
        expected: &'static str,
        target: String,
    },

    /// `place_state` references an operating-state name that isn't
    /// declared on the target place.
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}] (place_state): \
         place {place:?} has no operating_state {state:?}"
    )]
    FailureConditionUnknownPlaceState {
        id: String,
        place: String,
        state: String,
    },

    /// `place_state` references an operating-state whose predicate kind
    /// is recognized by the schema but not yet evaluable in PR 8.
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}] (place_state): \
         place {place:?} operating_state {state:?} uses predicate kind \
         {predicate:?} which is not evaluable in PR 8"
    )]
    FailureConditionPlaceStatePredicateUnsupported {
        id: String,
        place: String,
        state: String,
        predicate: &'static str,
    },

    /// `objective_breach_count` references an `objective_id` that
    /// wasn't declared in `objectives[]`.
    #[error(
        "scenario_language_v1.failure_conditions[{id:?}] (objective_breach_count): \
         unknown objective_id {objective_id:?}"
    )]
    FailureConditionUnknownObjective { id: String, objective_id: String },

    /// Empty / charset / length violation on a `victory_condition.id`.
    #[error("scenario_language_v1.victory_conditions: invalid id {id:?}")]
    VictoryConditionInvalidId { id: String },

    /// Two victory conditions share an id.
    #[error("scenario_language_v1.victory_conditions: duplicate id {id:?}")]
    VictoryConditionDuplicateId { id: String },

    /// `type` does not match any known victory-condition kind.
    #[error("scenario_language_v1.victory_conditions[{id:?}].type: unknown kind {kind:?}")]
    VictoryConditionUnknownType { id: String, kind: String },

    /// A required per-variant parameter is missing.
    #[error(
        "scenario_language_v1.victory_conditions[{id:?}] ({kind}): missing required field {field}"
    )]
    VictoryConditionMissingField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// A field that does not belong to this variant was set.
    #[error("scenario_language_v1.victory_conditions[{id:?}] ({kind}): unexpected field {field}")]
    VictoryConditionUnexpectedField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },

    /// `at_tick` exceeds [`MAX_OBJECTIVE_TICKS`].
    #[error(
        "scenario_language_v1.victory_conditions[{id:?}].at_tick: \
         {value} out of allowed range [1, {max}]"
    )]
    VictoryConditionAtTickOutOfRange { id: String, value: u64, max: u64 },

    // ---- PR 9 — observability (metrics / dashboards / alerts) ----
    /// More observability items than [`MAX_OBSERVABILITY_ITEMS`].
    /// Applies to each list (`metrics`, `dashboards`, `alerts`)
    /// independently.
    #[error(
        "scenario_language_v1.observability.{section}: \
         found {count} items, maximum {max}"
    )]
    ObservabilityTooManyItems {
        section: &'static str,
        count: usize,
        max: usize,
    },

    /// A metric `id` is empty, too long, or contains characters outside
    /// the allowed alphanumeric/`_`/`-` charset.
    #[error("scenario_language_v1.observability.metrics: invalid id {id:?}")]
    MetricInvalidId { id: String },

    /// Two metrics declared the same `id`.
    #[error("scenario_language_v1.observability.metrics: duplicate id {id:?}")]
    MetricDuplicateId { id: String },

    /// A metric's `source` discriminator did not match any supported
    /// kind. The list of supported kinds is enumerated by
    /// [`Sl1MetricSourceKind`] variants.
    #[error(
        "scenario_language_v1.observability.metrics[{id:?}].source: \
         unsupported source {source_kind:?}"
    )]
    MetricUnsupportedSource { id: String, source_kind: String },

    /// A metric source required a field but the field was missing.
    #[error(
        "scenario_language_v1.observability.metrics[{id:?}]: \
         source {source_kind:?} requires field {field:?}"
    )]
    MetricMissingField {
        id: String,
        source_kind: &'static str,
        field: &'static str,
    },

    /// A metric source had a field that the source does not consume
    /// (e.g. `dashboard_freshness` with a `place` field). Strict
    /// schema: extraneous fields are rejected to prevent silent typos.
    #[error(
        "scenario_language_v1.observability.metrics[{id:?}]: \
         source {source_kind:?} does not accept field {field:?}"
    )]
    MetricExtraField {
        id: String,
        source_kind: &'static str,
        field: &'static str,
    },

    /// A metric source referenced an undeclared place id.
    #[error(
        "scenario_language_v1.observability.metrics[{id:?}].place: \
         unknown place {place:?}"
    )]
    MetricUnknownPlace { id: String, place: String },

    /// A metric source referenced an undeclared thing id.
    #[error(
        "scenario_language_v1.observability.metrics[{id:?}].thing: \
         unknown thing {thing:?}"
    )]
    MetricUnknownThing { id: String, thing: String },

    /// A `place_capacity_used_percent` metric referenced a capacity
    /// bucket the named place does not declare.
    #[error(
        "scenario_language_v1.observability.metrics[{id:?}]: \
         place {place:?} has no capacity bucket {capacity:?}"
    )]
    MetricUnknownCapacityBucket {
        id: String,
        place: String,
        capacity: String,
    },

    /// A `place_inventory_count` metric referenced a `(place, thing)`
    /// pair where the place declares no storage slot for the thing.
    #[error(
        "scenario_language_v1.observability.metrics[{id:?}]: \
         place {place:?} has no storage slot for thing {thing:?}"
    )]
    MetricNoStorageSlot {
        id: String,
        place: String,
        thing: String,
    },

    /// A `dashboard_freshness` metric referenced an undeclared
    /// dashboard id.
    #[error(
        "scenario_language_v1.observability.metrics[{id:?}].dashboard: \
         unknown dashboard {dashboard:?}"
    )]
    MetricUnknownDashboard { id: String, dashboard: String },

    /// A dashboard `id` is empty, too long, or contains characters
    /// outside the allowed alphanumeric/`_`/`-` charset.
    #[error("scenario_language_v1.observability.dashboards: invalid id {id:?}")]
    DashboardInvalidId { id: String },

    /// Two dashboards declared the same `id`.
    #[error("scenario_language_v1.observability.dashboards: duplicate id {id:?}")]
    DashboardDuplicateId { id: String },

    /// A dashboard `type` did not match any supported
    /// [`Sl1DashboardKind`] variant.
    #[error(
        "scenario_language_v1.observability.dashboards[{id:?}].type: \
         unsupported kind {kind:?}"
    )]
    DashboardUnsupportedKind { id: String, kind: String },

    /// A dashboard's `depends_on` list contained an empty or
    /// duplicate entry.
    #[error(
        "scenario_language_v1.observability.dashboards[{id:?}].depends_on: \
         empty or duplicate entry {value:?}"
    )]
    DashboardInvalidDependsOn { id: String, value: String },

    /// A dashboard's `depends_on` entry referenced an undeclared
    /// thing id.
    #[error(
        "scenario_language_v1.observability.dashboards[{id:?}].depends_on: \
         unknown thing {thing:?}"
    )]
    DashboardUnknownThing { id: String, thing: String },

    /// A dashboard's `freshness_slo_ticks` is zero.
    #[error(
        "scenario_language_v1.observability.dashboards[{id:?}].freshness_slo_ticks: \
         must be > 0"
    )]
    DashboardFreshnessSloZero { id: String },

    /// An alert `id` is empty, too long, or contains characters
    /// outside the allowed alphanumeric/`_`/`-` charset.
    #[error("scenario_language_v1.observability.alerts: invalid id {id:?}")]
    AlertInvalidId { id: String },

    /// Two alerts declared the same `id`.
    #[error("scenario_language_v1.observability.alerts: duplicate id {id:?}")]
    AlertDuplicateId { id: String },

    /// An alert's `metric` referenced an undeclared metric id.
    #[error(
        "scenario_language_v1.observability.alerts[{id:?}].metric: \
         unknown metric {metric:?}"
    )]
    AlertUnknownMetric { id: String, metric: String },

    /// An `out_of_range` predicate's `min` exceeds its `max`.
    /// Such a predicate fires on every value, which is always an
    /// authoring mistake.
    #[error(
        "scenario_language_v1.observability.alerts[{id:?}].predicate: \
         out_of_range min {min} > max {max}"
    )]
    AlertOutOfRangeInverted { id: String, min: u64, max: u64 },

    /// An alert `severity` did not match any supported
    /// [`Sl1AlertSeverity`] variant.
    #[error(
        "scenario_language_v1.observability.alerts[{id:?}].severity: \
         unsupported severity {severity:?}"
    )]
    AlertUnsupportedSeverity { id: String, severity: String },

    // ---- PR 10 — agents ----------------------------------------------
    /// Empty / charset / length violation on an `agent.id`.
    #[error("scenario_language_v1.agents: invalid id {id:?}")]
    AgentInvalidId { id: String },

    /// Two agents share an id.
    #[error("scenario_language_v1.agents: duplicate id {id:?}")]
    AgentDuplicateId { id: String },

    /// `kind` did not match any known agent backend kind.
    #[error("scenario_language_v1.agents[{id:?}].kind: unknown kind {kind:?}")]
    AgentUnknownKind { id: String, kind: String },

    /// `role` is empty or whitespace-only.
    #[error("scenario_language_v1.agents[{id:?}].role: must be non-empty")]
    AgentRoleEmpty { id: String },

    /// `interval_ticks` is zero — the agent could never fire.
    #[error("scenario_language_v1.agents[{id:?}].interval_ticks: must be > 0")]
    AgentIntervalTicksZero { id: String },

    /// `interval_ticks` exceeds [`SL1_AGENT_MAX_INTERVAL_TICKS`].
    #[error(
        "scenario_language_v1.agents[{id:?}].interval_ticks: \
         {value} out of allowed range [1, {max}]"
    )]
    AgentIntervalTicksOutOfRange { id: String, value: u64, max: u64 },

    /// `observation_scope` entry was not formatted as `"<kind>:<id>"`
    /// with a known kind prefix.
    #[error(
        "scenario_language_v1.agents[{id:?}].observation_scope: \
         malformed entry {entry:?} (expected `<kind>:<id>`)"
    )]
    AgentObservationScopeMalformed { id: String, entry: String },

    /// `observation_scope` references a target id that does not
    /// resolve in the matching scene-level catalog.
    #[error(
        "scenario_language_v1.agents[{id:?}].observation_scope: \
         unknown {kind} id {target:?}"
    )]
    AgentObservationScopeUnknownId {
        id: String,
        kind: &'static str,
        target: String,
    },

    /// Two `observation_scope` entries are identical after parsing.
    #[error(
        "scenario_language_v1.agents[{id:?}].observation_scope: \
         duplicate entry {entry:?}"
    )]
    AgentObservationScopeDuplicate { id: String, entry: String },

    /// `allowed_actions` carries an unknown action kind.
    #[error(
        "scenario_language_v1.agents[{id:?}].allowed_actions: \
         unknown action kind {kind:?}"
    )]
    AgentAllowedActionsUnknownKind { id: String, kind: String },

    /// Two `allowed_actions` entries refer to the same action kind.
    #[error(
        "scenario_language_v1.agents[{id:?}].allowed_actions: \
         duplicate entry {kind:?}"
    )]
    AgentAllowedActionsDuplicate { id: String, kind: String },

    /// `budgets.max_cost_per_decision` is zero — the agent can never
    /// successfully act.
    #[error(
        "scenario_language_v1.agents[{id:?}].budgets.max_cost_per_decision: \
         must be > 0"
    )]
    AgentMaxCostPerDecisionZero { id: String },

    /// `budgets.cooldown_ticks` exceeds
    /// [`SL1_AGENT_MAX_COOLDOWN_TICKS`].
    #[error(
        "scenario_language_v1.agents[{id:?}].budgets.cooldown_ticks: \
         {value} out of allowed range [0, {max}]"
    )]
    AgentCooldownTicksOutOfRange { id: String, value: u64, max: u64 },

    /// An `objective_weights` value is NaN or infinite.
    #[error(
        "scenario_language_v1.agents[{id:?}].objective_weights[{objective:?}]: \
         value must be finite"
    )]
    AgentObjectiveWeightNonFinite { id: String, objective: String },

    /// An `objective_weights` value is outside `[0, 1]`.
    #[error(
        "scenario_language_v1.agents[{id:?}].objective_weights[{objective:?}]: \
         {value} out of range [0, 1]"
    )]
    AgentObjectiveWeightOutOfRange {
        id: String,
        objective: String,
        value: f64,
    },

    /// `objective_weights` references an objective id that is not
    /// declared on the scene.
    #[error(
        "scenario_language_v1.agents[{id:?}].objective_weights: \
         unknown objective {objective:?}"
    )]
    AgentObjectiveWeightUnknown { id: String, objective: String },

    /// `observation_scope`, `allowed_actions`, or `objective_weights`
    /// has more entries than the matching cap.
    #[error(
        "scenario_language_v1.agents[{id:?}].{field}: \
         {count} entries (max {max})"
    )]
    AgentTooManyEntries {
        id: String,
        field: &'static str,
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

    /// A transform's cadence slot fired but it could not start because
    /// one or more required inputs were missing. Emitted on entry to
    /// `Starved`; not re-emitted every tick.
    #[error("transform {transform_id:?} starved at tick {tick} (missing inputs)")]
    TransformStarved { transform_id: String, tick: u64 },

    /// A transform's cadence slot fired but it could not start because
    /// the place's typed capacity could not satisfy `capacity_cost`,
    /// or the output thing's storage capacity was already at the cap.
    /// Emitted on entry to `Blocked`.
    #[error("transform {transform_id:?} blocked at tick {tick}")]
    TransformBlocked { transform_id: String, tick: u64 },

    /// A scheduled instance has exceeded `scheduled_at + deadline_ticks`
    /// without completing. Emitted on entry to `Late`.
    #[error("transform {transform_id:?} late at tick {tick}")]
    TransformLate { transform_id: String, tick: u64 },

    /// A scheduled instance exhausted `max_attempts` (or was running
    /// under the `Drop` policy and missed its deadline). Emitted once
    /// per failed instance; the transform resets to `Idle` for its
    /// next cadence slot.
    #[error("transform {transform_id:?} failed at tick {tick} (attempt {attempt})")]
    TransformFailed {
        transform_id: String,
        tick: u64,
        attempt: u32,
    },

    /// A new cadence slot arrived for a transform that was still
    /// `Running` (or otherwise non-`Idle`) from a previous slot. The
    /// new slot is skipped to preserve single-instance semantics.
    /// Emitted once per skipped slot.
    #[error("transform {transform_id:?} missed cadence slot at tick {tick}")]
    TransformSlotMissed { transform_id: String, tick: u64 },

    /// A demand instance was not fulfilled before its deadline and has
    /// been dropped. PR 5 collapses Late and Dropped into a single
    /// terminal `Dropped` state; PR 8 may split them when the
    /// score/penalty model lands. `value` and `penalty_score` are
    /// surfaced as warning payload so PR 8 can wire score arithmetic
    /// without protocol changes. `sequence` is the per-demand
    /// monotonic instance counter so distinct dropped instances are
    /// distinguishable in event logs.
    #[error("demand {demand_id:?} instance {sequence} dropped at tick {tick}")]
    DemandDropped {
        demand_id: String,
        sequence: u64,
        tick: u64,
        value: u64,
        penalty_score: i64,
    },

    /// A demand reached [`MAX_DEMAND_OUTSTANDING`] Pending instances
    /// and a new spawn slot was suppressed. Emitted once per
    /// transition into overflow; cleared (and re-emittable on later
    /// re-entry) when outstanding drops below the cap.
    #[error("demand {demand_id:?} backlog overflow at tick {tick}")]
    DemandBacklogOverflow { demand_id: String, tick: u64 },

    /// A pressure activated, but its variant has no runtime effect in
    /// this build. Emitted exactly once per pressure instance, at the
    /// activation tick, so authors are never misled into thinking a
    /// scheduled pressure is silently driving behavior. Surfaces for
    /// `schema_drift`, `dashboard_storm`, `spot_eviction_wave`,
    /// `storage_metadata_storm`, and `cooling_degradation` in PR 7.
    #[error(
        "pressure {pressure_id:?} ({kind:?}) activated at tick {tick} \
         but no runtime effect is wired in this build"
    )]
    PressureUnsupportedInThisPr {
        pressure_id: String,
        kind: &'static str,
        tick: u64,
    },

    /// An objective with a recognized but not-yet-evaluated kind
    /// (`cost_budget`, `data_quality`, `query_latency`) was loaded.
    /// Emitted exactly once per objective on the first tick the
    /// objective evaluator runs, so authors are never misled into
    /// thinking their objective is constraining the run.
    #[error("objective {objective_id:?} ({kind:?}) has no runtime effect in this build")]
    ObjectiveUnsupportedInThisPr {
        objective_id: String,
        kind: &'static str,
        tick: u64,
    },
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

/// Freshness state of an inventory bucket (PR 3). Inventory + thing
/// timing together form the runtime model that later PRs (transforms,
/// demand, observability) mutate. PR 3 only reaches `NoData`, `Ok`,
/// and `Stale`. `Degraded` and `Invalid` are defined now for forward
/// compatibility so the protocol-mirror schema does not need to change
/// when later PRs land — but they are unreachable until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FreshnessState {
    /// Initial inventory was zero, so no observation has ever been
    /// recorded. Transitions to `Ok` when an inventory write lands
    /// (PRs 4/5).
    NoData,
    /// Most recent observation landed at `last_set_tick` and is still
    /// within the thing's `freshness_budget_ticks` window.
    Ok { last_set_tick: u64 },
    /// Most recent observation landed at `last_set_tick` but has aged
    /// past the thing's `freshness_budget_ticks` window.
    Stale { last_set_tick: u64 },
    /// Quality-contract evaluation marked the data as degraded.
    /// Reachable in PR 8 (objectives/failure conditions).
    Degraded,
    /// Quality-contract evaluation marked the data as invalid.
    /// Reachable in PR 8 (objectives/failure conditions).
    Invalid,
}

/// Per-scene SL1 runtime state (PR 3). Lives on `World` (not inside
/// [`Sl1Scene`]) so the boundary between immutable declarative data
/// and mutable per-tick state stays explicit. Constructed alongside
/// `world.sl1` in the loader.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1RuntimeState {
    /// Per-place, per-thing inventory counts. Outer key is place id,
    /// inner key is thing id. Populated from
    /// `places[*].storage[*].initial` at load time; later PRs (4 and
    /// 5) mutate via transforms and demand.
    pub inventories: std::collections::BTreeMap<String, std::collections::BTreeMap<String, u64>>,
    /// Per-(place, thing) freshness state. Same key pairs as
    /// `inventories`. Recomputed every tick by `crate::sl1_runtime::run`
    /// against `world.tick` and each thing's `freshness_budget_ticks`.
    pub freshness: std::collections::BTreeMap<(String, String), FreshnessState>,
    /// Per-transform live state (idle, running, starved, blocked, late).
    /// Outer key is transform id. PR 4 populates this from
    /// `scene.transforms` at load time; the SL1 runtime mutates it
    /// each tick in stable id order.
    pub transforms: std::collections::BTreeMap<String, Sl1TransformState>,
    /// Per-place typed capacity currently reserved by `Running`
    /// transforms. Outer key is place id, inner key is capacity-bucket
    /// name (matching `places[*].capacity` keys). Buckets are released
    /// when a transform leaves the `Running` state.
    pub place_capacity_used:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, u64>>,
    /// Per-`(place_id, thing_id)` in-flight output reservation. Each
    /// currently-`Running` transform contributes `output.amount` here
    /// for every entry in its `outputs[]` Vec. `try_start` consults
    /// this map alongside the current inventory before reserving a
    /// new instance — the headroom check is
    /// `inventory + pending + amount <= storage_capacity`. Without
    /// this, two transforms targeting the same `(place, thing)` can
    /// both pass the single-transform capacity check at the same tick
    /// and silently over-fill storage when both complete. Releases
    /// occur at completion (output lands in inventory) and at all
    /// `Running → not-Running` transitions where no output lands
    /// (Late-deadline failure, Drop-policy failure at deadline).
    pub pending_outputs: std::collections::BTreeMap<(String, String), u64>,
    /// Per-demand runtime state (PR 5). One entry per declared demand;
    /// the entry tracks the monotonic next instance sequence, the
    /// outstanding Pending instances in spawn order, aggregate
    /// fulfilled/dropped counters for observability and protocol
    /// snapshots, the next Scripted-schedule cursor (binary-search
    /// optimization), and a one-shot overflow flag so backlog
    /// warnings are not re-emitted every tick.
    pub demand: std::collections::BTreeMap<String, Sl1DemandRuntime>,
    /// Per-tick pressure overlay state (PR 7). Rebuilt at the start of
    /// every tick by `crate::sl1_pressure::run` before transforms and
    /// demand fire, so downstream systems read the overlay (effective
    /// capacity, effective demand spawn count, outaged links) instead
    /// of mutating the immutable base scene.
    pub pressure: Sl1PressureRuntime,
    /// Per-objective runtime state (PR 8). One entry per declared
    /// objective; status, cumulative breach-tick count, and the tick
    /// the status last changed. Stable order via `BTreeMap`.
    pub objectives: std::collections::BTreeMap<String, Sl1ObjectiveRuntime>,
    /// Per-failure-condition runtime state (PR 8). Tracks the current
    /// breach-streak length and (if fired) the tick it fired.
    pub failure_conditions: std::collections::BTreeMap<String, Sl1FailureConditionRuntime>,
    /// Per-victory-condition runtime state (PR 8). Tracks the tick the
    /// condition was met, if any.
    pub victory_conditions: std::collections::BTreeMap<String, Sl1VictoryConditionRuntime>,
    /// Current high-level outcome of the run. Sticky once terminal —
    /// `Won` and `Lost` never transition back to `InProgress`.
    pub game_outcome: GameOutcome,
    /// Server-side derived UX label. Derived after `game_outcome` is
    /// settled each tick so the frontend never has to reimplement the
    /// rule.
    pub game_phase: Sl1GamePhase,
    /// One-shot warning gate for objectives whose kind is recognized
    /// but unimplemented in PR 8. Mirrors `pressure.unsupported_warned`.
    pub unsupported_objectives_warned: std::collections::BTreeSet<String>,
    /// Per-metric runtime state (PR 9). One entry per declared metric;
    /// recomputed every tick by `crate::sl1_observability::run`.
    pub metric_states: std::collections::BTreeMap<String, Sl1MetricState>,
    /// Per-dashboard runtime state (PR 9). One entry per declared
    /// dashboard; recomputed every tick.
    pub dashboard_states: std::collections::BTreeMap<String, Sl1DashboardState>,
    /// Per-alert runtime state (PR 9). One entry per declared alert.
    /// Updated edge-triggered: `Inactive` → `Firing` emits
    /// `Sl1AlertFired`, `Firing` → `Inactive` emits `Sl1AlertCleared`.
    /// Same-state ticks do not emit events.
    pub alert_states: std::collections::BTreeMap<String, Sl1AlertState>,
    /// Per-agent runtime state (PR 10). One entry per declared agent.
    /// Updated each agent decision tick.
    pub agents: std::collections::BTreeMap<String, Sl1AgentRuntimeState>,
    /// Per-demand agent-imposed pause (PR 10). Maps a demand id to the
    /// tick (exclusive) at which its spawn loop may resume. While
    /// `now < pause_until_tick`, `sl1_runtime::run_demand`
    /// skips spawning new instances for that demand.
    ///
    /// Stable order via `BTreeMap`. Entries are dropped when they
    /// expire (no point keeping `0` sentinels in the protocol view).
    pub agent_demand_pauses: std::collections::BTreeMap<String, u64>,
}

/// Per-tick pressure overlay (PR 7).
///
/// `active` is the source of truth for which pressures are currently
/// firing; the other maps are derived overlays rebuilt from `active`
/// at the start of every tick. `source_inject_carry_milli` and
/// `unsupported_warned` persist across ticks because they encode
/// runtime state (fractional injection accumulators and one-shot
/// edge-triggered warnings) that derived-each-tick overlays cannot.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1PressureRuntime {
    /// Currently active pressure id → activation tick. Stable order via
    /// `BTreeMap`.
    pub active: std::collections::BTreeMap<String, u64>,
    /// Effective spawn multiplier per demand id from active
    /// `demand_growth` pressures. Final spawn count per scheduled
    /// firing is `1 + sum(multipliers)`.
    pub demand_spawn_multiplier: std::collections::BTreeMap<String, u32>,
    /// Effective capacity reduction percent per (place_id,
    /// capacity_bucket). Sum is capped at 100 to keep the effective
    /// capacity well-defined.
    pub quota_reduction: std::collections::BTreeMap<(String, String), u8>,
    /// Set of currently outaged link ids.
    pub outaged_links: std::collections::BTreeSet<String>,
    /// Per-(pressure_id, place_id, thing_id) fractional injection
    /// accumulator in milli-units. `run_pressure` adds the active
    /// pressure's `multiplier_milli` each tick and flushes whole units
    /// (≥ 1000) into `runtime.inventories`, clamping by the storage
    /// capacity. The remainder carries forward so fractional rates
    /// (e.g. 2.5x) stay deterministic across the entire active window.
    /// **Keying by pressure_id is essential** so a deactivated pressure's
    /// leftover carry cannot leak into a later, distinct pressure
    /// targeting the same (place, thing) — deactivation must remove
    /// all entries with that pressure_id.
    pub source_inject_carry_milli: std::collections::BTreeMap<(String, String, String), u64>,
    /// Pressure ids whose `PressureUnsupportedInThisPr` warning has
    /// already been emitted. Edge-triggered: cleared on deactivation
    /// so re-scheduled pressures (future PRs) re-arm the warning.
    pub unsupported_warned: std::collections::BTreeSet<String>,
}

/// Per-demand live state (PR 5).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1DemandRuntime {
    /// Next instance sequence number to assign. Monotonically
    /// increasing per demand for the lifetime of the run.
    pub next_sequence: u64,
    /// Outstanding (Pending) instances in spawn order. Fulfilled and
    /// Dropped instances are removed immediately after the terminal
    /// transition emits its event so the deque only carries
    /// in-flight work and `len()` is exactly the backlog.
    pub pending: std::collections::VecDeque<Sl1DemandInstance>,
    /// Cumulative fulfilled count for protocol/observability use.
    pub fulfilled_count: u64,
    /// Cumulative dropped count for protocol/observability use.
    pub dropped_count: u64,
    /// Cursor into the Scripted schedule (index of next tick to
    /// match). Unused for Fixed schedules. Cached because
    /// `ticks.contains(&now)` per tick is O(n) and a strictly
    /// increasing cursor is O(1) amortized.
    pub scripted_cursor: usize,
    /// True when the demand is currently in backlog overflow (Pending
    /// count ≥ [`MAX_DEMAND_OUTSTANDING`]). Edge-triggered: a single
    /// [`Sl1Warning::DemandBacklogOverflow`] fires on entry; the flag
    /// clears when Pending drops below the cap, enabling re-arm on
    /// later re-entry.
    pub overflow: bool,
}

/// One spawned demand instance (PR 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sl1DemandInstance {
    pub sequence: u64,
    pub spawned_at: u64,
    pub deadline_tick: u64,
}

/// Per-transform live state machine driven by `sl1_runtime::run`.
///
/// `Failed` is intentionally NOT a state here — a transform that
/// exhausts its retry budget emits [`Sl1Warning::TransformFailed`] and
/// resets to [`Sl1TransformState::Idle`] so the next cadence slot is
/// not silently disabled by a single transient failure. The
/// `last_failed_tick` field on `Idle` preserves observability across
/// resets.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sl1TransformState {
    /// No instance is currently scheduled or running. `last_*_tick`
    /// fields are observability-only.
    #[default]
    Idle,
    /// Inputs were consumed and capacity reserved; the instance will
    /// produce outputs at `tick == started_at + duration_ticks`.
    Running {
        scheduled_at: u64,
        started_at: u64,
        attempt: u32,
    },
    /// A cadence slot fired but one or more required inputs were
    /// missing. `since` is the tick where the state was entered;
    /// `attempts` is the number of times this scheduled instance has
    /// tried to start.
    Starved {
        scheduled_at: u64,
        since: u64,
        attempts: u32,
    },
    /// A cadence slot fired but the place's typed capacity (or output
    /// storage capacity) prevented starting. Same semantics as
    /// `Starved`.
    Blocked {
        scheduled_at: u64,
        since: u64,
        attempts: u32,
    },
    /// `world.tick > scheduled_at + deadline_ticks` without completing.
    /// Under `RetryThenWarn` the instance will retry up to
    /// `max_attempts` times; under `Drop` a single `Late` immediately
    /// emits `TransformFailed` and resets to `Idle`.
    Late {
        scheduled_at: u64,
        attempt: u32,
        since: u64,
    },
}

impl Sl1RuntimeState {
    /// Construct the initial runtime state from a validated scene.
    /// Inventories start at each storage slot's `initial` count;
    /// freshness starts as [`FreshnessState::Ok`] (with
    /// `last_set_tick: 0`) when initial > 0, else
    /// [`FreshnessState::NoData`].
    #[must_use]
    pub fn from_scene(scene: &Sl1Scene) -> Self {
        let mut inventories: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, u64>,
        > = std::collections::BTreeMap::new();
        let mut freshness: std::collections::BTreeMap<(String, String), FreshnessState> =
            std::collections::BTreeMap::new();
        for place in &scene.places {
            if place.storage.is_empty() {
                continue;
            }
            let mut place_slots: std::collections::BTreeMap<String, u64> =
                std::collections::BTreeMap::new();
            for (thing_id, slot) in &place.storage {
                place_slots.insert(thing_id.clone(), slot.initial);
                let key = (place.id.clone(), thing_id.clone());
                let state = if slot.initial > 0 {
                    FreshnessState::Ok { last_set_tick: 0 }
                } else {
                    FreshnessState::NoData
                };
                freshness.insert(key, state);
            }
            inventories.insert(place.id.clone(), place_slots);
        }
        let mut transforms: std::collections::BTreeMap<String, Sl1TransformState> =
            std::collections::BTreeMap::new();
        for t in &scene.transforms {
            transforms.insert(t.id.clone(), Sl1TransformState::Idle);
        }
        let mut place_capacity_used: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, u64>,
        > = std::collections::BTreeMap::new();
        for place in &scene.places {
            if place.capacity.is_empty() {
                continue;
            }
            let mut buckets: std::collections::BTreeMap<String, u64> =
                std::collections::BTreeMap::new();
            for key in place.capacity.keys() {
                buckets.insert(key.clone(), 0);
            }
            place_capacity_used.insert(place.id.clone(), buckets);
        }
        let mut demand: std::collections::BTreeMap<String, Sl1DemandRuntime> =
            std::collections::BTreeMap::new();
        for d in &scene.demand {
            demand.insert(d.id.clone(), Sl1DemandRuntime::default());
        }
        Self {
            inventories,
            freshness,
            transforms,
            place_capacity_used,
            pending_outputs: std::collections::BTreeMap::new(),
            demand,
            pressure: Sl1PressureRuntime::default(),
            objectives: scene
                .objectives
                .iter()
                .map(|o| (o.id.clone(), Sl1ObjectiveRuntime::default()))
                .collect(),
            failure_conditions: scene
                .failure_conditions
                .iter()
                .map(|fc| (fc.id.clone(), Sl1FailureConditionRuntime::default()))
                .collect(),
            victory_conditions: scene
                .victory_conditions
                .iter()
                .map(|vc| (vc.id.clone(), Sl1VictoryConditionRuntime::default()))
                .collect(),
            game_outcome: GameOutcome::InProgress,
            game_phase: Sl1GamePhase::Stabilizing,
            unsupported_objectives_warned: std::collections::BTreeSet::new(),
            metric_states: scene
                .observability
                .as_ref()
                .map(|o| {
                    o.metrics
                        .iter()
                        .map(|m| (m.id.clone(), Sl1MetricState::default()))
                        .collect()
                })
                .unwrap_or_default(),
            dashboard_states: scene
                .observability
                .as_ref()
                .map(|o| {
                    o.dashboards
                        .iter()
                        .map(|d| (d.id.clone(), Sl1DashboardState::default()))
                        .collect()
                })
                .unwrap_or_default(),
            alert_states: scene
                .observability
                .as_ref()
                .map(|o| {
                    o.alerts
                        .iter()
                        .map(|a| (a.id.clone(), Sl1AlertState::default()))
                        .collect()
                })
                .unwrap_or_default(),
            agents: scene
                .agents
                .iter()
                .map(|a| (a.id.clone(), Sl1AgentRuntimeState::default()))
                .collect(),
            agent_demand_pauses: std::collections::BTreeMap::new(),
        }
    }
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

    /// Compact wire string for the variant (no payload). Used in
    /// snapshots and hash baselines so the renderer / replay never
    /// double-encodes the discriminator.
    #[must_use]
    pub fn variant_str(&self) -> &'static str {
        match self {
            GameOutcome::InProgress => "in_progress",
            GameOutcome::Won => "won",
            GameOutcome::Lost { .. } => "lost",
        }
    }
}

// ---------------------------------------------------------------------------
// Objective / FailureCondition / VictoryCondition runtime (PR 8).
// ---------------------------------------------------------------------------

/// Status of a single objective evaluation. Stable wire strings via
/// [`Sl1ObjectiveStatus::as_str`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sl1ObjectiveStatus {
    /// Not yet evaluated (first tick before the evaluator runs).
    #[default]
    Unknown,
    /// Objective currently satisfied.
    Met,
    /// Objective currently violated.
    Breached,
    /// Recognized-but-unsupported kind. Status never changes.
    Unsupported,
}

impl Sl1ObjectiveStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Met => "met",
            Self::Breached => "breached",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Per-objective runtime state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1ObjectiveRuntime {
    pub status: Sl1ObjectiveStatus,
    /// Number of ticks the objective has been observed in the
    /// `Breached` state since scene start. Consumed by
    /// `objective_breach_count` failure conditions.
    pub breach_tick_count: u64,
    /// Tick of the most recent `status` transition (0 if status is
    /// still `Unknown`). Used for deterministic event emission.
    pub last_change_tick: u64,
}

/// Per-failure-condition runtime state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1FailureConditionRuntime {
    /// Consecutive ticks the breach predicate has been true. Reset to
    /// zero on the first tick the predicate is false. The FC fires
    /// once `breach_streak_ticks > grace_ticks` (strict gt; a
    /// `grace_ticks = 0` FC fires the first tick the predicate is
    /// true).
    pub breach_streak_ticks: u64,
    /// First tick the FC fired, if ever. Sticky.
    pub fired_at_tick: Option<u64>,
}

/// Per-victory-condition runtime state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sl1VictoryConditionRuntime {
    /// First tick the VC was satisfied, if ever. Sticky.
    pub met_at_tick: Option<u64>,
}

/// High-level UX label derived deterministically server-side after
/// `game_outcome` is settled on each tick. The frontend reads this
/// string directly — no logic on the client.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Sl1GamePhase {
    /// Initial state and the catch-all when the run is in progress and
    /// no objectives have transitioned yet (or all are `Unknown`).
    #[default]
    Stabilizing,
    /// In progress, at least one objective declared, every objective
    /// currently `Met`.
    Winning,
    /// In progress, at least one objective currently `Breached`, no
    /// failure-condition currently accumulating grace.
    Losing,
    /// In progress, at least one failure condition currently has
    /// `breach_streak_ticks > 0`. Stronger signal than `Losing` —
    /// indicates a Lost is imminent.
    Spiraling,
    /// Run won.
    Won,
    /// Run lost.
    Lost,
}

impl Sl1GamePhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stabilizing => "stabilizing",
            Self::Winning => "winning",
            Self::Losing => "losing",
            Self::Spiraling => "spiraling",
            Self::Won => "won",
            Self::Lost => "lost",
        }
    }
}

// ---------------------------------------------------------------------------
// Validation.
// ---------------------------------------------------------------------------

/// Validate a parsed [`RawSl1Scene`] into a [`Sl1Scene`].
///
/// PR 3 enforces (in addition to PR 1 and PR 2):
/// - `schema_version` must equal [`SL1_SCHEMA_VERSION`].
/// - Unknown top-level fields land in [`RawSl1Scene::extra`] and are
///   rejected with [`Sl1LoadError::UnknownField`].
/// - `places` entries are typed and cross-validated against `things[]`:
///   `storage` map keys must reference declared things; `accepts` and
///   `produces` entries must reference a declared thing id or tag.
/// - `links` entries are typed; `compatibility` entries are
///   cross-validated against declared thing ids or tags.
/// - `things` entries are typed; each is validated for id charset/length
///   uniqueness, non-empty `kind`, deduplicated non-empty tags, optional
///   non-zero `schema_version`, optional `freshness_budget_ticks` in
///   `1..=`[`MAX_THING_FRESHNESS_BUDGET`], optional `quality_contract`
///   with finite `max_drop_percent` in `0.0..=1.0`, `max_late_ticks` in
///   `1..=`[`MAX_THING_LATE_TICKS`] if present, deduplicated non-empty
///   `required_fields`, and an optional render hint with a non-empty
///   `glyph`.
/// - All remaining behavior-bearing primitives must be empty —
///   [`Sl1LoadError::PrimitiveNotImplemented`] for any
///   `transforms`/`demand`/`pressure`/`objectives`/
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
    check_section_cap("victory_conditions", raw.victory_conditions.len())?;
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
    reject_non_empty!(raw.milestones, "milestones");

    // Validate things (PR 3) FIRST so places + links can cross-check
    // against the declared thing catalog.
    let mut things: Vec<Sl1Thing> = Vec::with_capacity(raw.things.len());
    let mut seen_thing_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for raw_thing in raw.things {
        let thing = validate_thing(raw_thing)?;
        if !seen_thing_ids.insert(thing.id.clone()) {
            return Err(Sl1LoadError::ThingDuplicateId { id: thing.id });
        }
        things.push(thing);
    }
    // Sort things by id so engine iteration order is independent of
    // declaration order.
    things.sort_by(|a, b| a.id.cmp(&b.id));
    // Build a flat set of every declared thing tag for O(1)
    // accepts/produces/compatibility cross-checks.
    let thing_ids: std::collections::BTreeSet<String> =
        things.iter().map(|t| t.id.clone()).collect();
    let thing_tags: std::collections::BTreeSet<String> =
        things.iter().flat_map(|t| t.tags.iter().cloned()).collect();

    // Validate places (PR 1) + cross-validate against the typed thing
    // catalog (PR 3).
    let mut places: Vec<Sl1Place> = Vec::with_capacity(raw.places.len());
    let mut seen_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for raw_place in raw.places {
        let place = validate_place(raw_place)?;
        if !seen_ids.insert(place.id.clone()) {
            return Err(Sl1LoadError::PlaceDuplicateId { id: place.id });
        }
        // Storage map keys must reference declared thing ids. PR 3
        // adds this cross-check now that things are typed; prior PRs
        // accepted any non-empty key.
        for slot_key in place.storage.keys() {
            if !thing_ids.contains(slot_key) {
                return Err(Sl1LoadError::PlaceStorageUnknownThing {
                    place_id: place.id.clone(),
                    thing_id: slot_key.clone(),
                });
            }
        }
        // accepts / produces must reference a declared thing id or
        // tag — the SL1 strict-schema rule prevents silent typos
        // exactly here.
        for value in &place.accepts {
            if !thing_ids.contains(value) && !thing_tags.contains(value) {
                return Err(Sl1LoadError::PlaceUnknownThingReference {
                    place_id: place.id.clone(),
                    field: "accepts",
                    value: value.clone(),
                });
            }
        }
        for value in &place.produces {
            if !thing_ids.contains(value) && !thing_tags.contains(value) {
                return Err(Sl1LoadError::PlaceUnknownThingReference {
                    place_id: place.id.clone(),
                    field: "produces",
                    value: value.clone(),
                });
            }
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
        // PR 3 cross-check: compatibility entries must reference a
        // declared thing id or tag.
        for value in &link.compatibility {
            if !thing_ids.contains(value) && !thing_tags.contains(value) {
                return Err(Sl1LoadError::LinkCompatibilityUnknownReference {
                    id: link.id.clone(),
                    value: value.clone(),
                });
            }
        }
        links.push(link);
    }
    // Sort links by id so engine iteration order is independent of
    // declaration order.
    links.sort_by(|a, b| a.id.cmp(&b.id));

    // Validate transforms (PR 4). Transforms cross-reference declared
    // places (via `runs_on` and `capacity_cost` keys) and declared
    // thing ids (via `inputs[].thing` / `outputs[].thing`). Tags are
    // NOT accepted in transform IO because amounts are typed.
    let places_by_id: std::collections::BTreeMap<&str, &Sl1Place> =
        places.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut transforms: Vec<Sl1Transform> = Vec::with_capacity(raw.transforms.len());
    let mut seen_transform_ids: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for raw_transform in raw.transforms {
        let transform = validate_transform(raw_transform, &places_by_id, &thing_ids)?;
        if !seen_transform_ids.insert(transform.id.clone()) {
            return Err(Sl1LoadError::TransformDuplicateId { id: transform.id });
        }
        transforms.push(transform);
    }
    transforms.sort_by(|a, b| a.id.cmp(&b.id));

    // Validate demand (PR 5). Cross-validates targets against place
    // ids and requires against thing ids. Tags are NOT accepted in
    // requires because fulfillment is keyed by exact id.
    let place_ids: std::collections::BTreeSet<String> =
        places.iter().map(|p| p.id.clone()).collect();
    let mut demand: Vec<Sl1Demand> = Vec::with_capacity(raw.demand.len());
    let mut seen_demand_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for raw_demand in raw.demand {
        let d = validate_demand(raw_demand, &place_ids, &thing_ids)?;
        if !seen_demand_ids.insert(d.id.clone()) {
            return Err(Sl1LoadError::DemandDuplicateId { id: d.id });
        }
        demand.push(d);
    }
    demand.sort_by(|a, b| a.id.cmp(&b.id));

    // Validate pressure (PR 7). Cross-validates targets against
    // declared place / demand / link ids (per-variant), and
    // type-specific parameters such as multiplier / spawn_multiplier /
    // reduction_percent / capacity bucket / storage slot.
    let link_ids: std::collections::BTreeSet<String> = links.iter().map(|l| l.id.clone()).collect();
    let demand_ids: std::collections::BTreeSet<String> =
        demand.iter().map(|d| d.id.clone()).collect();
    let mut pressure: Vec<Sl1Pressure> = Vec::with_capacity(raw.pressure.len());
    let mut seen_pressure_ids: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for raw_pressure in raw.pressure {
        let p = validate_pressure(
            raw_pressure,
            &places_by_id,
            &thing_ids,
            &demand_ids,
            &link_ids,
        )?;
        if !seen_pressure_ids.insert(p.id.clone()) {
            return Err(Sl1LoadError::PressureDuplicateId { id: p.id });
        }
        pressure.push(p);
    }
    pressure.sort_by(|a, b| a.id.cmp(&b.id));

    // Observability is validated after PR 8 sections (objectives,
    // failure conditions, victory conditions) because the metric/
    // dashboard/alert cross-reference checks need the same
    // place/thing/transform id sets that those sections built.
    let raw_observability = raw.observability;

    // ---- PR 8 — objectives ----
    let mut objectives: Vec<Sl1Objective> = Vec::with_capacity(raw.objectives.len());
    let mut seen_objective_ids: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let place_ids: std::collections::BTreeSet<&str> =
        places.iter().map(|p| p.id.as_str()).collect();
    let thing_ids: std::collections::BTreeSet<&str> =
        things.iter().map(|t| t.id.as_str()).collect();
    let demand_ids: std::collections::BTreeSet<&str> =
        demand.iter().map(|d| d.id.as_str()).collect();
    let transform_ids: std::collections::BTreeSet<&str> =
        transforms.iter().map(|t| t.id.as_str()).collect();
    for raw_obj in raw.objectives {
        let obj = validate_objective(
            raw_obj,
            &places,
            &place_ids,
            &thing_ids,
            &transform_ids,
            &demand_ids,
        )?;
        if !seen_objective_ids.insert(obj.id.clone()) {
            return Err(Sl1LoadError::ObjectiveDuplicateId { id: obj.id });
        }
        objectives.push(obj);
    }
    objectives.sort_by(|a, b| a.id.cmp(&b.id));

    // ---- PR 8 — failure conditions (depend on objectives for refs) ----
    let mut failure_conditions: Vec<Sl1FailureCondition> =
        Vec::with_capacity(raw.failure_conditions.len());
    let mut seen_fc_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let objective_ids: std::collections::BTreeSet<&str> =
        objectives.iter().map(|o| o.id.as_str()).collect();
    for raw_fc in raw.failure_conditions {
        let fc = validate_failure_condition(raw_fc, &places, &thing_ids, &objective_ids)?;
        if !seen_fc_ids.insert(fc.id.clone()) {
            return Err(Sl1LoadError::FailureConditionDuplicateId { id: fc.id });
        }
        failure_conditions.push(fc);
    }
    failure_conditions.sort_by(|a, b| a.id.cmp(&b.id));

    // ---- PR 8 — victory conditions ----
    let mut victory_conditions: Vec<Sl1VictoryCondition> =
        Vec::with_capacity(raw.victory_conditions.len());
    let mut seen_vc_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for raw_vc in raw.victory_conditions {
        let vc = validate_victory_condition(raw_vc)?;
        if !seen_vc_ids.insert(vc.id.clone()) {
            return Err(Sl1LoadError::VictoryConditionDuplicateId { id: vc.id });
        }
        victory_conditions.push(vc);
    }
    victory_conditions.sort_by(|a, b| a.id.cmp(&b.id));
    let _ = place_ids; // re-used for observability validation below.

    // ---- PR 9 — observability ----
    let observability = if let Some(raw_obs) = raw_observability {
        Some(validate_observability(raw_obs, &places, &things)?)
    } else {
        None
    };

    // ---- PR 10 — agents ----
    // Validated last so it can cross-reference every primitive
    // (places, transforms, demand, dashboards, metrics, objectives).
    let mut agents: Vec<Sl1Agent> = Vec::with_capacity(raw.agents.len());
    let mut seen_agent_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let dashboard_ids: std::collections::BTreeSet<&str> = observability
        .as_ref()
        .map(|o| o.dashboards.iter().map(|d| d.id.as_str()).collect())
        .unwrap_or_default();
    let metric_ids: std::collections::BTreeSet<&str> = observability
        .as_ref()
        .map(|o| o.metrics.iter().map(|m| m.id.as_str()).collect())
        .unwrap_or_default();
    let objective_ids_ref: std::collections::BTreeSet<&str> =
        objectives.iter().map(|o| o.id.as_str()).collect();
    let place_ids_ref: std::collections::BTreeSet<&str> =
        places.iter().map(|p| p.id.as_str()).collect();
    let transform_ids_ref: std::collections::BTreeSet<&str> =
        transforms.iter().map(|t| t.id.as_str()).collect();
    let demand_ids_ref: std::collections::BTreeSet<&str> =
        demand.iter().map(|d| d.id.as_str()).collect();
    for raw_agent in raw.agents {
        let agent = validate_agent(
            raw_agent,
            &place_ids_ref,
            &transform_ids_ref,
            &demand_ids_ref,
            &dashboard_ids,
            &metric_ids,
            &objective_ids_ref,
        )?;
        if !seen_agent_ids.insert(agent.id.clone()) {
            return Err(Sl1LoadError::AgentDuplicateId { id: agent.id });
        }
        agents.push(agent);
    }
    agents.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(Sl1Scene {
        schema_version: raw.schema_version,
        places,
        links,
        things,
        transforms,
        demand,
        pressure,
        objectives,
        failure_conditions,
        victory_conditions,
        agents,
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
// Observability validation (PR 9).
// ---------------------------------------------------------------------------

fn validate_observability(
    raw: RawSl1Observability,
    places: &[Sl1Place],
    things: &[Sl1Thing],
) -> Result<Sl1Observability, Sl1LoadError> {
    // Per-list caps.
    if raw.metrics.len() > MAX_OBSERVABILITY_ITEMS {
        return Err(Sl1LoadError::ObservabilityTooManyItems {
            section: "metrics",
            count: raw.metrics.len(),
            max: MAX_OBSERVABILITY_ITEMS,
        });
    }
    if raw.dashboards.len() > MAX_OBSERVABILITY_ITEMS {
        return Err(Sl1LoadError::ObservabilityTooManyItems {
            section: "dashboards",
            count: raw.dashboards.len(),
            max: MAX_OBSERVABILITY_ITEMS,
        });
    }
    if raw.alerts.len() > MAX_OBSERVABILITY_ITEMS {
        return Err(Sl1LoadError::ObservabilityTooManyItems {
            section: "alerts",
            count: raw.alerts.len(),
            max: MAX_OBSERVABILITY_ITEMS,
        });
    }

    let thing_ids: std::collections::BTreeSet<&str> =
        things.iter().map(|t| t.id.as_str()).collect();

    // Dashboards first — metrics that reference dashboards need the
    // dashboard set built and validated.
    let mut dashboards: Vec<Sl1Dashboard> = Vec::with_capacity(raw.dashboards.len());
    let mut seen_dashboard_ids: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for raw_d in raw.dashboards {
        let d = validate_dashboard(raw_d, &thing_ids)?;
        if !seen_dashboard_ids.insert(d.id.clone()) {
            return Err(Sl1LoadError::DashboardDuplicateId { id: d.id });
        }
        dashboards.push(d);
    }
    dashboards.sort_by(|a, b| a.id.cmp(&b.id));
    let dashboard_ids: std::collections::BTreeSet<&str> =
        dashboards.iter().map(|d| d.id.as_str()).collect();

    // Metrics next.
    let mut metrics: Vec<Sl1Metric> = Vec::with_capacity(raw.metrics.len());
    let mut seen_metric_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for raw_m in raw.metrics {
        let m = validate_metric(raw_m, places, &thing_ids, &dashboard_ids)?;
        if !seen_metric_ids.insert(m.id.clone()) {
            return Err(Sl1LoadError::MetricDuplicateId { id: m.id });
        }
        metrics.push(m);
    }
    metrics.sort_by(|a, b| a.id.cmp(&b.id));
    let metric_ids: std::collections::BTreeSet<&str> =
        metrics.iter().map(|m| m.id.as_str()).collect();

    // Alerts last.
    let mut alerts: Vec<Sl1Alert> = Vec::with_capacity(raw.alerts.len());
    let mut seen_alert_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for raw_a in raw.alerts {
        let a = validate_alert(raw_a, &metric_ids)?;
        if !seen_alert_ids.insert(a.id.clone()) {
            return Err(Sl1LoadError::AlertDuplicateId { id: a.id });
        }
        alerts.push(a);
    }
    alerts.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(Sl1Observability {
        metrics,
        dashboards,
        alerts,
    })
}

fn validate_dashboard(
    raw: RawSl1Dashboard,
    thing_ids: &std::collections::BTreeSet<&str>,
) -> Result<Sl1Dashboard, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::DashboardInvalidId { id: raw.id });
    }
    let kind = match raw.kind.as_str() {
        "report" => Sl1DashboardKind::Report,
        "live" => Sl1DashboardKind::Live,
        "ad_hoc" => Sl1DashboardKind::AdHoc,
        _ => {
            return Err(Sl1LoadError::DashboardUnsupportedKind {
                id: raw.id,
                kind: raw.kind,
            });
        }
    };
    if raw.freshness_slo_ticks == 0 {
        return Err(Sl1LoadError::DashboardFreshnessSloZero { id: raw.id });
    }
    // No upper-bound error variant for freshness_slo — the schema cap
    // is enforced via clamping in the runtime to avoid an extra error
    // variant; values above MAX_DASHBOARD_FRESHNESS_SLO_TICKS still
    // produce a deterministic outcome (`Stale` once age exceeds the
    // declared SLO, capped to u64).
    let mut depends_on: Vec<String> = Vec::with_capacity(raw.depends_on.len());
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in raw.depends_on {
        if entry.trim().is_empty() || !seen.insert(entry.clone()) {
            return Err(Sl1LoadError::DashboardInvalidDependsOn {
                id: raw.id,
                value: entry,
            });
        }
        if !thing_ids.contains(entry.as_str()) {
            return Err(Sl1LoadError::DashboardUnknownThing {
                id: raw.id,
                thing: entry,
            });
        }
        depends_on.push(entry);
    }
    depends_on.sort();
    Ok(Sl1Dashboard {
        id: raw.id,
        kind,
        depends_on,
        freshness_slo_ticks: raw.freshness_slo_ticks,
    })
}

fn validate_metric(
    raw: RawSl1Metric,
    places: &[Sl1Place],
    thing_ids: &std::collections::BTreeSet<&str>,
    dashboard_ids: &std::collections::BTreeSet<&str>,
) -> Result<Sl1Metric, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::MetricInvalidId { id: raw.id });
    }
    let kind = match raw.source.as_str() {
        "place_capacity_used_percent" => Sl1MetricSourceKind::PlaceCapacityUsedPercent,
        "place_inventory_count" => Sl1MetricSourceKind::PlaceInventoryCount,
        "dashboard_freshness" => Sl1MetricSourceKind::DashboardFreshness,
        _ => {
            return Err(Sl1LoadError::MetricUnsupportedSource {
                id: raw.id,
                source_kind: raw.source,
            });
        }
    };
    // Per-variant required / extra field checks.
    let id_str = raw.id.clone();
    let source_str = kind.as_str();
    let take = |opt: Option<String>, field: &'static str| -> Result<String, Sl1LoadError> {
        opt.ok_or_else(|| Sl1LoadError::MetricMissingField {
            id: id_str.clone(),
            source_kind: source_str,
            field,
        })
    };
    let forbid = |present: bool, field: &'static str| -> Result<(), Sl1LoadError> {
        if present {
            return Err(Sl1LoadError::MetricExtraField {
                id: id_str.clone(),
                source_kind: source_str,
                field,
            });
        }
        Ok(())
    };
    let source = match kind {
        Sl1MetricSourceKind::PlaceCapacityUsedPercent => {
            forbid(raw.thing.is_some(), "thing")?;
            forbid(raw.dashboard.is_some(), "dashboard")?;
            let place = take(raw.place, "place")?;
            let capacity = take(raw.capacity, "capacity")?;
            let Some(place_ref) = places.iter().find(|p| p.id == place) else {
                return Err(Sl1LoadError::MetricUnknownPlace { id: raw.id, place });
            };
            if !place_ref.capacity.contains_key(&capacity) {
                return Err(Sl1LoadError::MetricUnknownCapacityBucket {
                    id: raw.id,
                    place,
                    capacity,
                });
            }
            Sl1MetricSource::PlaceCapacityUsedPercent { place, capacity }
        }
        Sl1MetricSourceKind::PlaceInventoryCount => {
            forbid(raw.capacity.is_some(), "capacity")?;
            forbid(raw.dashboard.is_some(), "dashboard")?;
            let place = take(raw.place, "place")?;
            let thing = take(raw.thing, "thing")?;
            let Some(place_ref) = places.iter().find(|p| p.id == place) else {
                return Err(Sl1LoadError::MetricUnknownPlace { id: raw.id, place });
            };
            if !thing_ids.contains(thing.as_str()) {
                return Err(Sl1LoadError::MetricUnknownThing { id: raw.id, thing });
            }
            if !place_ref.storage.contains_key(&thing) {
                return Err(Sl1LoadError::MetricNoStorageSlot {
                    id: raw.id,
                    place,
                    thing,
                });
            }
            Sl1MetricSource::PlaceInventoryCount { place, thing }
        }
        Sl1MetricSourceKind::DashboardFreshness => {
            forbid(raw.place.is_some(), "place")?;
            forbid(raw.thing.is_some(), "thing")?;
            forbid(raw.capacity.is_some(), "capacity")?;
            let dashboard = take(raw.dashboard, "dashboard")?;
            if !dashboard_ids.contains(dashboard.as_str()) {
                return Err(Sl1LoadError::MetricUnknownDashboard {
                    id: raw.id,
                    dashboard,
                });
            }
            Sl1MetricSource::DashboardFreshness { dashboard }
        }
    };
    Ok(Sl1Metric { id: raw.id, source })
}

fn validate_alert(
    raw: RawSl1Alert,
    metric_ids: &std::collections::BTreeSet<&str>,
) -> Result<Sl1Alert, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::AlertInvalidId { id: raw.id });
    }
    if !metric_ids.contains(raw.metric.as_str()) {
        return Err(Sl1LoadError::AlertUnknownMetric {
            id: raw.id,
            metric: raw.metric,
        });
    }
    let predicate = match raw.predicate {
        RawSl1AlertPredicate::Gt { threshold } => Sl1AlertPredicate::Gt { threshold },
        RawSl1AlertPredicate::Lt { threshold } => Sl1AlertPredicate::Lt { threshold },
        RawSl1AlertPredicate::OutOfRange { min, max } => {
            if min > max {
                return Err(Sl1LoadError::AlertOutOfRangeInverted {
                    id: raw.id,
                    min,
                    max,
                });
            }
            Sl1AlertPredicate::OutOfRange { min, max }
        }
    };
    let severity = match raw.severity.as_str() {
        "info" => Sl1AlertSeverity::Info,
        "warning" => Sl1AlertSeverity::Warning,
        "critical" => Sl1AlertSeverity::Critical,
        _ => {
            return Err(Sl1LoadError::AlertUnsupportedSeverity {
                id: raw.id,
                severity: raw.severity,
            });
        }
    };
    Ok(Sl1Alert {
        id: raw.id,
        metric: raw.metric,
        predicate,
        severity,
    })
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

// ---------------------------------------------------------------------------
// Agent validation (PR 10).
// ---------------------------------------------------------------------------

fn validate_agent(
    raw: RawSl1Agent,
    place_ids: &std::collections::BTreeSet<&str>,
    transform_ids: &std::collections::BTreeSet<&str>,
    demand_ids: &std::collections::BTreeSet<&str>,
    dashboard_ids: &std::collections::BTreeSet<&str>,
    metric_ids: &std::collections::BTreeSet<&str>,
    objective_ids: &std::collections::BTreeSet<&str>,
) -> Result<Sl1Agent, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::AgentInvalidId { id: raw.id });
    }

    let kind = match raw.kind.as_str() {
        "mock" => Sl1AgentKind::Mock,
        "builtin" => Sl1AgentKind::Builtin,
        "llm" => Sl1AgentKind::Llm,
        _ => {
            return Err(Sl1LoadError::AgentUnknownKind {
                id: raw.id,
                kind: raw.kind,
            });
        }
    };

    if raw.role.trim().is_empty() {
        return Err(Sl1LoadError::AgentRoleEmpty { id: raw.id });
    }

    if raw.interval_ticks == 0 {
        return Err(Sl1LoadError::AgentIntervalTicksZero { id: raw.id });
    }
    if raw.interval_ticks > SL1_AGENT_MAX_INTERVAL_TICKS {
        return Err(Sl1LoadError::AgentIntervalTicksOutOfRange {
            id: raw.id,
            value: raw.interval_ticks,
            max: SL1_AGENT_MAX_INTERVAL_TICKS,
        });
    }

    if raw.observation_scope.len() > SL1_AGENT_MAX_LIST_LEN {
        return Err(Sl1LoadError::AgentTooManyEntries {
            id: raw.id,
            field: "observation_scope",
            count: raw.observation_scope.len(),
            max: SL1_AGENT_MAX_LIST_LEN,
        });
    }
    if raw.allowed_actions.len() > SL1_AGENT_MAX_LIST_LEN {
        return Err(Sl1LoadError::AgentTooManyEntries {
            id: raw.id,
            field: "allowed_actions",
            count: raw.allowed_actions.len(),
            max: SL1_AGENT_MAX_LIST_LEN,
        });
    }
    if raw.objective_weights.len() > SL1_AGENT_MAX_OBJECTIVE_WEIGHTS {
        return Err(Sl1LoadError::AgentTooManyEntries {
            id: raw.id,
            field: "objective_weights",
            count: raw.objective_weights.len(),
            max: SL1_AGENT_MAX_OBJECTIVE_WEIGHTS,
        });
    }

    let mut observation_scope: Vec<Sl1AgentObservationTarget> =
        Vec::with_capacity(raw.observation_scope.len());
    let mut seen_scope: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in raw.observation_scope {
        if !seen_scope.insert(entry.clone()) {
            return Err(Sl1LoadError::AgentObservationScopeDuplicate { id: raw.id, entry });
        }
        let target = parse_observation_target(
            &raw.id,
            &entry,
            place_ids,
            transform_ids,
            demand_ids,
            dashboard_ids,
            metric_ids,
        )?;
        observation_scope.push(target);
    }
    observation_scope.sort();

    let mut allowed_actions: Vec<Sl1AgentActionKind> =
        Vec::with_capacity(raw.allowed_actions.len());
    let mut seen_action: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for action in raw.allowed_actions {
        if !seen_action.insert(action.clone()) {
            return Err(Sl1LoadError::AgentAllowedActionsDuplicate {
                id: raw.id,
                kind: action,
            });
        }
        let parsed = match action.as_str() {
            "set_job_priority" => Sl1AgentActionKind::SetJobPriority,
            "throttle_demand" => Sl1AgentActionKind::ThrottleDemand,
            "scale_place_capacity" => Sl1AgentActionKind::ScalePlaceCapacity,
            "warm_cache" => Sl1AgentActionKind::WarmCache,
            "prioritize_transform" => Sl1AgentActionKind::PrioritizeTransform,
            "pause_report_refresh" => Sl1AgentActionKind::PauseReportRefresh,
            _ => {
                return Err(Sl1LoadError::AgentAllowedActionsUnknownKind {
                    id: raw.id,
                    kind: action,
                });
            }
        };
        allowed_actions.push(parsed);
    }
    allowed_actions.sort();

    if raw.budgets.max_cost_per_decision == 0 {
        return Err(Sl1LoadError::AgentMaxCostPerDecisionZero { id: raw.id });
    }
    if raw.budgets.cooldown_ticks > SL1_AGENT_MAX_COOLDOWN_TICKS {
        return Err(Sl1LoadError::AgentCooldownTicksOutOfRange {
            id: raw.id,
            value: raw.budgets.cooldown_ticks,
            max: SL1_AGENT_MAX_COOLDOWN_TICKS,
        });
    }

    for (objective, weight) in &raw.objective_weights {
        if !weight.is_finite() {
            return Err(Sl1LoadError::AgentObjectiveWeightNonFinite {
                id: raw.id,
                objective: objective.clone(),
            });
        }
        if !(0.0..=1.0).contains(weight) {
            return Err(Sl1LoadError::AgentObjectiveWeightOutOfRange {
                id: raw.id,
                objective: objective.clone(),
                value: *weight,
            });
        }
        if !objective_ids.contains(objective.as_str()) {
            return Err(Sl1LoadError::AgentObjectiveWeightUnknown {
                id: raw.id,
                objective: objective.clone(),
            });
        }
    }

    Ok(Sl1Agent {
        id: raw.id,
        kind,
        role: raw.role,
        interval_ticks: raw.interval_ticks,
        observation_scope,
        allowed_actions,
        max_cost_per_decision: raw.budgets.max_cost_per_decision,
        cooldown_ticks: raw.budgets.cooldown_ticks,
        objective_weights: raw.objective_weights,
    })
}

fn parse_observation_target(
    agent_id: &str,
    entry: &str,
    place_ids: &std::collections::BTreeSet<&str>,
    transform_ids: &std::collections::BTreeSet<&str>,
    demand_ids: &std::collections::BTreeSet<&str>,
    dashboard_ids: &std::collections::BTreeSet<&str>,
    metric_ids: &std::collections::BTreeSet<&str>,
) -> Result<Sl1AgentObservationTarget, Sl1LoadError> {
    let Some((kind, id)) = entry.split_once(':') else {
        return Err(Sl1LoadError::AgentObservationScopeMalformed {
            id: agent_id.to_string(),
            entry: entry.to_string(),
        });
    };
    if id.is_empty() {
        return Err(Sl1LoadError::AgentObservationScopeMalformed {
            id: agent_id.to_string(),
            entry: entry.to_string(),
        });
    }
    let target = match kind {
        "place" => {
            if !place_ids.contains(id) {
                return Err(Sl1LoadError::AgentObservationScopeUnknownId {
                    id: agent_id.to_string(),
                    kind: "place",
                    target: id.to_string(),
                });
            }
            Sl1AgentObservationTarget::Place(id.to_string())
        }
        "transform" => {
            if !transform_ids.contains(id) {
                return Err(Sl1LoadError::AgentObservationScopeUnknownId {
                    id: agent_id.to_string(),
                    kind: "transform",
                    target: id.to_string(),
                });
            }
            Sl1AgentObservationTarget::Transform(id.to_string())
        }
        "demand" => {
            if !demand_ids.contains(id) {
                return Err(Sl1LoadError::AgentObservationScopeUnknownId {
                    id: agent_id.to_string(),
                    kind: "demand",
                    target: id.to_string(),
                });
            }
            Sl1AgentObservationTarget::Demand(id.to_string())
        }
        "dashboard" => {
            if !dashboard_ids.contains(id) {
                return Err(Sl1LoadError::AgentObservationScopeUnknownId {
                    id: agent_id.to_string(),
                    kind: "dashboard",
                    target: id.to_string(),
                });
            }
            Sl1AgentObservationTarget::Dashboard(id.to_string())
        }
        "metric" => {
            if !metric_ids.contains(id) {
                return Err(Sl1LoadError::AgentObservationScopeUnknownId {
                    id: agent_id.to_string(),
                    kind: "metric",
                    target: id.to_string(),
                });
            }
            Sl1AgentObservationTarget::Metric(id.to_string())
        }
        _ => {
            return Err(Sl1LoadError::AgentObservationScopeMalformed {
                id: agent_id.to_string(),
                entry: entry.to_string(),
            });
        }
    };
    Ok(target)
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

// ---------------------------------------------------------------------------
// Thing validation helpers (PR 3).
// ---------------------------------------------------------------------------

fn validate_thing(raw: RawSl1Thing) -> Result<Sl1Thing, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::ThingInvalidId { id: raw.id });
    }
    if raw.kind.trim().is_empty() {
        return Err(Sl1LoadError::ThingEmptyKind { id: raw.id });
    }
    let tags = canonicalize_thing_tags(&raw.id, raw.tags)?;
    if let Some(version) = raw.schema_version {
        if version == 0 {
            return Err(Sl1LoadError::ThingSchemaVersionZero { id: raw.id });
        }
    }
    if let Some(budget) = raw.freshness_budget_ticks {
        if budget == 0 {
            return Err(Sl1LoadError::ThingFreshnessBudgetZero { id: raw.id });
        }
        if budget > MAX_THING_FRESHNESS_BUDGET {
            return Err(Sl1LoadError::ThingFreshnessBudgetOutOfRange {
                id: raw.id,
                value: budget,
                max: MAX_THING_FRESHNESS_BUDGET,
            });
        }
    }
    let quality_contract = match raw.quality_contract {
        None => None,
        Some(qc) => Some(validate_thing_quality_contract(&raw.id, qc)?),
    };
    let render = match raw.render {
        None => None,
        Some(hint) => {
            if hint.glyph.trim().is_empty() {
                return Err(Sl1LoadError::ThingEmptyRenderGlyph { id: raw.id });
            }
            Some(Sl1ThingRenderHint {
                glyph: hint.glyph,
                color: hint.color,
            })
        }
    };
    Ok(Sl1Thing {
        id: raw.id,
        kind: raw.kind,
        tags,
        schema_version: raw.schema_version,
        freshness_budget_ticks: raw.freshness_budget_ticks,
        quality_contract,
        render,
    })
}

fn validate_thing_quality_contract(
    thing_id: &str,
    raw: RawSl1ThingQualityContract,
) -> Result<Sl1ThingQualityContract, Sl1LoadError> {
    let max_drop_percent = match raw.max_drop_percent {
        None => None,
        Some(v) => {
            if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                return Err(Sl1LoadError::ThingQualityMaxDropPercentOutOfRange {
                    id: thing_id.to_string(),
                    value: v,
                });
            }
            // Canonicalize -0.0 to 0.0 so the deterministic hash never
            // distinguishes the two encodings.
            Some(if v == 0.0 { 0.0 } else { v })
        }
    };
    if let Some(late) = raw.max_late_ticks {
        if late == 0 {
            return Err(Sl1LoadError::ThingQualityMaxLateTicksZero {
                id: thing_id.to_string(),
            });
        }
        if late > MAX_THING_LATE_TICKS {
            return Err(Sl1LoadError::ThingQualityMaxLateTicksOutOfRange {
                id: thing_id.to_string(),
                value: late,
                max: MAX_THING_LATE_TICKS,
            });
        }
    }
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for field in &raw.required_fields {
        if field.trim().is_empty() {
            return Err(Sl1LoadError::ThingQualityRequiredFieldEmpty {
                id: thing_id.to_string(),
            });
        }
        if !seen.insert(field.clone()) {
            return Err(Sl1LoadError::ThingQualityRequiredFieldDuplicate {
                id: thing_id.to_string(),
                value: field.clone(),
            });
        }
    }
    Ok(Sl1ThingQualityContract {
        max_drop_percent,
        max_late_ticks: raw.max_late_ticks,
        required_fields: seen.into_iter().collect(),
    })
}

fn canonicalize_thing_tags(thing_id: &str, raw: Vec<String>) -> Result<Vec<String>, Sl1LoadError> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in &raw {
        if entry.trim().is_empty() {
            return Err(Sl1LoadError::ThingEmptyTag {
                id: thing_id.to_string(),
            });
        }
        if !seen.insert(entry.clone()) {
            return Err(Sl1LoadError::ThingDuplicateTag {
                id: thing_id.to_string(),
                value: entry.clone(),
            });
        }
    }
    Ok(seen.into_iter().collect())
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
// Transform validation helpers (PR 4).
// ---------------------------------------------------------------------------

fn validate_transform(
    raw: RawSl1Transform,
    places_by_id: &std::collections::BTreeMap<&str, &Sl1Place>,
    thing_ids: &std::collections::BTreeSet<String>,
) -> Result<Sl1Transform, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::TransformInvalidId { id: raw.id });
    }
    if raw.kind.trim().is_empty() {
        return Err(Sl1LoadError::TransformEmptyType { id: raw.id });
    }
    let place = places_by_id
        .get(raw.runs_on.as_str())
        .copied()
        .ok_or_else(|| Sl1LoadError::TransformUnknownPlace {
            id: raw.id.clone(),
            place: raw.runs_on.clone(),
        })?;

    let inputs = validate_transform_io(&raw.id, "inputs", raw.inputs, thing_ids)?;
    if raw.outputs.is_empty() {
        return Err(Sl1LoadError::TransformEmptyOutputs { id: raw.id });
    }
    let outputs = validate_transform_io(&raw.id, "outputs", raw.outputs, thing_ids)?;

    if raw.cadence_ticks == 0 {
        return Err(Sl1LoadError::TransformCadenceZero { id: raw.id });
    }
    if raw.cadence_ticks > MAX_TRANSFORM_TICKS {
        return Err(Sl1LoadError::TransformTicksOutOfRange {
            id: raw.id,
            field: "cadence_ticks",
            value: raw.cadence_ticks,
            max: MAX_TRANSFORM_TICKS,
        });
    }
    if raw.duration_ticks == 0 {
        return Err(Sl1LoadError::TransformDurationZero { id: raw.id });
    }
    if raw.duration_ticks > MAX_TRANSFORM_TICKS {
        return Err(Sl1LoadError::TransformTicksOutOfRange {
            id: raw.id,
            field: "duration_ticks",
            value: raw.duration_ticks,
            max: MAX_TRANSFORM_TICKS,
        });
    }
    if raw.deadline_ticks == 0 {
        return Err(Sl1LoadError::TransformDeadlineZero { id: raw.id });
    }
    if raw.deadline_ticks > MAX_TRANSFORM_TICKS {
        return Err(Sl1LoadError::TransformTicksOutOfRange {
            id: raw.id,
            field: "deadline_ticks",
            value: raw.deadline_ticks,
            max: MAX_TRANSFORM_TICKS,
        });
    }
    if raw.deadline_ticks < raw.duration_ticks {
        return Err(Sl1LoadError::TransformDeadlineLessThanDuration {
            id: raw.id,
            deadline: raw.deadline_ticks,
            duration: raw.duration_ticks,
        });
    }

    // Validate capacity_cost — keys must reference declared place
    // capacity buckets, values must be in 1..=MAX_TRANSFORM_CAPACITY_COST.
    for (key, value) in &raw.capacity_cost {
        if key.trim().is_empty() {
            return Err(Sl1LoadError::TransformCapacityCostEmptyKey { id: raw.id });
        }
        if *value == 0 {
            return Err(Sl1LoadError::TransformCapacityCostZero {
                id: raw.id,
                key: key.clone(),
            });
        }
        if *value > MAX_TRANSFORM_CAPACITY_COST {
            return Err(Sl1LoadError::TransformCapacityCostOutOfRange {
                id: raw.id,
                key: key.clone(),
                value: *value,
                max: MAX_TRANSFORM_CAPACITY_COST,
            });
        }
        if !place.capacity.contains_key(key) {
            return Err(Sl1LoadError::TransformUnknownCapacityKey {
                id: raw.id,
                key: key.clone(),
                place: raw.runs_on.clone(),
            });
        }
    }

    let failure_policy = match raw.failure_policy.as_str() {
        "retry_then_warn" => Sl1FailurePolicy::RetryThenWarn,
        "drop" => Sl1FailurePolicy::Drop,
        _ => {
            return Err(Sl1LoadError::TransformInvalidFailurePolicy {
                id: raw.id,
                policy: raw.failure_policy,
            });
        }
    };

    if raw.max_attempts == 0 {
        return Err(Sl1LoadError::TransformMaxAttemptsZero { id: raw.id });
    }
    if raw.max_attempts > MAX_TRANSFORM_MAX_ATTEMPTS {
        return Err(Sl1LoadError::TransformMaxAttemptsOutOfRange {
            id: raw.id,
            value: raw.max_attempts,
            max: MAX_TRANSFORM_MAX_ATTEMPTS,
        });
    }

    Ok(Sl1Transform {
        id: raw.id,
        kind: raw.kind,
        runs_on: raw.runs_on,
        inputs,
        outputs,
        cadence_ticks: raw.cadence_ticks,
        duration_ticks: raw.duration_ticks,
        deadline_ticks: raw.deadline_ticks,
        capacity_cost: raw.capacity_cost,
        failure_policy,
        max_attempts: raw.max_attempts,
    })
}

fn validate_transform_io(
    transform_id: &str,
    field: &'static str,
    raw: Vec<RawSl1TransformIo>,
    thing_ids: &std::collections::BTreeSet<String>,
) -> Result<Vec<Sl1TransformIo>, Sl1LoadError> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out: Vec<Sl1TransformIo> = Vec::with_capacity(raw.len());
    for entry in raw {
        if !thing_ids.contains(&entry.thing) {
            return Err(Sl1LoadError::TransformUnknownThing {
                id: transform_id.to_owned(),
                field,
                value: entry.thing,
            });
        }
        if !seen.insert(entry.thing.clone()) {
            return Err(Sl1LoadError::TransformDuplicateIo {
                id: transform_id.to_owned(),
                field,
                value: entry.thing,
            });
        }
        if entry.amount == 0 {
            return Err(Sl1LoadError::TransformIoAmountZero {
                id: transform_id.to_owned(),
                field,
                thing: entry.thing,
            });
        }
        if entry.amount > MAX_TRANSFORM_AMOUNT {
            return Err(Sl1LoadError::TransformIoAmountOutOfRange {
                id: transform_id.to_owned(),
                field,
                thing: entry.thing,
                value: entry.amount,
                max: MAX_TRANSFORM_AMOUNT,
            });
        }
        out.push(Sl1TransformIo {
            thing_id: entry.thing,
            amount: entry.amount,
        });
    }
    // Canonicalize by thing id so hash and protocol output are
    // declaration-order independent.
    out.sort_by(|a, b| a.thing_id.cmp(&b.thing_id));
    Ok(out)
}

// ---------------------------------------------------------------------------
// Demand validation helpers (PR 5).
// ---------------------------------------------------------------------------

fn validate_demand(
    raw: RawSl1Demand,
    place_ids: &std::collections::BTreeSet<String>,
    thing_ids: &std::collections::BTreeSet<String>,
) -> Result<Sl1Demand, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::DemandInvalidId { id: raw.id });
    }
    if raw.kind.trim().is_empty() {
        return Err(Sl1LoadError::DemandEmptyType { id: raw.id });
    }

    let target = validate_demand_target(&raw.id, raw.target, place_ids)?;
    let requires = validate_demand_requires(&raw.id, raw.requires, thing_ids)?;
    let spawn_schedule = validate_demand_schedule(&raw.id, raw.spawn_schedule)?;

    if raw.deadline_ticks == 0 {
        return Err(Sl1LoadError::DemandDeadlineZero { id: raw.id });
    }
    if raw.deadline_ticks > MAX_DEMAND_TICKS {
        return Err(Sl1LoadError::DemandDeadlineOutOfRange {
            id: raw.id,
            value: raw.deadline_ticks,
            max: MAX_DEMAND_TICKS,
        });
    }

    let priority = match raw.priority.as_str() {
        "low" => Sl1DemandPriority::Low,
        "normal" => Sl1DemandPriority::Normal,
        "high" => Sl1DemandPriority::High,
        "critical" => Sl1DemandPriority::Critical,
        _ => {
            return Err(Sl1LoadError::DemandInvalidPriority {
                id: raw.id,
                priority: raw.priority,
            });
        }
    };

    if raw.value > MAX_DEMAND_VALUE {
        return Err(Sl1LoadError::DemandValueOutOfRange {
            id: raw.id,
            value: raw.value,
            max: MAX_DEMAND_VALUE,
        });
    }

    let penalty = validate_demand_penalty(&raw.id, raw.penalty)?;

    Ok(Sl1Demand {
        id: raw.id,
        kind: raw.kind,
        target,
        requires,
        spawn_schedule,
        deadline_ticks: raw.deadline_ticks,
        priority,
        value: raw.value,
        penalty,
    })
}

fn validate_demand_target(
    demand_id: &str,
    raw: RawSl1DemandTarget,
    place_ids: &std::collections::BTreeSet<String>,
) -> Result<Sl1DemandTarget, Sl1LoadError> {
    match raw.kind.as_str() {
        "place" => {
            if !place_ids.contains(&raw.id) {
                return Err(Sl1LoadError::DemandUnknownTarget {
                    id: demand_id.to_string(),
                    target: raw.id,
                });
            }
            Ok(Sl1DemandTarget::Place(raw.id))
        }
        "transform" => Err(Sl1LoadError::DemandTargetKindNotImplemented {
            id: demand_id.to_string(),
            kind: "transform",
        }),
        "dashboard" => Err(Sl1LoadError::DemandTargetKindNotImplemented {
            id: demand_id.to_string(),
            kind: "dashboard",
        }),
        "virtual_sink" => Err(Sl1LoadError::DemandTargetKindNotImplemented {
            id: demand_id.to_string(),
            kind: "virtual_sink",
        }),
        _ => Err(Sl1LoadError::DemandUnknownTargetKind {
            id: demand_id.to_string(),
            kind: raw.kind,
        }),
    }
}

fn validate_demand_requires(
    demand_id: &str,
    raw: Vec<String>,
    thing_ids: &std::collections::BTreeSet<String>,
) -> Result<Vec<String>, Sl1LoadError> {
    if raw.is_empty() {
        return Err(Sl1LoadError::DemandRequiresEmpty {
            id: demand_id.to_string(),
        });
    }
    if raw.len() > MAX_DEMAND_REQUIRES {
        return Err(Sl1LoadError::DemandRequiresTooMany {
            id: demand_id.to_string(),
            count: raw.len(),
            max: MAX_DEMAND_REQUIRES,
        });
    }
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    for value in raw {
        if !thing_ids.contains(&value) {
            return Err(Sl1LoadError::DemandUnknownRequires {
                id: demand_id.to_string(),
                value,
            });
        }
        if !seen.insert(value.clone()) {
            return Err(Sl1LoadError::DemandDuplicateRequires {
                id: demand_id.to_string(),
                value,
            });
        }
        out.push(value);
    }
    // Canonicalize so hash + protocol output are declaration-order
    // independent.
    out.sort();
    Ok(out)
}

fn validate_demand_schedule(
    demand_id: &str,
    raw: RawSl1DemandSchedule,
) -> Result<Sl1DemandSchedule, Sl1LoadError> {
    match raw.kind.as_str() {
        "fixed" => {
            if raw.ticks.is_some() {
                return Err(Sl1LoadError::DemandScheduleUnexpectedField {
                    id: demand_id.to_string(),
                    kind: "fixed",
                    field: "ticks",
                });
            }
            let every_ticks =
                raw.every_ticks
                    .ok_or_else(|| Sl1LoadError::DemandScheduleMissingField {
                        id: demand_id.to_string(),
                        kind: "fixed",
                        field: "every_ticks",
                    })?;
            let start_tick =
                raw.start_tick
                    .ok_or_else(|| Sl1LoadError::DemandScheduleMissingField {
                        id: demand_id.to_string(),
                        kind: "fixed",
                        field: "start_tick",
                    })?;
            if every_ticks == 0 {
                return Err(Sl1LoadError::DemandScheduleFieldZero {
                    id: demand_id.to_string(),
                    field: "every_ticks",
                });
            }
            if start_tick == 0 {
                return Err(Sl1LoadError::DemandScheduleFieldZero {
                    id: demand_id.to_string(),
                    field: "start_tick",
                });
            }
            if every_ticks > MAX_DEMAND_TICKS {
                return Err(Sl1LoadError::DemandScheduleFieldOutOfRange {
                    id: demand_id.to_string(),
                    field: "every_ticks",
                    value: every_ticks,
                    max: MAX_DEMAND_TICKS,
                });
            }
            if start_tick > MAX_DEMAND_TICKS {
                return Err(Sl1LoadError::DemandScheduleFieldOutOfRange {
                    id: demand_id.to_string(),
                    field: "start_tick",
                    value: start_tick,
                    max: MAX_DEMAND_TICKS,
                });
            }
            Ok(Sl1DemandSchedule::Fixed {
                every_ticks,
                start_tick,
            })
        }
        "scripted" => {
            if raw.every_ticks.is_some() {
                return Err(Sl1LoadError::DemandScheduleUnexpectedField {
                    id: demand_id.to_string(),
                    kind: "scripted",
                    field: "every_ticks",
                });
            }
            if raw.start_tick.is_some() {
                return Err(Sl1LoadError::DemandScheduleUnexpectedField {
                    id: demand_id.to_string(),
                    kind: "scripted",
                    field: "start_tick",
                });
            }
            let ticks = raw
                .ticks
                .ok_or_else(|| Sl1LoadError::DemandScheduleMissingField {
                    id: demand_id.to_string(),
                    kind: "scripted",
                    field: "ticks",
                })?;
            if ticks.is_empty() {
                return Err(Sl1LoadError::DemandScheduleScriptedEmpty {
                    id: demand_id.to_string(),
                });
            }
            if ticks.len() > MAX_DEMAND_SCRIPTED_TICKS {
                return Err(Sl1LoadError::DemandScheduleScriptedTooMany {
                    id: demand_id.to_string(),
                    count: ticks.len(),
                    max: MAX_DEMAND_SCRIPTED_TICKS,
                });
            }
            let mut prev: Option<u64> = None;
            for &tick in &ticks {
                if tick == 0 {
                    return Err(Sl1LoadError::DemandScheduleScriptedTickZero {
                        id: demand_id.to_string(),
                    });
                }
                if tick > MAX_DEMAND_TICKS {
                    return Err(Sl1LoadError::DemandScheduleFieldOutOfRange {
                        id: demand_id.to_string(),
                        field: "ticks",
                        value: tick,
                        max: MAX_DEMAND_TICKS,
                    });
                }
                if let Some(p) = prev {
                    if tick <= p {
                        return Err(Sl1LoadError::DemandScheduleScriptedNotIncreasing {
                            id: demand_id.to_string(),
                            tick,
                        });
                    }
                }
                prev = Some(tick);
            }
            Ok(Sl1DemandSchedule::Scripted { ticks })
        }
        "wave" => Err(Sl1LoadError::DemandScheduleNotImplemented {
            id: demand_id.to_string(),
            kind: "wave",
        }),
        _ => Err(Sl1LoadError::DemandUnknownScheduleType {
            id: demand_id.to_string(),
            kind: raw.kind,
        }),
    }
}

fn validate_demand_penalty(
    demand_id: &str,
    raw: RawSl1DemandPenalty,
) -> Result<Sl1DemandPenalty, Sl1LoadError> {
    if raw.score > 0 {
        return Err(Sl1LoadError::DemandPenaltyScorePositive {
            id: demand_id.to_string(),
            score: raw.score,
        });
    }
    let abs = raw.score.saturating_abs();
    if abs > MAX_DEMAND_PENALTY_SCORE {
        return Err(Sl1LoadError::DemandPenaltyScoreOutOfRange {
            id: demand_id.to_string(),
            abs,
            max: MAX_DEMAND_PENALTY_SCORE,
        });
    }
    let warning = if let Some(w) = raw.warning {
        let trimmed = w.trim();
        if trimmed.is_empty() {
            return Err(Sl1LoadError::DemandPenaltyWarningEmpty {
                id: demand_id.to_string(),
            });
        }
        Some(w)
    } else {
        None
    };
    Ok(Sl1DemandPenalty {
        score: raw.score,
        warning,
    })
}

// ---------------------------------------------------------------------------
// Pressure validation (PR 7).
// ---------------------------------------------------------------------------

fn validate_pressure(
    raw: RawSl1Pressure,
    places_by_id: &std::collections::BTreeMap<&str, &Sl1Place>,
    thing_ids: &std::collections::BTreeSet<String>,
    demand_ids: &std::collections::BTreeSet<String>,
    link_ids: &std::collections::BTreeSet<String>,
) -> Result<Sl1Pressure, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(Sl1LoadError::PressureInvalidId { id: raw.id });
    }
    if raw.duration_ticks == 0 {
        return Err(Sl1LoadError::PressureDurationZero { id: raw.id });
    }
    if raw.at_tick > MAX_PRESSURE_TICKS {
        return Err(Sl1LoadError::PressureAtTickOutOfRange {
            id: raw.id,
            value: raw.at_tick,
            max: MAX_PRESSURE_TICKS,
        });
    }
    if raw.duration_ticks > MAX_PRESSURE_TICKS {
        return Err(Sl1LoadError::PressureDurationOutOfRange {
            id: raw.id,
            value: raw.duration_ticks,
            max: MAX_PRESSURE_TICKS,
        });
    }
    let end = raw.at_tick.checked_add(raw.duration_ticks);
    match end {
        None => {
            return Err(Sl1LoadError::PressureEndOverflow {
                id: raw.id,
                at_tick: raw.at_tick,
                duration_ticks: raw.duration_ticks,
                max: MAX_PRESSURE_TICKS,
            });
        }
        Some(e) if e > MAX_PRESSURE_TICKS => {
            return Err(Sl1LoadError::PressureEndOverflow {
                id: raw.id,
                at_tick: raw.at_tick,
                duration_ticks: raw.duration_ticks,
                max: MAX_PRESSURE_TICKS,
            });
        }
        Some(_) => {}
    }

    // Resolve the typed kind via canonical string. Unknown kinds
    // surface a dedicated load error rather than serde's generic
    // "unknown variant" message.
    let kind = match raw.kind.as_str() {
        "source_multiplier" => Sl1PressureKind::SourceMultiplier,
        "demand_growth" => Sl1PressureKind::DemandGrowth,
        "quota_reduction" => Sl1PressureKind::QuotaReduction,
        "path_outage" => Sl1PressureKind::PathOutage,
        "schema_drift" => Sl1PressureKind::SchemaDrift,
        "dashboard_storm" => Sl1PressureKind::DashboardStorm,
        "spot_eviction_wave" => Sl1PressureKind::SpotEvictionWave,
        "storage_metadata_storm" => Sl1PressureKind::StorageMetadataStorm,
        "cooling_degradation" => Sl1PressureKind::CoolingDegradation,
        other => {
            return Err(Sl1LoadError::PressureUnknownType {
                id: raw.id,
                kind: other.to_string(),
            });
        }
    };

    // Target must be non-empty regardless of variant — the runtime
    // always reports the pressure in events keyed by `(id, target)`.
    if raw.target.trim().is_empty() {
        return Err(Sl1LoadError::PressureMissingField {
            id: raw.id,
            kind: kind.as_str(),
            field: "target",
        });
    }

    // Per-variant parameter validation. We also reject parameters
    // that this variant does not consume to catch authoring typos
    // (e.g. setting `multiplier` on a `quota_reduction`).
    let params = match kind {
        Sl1PressureKind::SourceMultiplier => {
            // Disallow stray fields for other variants.
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "spawn_multiplier",
                raw.spawn_multiplier.is_some(),
            )?;
            reject_pressure_field(&raw.id, kind.as_str(), "capacity", raw.capacity.is_some())?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "reduction_percent",
                raw.reduction_percent.is_some(),
            )?;

            let thing = require_pressure_field(&raw.id, kind.as_str(), "thing", raw.thing.clone())?;
            if !thing_ids.contains(&thing) {
                return Err(Sl1LoadError::PressureUnknownThing { id: raw.id, thing });
            }
            let place = places_by_id.get(raw.target.as_str()).ok_or_else(|| {
                Sl1LoadError::PressureUnknownTarget {
                    id: raw.id.clone(),
                    expected: "place",
                    target: raw.target.clone(),
                }
            })?;
            if !place.storage.contains_key(&thing) {
                return Err(Sl1LoadError::PressureNoStorageSlot {
                    id: raw.id,
                    place: raw.target,
                    thing,
                });
            }
            let multiplier =
                require_pressure_field(&raw.id, kind.as_str(), "multiplier", raw.multiplier)?;
            // Convert to milli-units. Reject non-finite or
            // non-positive values; cap at MAX_PRESSURE_MULTIPLIER_MILLI.
            if !multiplier.is_finite() || multiplier <= 0.0 {
                return Err(Sl1LoadError::PressureMultiplierOutOfRange {
                    id: raw.id,
                    value: format!("{multiplier}"),
                    max_milli: MAX_PRESSURE_MULTIPLIER_MILLI,
                });
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let milli = (multiplier * 1000.0).round() as u64;
            if milli == 0 || milli > MAX_PRESSURE_MULTIPLIER_MILLI {
                return Err(Sl1LoadError::PressureMultiplierOutOfRange {
                    id: raw.id,
                    value: format!("{multiplier}"),
                    max_milli: MAX_PRESSURE_MULTIPLIER_MILLI,
                });
            }
            Sl1PressureParams::SourceMultiplier {
                thing,
                multiplier_milli: milli,
            }
        }
        Sl1PressureKind::DemandGrowth => {
            reject_pressure_field(&raw.id, kind.as_str(), "thing", raw.thing.is_some())?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "multiplier",
                raw.multiplier.is_some(),
            )?;
            reject_pressure_field(&raw.id, kind.as_str(), "capacity", raw.capacity.is_some())?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "reduction_percent",
                raw.reduction_percent.is_some(),
            )?;
            if !demand_ids.contains(&raw.target) {
                return Err(Sl1LoadError::PressureUnknownTarget {
                    id: raw.id,
                    expected: "demand",
                    target: raw.target,
                });
            }
            let spawn = require_pressure_field(
                &raw.id,
                kind.as_str(),
                "spawn_multiplier",
                raw.spawn_multiplier,
            )?;
            if spawn == 0 || spawn > MAX_PRESSURE_SPAWN_MULTIPLIER {
                return Err(Sl1LoadError::PressureSpawnMultiplierOutOfRange {
                    id: raw.id,
                    value: spawn,
                    max: MAX_PRESSURE_SPAWN_MULTIPLIER,
                });
            }
            Sl1PressureParams::DemandGrowth {
                spawn_multiplier: spawn,
            }
        }
        Sl1PressureKind::QuotaReduction => {
            reject_pressure_field(&raw.id, kind.as_str(), "thing", raw.thing.is_some())?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "multiplier",
                raw.multiplier.is_some(),
            )?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "spawn_multiplier",
                raw.spawn_multiplier.is_some(),
            )?;
            let place = places_by_id.get(raw.target.as_str()).ok_or_else(|| {
                Sl1LoadError::PressureUnknownTarget {
                    id: raw.id.clone(),
                    expected: "place",
                    target: raw.target.clone(),
                }
            })?;
            let capacity =
                require_pressure_field(&raw.id, kind.as_str(), "capacity", raw.capacity.clone())?;
            if !place.capacity.contains_key(&capacity) {
                return Err(Sl1LoadError::PressureUnknownCapacityBucket {
                    id: raw.id,
                    place: raw.target,
                    capacity,
                });
            }
            let reduction = require_pressure_field(
                &raw.id,
                kind.as_str(),
                "reduction_percent",
                raw.reduction_percent,
            )?;
            if reduction == 0 || reduction > MAX_PRESSURE_REDUCTION_PERCENT {
                return Err(Sl1LoadError::PressureReductionPercentOutOfRange {
                    id: raw.id,
                    value: reduction,
                    max: MAX_PRESSURE_REDUCTION_PERCENT,
                });
            }
            Sl1PressureParams::QuotaReduction {
                capacity,
                reduction_percent: reduction,
            }
        }
        Sl1PressureKind::PathOutage => {
            reject_pressure_field(&raw.id, kind.as_str(), "thing", raw.thing.is_some())?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "multiplier",
                raw.multiplier.is_some(),
            )?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "spawn_multiplier",
                raw.spawn_multiplier.is_some(),
            )?;
            reject_pressure_field(&raw.id, kind.as_str(), "capacity", raw.capacity.is_some())?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "reduction_percent",
                raw.reduction_percent.is_some(),
            )?;
            if !link_ids.contains(&raw.target) {
                return Err(Sl1LoadError::PressureUnknownTarget {
                    id: raw.id,
                    expected: "link",
                    target: raw.target,
                });
            }
            Sl1PressureParams::PathOutage
        }
        // Recognized-but-unsupported. These accept the common fields
        // but reject all type-specific fields so the schema stays
        // honest and authors are warned that nothing happens at run
        // time. Activation emits Sl1Warning::PressureUnsupportedInThisPr.
        Sl1PressureKind::SchemaDrift
        | Sl1PressureKind::DashboardStorm
        | Sl1PressureKind::SpotEvictionWave
        | Sl1PressureKind::StorageMetadataStorm
        | Sl1PressureKind::CoolingDegradation => {
            reject_pressure_field(&raw.id, kind.as_str(), "thing", raw.thing.is_some())?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "multiplier",
                raw.multiplier.is_some(),
            )?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "spawn_multiplier",
                raw.spawn_multiplier.is_some(),
            )?;
            reject_pressure_field(&raw.id, kind.as_str(), "capacity", raw.capacity.is_some())?;
            reject_pressure_field(
                &raw.id,
                kind.as_str(),
                "reduction_percent",
                raw.reduction_percent.is_some(),
            )?;
            // Even though these variants do not yet mutate the world,
            // the `target` must still resolve to a declared scene id so
            // typos surface as a typed load error instead of a generic
            // PressureUnsupportedInThisPr warning at activation. The
            // expected target category is fixed per variant per the SL1
            // spec.
            let expected: &'static str = match kind {
                Sl1PressureKind::SchemaDrift => "thing",
                Sl1PressureKind::DashboardStorm
                | Sl1PressureKind::SpotEvictionWave
                | Sl1PressureKind::StorageMetadataStorm
                | Sl1PressureKind::CoolingDegradation => "place",
                _ => unreachable!("supported kinds handled in earlier arms"),
            };
            let resolved = match expected {
                "thing" => thing_ids.contains(&raw.target),
                "place" => places_by_id.contains_key(raw.target.as_str()),
                _ => false,
            };
            if !resolved {
                return Err(Sl1LoadError::PressureUnknownTarget {
                    id: raw.id,
                    expected,
                    target: raw.target,
                });
            }
            Sl1PressureParams::UnsupportedInThisPr
        }
    };

    Ok(Sl1Pressure {
        id: raw.id,
        kind,
        at_tick: raw.at_tick,
        duration_ticks: raw.duration_ticks,
        target: raw.target,
        params,
    })
}

fn require_pressure_field<T>(
    id: &str,
    kind: &'static str,
    field: &'static str,
    value: Option<T>,
) -> Result<T, Sl1LoadError> {
    value.ok_or_else(|| Sl1LoadError::PressureMissingField {
        id: id.to_string(),
        kind,
        field,
    })
}

fn reject_pressure_field(
    id: &str,
    kind: &'static str,
    field: &'static str,
    present: bool,
) -> Result<(), Sl1LoadError> {
    if present {
        return Err(Sl1LoadError::PressureUnexpectedField {
            id: id.to_string(),
            kind,
            field,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Objective / FailureCondition / VictoryCondition validation (PR 8).
// ---------------------------------------------------------------------------

fn objective_id_invalid(id: &str) -> Sl1LoadError {
    Sl1LoadError::ObjectiveInvalidId { id: id.to_string() }
}

fn fc_id_invalid(id: &str) -> Sl1LoadError {
    Sl1LoadError::FailureConditionInvalidId { id: id.to_string() }
}

fn vc_id_invalid(id: &str) -> Sl1LoadError {
    Sl1LoadError::VictoryConditionInvalidId { id: id.to_string() }
}

fn require_objective_field<T>(
    id: &str,
    kind: &'static str,
    field: &'static str,
    value: Option<T>,
) -> Result<T, Sl1LoadError> {
    value.ok_or_else(|| Sl1LoadError::ObjectiveMissingField {
        id: id.to_string(),
        kind,
        field,
    })
}

fn reject_objective_field(
    id: &str,
    kind: &'static str,
    field: &'static str,
    present: bool,
) -> Result<(), Sl1LoadError> {
    if present {
        return Err(Sl1LoadError::ObjectiveUnexpectedField {
            id: id.to_string(),
            kind,
            field,
        });
    }
    Ok(())
}

fn require_fc_field<T>(
    id: &str,
    kind: &'static str,
    field: &'static str,
    value: Option<T>,
) -> Result<T, Sl1LoadError> {
    value.ok_or_else(|| Sl1LoadError::FailureConditionMissingField {
        id: id.to_string(),
        kind,
        field,
    })
}

fn reject_fc_field(
    id: &str,
    kind: &'static str,
    field: &'static str,
    present: bool,
) -> Result<(), Sl1LoadError> {
    if present {
        return Err(Sl1LoadError::FailureConditionUnexpectedField {
            id: id.to_string(),
            kind,
            field,
        });
    }
    Ok(())
}

fn require_vc_field<T>(
    id: &str,
    kind: &'static str,
    field: &'static str,
    value: Option<T>,
) -> Result<T, Sl1LoadError> {
    value.ok_or_else(|| Sl1LoadError::VictoryConditionMissingField {
        id: id.to_string(),
        kind,
        field,
    })
}

fn objective_kind_from_str(id: &str, s: &str) -> Result<Sl1ObjectiveKind, Sl1LoadError> {
    match s {
        "keep_fresh" => Ok(Sl1ObjectiveKind::KeepFresh),
        "complete_jobs_before_deadline" => Ok(Sl1ObjectiveKind::CompleteJobsBeforeDeadline),
        "maintain_utilization" => Ok(Sl1ObjectiveKind::MaintainUtilization),
        "cost_budget" => Ok(Sl1ObjectiveKind::CostBudget),
        "data_quality" => Ok(Sl1ObjectiveKind::DataQuality),
        "query_latency" => Ok(Sl1ObjectiveKind::QueryLatency),
        _ => Err(Sl1LoadError::ObjectiveUnknownType {
            id: id.to_string(),
            kind: s.to_string(),
        }),
    }
}

fn fc_kind_from_str(id: &str, s: &str) -> Result<Sl1FailureConditionKind, Sl1LoadError> {
    match s {
        "stale_target" => Ok(Sl1FailureConditionKind::StaleTarget),
        "place_state" => Ok(Sl1FailureConditionKind::PlaceState),
        "objective_breach_count" => Ok(Sl1FailureConditionKind::ObjectiveBreachCount),
        _ => Err(Sl1LoadError::FailureConditionUnknownType {
            id: id.to_string(),
            kind: s.to_string(),
        }),
    }
}

fn vc_kind_from_str(id: &str, s: &str) -> Result<Sl1VictoryConditionKind, Sl1LoadError> {
    match s {
        "survive_until" => Ok(Sl1VictoryConditionKind::SurviveUntil),
        _ => Err(Sl1LoadError::VictoryConditionUnknownType {
            id: id.to_string(),
            kind: s.to_string(),
        }),
    }
}

fn check_ticks_in_range(
    id: &str,
    field: &'static str,
    value: u64,
    min: u64,
) -> Result<(), Sl1LoadError> {
    if value < min || value > MAX_OBJECTIVE_TICKS {
        return Err(Sl1LoadError::ObjectiveValueOutOfRange {
            id: id.to_string(),
            field,
            value,
            max: MAX_OBJECTIVE_TICKS,
        });
    }
    Ok(())
}

fn validate_objective(
    raw: RawSl1Objective,
    places: &[Sl1Place],
    place_ids: &std::collections::BTreeSet<&str>,
    thing_ids: &std::collections::BTreeSet<&str>,
    transform_ids: &std::collections::BTreeSet<&str>,
    demand_ids: &std::collections::BTreeSet<&str>,
) -> Result<Sl1Objective, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(objective_id_invalid(&raw.id));
    }
    let kind = objective_kind_from_str(&raw.id, &raw.kind)?;
    let weight = raw.weight.unwrap_or(1);
    if weight == 0 || weight > MAX_OBJECTIVE_WEIGHT {
        return Err(Sl1LoadError::ObjectiveWeightOutOfRange {
            id: raw.id,
            value: weight,
            max: MAX_OBJECTIVE_WEIGHT,
        });
    }
    let id = raw.id;

    let params = match kind {
        Sl1ObjectiveKind::KeepFresh => {
            let kind_str = kind.as_str();
            let place = require_objective_field(&id, kind_str, "place", raw.place)?;
            let thing = require_objective_field(&id, kind_str, "thing", raw.thing)?;
            let max_stale_ticks =
                require_objective_field(&id, kind_str, "max_stale_ticks", raw.max_stale_ticks)?;
            reject_objective_field(&id, kind_str, "demand", raw.demand.is_some())?;
            reject_objective_field(&id, kind_str, "max_missed", raw.max_missed.is_some())?;
            reject_objective_field(&id, kind_str, "capacity", raw.capacity.is_some())?;
            reject_objective_field(&id, kind_str, "min_percent", raw.min_percent.is_some())?;
            reject_objective_field(&id, kind_str, "max_percent", raw.max_percent.is_some())?;
            reject_objective_field(&id, kind_str, "max_cost", raw.max_cost.is_some())?;
            reject_objective_field(
                &id,
                kind_str,
                "max_contract_violations",
                raw.max_contract_violations.is_some(),
            )?;
            reject_objective_field(&id, kind_str, "p95_max_ticks", raw.p95_max_ticks.is_some())?;
            reject_objective_field(&id, kind_str, "target", raw.target.is_some())?;
            check_ticks_in_range(&id, "max_stale_ticks", max_stale_ticks, 1)?;
            let place_obj = places.iter().find(|p| p.id == place).ok_or_else(|| {
                Sl1LoadError::ObjectiveUnknownTarget {
                    id: id.clone(),
                    expected: "place",
                    target: place.clone(),
                }
            })?;
            if !thing_ids.contains(thing.as_str()) {
                return Err(Sl1LoadError::ObjectiveUnknownTarget {
                    id,
                    expected: "thing",
                    target: thing,
                });
            }
            if !place_obj.storage.contains_key(thing.as_str()) {
                return Err(Sl1LoadError::ObjectiveNoStorageSlot { id, place, thing });
            }
            Sl1ObjectiveParams::KeepFresh {
                place,
                thing,
                max_stale_ticks,
            }
        }
        Sl1ObjectiveKind::CompleteJobsBeforeDeadline => {
            let kind_str = kind.as_str();
            let demand = require_objective_field(&id, kind_str, "demand", raw.demand)?;
            let max_missed = require_objective_field(&id, kind_str, "max_missed", raw.max_missed)?;
            reject_objective_field(&id, kind_str, "place", raw.place.is_some())?;
            reject_objective_field(&id, kind_str, "thing", raw.thing.is_some())?;
            reject_objective_field(
                &id,
                kind_str,
                "max_stale_ticks",
                raw.max_stale_ticks.is_some(),
            )?;
            reject_objective_field(&id, kind_str, "capacity", raw.capacity.is_some())?;
            reject_objective_field(&id, kind_str, "min_percent", raw.min_percent.is_some())?;
            reject_objective_field(&id, kind_str, "max_percent", raw.max_percent.is_some())?;
            reject_objective_field(&id, kind_str, "max_cost", raw.max_cost.is_some())?;
            reject_objective_field(
                &id,
                kind_str,
                "max_contract_violations",
                raw.max_contract_violations.is_some(),
            )?;
            reject_objective_field(&id, kind_str, "p95_max_ticks", raw.p95_max_ticks.is_some())?;
            reject_objective_field(&id, kind_str, "target", raw.target.is_some())?;
            check_ticks_in_range(&id, "max_missed", max_missed, 1)?;
            if !demand_ids.contains(demand.as_str()) {
                return Err(Sl1LoadError::ObjectiveUnknownTarget {
                    id,
                    expected: "demand",
                    target: demand,
                });
            }
            Sl1ObjectiveParams::CompleteJobsBeforeDeadline { demand, max_missed }
        }
        Sl1ObjectiveKind::MaintainUtilization => {
            let kind_str = kind.as_str();
            let place = require_objective_field(&id, kind_str, "place", raw.place)?;
            let capacity = require_objective_field(&id, kind_str, "capacity", raw.capacity)?;
            let min_percent =
                require_objective_field(&id, kind_str, "min_percent", raw.min_percent)?;
            let max_percent =
                require_objective_field(&id, kind_str, "max_percent", raw.max_percent)?;
            reject_objective_field(&id, kind_str, "thing", raw.thing.is_some())?;
            reject_objective_field(
                &id,
                kind_str,
                "max_stale_ticks",
                raw.max_stale_ticks.is_some(),
            )?;
            reject_objective_field(&id, kind_str, "demand", raw.demand.is_some())?;
            reject_objective_field(&id, kind_str, "max_missed", raw.max_missed.is_some())?;
            reject_objective_field(&id, kind_str, "max_cost", raw.max_cost.is_some())?;
            reject_objective_field(
                &id,
                kind_str,
                "max_contract_violations",
                raw.max_contract_violations.is_some(),
            )?;
            reject_objective_field(&id, kind_str, "p95_max_ticks", raw.p95_max_ticks.is_some())?;
            reject_objective_field(&id, kind_str, "target", raw.target.is_some())?;
            if min_percent > 100 || max_percent > 100 || min_percent > max_percent {
                return Err(Sl1LoadError::ObjectiveInvalidPercentRange {
                    id,
                    min: min_percent,
                    max: max_percent,
                });
            }
            let place_obj = places.iter().find(|p| p.id == place).ok_or_else(|| {
                Sl1LoadError::ObjectiveUnknownTarget {
                    id: id.clone(),
                    expected: "place",
                    target: place.clone(),
                }
            })?;
            if !place_obj.capacity.contains_key(capacity.as_str()) {
                return Err(Sl1LoadError::ObjectiveUnknownCapacityBucket {
                    id,
                    place,
                    capacity,
                });
            }
            Sl1ObjectiveParams::MaintainUtilization {
                place,
                capacity,
                min_percent,
                max_percent,
            }
        }
        Sl1ObjectiveKind::CostBudget
        | Sl1ObjectiveKind::DataQuality
        | Sl1ObjectiveKind::QueryLatency => {
            // Permissive on parameters — they are not consumed, but
            // still validated against their max range / id-resolution
            // so authors get useful diagnostics when the variants are
            // implemented.
            if let Some(v) = raw.max_cost {
                if v == 0 || v > MAX_OBJECTIVE_TICKS {
                    return Err(Sl1LoadError::ObjectiveValueOutOfRange {
                        id,
                        field: "max_cost",
                        value: v,
                        max: MAX_OBJECTIVE_TICKS,
                    });
                }
            }
            if let Some(v) = raw.max_contract_violations {
                if v == 0 || v > MAX_OBJECTIVE_TICKS {
                    return Err(Sl1LoadError::ObjectiveValueOutOfRange {
                        id,
                        field: "max_contract_violations",
                        value: v,
                        max: MAX_OBJECTIVE_TICKS,
                    });
                }
            }
            if let Some(v) = raw.p95_max_ticks {
                if v == 0 || v > MAX_OBJECTIVE_TICKS {
                    return Err(Sl1LoadError::ObjectiveValueOutOfRange {
                        id,
                        field: "p95_max_ticks",
                        value: v,
                        max: MAX_OBJECTIVE_TICKS,
                    });
                }
            }
            // If `target` is supplied, require it to resolve to a
            // currently declared id (place, thing, transform, or
            // demand). The exact target-kind contract for each of
            // these variants lands with their implementation PR
            // (observability), but typos must fail load today so
            // strict-schema posture is preserved.
            if let Some(target) = raw.target.as_deref() {
                if !place_ids.contains(target)
                    && !thing_ids.contains(target)
                    && !transform_ids.contains(target)
                    && !demand_ids.contains(target)
                {
                    return Err(Sl1LoadError::ObjectiveUnknownTarget {
                        id,
                        expected: "place|thing|transform|demand",
                        target: target.to_string(),
                    });
                }
            }
            Sl1ObjectiveParams::UnsupportedInThisPr
        }
    };

    Ok(Sl1Objective {
        id,
        kind,
        weight,
        params,
    })
}

fn validate_failure_condition(
    raw: RawSl1FailureCondition,
    places: &[Sl1Place],
    thing_ids: &std::collections::BTreeSet<&str>,
    objective_ids: &std::collections::BTreeSet<&str>,
) -> Result<Sl1FailureCondition, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(fc_id_invalid(&raw.id));
    }
    let kind = fc_kind_from_str(&raw.id, &raw.kind)?;
    let id = raw.id;
    let kind_str = kind.as_str();

    let params = match kind {
        Sl1FailureConditionKind::StaleTarget => {
            let place = require_fc_field(&id, kind_str, "place", raw.place)?;
            let thing = require_fc_field(&id, kind_str, "thing", raw.thing)?;
            let threshold_ticks =
                require_fc_field(&id, kind_str, "threshold_ticks", raw.threshold_ticks)?;
            let grace_ticks = raw.grace_ticks.unwrap_or(0);
            reject_fc_field(&id, kind_str, "state", raw.state.is_some())?;
            reject_fc_field(&id, kind_str, "objective_id", raw.objective_id.is_some())?;
            reject_fc_field(&id, kind_str, "max_count", raw.max_count.is_some())?;
            if threshold_ticks == 0 || threshold_ticks > MAX_OBJECTIVE_TICKS {
                return Err(Sl1LoadError::FailureConditionThresholdOutOfRange {
                    id,
                    value: threshold_ticks,
                    max: MAX_OBJECTIVE_TICKS,
                });
            }
            if grace_ticks > MAX_OBJECTIVE_TICKS {
                return Err(Sl1LoadError::FailureConditionGraceOutOfRange {
                    id,
                    value: grace_ticks,
                    max: MAX_OBJECTIVE_TICKS,
                });
            }
            let place_obj = places.iter().find(|p| p.id == place).ok_or_else(|| {
                Sl1LoadError::FailureConditionUnknownTarget {
                    id: id.clone(),
                    expected: "place",
                    target: place.clone(),
                }
            })?;
            if !thing_ids.contains(thing.as_str()) {
                return Err(Sl1LoadError::FailureConditionUnknownTarget {
                    id,
                    expected: "thing",
                    target: thing,
                });
            }
            if !place_obj.storage.contains_key(thing.as_str()) {
                return Err(Sl1LoadError::FailureConditionNoStorageSlot { id, place, thing });
            }
            Sl1FailureConditionParams::StaleTarget {
                place,
                thing,
                threshold_ticks,
                grace_ticks,
            }
        }
        Sl1FailureConditionKind::PlaceState => {
            let place = require_fc_field(&id, kind_str, "place", raw.place)?;
            let state = require_fc_field(&id, kind_str, "state", raw.state)?;
            let grace_ticks = raw.grace_ticks.unwrap_or(0);
            reject_fc_field(&id, kind_str, "thing", raw.thing.is_some())?;
            reject_fc_field(
                &id,
                kind_str,
                "threshold_ticks",
                raw.threshold_ticks.is_some(),
            )?;
            reject_fc_field(&id, kind_str, "objective_id", raw.objective_id.is_some())?;
            reject_fc_field(&id, kind_str, "max_count", raw.max_count.is_some())?;
            if grace_ticks > MAX_OBJECTIVE_TICKS {
                return Err(Sl1LoadError::FailureConditionGraceOutOfRange {
                    id,
                    value: grace_ticks,
                    max: MAX_OBJECTIVE_TICKS,
                });
            }
            let place_obj = places.iter().find(|p| p.id == place).ok_or_else(|| {
                Sl1LoadError::FailureConditionUnknownTarget {
                    id: id.clone(),
                    expected: "place",
                    target: place.clone(),
                }
            })?;
            let op_state = place_obj.operating_states.get(&state).ok_or_else(|| {
                Sl1LoadError::FailureConditionUnknownPlaceState {
                    id: id.clone(),
                    place: place.clone(),
                    state: state.clone(),
                }
            })?;
            // PR 8 only evaluates `UsedPercentGte`. Reject other
            // predicate kinds with a clear "supported in a later PR"
            // diagnostic.
            match &op_state.predicate {
                Sl1OperatingPredicate::UsedPercentGte { .. } => {}
                Sl1OperatingPredicate::OverloadedTicksGt { .. } => {
                    return Err(
                        Sl1LoadError::FailureConditionPlaceStatePredicateUnsupported {
                            id,
                            place,
                            state,
                            predicate: "overloaded_ticks_gt",
                        },
                    );
                }
            }
            Sl1FailureConditionParams::PlaceState {
                place,
                state,
                grace_ticks,
            }
        }
        Sl1FailureConditionKind::ObjectiveBreachCount => {
            let objective_id = require_fc_field(&id, kind_str, "objective_id", raw.objective_id)?;
            let max_count = require_fc_field(&id, kind_str, "max_count", raw.max_count)?;
            reject_fc_field(&id, kind_str, "place", raw.place.is_some())?;
            reject_fc_field(&id, kind_str, "thing", raw.thing.is_some())?;
            reject_fc_field(
                &id,
                kind_str,
                "threshold_ticks",
                raw.threshold_ticks.is_some(),
            )?;
            reject_fc_field(&id, kind_str, "grace_ticks", raw.grace_ticks.is_some())?;
            reject_fc_field(&id, kind_str, "state", raw.state.is_some())?;
            if max_count == 0 || max_count > MAX_OBJECTIVE_BREACH_COUNT {
                return Err(Sl1LoadError::FailureConditionMaxCountOutOfRange {
                    id,
                    value: max_count,
                    max: MAX_OBJECTIVE_BREACH_COUNT,
                });
            }
            if !objective_ids.contains(objective_id.as_str()) {
                return Err(Sl1LoadError::FailureConditionUnknownObjective { id, objective_id });
            }
            Sl1FailureConditionParams::ObjectiveBreachCount {
                objective_id,
                max_count,
            }
        }
    };

    Ok(Sl1FailureCondition { id, kind, params })
}

fn validate_victory_condition(
    raw: RawSl1VictoryCondition,
) -> Result<Sl1VictoryCondition, Sl1LoadError> {
    if !is_valid_sl1_id(&raw.id) {
        return Err(vc_id_invalid(&raw.id));
    }
    let kind = vc_kind_from_str(&raw.id, &raw.kind)?;
    let id = raw.id;
    let kind_str = kind.as_str();

    let params = match kind {
        Sl1VictoryConditionKind::SurviveUntil => {
            let at_tick = require_vc_field(&id, kind_str, "at_tick", raw.at_tick)?;
            if at_tick == 0 || at_tick > MAX_OBJECTIVE_TICKS {
                return Err(Sl1LoadError::VictoryConditionAtTickOutOfRange {
                    id,
                    value: at_tick,
                    max: MAX_OBJECTIVE_TICKS,
                });
            }
            Sl1VictoryConditionParams::SurviveUntil { at_tick }
        }
    };
    Ok(Sl1VictoryCondition { id, kind, params })
}

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
        // PR 11 has no behavior for its primitive — even a
        // perfectly-shaped (empty) entry must fail load, otherwise a
        // proto-SL1 scene would silently no-op. PR 1 removed `places`,
        // PR 2 removed `links`, PR 3 removed `things`, PR 4 removed
        // `transforms`, PR 5 removed `demand`, PR 7 removed
        // `pressure`, PR 8 removed `objectives` / `failure_conditions`
        // / `victory_conditions`, and PR 10 removed `agents` because
        // all are now typed and validated.
        let json = r#"{"milestones": [{}]}"#;
        let expected_section = "milestones";
        let err = load_str(json).unwrap_err();
        match err {
            Sl1LoadError::PrimitiveNotImplemented { section } => {
                assert_eq!(section, expected_section, "json was {json}");
            }
            other => panic!("expected PrimitiveNotImplemented for {json}, got {other:?}"),
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
        let err = load_str(r#"{"mystery": 1, "milestones": [{}]}"#).unwrap_err();
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
    fn empty_observability_loads_with_zero_items() {
        // observability is now a typed struct (PR 9). An empty block
        // loads successfully and creates an Sl1Observability with all
        // three lists empty.
        let json = r#"{"observability": {}}"#;
        let scene = load_str(json).expect("empty observability should load");
        let obs = scene
            .observability
            .as_ref()
            .expect("observability is present");
        assert!(obs.metrics.is_empty());
        assert!(obs.dashboards.is_empty());
        assert!(obs.alerts.is_empty());
    }

    #[test]
    fn observability_with_empty_lists_loads() {
        // Empty alerts list explicitly declared. PR 9 accepts this.
        let scene = load_str(r#"{"observability": {"alerts": []}}"#)
            .expect("empty alerts list should load");
        let obs = scene
            .observability
            .as_ref()
            .expect("observability is present");
        assert!(obs.alerts.is_empty());
    }

    #[test]
    fn observability_must_be_object() {
        // PR 9 review: arrays (or any non-object payload) must be a
        // typed load error rather than silently parsing as an empty
        // observability block. `deserialize_observability` enforces
        // this so nested strict-schema is honored at this seam too.
        let err = load_str(r#"{"observability": []}"#).expect_err("array payload must reject");
        assert!(
            matches!(err, Sl1LoadError::Parse { .. }),
            "expected Sl1LoadError::Parse, got {err:?}"
        );

        let err = load_str(r#"{"observability": "nope"}"#).expect_err("string payload must reject");
        assert!(matches!(err, Sl1LoadError::Parse { .. }));

        let err = load_str(r#"{"observability": 7}"#).expect_err("number payload must reject");
        assert!(matches!(err, Sl1LoadError::Parse { .. }));

        // Sanity: `null` and `{}` remain accepted (= empty observability).
        let scene = load_str(r#"{"observability": null}"#).expect("null is fine");
        assert!(scene.observability.is_none());
        let scene = load_str(r#"{"observability": {}}"#).expect("empty object is fine");
        assert!(scene.observability.is_some());
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
