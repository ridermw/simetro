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
    /// Per-demand runtime state (PR 5). One entry per declared demand;
    /// the entry tracks the monotonic next instance sequence, the
    /// outstanding Pending instances in spawn order, aggregate
    /// fulfilled/dropped counters for observability and protocol
    /// snapshots, the next Scripted-schedule cursor (binary-search
    /// optimization), and a one-shot overflow flag so backlog
    /// warnings are not re-emitted every tick.
    pub demand: std::collections::BTreeMap<String, Sl1DemandRuntime>,
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
            demand,
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
    reject_non_empty!(raw.pressure, "pressure");
    reject_non_empty!(raw.objectives, "objectives");
    reject_non_empty!(raw.failure_conditions, "failure_conditions");
    reject_non_empty!(raw.agents, "agents");
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
        things,
        transforms,
        demand,
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
        // PRs 7-11 have no behavior for their primitive — even a
        // perfectly-shaped (empty) entry must fail load, otherwise a
        // proto-SL1 scene would silently no-op. PR 1 removed `places`,
        // PR 2 removed `links`, PR 3 removed `things`, PR 4 removed
        // `transforms`, and PR 5 removed `demand` from this guard
        // because all five are now typed and validated.
        for (json, expected_section) in [
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
        let err = load_str(r#"{"mystery": 1, "agents": [{}]}"#).unwrap_err();
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
