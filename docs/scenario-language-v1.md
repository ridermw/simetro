# scenario_language_v1 (SL1)

> **Status:** PR 3 — Things landed. The SL1 root, taxonomy, Place,
> Link, and Thing primitives ship; later primitives (transforms,
> demand, pressure, …) arrive in subsequent PRs.
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

## Things (PR 3)

A **thing** is a typed, countable resource that flows through places
and links and is produced/consumed by transforms. PR 3 introduces the
typed registry, per-place typed inventories, and per-(place, thing)
freshness aging. Transform/demand mutation of inventories arrives in
PRs 4 and 5; PR 3 is load + initial-state + runtime aging only.

### Schema

```jsonc
"things": [
  {
    "id": "widget",
    "kind": "product",
    "tags": ["finished", "sellable"],
    "schema_version": 1,            // optional u32
    "freshness_budget_ticks": 600,  // optional u64; absent = "not time-budgeted"
    "quality_contract": {           // optional, deny_unknown_fields
      "max_drop_percent": 0.05,     // optional f64, 0.0..=1.0 (fraction)
      "max_late_ticks": 30,         // optional u64
      "required_fields": ["sku", "qty"]
    },
    "render": {                     // optional, deny_unknown_fields
      "glyph": "W",
      "color": 16763310             // optional u32 (RGB packed)
    }
  }
]
```

### Validation rules

| Rule | Error |
|---|---|
| id matches `[a-zA-Z0-9_-]{1,64}` | `ThingInvalidId` |
| id unique across `things[]` | `ThingDuplicateId` |
| `kind` non-empty (trimmed) | `ThingEmptyKind` |
| `tags[]` entries non-empty, deduped | `ThingEmptyTag`, `ThingDuplicateTag` |
| `freshness_budget_ticks` (if present) > 0 | `ThingFreshnessBudgetZero` |
| `quality_contract.max_drop_percent` finite, in `[0.0, 1.0]` | `ThingQualityMaxDropPercentOutOfRange` |
| `quality_contract.required_fields[]` non-empty + unique | `ThingQualityRequiredFieldEmpty`, `ThingQualityRequiredFieldDuplicate` |
| `render.glyph` non-empty | `ThingEmptyRenderGlyph` |
| Unknown nested fields | `Sl1LoadError::Parse` (via `deny_unknown_fields`) |

### Cross-validation

The validator walks `places[]` and `links[]` after `things[]` is
canonicalized. Any reference to an undeclared thing id or tag is
rejected:

| Source | Error |
|---|---|
| `places[].storage.<key>` references undeclared thing id | `PlaceUnknownThingReference` |
| `places[].accepts[]` references undeclared id or tag | `PlaceUnknownThingReference` |
| `places[].produces[]` references undeclared id or tag | `PlaceUnknownThingReference` |
| `links[].compatibility[]` references undeclared id or tag | `LinkCompatibilityUnknownReference` |

`accepts` / `produces` / `compatibility` accept **either** a declared
thing id or a declared tag — duplicate canonicalized entries are
rejected at link/place validation time (per PRs 1 and 2).

### Initial inventories

`places[].storage.<thing_id>.initial` (already validated against
`capacity` in PR 1) populates `World.sl1_runtime.inventories`:
`BTreeMap<place_id, BTreeMap<thing_id, count>>`. Initial counts are
the only mutation path in PR 3. PRs 4 and 5 add transform output and
demand consumption.

### Freshness state machine

Each (place, thing) entry in `World.sl1_runtime.freshness` carries a
`FreshnessState`:

| State | Meaning |
|---|---|
| `NoData` | No write has occurred yet for this (place, thing). |
| `Ok { last_set_tick }` | Last write at tick `t`, still within budget. |
| `Stale` | Last write older than `freshness_budget_ticks`. |
| `Degraded` | (Reserved; emitted starting in PR 8.) |
| `Invalid` | (Reserved; emitted starting in PR 8.) |

