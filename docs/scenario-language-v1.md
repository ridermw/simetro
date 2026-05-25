# scenario_language_v1 (SL1)

> **Status:** PR 2 — Links landed. The SL1 root, taxonomy, Place
> primitive, and Link primitive ship; later primitives (things,
> transforms, demand, …) arrive in subsequent PRs.
>
> **Authoritative spec:**
> [`docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`](superpowers/specs/2026-05-24-scenario_language_v1-plan.md)

`scenario_language_v1` (SL1) is the unified JSON grammar that turns
simetro from a kinetic-toy framework into a view-only, AI-operated
systems game. A scene authored in SL1 has visible objectives, pressure,
typed failure modes, and a terminal `GameOutcome` (`InProgress`,
`Won`, or `Lost`). A human or an LLM can read the dashboard and
understand within 30 seconds: "what is the AI trying to save", "what
is going wrong", and "did the last action help".

This document grows one section per PR. PR 0 establishes only the
skeleton.

## Where SL1 lives in a scene

SL1 is a **sibling block** at the top of the scene JSON, alongside the
legacy `pieces.{nodes,paths,movers}` grammar:

```jsonc
{
  "schema_version": 1,
  "name": "...",
  "theme": { /* ... */ },
  "pieces": { /* ... legacy grammar ... */ },
  "scenario_language_v1": {
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
    "observability": null,
    "milestones": []
  }
}
```

The SL1 block is optional. Legacy scenes without it continue to load
and tick exactly as before — backward compatibility is preserved for
the entire `scenario_language_v1` roadmap.

The SL1 block carries its own independent `schema_version` (currently
`1`). The surrounding scene's top-level `schema_version` is unchanged;
authors do not need to bump it to adopt SL1.

## Strict-schema rule

Every behavior-bearing field inside the SL1 block is strict-schema.
Unknown fields are a typed load error rather than a silent no-op. The
only permissive section is `catalog`, which holds free-form
author-facing metadata (titles, palette notes, etc.).

Today the only behavior-bearing rule is `schema_version`: PR 0 accepts
only empty primitive arrays, accepts an omitted/empty `observability`
block, and rejects placeholder entries like `"places": [{}]` as
reserved for later PRs. Each later PR adds per-primitive validation.
Unknown top-level fields are detected programmatically via a
`#[serde(flatten)]` "extra" map on `RawSl1Scene` and surface as
`LoadError::Sl1(Sl1LoadError::UnknownField { field })` — never as a
raw serde parse error. Type-shape mismatches (e.g. `"places": 42`)
surface as `LoadError::Sl1(Sl1LoadError::Parse { message })`. An
explicit `"scenario_language_v1": null` is rejected with
`LoadError::Sl1(Sl1LoadError::ExpectedObject { found: "null" })` so a
scene cannot bypass SL1 validation by writing null.

Misspelled or future-versioned top-level keys are also rejected. Any
top-level field starting with `scenario_` other than the canonical
`scenario_language_v1` surfaces as
`LoadError::Sl1MisspelledTopLevelKey { name }`. This closes a
fail-open trap where a typo such as `scenario_langauge_v1` (note
`au` ↔ `ua`) would otherwise be silently dropped and the scene
would load as legacy with no SL1 validation. Forward-looking keys
like `scenario_language_v2` are blocked the same way until the
engine declares support for them.

## Taxonomy

PR 0 introduces four typed surfaces that later PRs populate:

| Type | Purpose | Populated in |
|---|---|---|
| `Sl1LoadError` | Load-time failures (schema, unknown field, …) | PR 0 + each later PR |
| `Sl1Warning` | Non-fatal in-run conditions (starvation, late demand, stale dashboard, invalid agent action, …) | PRs 4, 5, 9, 10 |
| `Sl1Fault` | Fatal engine faults under SL1 (objective evaluator panic, schema drift, …) | PRs 8, 9 |
| `GameOutcome` | Terminal scenario state: `InProgress` \| `Won` \| `Lost { reason }` | PR 8 |

All four are `#[non_exhaustive]` so adding variants in later PRs is
not a breaking change to downstream pattern matches.

## Places (PR 1)

A **Place** is a typed location where work happens or things accumulate.
It is the foundational SL1 primitive — links, transforms, demand,
agents, and observability all reference places by id.

### Schema

