# scenario_language_v1 (SL1)

> **Status:** PR 0 skeleton. Only the grammar shape and load-time
> validation exist; no engine behavior lands until later PRs.
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