If `initial > 0`, the loader seeds `Ok { last_set_tick: 0 }`. If
`initial == 0`, it seeds `NoData`. Each tick, `sl1_runtime::run`
ages `Ok { last_set_tick: t }` → `Stale` when
`world.tick.saturating_sub(t) > freshness_budget_ticks`. Things
without `freshness_budget_ticks` are never aged (a deliberate
"not time-budgeted" signal — sticky `Ok` / `NoData`).

### State hash

`feed_sl1` extends the canonical state hash with one fingerprint per
thing (id, kind, sorted tags, optional schema_version, optional
budget, optional contract, optional render hint). The per-thing loop
is gated on `!things.is_empty()` so the empty-SL1 baseline
(`sl1-empty.hash`) is unchanged. Thing-bearing fixtures
(`sl1-places.hash`, `sl1-links.hash`, `sl1-things.hash`) carry new
hashes that travel with the PR.

### Protocol mirror

`StaticPayload` carries `sl1_things: Vec<Sl1ThingView>`; each tick's
`SnapshotPayload` carries
`sl1_place_inventories: Vec<Sl1PlaceInventoryView>` with one entry
per (place, thing) including `count` and a `FreshnessStateView`. The
view is internally tagged on `"state"` (snake_case), e.g.

```jsonc
{"state": "no_data"}
{"state": "ok",       "last_set_tick": 0}
{"state": "stale",    "last_set_tick": 0}
{"state": "degraded"} // reserved, PR 8+
{"state": "invalid"}  // reserved, PR 8+
```

Both new fields use `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
so legacy + SL1-empty scenes serialize without them. Snapshot
encoding clears `sl1_place_inventories` every tick (mirroring the
`movers` pattern) and rebuilds it from `World.sl1_runtime`.

## Transforms (PR 4)

Transforms describe cadence-driven typed work: "at place P, every N
ticks, consume X of thing T and produce Y of thing U, with capacity
cost C, deadline D, and failure policy F". They are the primitive
that turns inventories into game pressure.

### Grammar

| Field | Required | Type | Notes |
|---|---|---|---|
| `id` | yes | `string` | Stable SL1 id (`[a-z][a-z0-9_-]*`, ≤64 chars). |
| `type` | yes | `string` | Open string, non-empty. Used for grouping/HUD. |
| `runs_on` | yes | `PlaceId` | Must reference a declared place. |
| `inputs` | no | `[{thing, amount}]` | Things must be declared; amounts > 0. |
| `outputs` | yes | `[{thing, amount}]` | At least one entry. Outputs are written to `runs_on`. |
| `cadence_ticks` | yes | `u64` | > 0. Cadence fires when `tick > 0 && tick % cadence_ticks == 0`. |
| `duration_ticks` | yes | `u64` | > 0. Ticks the transform stays Running once started. |
| `deadline_ticks` | yes | `u64` | > 0 and ≥ `duration_ticks`. Measured from `scheduled_at`. |
| `capacity_cost` | no | `{string: u64}` | Keys must be capacity buckets declared on `runs_on`. |
| `failure_policy` | yes | `retry_then_warn` \| `drop` | PR 4 supports these two. `degrade_quality` is reserved for PR 8 and rejected at load until then. |
| `max_attempts` | no | `u32` | Default `1` (single attempt). For `retry_then_warn`, set to `>1` to allow retries. Must be > 0 and ≤ `MAX_TRANSFORM_MAX_ATTEMPTS`. |

Unknown fields on a transform or on an io entry fail the load with
`Sl1LoadError::Parse`. `inputs` and `outputs` may not repeat the same
thing — sum amounts in JSON instead.

### State machine

```
                       cadence fires (now>0, now%cadence==0)
   ┌──────────────────────────────────────────────────────┐
   │                                                      │
   ▼                                                      │
 Idle ──start_attempt──▶ Running ───completion──────────▶ Idle
   │                       │
   │ start fails           │ now > deadline
   ▼                       ▼
 Starved / Blocked    Late (RetryThenWarn only)
   │                       │
   │ now > deadline        ├── try_start succeeds ──▶ Running (fresh deadline)
   ▼                       │                              (attempt counter unchanged)
 (failure path)            ├── try_start fails, attempt+1 ≤ max ──▶ Late (incremented)
                           └── try_start fails, attempt+1 > max  ──▶ emit Failed → Idle