```jsonc
{
  "id": "kusto-cluster",         // required, [a-zA-Z0-9_-]{1..=64}, unique
  "role": "compute",             // required, free-form non-empty string
  "pos": [120.0, 80.0],          // required, two finite f32 in [-1e6, 1e6]
  "shape": "hexagon",            // optional render hint; carried opaquely
  "color": 2,                    // optional palette index; carried opaquely
  "capacity": {                  // optional map<string, u64>
    "query_slots": 64,           // bucket name → declared capacity
    "cooling_tons": 0            // 0 is allowed (declared-but-unavailable)
  },
  "storage": {                   // optional map<thing_id, {capacity, initial}>
    "hot_cache": {
      "capacity": 1024,          // u64 > 0 (capacity=0 is rejected)
      "initial": 256             // u64, must be ≤ capacity
    }
  },
  "accepts":  ["query"],         // optional set<string>; canonicalized
  "produces": ["result"],        // optional set<string>; canonicalized
  "failure_domains": ["az1"],    // optional set<string>; canonicalized
  "operating_states": {          // optional map<state_name, {when, grace_ticks?}>
    "strained":   { "when": "query_slots.used_percent >= 80" },
    "overloaded": { "when": "query_slots.used_percent >= 95", "grace_ticks": 120 },
    "failed":     { "when": "overloaded_ticks > 600" }
  }
}
```

Unknown fields on a place are rejected at the serde layer
(`#[serde(deny_unknown_fields)]`) and surface as
`Sl1LoadError::Parse { message }`.

### Predicate templates

PR 1 supports exactly two operating-state predicate templates. There
is **no expression language** — predicate strings are matched against
fixed templates.

| Template                                | Variant                |
|-----------------------------------------|------------------------|
| `<metric>.used_percent >= <0..=100>`    | `UsedPercentGte`       |
| `overloaded_ticks > <ticks>`            | `OverloadedTicksGt`    |

Additional predicates (`inventory_gte`, generic `metric_gte`) land
with later PRs once those metrics exist (PRs 3 and 9).

### Validation rules

| Rule | Error variant |
|---|---|
| `id` non-empty, ≤64 chars, `[a-zA-Z0-9_-]` only | `PlaceInvalidId` |
| `id` unique across all places | `PlaceDuplicateId` |
| `role` non-empty (whitespace trimmed) | `PlaceEmptyRole` |
| `pos[0]`, `pos[1]` finite and in `[-1e6, 1e6]` | `PlaceInvalidPos` |
| Each `storage` slot has `capacity > 0` | `PlaceStorageCapacityZero` |
| Each `storage` slot has `initial <= capacity` | `PlaceStorageInitialExceedsCapacity` |
| `accepts`/`produces`/`failure_domains`/`capacity`/`storage` entries non-empty | `PlaceEmptyEntry` |
| `accepts`/`produces`/`failure_domains` entries unique | `PlaceDuplicateEntry` |
| Operating-state predicate matches a template | `PlaceUnsupportedPredicate` |
| `used_percent >=` threshold in `0..=100` | `PlacePercentThresholdOutOfRange` |
| Operating-state name non-empty | `PlaceEmptyOperatingStateName` |
| `used_percent` predicate references a metric in `capacity` | `PlacePredicateUnknownMetric` |
| `shape` non-empty when present (omit field for default) | `PlaceEmptyShape` |

`accepts`, `produces`, and `failure_domains` are stored sorted
ascending and de-duplicated. This eliminates cosmetic JSON ordering
as a determinism-baseline drift source.

### Deterministic exposure

Places are iterated in stable id order in every system, hashed in
that order in `state_hash::feed_sl1`, and serialized in that order
into `StaticPayload.sl1_places`. The empty-SL1 hash baseline
(`tests/baselines/sl1-empty.hash`) is preserved because the per-place
loop runs zero times when no places are declared.

### Protocol mirror

`StaticPayload` carries an `sl1_places: Vec<Sl1PlaceView>` field that
mirrors the engine struct one-to-one. The field uses
`#[serde(default, skip_serializing_if = "Vec::is_empty")]`, so legacy
non-SL1 scenes serialize without an `sl1_places` field at all and
non-Rust consumers can ignore the field until they need it.

## Links (PR 2)

A **Link** is a typed declarative edge between two places. PR 2 ships
loader + validation + deterministic hash + protocol mirror only —
runtime queue mutation, backpressure execution, and frontend
rendering arrive in later PRs (transforms/demand and PR 6's first
frontend touch).

### Schema

