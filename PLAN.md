# Plan

The active roadmap is:

**[`docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`](docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md)**

That spec is the single source of truth for new implementation work.

## Active implementation thesis

simetro is moving from polished kinetic scenes to stakes-bearing,
AI-operated simulations. The next roadmap centers on one JSON grammar:

- places
- links
- typed things
- transforms
- demand
- pressure
- outcomes
- agency
- observability
- milestones

The first vertical slice is **GPU Launch Week**: a simulated HPC/data
operations scenario where agents protect critical jobs and health
dashboards under GPU job surge, telemetry faults, schema drift, storage
pressure, quota/cost pressure, and dashboard freshness constraints.

## Non-negotiables

- The first v3 scene must be visibly winnable and losable.
- Within 30 seconds, a viewer should understand what the AI is trying to
  save, what is going wrong, and whether the latest AI action helped.
- Keep Azure/Kusto/Fabric/Power BI/HPC/autoresearch concepts simulated.
  Do not add live cloud integrations as part of the `scenario_language_v1` slice.
- Keep engine behavior deterministic: stable IDs, stable system order,
  typed predicates, bounded queues, and replayable outcomes.
- Use explicit v3 `LoadError`, `Warning`, `Fault`, and `GameOutcome`
  surfaces. No silent starvation, backpressure, stale metrics, or
  invalid actions.
- Extend existing `World` / `Node` / `Path` only through nested semantic
  structs and typed maps. Do not add flat optional-field sprawl.

See also:

- [`AGENTS.md`](AGENTS.md) — contributor and agent guardrails.
- [`.github/copilot-instructions.md`](.github/copilot-instructions.md) —
  Copilot review priorities.
- [`docs/testing.md`](docs/testing.md) — validation commands and PR
  delivery loop.