Drop on deadline breach   ──▶ emit Failed → Idle (no Late transition)
```

`Failed` is **not** a persistent state — it is an event surfaced as
a one-shot `WarningPayload::Sl1Transform { event: Failed, ... }` and
the transform immediately resets to `Idle` so subsequent cadences
keep firing.

### Failure policies

- `drop`: a single attempt per cadence slot. If the running attempt
  breaches the deadline, emit `Failed` once and reset to `Idle`. Drop
  never visits `Late`.
- `retry_then_warn`: on deadline breach the transform enters `Late`.
  Each Late tick `advance_late` first calls `try_start`:
  - If `try_start` succeeds, transition to `Running` with a **fresh**
    `scheduled_at = now` so the retry receives a full deadline budget
    (required for any `duration_ticks > 1` retry to be able to
    complete).
  - If `try_start` fails, increment `attempt`. When `attempt`
    exceeds `max_attempts`, emit `Failed` and reset to `Idle`.

The capacity reserved by a failed Running attempt is released before
the failure-policy decision so that retries (and other transforms
sharing the bucket) can immediately compete for it.

### Capacity contention

Transforms are processed in stable id (BTreeMap) order each tick.
When two transforms target the same capacity bucket on the same
place, the lower id is offered the slot first. Reservations are
released on completion **and** on failure. Inputs are only consumed
once all gates (inputs, capacity, output storage) pass.

### Warnings

The `Sl1Transform` warning payload carries `transform_id`, `event`,
`tick`, and an optional `attempt`. The `event` discriminator is one
of:

- `Starved` — emitted once on entry to `Starved` (missing inputs).
- `Blocked` — emitted once on entry to `Blocked` (capacity or output
  storage refuses the start).
- `Late` — emitted once when the deadline is breached.
- `Failed` — emitted once on terminal failure (Drop, or
  RetryThenWarn at max_attempts).
- `SlotMissed` — emitted once each time a cadence fires while the
  previous instance is still non-`Idle`.

Re-entry into `Starved` or `Blocked` does **not** re-emit; warnings
are state-class change events, not per-tick.

## Demand (PR 5)

A `demand` is a deterministic spawner of "someone is waiting for
something". Each spawn creates a pending instance; the runtime
attempts to fulfill it by observing required things at the target
place, drops it if its deadline passes, and emits a typed warning
on every drop or backlog overflow.

PR 5 fulfillment is **observation-only**: when every `requires`
thing has count ≥ 1 in the target place's inventory, the oldest
Pending instance becomes Fulfilled. No inventory is decremented.
This matches the `report_refresh` example from the spec where
dashboards observe data freshness rather than consume it. A future
PR may introduce a consuming variant.

### Grammar

```json
{
  "id": "fixed_dashboard_refresh",
  "type": "report_refresh",
  "target": { "type": "place", "id": "dashboard" },
  "requires": ["report"],
  "spawn_schedule": {
    "type": "fixed",
    "every_ticks": 10,
    "start_tick": 5
  },
  "deadline_ticks": 12,
  "priority": "normal",
  "value": 10,
  "penalty": { "score": -3, "warning": "report stale" }
}
```

| Field | Notes |
|---|---|
| `target.type` | Closed for PR 5: only `place` is honored. `transform`, `dashboard`, `virtual_sink` are recognized vocabulary but rejected at load with `DemandTargetKindNotImplemented` until PR 8/9. |
| `requires` | At least one ThingId, ≤ `MAX_DEMAND_REQUIRES`. Canonicalized (sorted, deduped) so hash + protocol are declaration-order independent. |
| `spawn_schedule.type` | Closed for PR 5: `fixed` and `scripted`. `wave` is rejected at load with `DemandScheduleNotImplemented` until PR 7. |
| `spawn_schedule` (fixed) | Requires `every_ticks > 0` and `start_tick > 0`, both ≤ `MAX_DEMAND_TICKS`. Spawns at `tick == start_tick + k*every_ticks` for `k ≥ 0`. |
| `spawn_schedule` (scripted) | Requires non-empty `ticks` array, each entry > 0, strictly increasing, length ≤ `MAX_DEMAND_SCRIPTED_TICKS`. |
| `deadline_ticks` | Must be > 0 and ≤ `MAX_DEMAND_TICKS`. An instance is dropped when `now > spawned_at + deadline_ticks`. |
| `priority` | Closed enum: `low`, `normal`, `high`, `critical`. PR 5 does not act on priority — it is carried for PR 8 scheduling/scoring. |
| `value` | Reward awarded on fulfillment, ≤ `MAX_DEMAND_VALUE`. Carried in the Dropped warning so PR 8 can wire score arithmetic without a protocol bump. |
| `penalty.score` | Must be ≤ 0 (positive scores rejected) and `\|score\| ≤ MAX_DEMAND_PENALTY_SCORE`. |
| `penalty.warning` | Optional opaque author-supplied severity tag carried in runtime warnings. Empty/whitespace strings rejected. |

All nested types (`RawSl1DemandTarget`, `RawSl1DemandSchedule`,
`RawSl1DemandPenalty`) use `#[serde(deny_unknown_fields)]` so a
typo in any sub-field is a typed load error, not a silent no-op.