```jsonc
{
  "id": "telemetry-to-normalizer",     // required, same id grammar as places
  "type": "data_stream",               // required, free-form non-empty
  "from": "mycroft-gpu-heartbeats",    // required, must reference a declared place
  "to":   "normalize-heartbeats",      // required, must reference a declared place
  "direction": "forward",              // required: "forward" | "bidirectional"
  "capacity": {                        // optional map<string, u64>
    "events_per_tick": 120
  },
  "travel_ticks": 1,                   // required, u64 in 1..=MAX_LINK_TRAVEL_TICKS
  "compatibility": ["gpu_heartbeat"],  // optional set<string>; canonicalized
  "queue_capacity": 1000,              // required, u64 in 1..=MAX_LINK_QUEUE_CAPACITY
  "backpressure": "block_upstream",    // required: see backpressure table
  "render": {                          // optional render hint, carried opaquely
    "style": "flow",                   // required non-empty if `render` present
    "color": 3                         // optional palette index (u32)
  }
}
```

Unknown fields on a link or on its `render` block are rejected at the
serde layer (`#[serde(deny_unknown_fields)]`) and surface as
`Sl1LoadError::Parse { message }`.

### Closed enums

| Field          | Allowed values                                                              |
|----------------|-----------------------------------------------------------------------------|
| `direction`    | `forward`, `bidirectional`                                                  |
| `backpressure` | `block_upstream`, `drop_low_priority`, `spill_to_buffer`, `degrade_quality` |

Both fields are **required**. Omission and unknown-value cases produce
*distinct* typed errors (e.g. `LinkMissingDirection` vs
`LinkUnknownDirection`) so authoring tools can pinpoint the problem.

### Validation rules

| Rule | Error variant |
|---|---|
| `id` non-empty, ≤64 chars, `[a-zA-Z0-9_-]` only | `LinkInvalidId` |
| `id` unique across all links | `LinkDuplicateId` |
| `type` non-empty | `LinkEmptyType` |
| `from` references a declared place | `LinkUnknownPlace { which: "from" }` |
| `to` references a declared place | `LinkUnknownPlace { which: "to" }` |
| `from != to` (no self-loops) | `LinkSelfLoop` |
| `direction` present | `LinkMissingDirection` |
| `direction` value is closed-enum | `LinkUnknownDirection` |
| `backpressure` present | `LinkMissingBackpressure` |
| `backpressure` value is closed-enum | `LinkUnknownBackpressure` |
| `capacity` map keys non-empty | `LinkEmptyEntry { field: "capacity" }` |
| `compatibility` entries non-empty | `LinkEmptyEntry { field: "compatibility" }` |
| `compatibility` entries unique | `LinkDuplicateCompatibility` |
| `travel_ticks > 0` | `LinkTravelTicksZero` |
| `travel_ticks <= MAX_LINK_TRAVEL_TICKS` (1_000_000_000) | `LinkTravelTicksOutOfRange` |
| `queue_capacity > 0` | `LinkQueueCapacityZero` |
| `queue_capacity <= MAX_LINK_QUEUE_CAPACITY` (1_000_000_000) | `LinkQueueCapacityOutOfRange` |
| `render.style` non-empty when `render` present | `LinkEmptyRenderStyle` |

`compatibility` is stored sorted ascending and de-duplicated. The
`capacity` map's keys are deduplicated naturally by serde (last-wins
for duplicate JSON keys); only the `Vec<String>` form gets explicit
duplicate detection.

Cross-checking `compatibility` against declared `things[]` is **deferred
to PR 3** because the `things` primitive itself is not yet typed.

### Deterministic exposure

Links are iterated in stable id order in every system, hashed in that
order in `state_hash::feed_sl1`, and serialized in that order into
`StaticPayload.sl1_links`. The empty-SL1 and places-only hash
baselines are preserved because the per-link loop runs zero times
when no links are declared.

### Protocol mirror

`StaticPayload` carries an `sl1_links: Vec<Sl1LinkView>` field that
mirrors `Sl1Link` one-to-one, including typed `direction` and
`backpressure` enum views and an optional `render` hint. The field
uses `#[serde(default, skip_serializing_if = "Vec::is_empty")]` so
non-SL1 (or SL1 scenes with no links) serialize without the field.

## Roadmap (per `plan.md`)

| PR | Adds |
|---|---|
| 0  | Skeleton: empty SL1 block, strict schema, taxonomy stubs |
| 1  | Places |
| 2  | Links |
| 3  | Things + typed inventories |
| 4  | Transforms |
| 5  | Demand |
| 6  | GPU Launch Week scene v0 |
| 7  | Pressure events |
| 8  | Objectives, failure conditions, `GameOutcome` evaluation |
| 9  | Observability: metrics, dashboards, alerts |
| 10 | Agents + scoped actions |
| 11 | Milestones + DecisionTimeline integration |
| 12 | GPU Launch Week polish + 30-second viewer litmus |
| 13 | Policy-search runner |
| 14 | Hardening + final docs |

See the canonical spec for the full grammar.