### Runtime pipeline

The demand system runs **after** transforms in the per-tick driver
so any same-tick produced outputs (e.g., a transform finishes
`refresh_report` on tick `T`) are visible to fulfillment on tick
`T`. Per demand, in stable id order:

1. **Spawn.** If the schedule fires at `now`:
   - if the pending backlog is at `MAX_DEMAND_OUTSTANDING`, the
     spawn is suppressed and the overflow flag edge-triggers a
     `BacklogOverflow` warning (only on the rising edge).
   - otherwise, append a new `Pending` instance with monotonic
     sequence and `deadline_tick = now + deadline_ticks`.
2. **Fulfill.** If the oldest Pending exists AND every `requires`
   thing has count ≥ 1 at the target place's inventory, pop it and
   bump `fulfilled_count`.
3. **Drop.** Drain past-deadline instances from the front
   (`now > deadline_tick`). Each drop bumps `dropped_count` and
   emits a `Dropped` warning carrying the instance sequence, the
   demand's `value`, and the demand's `penalty.score` so PR 8 can
   wire score arithmetic without a protocol change.
4. **Rearm.** If the backlog has drained below the cap, clear the
   overflow flag so a future spawn can trip it again.

### Warnings

The `Sl1Demand` warning payload carries `demand_id`, `event` (one of
`Dropped` or `BacklogOverflow`), `tick`, and (for `Dropped`) the
instance `sequence`, `value`, and `penalty_score`. `BacklogOverflow`
carries no sequence/value/penalty.

### Bounded sizes

- `MAX_DEMAND_OUTSTANDING` — per-definition pending backlog cap.
- `MAX_DEMAND_TICKS` — upper bound on any scheduling tick field.
- `MAX_DEMAND_VALUE` — upper bound on `value`.
- `MAX_DEMAND_PENALTY_SCORE` — upper bound on `|penalty.score|`.
- `MAX_DEMAND_REQUIRES` — max number of required things.
- `MAX_DEMAND_SCRIPTED_TICKS` — max scripted schedule length.

### Deterministic exposure

The per-definition static fingerprint and per-tick runtime
fingerprint are gated on `if !sl1.demand.is_empty()` so scenes
without any `demand` keep their existing baseline hashes stable.

### Protocol mirror

`StaticPayload.sl1_demand: Vec<Sl1DemandView>` carries the
declaration, and `SnapshotPayload.sl1_demand_states: Vec<Sl1DemandRuntimeView>`
carries `{ demand_id, outstanding, fulfilled_count, dropped_count,
next_sequence }` per tick.

## Frontend HUD (PR 12b)

simetro's browser frontend exposes scenario_language_v1 runtime state
through a small set of HUD components that mount alongside the
existing fault / warning / heartbeat overlays. These components
answer the 30-second **viewer litmus** for any SL1 scene:

1. **What is the AI trying to save?** → `Sl1StatusPanel`
   (`#simetro-sl1-status`) surfaces the current `GameOutcome` state
   (`in_progress` / `won` / `lost`), the derived game phase
   (`winning` / `losing` / `stabilizing` / `spiraling`), and the
   loss reason when applicable.
2. **What is going wrong?** → `Sl1DashboardChips`
   (`#simetro-sl1-dashboards`) shows one chip per dashboard with its
   freshness state colour-coded (`ok` / `stale` / `no_data`).
   `Sl1AlertStrip` (`#simetro-sl1-alerts`) renders a severity-coded
   pill per firing alert and removes the pill once the alert
   resolves.
3. **Did the last action help?** → `Sl1MilestoneStrip`
   (`#simetro-sl1-milestones`) appends one chip per fired milestone
   in fire order, deduplicated by `milestone_id` for replay safety.

### Safe-text contract

Every author-supplied string surfaced by these components
(milestone labels, dashboard ids, alert ids, outcome reasons)
renders via `textContent`. This is the SL1 reviewer policy: any
string ultimately sourced from JSON, user input, or an LLM is a
prompt-injection / XSS vector, and `innerHTML` is forbidden for it.
Unit tests in `frontend/src/tests/unit/sl1_hud.test.ts` exercise
this by feeding `<script>` and `<svg onload=…>` payloads to every
component and asserting they appear verbatim (and that no script
node is created).

### Browser-only SL1 demo (`?sl1demo=1`)

The browser-only `MockTransport` accepts an optional `sl1Mode`
flag, surfaced as the `?sl1demo=1` query parameter on the dev
server. When enabled, the mock decorates the demo scene with one
dashboard, one alert, and two milestones, and runs a short
scripted timeline so the SL1 HUD exercises every state transition
(ok → stale → ok, milestone fires, outcome flips to `won`). This
is what the Playwright suite at
`frontend/src/tests/e2e/sl1_hud.spec.ts` drives — the Tauri shell
is not required.

### Scene-switch reset

`resetLocalSceneState` (`frontend/src/app/scene_switch.ts`) calls
`hud.reset()` so every chip strip, status panel, and alert pill
clears when the registry-backed scene switch fires. Non-SL1 scenes
hide the panels (`display: none`) so the canvas surface stays
visually unchanged.

## Policy-search runner (PR 13)

The `simetro-headless policy-search` subcommand is an
autoresearch-style policy search loop over a fixed SL1 scenario.
It runs one deterministic baseline trial plus one trial per
candidate policy, scores each trial, and keeps candidates that
strictly beat the baseline.

### CLI shape

```
simetro-headless policy-search \
  --scene games/gpu-launch-week.json \
  --baseline policies/gpu-launch-week-baseline.json \
  --candidate policies/gpu-launch-week-throttler-aggressive.json \
  [--candidate <more.json> ...] \
  --ticks 2000 \
  [--seed 0] \
  [--output trials.jsonl]
```

- All trials use the same scene, same seed, same pressure schedule,
  same tick budget. Only the policy artifact differs.
- Output is JSONL: N trial rows followed by one `summary` row.
  Each row carries a `type` discriminator (`"trial" | "summary"`).
- Stdout if `--output` omitted; trial rows still go to stderr-free
  stdout in declaration order (baseline first, then candidates in
  CLI order).

### Policy artifact format

```jsonc
{
  "name": "gpu-launch-week-throttler-aggressive",
  "description": "Optional human-readable description.",
  "overrides": {
    "agents": {
      "demand-throttler": {
        "interval_ticks": 30,
        "cooldown_ticks": 60,
        "max_cost_per_decision": 25,
        "objective_weights": { "keep-jobs-fresh": 0.8 },
        "allowed_actions": ["throttle_demand"]
      }
    }
  }
}
```

- `#[serde(deny_unknown_fields)]` everywhere. Unknown top-level
  keys, unknown nested keys, and unknown agent override keys all
  fail load.
- Allowed agent override keys (whitelist):
  `interval_ticks`, `cooldown_ticks`, `max_cost_per_decision`,
  `objective_weights`, `allowed_actions`.
- `cooldown_ticks` and `max_cost_per_decision` are applied under
  `agent.budgets.{...}` in the scene. The remainder are top-level
  on the agent.
- `objective_weights` values must be `f64` in `[0, 1]` to match the
  engine loader's clamp. Values outside that range fail with
  `PolicyApplyError::ObjectiveWeightOutOfRange`.
- Unknown agent ids fail with `PolicyApplyError::UnknownAgent`
  (typed; never silent).

### Score formula (lexicographic)

Each trial produces a `TrialScore { class, weighted }`:

- **Primary key (`class`):** `OutcomeClass` enum with strict order
  `Lost(0) < InProgress(1) < Won(2)`. A `Lost` trial can never beat
  an `InProgress` trial regardless of weighted score.
- **Secondary key (`weighted: f64`):**

  ```
  weighted =
      sum(objective.weight)               for each Met objective
    - sum(objective.weight)               for each Breached objective
    + sum(fulfilled * demand.value)       for each declared demand
    - sum(dropped * demand.penalty.score) for each declared demand
    - 50.0 * fired_failure_conditions
  ```

Comparison is `(class, weighted)` lexicographic. The
`TrialScore::beats(other)` method is the source of truth.

### Trial state machine

```
Baseline ──> first trial; records baseline_score
Kept     ──> candidate.score > baseline_score (lexicographic)
Discarded──> candidate.score <= baseline_score
Failed   ──> engine panicked (catch_unwind caught it)
Blocked  ──> policy artifact failed validation (typed error)
```

`Failed` is only reached on a real Rust panic from the engine.
Every expected error (bad policy / bad scene / unknown agent) is
typed and surfaces as `Blocked`.

### Exit codes

- `0` — every trial produced a comparable score.
- `2` — at least one trial blocked (policy artifact invalid).
- `3` — at least one trial failed (engine panic).
- `4` — scene or IO error before any trial row was emitted.

### JSONL output schema

Trial row:

```jsonc
{
  "type": "trial",
  "trial_id": 0,
  "policy_name": "gpu-launch-week-baseline",
  "status": "baseline",
  "seed": 0,
  "ticks": 2000,
  "score": { "class": "in_progress", "weighted": 3206.0 },
  "outcome": "in_progress",
  "hash": "e8b1d03f..."
}
```

Summary row:

```jsonc
{
  "type": "summary",
  "trials": 3,
  "kept": 0,
  "discarded": 2,
  "failed": 0,
  "blocked": 0
}
```

Optional fields on trial rows:

- `baseline_score` — copy of the baseline's score (on candidates).
- `delta` — `candidate.weighted - baseline.weighted` (candidates).
- `lost_reason` — populated when `outcome = lost`.
- `error` — typed error message on `Blocked` / `Failed`.

### Determinism guarantee

The trial loop calls `hash_run(&mut world, &mut runner, ticks)`,
which both produces a SHA-256 fingerprint of
`(initial_world, per-tick events+messages, final_world)` AND
advances the world. The runner then reads `world.sl1_runtime`
directly for scoring — there is no separate tick loop. Two
invocations of the same policy against the same scene with the
same seed produce byte-identical JSONL (modulo timestamps, which
the runner does not emit).

### Reference artifacts

- `policies/gpu-launch-week-baseline.json` — empty-overrides
  baseline.
- `policies/gpu-launch-week-throttler-aggressive.json` — example
  candidate: `interval_ticks: 60 → 30`, `cooldown_ticks: 120 → 60`,
  modest `objective_weights` tweak on the `demand-throttler` agent.

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
