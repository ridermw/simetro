# Stakes v1 world model plan

## Final review scope

This plan intentionally preserves the full brainstorm for review. It should not be narrowed to only one genre or one showcase yet. The review should evaluate whether simetro's next game-language direction can support all of these inspirations through one coherent JSON grammar:

- Mini-Metro-style stakes: meaningful stations, directional routes, capacity, queues, overload, objectives, and failure.
- shapez / Factorio-style systems without an avatar: typed things moving through links, transfer rules, buffers, backpressure, processors, recipes, build constraints, and throughput contracts.
- Typed operational-data factory: Mycroft events, Infinity Band signals, GPU telemetry, fault events, deployment records, capacity snapshots, and repair actions transformed into uptime facts, fault facts, Kusto dashboards, Power BI reports, Lens jobs, alerts, and remediation briefs.
- Autoresearch-style autonomous experiment loops: agents propose bounded code/model experiments, consume fixed compute windows, evaluate one trusted metric, keep/discard results, and leave a reviewable experiment trail.
- Azure datacenter operations: workload placement, power, cooling, network congestion, failure domains, SLOs, maintenance, quota, cost, carbon/water pressure, and incident response.
- HPC / GPU datacenter operations: CycleCloud/Slurm-like schedulers, HB/HC/ND-style compute pools, RDMA/InfiniBand placement, parallel storage, checkpoints, autoscale lag, quota, spot eviction, job deadlines, GPU utilization, and Well-Architected tradeoffs.
- Multi-agent operations: incident commander, scheduler operator, capacity planner, storage operator, fabric/network operator, Kusto/observability operator, data quality guardian, dashboard/report owner, and cost governor.
- Concrete next artifacts: a unified schema draft, a GPU launch-week scene JSON, minimum-slice validation/pruning, showcase-to-grammar mapping, and a staged implementation plan.

The central design thesis is: the engine should not know "Azure", "Factorio", or "Metro" as special modes. It should know places, links, typed things, transforms, demand, pressure, outcomes, agency, observability, and milestones. The domain should live in JSON content and theme.

## SPAR + mega-review synthesis

The post-plan SPAR and mega-review sharpened the plan into one key tension:

- **Platform ambition:** simetro can become a deterministic systems-game DSL where Metro congestion, factory backpressure, Kusto dashboard freshness, GPU job queues, and datacenter cooling pressure are all skins over the same grammar.
- **Playable/operable clarity:** if the first implementation is too abstract, it becomes a beautiful ops dashboard or an unmaintainable `World` junk drawer instead of a legible game.

The plan should keep the cathedral vision, but the first implementation must pass this litmus test:

> Within 30 seconds, a viewer should know what the AI is trying to save, what is going wrong, and whether the AI's last action helped or hurt.

### Decisions carried forward

- Use **one unified grammar**: places, links, typed things, transforms, demand, pressure, outcomes, agency, observability, and milestones.
- Keep Azure, Kusto, Databricks/Fabric, Power BI, Lens, HPC, and Factorio-like concepts as **simulated domain nouns**, not live integrations.
- Extend existing `World` / `Node` / `Path` directly, per user direction, but only through **nested semantic structs** and typed maps. Do not add flat optional-field sprawl.
- Use a **strict v3 behavior schema**. Unknown behavior-bearing fields are load errors; only `catalog`/metadata remains permissive.
- Use **typed predicates only**. No expression strings or script-like condition language.
- Add explicit v3 `LoadError`, `Warning`, `Fault`, and `GameOutcome` taxonomy before implementation.
- Add explicit metric states: `ok`, `no_data`, `stale`, `degraded`, `invalid`; never confuse zero with missing telemetry.
- Add target-scoped agent capabilities and target-version preconditions so multi-agent actions cannot silently stomp stale state.
- Require visible stakes for every showcase: timer/deadline, pressure source, failure condition, victory condition, and narratable AI actions.
- Treat autoresearch-style "program files" as **agent policy/heuristic artifacts** inside the simulation, not as arbitrary executable scripts.

### First-showcase direction after review

The recommended first showcase is no longer generic network pressure. It should be **GPU Launch Week** because it exercises the unique product thesis:

- HPC/GPU cluster pressure: scheduler queues, GPU pools, RDMA placement, checkpoint/storage pressure, spot evictions, quota, utilization, cost.
- Typed data-factory pressure: Mycroft events, Infinity Band signals, GPU heartbeats/faults, deployment and repair records becoming uptime/fault facts.
- Observability pressure: Kusto GPU health dashboard, Power BI uptime report, Lens fault review, dashboard storm, schema drift, freshness/correctness SLOs.
- Multi-agent operations: scheduler operator, capacity planner, storage operator, fabric/network operator, Kusto/observability operator, data quality guardian, cost governor, incident commander.
- Autonomous research pressure: the game scenario is fixed, but the agent iterates on one gameplay heuristic/algorithm change at a time, reruns the same simulation, compares outcome metrics, keeps/discards the policy, and builds an overnight progress log.

This scene should open with a calm state, then quickly introduce a legible crisis: a GPU job surge and fault storm threaten a critical launch-review dashboard. The viewer should see queues grow, dashboards age, storage saturate, and agents choose visible interventions.

### Autoresearch fit after review

`karpathy/autoresearch` is in scope as a **simulated policy-search environment** layered on the same grammar. The important interpretation for simetro is not "agent mutates training code"; it is "agent changes how it plays a fixed game scenario, reruns the scenario, measures outcome quality, and keeps/discards the heuristic."

- A human edits a lightweight `program.md` that defines the autonomous research org / policy search doctrine.
- The agent is constrained to one mutable artifact: its gameplay heuristic or policy configuration for a fixed scenario.
- The scenario mechanics, seed, pressure schedule, evaluator, and scoring contract are fixed for the experiment batch.
- Each trial runs the same simulated game under a fixed tick/time budget, producing comparable results.
- The trusted metric is scenario-specific: e.g. final score, loss avoided, dashboard freshness, cost, critical job completion, data correctness, or a weighted objective score.
- The agent records result rows, keeps policy changes that improve the trusted metric, discards regressions/crashes, and loops repeatedly.

In simetro, this should **not** mean running real training jobs or letting agents mutate real source code inside the engine. It should mean modeling an autoresearch-like loop as **policy search over fixed game mechanics**:

- `program.md` becomes an agent policy/search-strategy artifact.
- `train.py` maps to the **agent heuristic under test**, not the world code.
- `prepare.py` maps to the immutable scenario/evaluator: same map, same seed, same pressure schedule, same scoring.
- Fixed 5-minute runs map to bounded simulation trials: e.g. `run scenario for 10,000 ticks`.
- `val_bpb` maps to a trusted scenario metric: e.g. weighted objective score or "loss avoided with minimum cost."
- Peak VRAM maps to policy complexity/cost: e.g. action count, compute budget, intervention cost, or strategy complexity.
- Crash status maps to invalid policy, timeout, illegal action storm, or terminal scenario failure.
- Keep/discard/reset becomes a deterministic policy-selection state machine.
- `results.tsv` becomes a simulated experiment log / dashboard / replay artifact.

This is impactful for real-world roleplay scenarios because it answers a different question than a single AI run:

> Given fixed mechanics that approximate a real-world sandbox, what play heuristic repeatedly performs best under the same pressure?

That makes simetro useful not only as an AI-played game, but as a **policy laboratory**. The world stays constant; the agent's operating doctrine evolves.

### Policy-search loop

This is **easily added to the plan** because it reuses the same primitives: places, links, typed things, transforms, demand, pressure, outcomes, agency, observability, and milestones. It should be added as an adjacent scenario or sub-loop, not as part of the first engine implementation unless the GPU Launch Week slice already has jobs, metrics, and dashboards working.

```text
fixed scenario + seed + evaluator
  │
  ▼
baseline policy run
  │
  ▼
agent proposes exactly one heuristic change
  │
  ▼
run same scenario under fixed budget
  │
  ├── invalid policy / timeout / illegal action storm ──▶ discard + log failure
  ├── worse metric ─────────────────────────────────────▶ discard + revert policy
  ├── same metric but simpler policy ───────────────────▶ optionally keep simplification
  └── better metric ────────────────────────────────────▶ keep + advance baseline
  │
  ▼
experiment log + replay diff + next hypothesis
```

Policy candidate state machine:

```text
baseline → proposed → trial_running → evaluated → kept
                         │              │
                         │              ├── worse_or_equal_complex → discarded
                         │              ├── equal_but_simpler ─────▶ kept_simplification
                         │              └── invalid/crash/timeout ─▶ failed
                         └── budget_denied ───────────────────────▶ blocked
```

### Example policy knobs for GPU Launch Week

The world mechanics do not change. Only one heuristic changes per trial:

- Scheduler policy: prioritize critical jobs by deadline vs by GPU-hours remaining.
- Dashboard policy: reserve fixed Kusto query slots for health dashboard vs opportunistic refresh.
- Storage policy: checkpoint every N ticks vs adaptive checkpointing under eviction risk.
- Data-quality policy: block stale facts vs allow degraded reports with warning.
- Cost policy: use spot aggressively vs reserve dedicated nodes for critical jobs.
- Incident policy: let specialized agents act independently vs incident commander override mode.

Outcome metrics can be multi-objective but must collapse into one trusted comparison score for keep/discard:

```text
policy_score =
  + completed_critical_jobs
  + dashboard_freshness_score
  + data_quality_score
  + gpu_utilization_score
  - cost_penalty
  - illegal_action_penalty
  - stale_report_penalty
  - terminal_loss_penalty
```

### Scope recommendation update

This is **more in scope than the previous interpretation**. It does not require simulating real model training. It requires:

1. a fixed deterministic scenario runner,
2. policy artifacts that agents can modify safely,
3. a trusted evaluator,
4. a trial log,
5. replay diffing between baseline and candidate policies.

It should influence the first v3 design now, even if full overnight policy search ships later. The GPU Launch Week scene should be designed so it can eventually run in two modes:

- **single-run spectator mode:** watch agents handle one crisis;
- **autoresearch policy-search mode:** run many trials to discover better agent heuristics for that same crisis.

### Non-negotiable review constraints

- The first v3 scene must be winnable and losable.
- The HUD must answer: "Are we winning, losing, stabilizing, or spiraling?"
- Backpressure, starvation, stale dashboards, data-quality violations, invalid actions, and objective breaches must never be silent.
- Frontend/replay/inspector support is not polish; it is required for the gameplay loop.
- The first implementation should stay vertical and small even though the vision remains broad.

## Problem

The current simetro worlds are attractive animated networks, but the world model does not yet create game tension. Nodes are mostly coordinates plus shape/color. Paths are directed edges visually used for movement, but they have no capacity, congestion, cost, ownership, mode, or strategic tradeoff. Nodes have no queue, demand, capacity, health, failure threshold, production role, or win/loss relevance. The only goal is `loop_forever`, so the viewer cannot tell whether the AI is winning, losing, surviving, optimizing, or making a mistake.

Current implementation facts:

- `Node` is `id`, `pos`, `shape`, `color`; shape/color are visual language only.
- `Path` is `id`, `from`, `to`, `color`; paths are directional by `from -> to`, but this is only a traversal direction.
- Routing chooses the lowest-id outgoing path from the current node, not a useful or agent-directed route.
- `Mover.home_path` seeds initial placement, but the current interaction system does not use it for ongoing route choice.
- There is no node capacity, path capacity, queueing, overcrowding, cargo/passenger destination, deadline, score, or failure condition.
- Schema v2 has resources/producers/consumers, but they are global inventory counters, not spatial factory/logistics dynamics.

## Proposed approach

Build a small, vertical "stakes v1" world model before expanding more worlds. The goal is to make one showcase scene feel like an AI is playing a game under pressure, then generalize the JSON schema once the loop is fun.

### Core design

1. Make nodes meaningful.

Add node roles such as `origin`, `destination`, `hub`, `processor`, `storage`, `sink`, or `hazard`.Add `capacity` and current `queue`/`load`.Add per-node failure pressure: e.g. `overflow_threshold`, `grace_ticks`, `deadline_penalty`, or `criticality`.Preserve shape/color as visual language, but no longer rely on them as the only semantics.

2. Make paths strategic.

Keep paths explicitly directional by default; bidirectional connections should be represented by two directed paths unless a helper expands them at load time.Add `capacity` or `slots` so only N movers/items can occupy a path at once.Add `travel_ticks` or `speed_limit` so paths have meaningful cost.Add optional `mode`/`cargo_filter`/`resource_filter` so not every path is equally useful.Add congestion state for renderer and agent observations.

3. Add demand and stakes.

Introduce demand units: passenger/order/cargo with `source`, `target`, `spawn_schedule`, `deadline_ticks`, `value`, and `penalty`.Track delivered, waiting, late, dropped, and failed counts.Add scored objectives: `deliver_count`, `survive_until`, `maintain_service_level`, `accumulate_resource`, or `complete_orders`.Add loss conditions: node overflow, too many missed deadlines, blackout/outage, negative score, or critical demand failure.

4. Give AI actual levers.

In the first slice, keep the action surface narrow: route/dispatch/allocate capacity before adding full editor-like construction.Candidate actions: `set_route`, `dispatch_mover`, `prioritize_node`, `throttle_source`, `upgrade_node_capacity`, `upgrade_path_capacity`.Later author actions can place nodes/paths/processors once the pressure loop is visible.

5. Surface the game state.

Static/snapshot protocol should expose node load/capacity, path occupancy/capacity, active demand, score, objective progress, and failure risk.Renderer should make drama visible: queue rings, red overload halos, path saturation pulses, deadline warnings, score/progress HUD.Agent observations must include the same pressure information, not just mover speed/state.

## Ten ways to expand the JSON game language

These are speculative primitives the JSON format could grow so simetro becomes an open-ended AI-played game platform instead of a fixed transit clone or fixed factory clone. The common thread: every primitive should create visible tension, agent-relevant choices, and measurable win/loss pressure.

1. Semantic node roles.

Give nodes gameplay identity: `station`, `mine`, `processor`, `warehouse`, `sink`, `hub`, `hospital`, `substation`, `market`, `hazard`, `checkpoint`.Example fields: `role`, `accepts`, `produces`, `transforms`, `stores`, `criticality`.Why it matters: a square/circle stops being decoration and becomes a place with obligations and consequences.

2. Node capacity, queues, and failure thresholds.

Add node-level `capacity`, `queue_capacity`, `overflow_grace_ticks`, `service_rate`, and `failure_mode`.A node can be "healthy", "strained", "overloaded", then "failed".This creates Mini-Metro-style drama: visible crowding and a countdown to death.

3. Path capabilities and constraints.

Keep paths directional by default, but let JSON declare `bidirectional: true` as authoring sugar if useful.Add `capacity`, `travel_ticks`, `speed_limit`, `allowed_cargo`, `mode`, `reliability`, `toll`, `cooldown_ticks`.This makes route choice strategic: the shortest route may be saturated, slow, unsafe, one-way, or incompatible.

4. Demand/order language.

Add explicit demand objects that spawn passengers, cargo, packets, emergencies, power loads, or shape orders.Example fields: `source`, `destination`, `resource`, `amount`, `spawn_schedule`, `deadline_ticks`, `value`, `penalty`, `priority`.This is the main missing stakes primitive: the world asks for something, and the AI either satisfies it or falls behind.

5. Spatial resources and extraction.

Replace or augment global inventory with map-tied deposits/sources.Example fields: `resource_nodes`, `yield_per_tick`, `depletion`, `regeneration`, `distance_pressure`.This supports shapez-style expansion: resources are not just counters, they are places the network must reach.

6. Recipes, processors, and transformations.

Let JSON define recipes as deterministic transforms: inputs -> outputs, with processing time and capacity.Example fields: `recipes`, `processors`, `input_ports`, `output_ports`, `batch_size`, `process_ticks`.This is the factory-language counterpart to transit demand: the AI must build/route a chain, not just move generic dots.

7. Objectives, scoring, and loss conditions.

Add top-level `objectives`, `scoring`, and `failure_conditions`.Examples: `survive_until`, `deliver_count`, `complete_orders`, `maintain_uptime`, `avoid_overflow`, `hit_throughput`, `protect_critical_nodes`.The viewer needs a HUD answer to: are we winning, losing, stabilizing, or spiraling?

8. Pressure curves and timed world mutations.

Add scheduled events: new stations, demand growth, disasters, resource depletion, target recipe changes, path outages.Example fields: `timeline`, `waves`, `growth_rate`, `mutation_schedule`, `difficulty_curve`.This creates drama over time: an AI strategy that works early can become insufficient later.

9. Agent affordances and policy limits.

JSON should declare what the AI is allowed to do in this scene.Example fields: `allowed_actions`, `budgets`, `upgrade_costs`, `build_limits`, `cooldowns`, `route_policy`.This prevents the game from becoming "the AI can magically fix anything" and creates strategic scarcity.

10. Visibility, drama, and narrative annotations.

Add author-controlled hints for rendering and interpretation: `danger_zones`, `bottleneck_labels`, `milestones`, `camera_focus`, `music_intensity_rules`.These should not drive hidden game logic, but they help the viewer understand why a moment matters.For an AI-played view-only game, legibility is part of the game language.

## Platform-level schema direction

Do not hard-code "Metro" or "Factory" as the only two modes. Instead, use a small shared grammar:

- **Places:** nodes with roles, capacity, state, and failure pressure.
- **Links:** directed paths with cost, capacity, compatibility, and congestion.
- **Things:** passengers, cargo, packets, power, shapes, orders, vehicles, or abstract demand units.
- **Transforms:** recipes/processors/services that change things over time.
- **Pressure:** demand schedules, deadlines, growth, disasters, depletion, and scarcity.
- **Outcomes:** score, objective progress, victory, loss, warnings, and visible risk.
- **Agency:** explicit AI actions, limits, budgets, cooldowns, and observations.

That grammar can express one or two simple showcase games first:

- Mini-Metro-like: places + links + passenger demand + vehicle dispatch + overflow loss.
- shapez-like: places + links + resources + recipes + throughput goals + order deadlines.
- Factorio-like without a spaceman: autonomous construction, extraction, power, belts, inserters, assemblers, research, pollution/pressure, defense, and logistics operated by one or more agents rather than by an avatar.
- Azure-datacenter-like: capacity planning, workload placement, networking, power/cooling, SLOs, incidents, maintenance, quota, cost, and carbon/water pressure managed by specialized operations agents.
- Hybrid: emergency logistics, power-grid balancing, packet routing, garden pollination, cargo ports.

## Unified platform grammar

The brainstorms should converge on one schema grammar, not separate engines for Metro, factory, Azure datacenter, data platform, and HPC. The domain names change, but the game language should stay stable.

### 1. Places

Places are where capacity, state, inventory, work, and failure live.

Examples:

- Metro: station, interchange, depot.
- Factory: extractor, processor, buffer, sink.
- Azure datacenter: region, availability zone, hall, rack, server, cooling loop.
- Data platform: Kusto cluster, Fabric pipeline, lakehouse, report workspace.
- HPC: scheduler, compute pool, RDMA fabric zone, Lustre/NetApp storage pool.

Core fields:

```jsonc
{
  "id": "gpu-dedicated-nd",
  "role": "compute_pool",
  "pos": [420, 180],
  "capacity": { "gpu": 512, "power_kw": 2400, "query_slots": 0 },
  "state": "normal",
  "failure_domains": ["az1", "power-feed-a", "rdma-fabric-1"],
  "accepts": ["gpu_training_job", "gpu_inference_job"],
  "produces": ["gpu_telemetry", "job_completion_event"]
}
```

### 2. Links

Links connect places and constrain movement or dependency flow.

Examples:

- Metro: track segment.
- Factory: belt, pipe, rail, power line.
- Datacenter: network link, power feed, cooling line.
- Data platform: lineage edge, ingestion stream, report dependency.
- HPC: RDMA fabric, storage mount, data transfer path.

Core fields:

```jsonc
{
  "id": "rdma-fabric-a",
  "type": "rdma_network",
  "from": "mpi-hb-rdma",
  "to": "lustre-scratch",
  "direction": "bidirectional",
  "capacity": { "gbps": 800 },
  "latency_ticks": 1,
  "compatibility": ["mpi_job", "checkpoint_data"],
  "failure_mode": "congested"
}
```

### 3. Things

Things are the units that move, queue, transform, decay, or satisfy demand. They must be typed; generic resources are not enough.

Examples:

- Metro: passenger, train, emergency request.
- Factory: ore, plate, intermediate product.
- Typed data factory: Mycroft event, Infinity Band signal, GPU heartbeat, fault event, uptime fact.
- HPC: job, checkpoint, dataset, scheduler event, telemetry record.

Core fields:

```jsonc
{
  "id": "gpu_fault_event",
  "kind": "raw_signal",
  "tags": ["gpu", "fault", "telemetry"],
  "schema_version": 3,
  "freshness_budget_ticks": 120,
  "quality": { "max_drop_percent": 0.1 }
}
```

### 4. Transforms

Transforms consume things, time, capacity, and sometimes power/cost to produce higher-order things.

Examples:

- Factory: recipe, assembler, smelter.
- Data platform: normalization job, materialized view, Lens job, Power BI refresh.
- HPC: job execution, checkpoint write, telemetry aggregation.
- Datacenter: autoscale action, workload migration, repair workflow.

Core fields:

```jsonc
{
  "id": "build-gpu-uptime-facts",
  "type": "materialized_view",
  "inputs": ["normalized_gpu_heartbeat", "cluster_inventory"],
  "outputs": ["gpu_uptime_fact"],
  "runs_on": "kusto-gpu-health",
  "cadence_ticks": 120,
  "deadline_ticks": 60,
  "capacity_cost": { "query_slots": 4, "ingest_mb": 20 }
}
```

### 5. Demand

Demand is what the world asks for. It creates the "why" behind movement and transformation.

Examples:

- Deliver passengers before stations overflow.
- Produce dashboard facts before freshness SLO expires.
- Complete GPU training jobs before deadline.
- Keep report queries under latency budget during dashboard storm.

Core fields:

```jsonc
{
  "id": "leadership-gpu-health-refresh",
  "kind": "report_demand",
  "requires": ["gpu_uptime_fact", "gpu_fault_fact"],
  "target": "kusto-gpu-health-dashboard",
  "spawn_schedule": { "at_tick": 2400, "multiplier": 10 },
  "deadline_ticks": 300,
  "priority": "critical",
  "penalty": { "score": -500, "incident": "sev2" }
}
```

### 6. Pressure

Pressure mutates conditions over time. This is what stops the simulation from being a flat screensaver.

Examples:

- Station demand growth.
- Resource depletion.
- Schema drift.
- Kusto dashboard storm.
- Spot eviction wave.
- Cooling degradation.
- RDMA placement constraint.
- Storage metadata storm.

Core fields:

```jsonc
{
  "type": "spot_eviction_wave",
  "at_tick": 3600,
  "target": "gpu-spot-nd",
  "eviction_percent": 35,
  "duration_ticks": 900
}
```

### 7. Outcomes

Outcomes make the game legible: score, victory, loss, warnings, and current risk.

Examples:

- `keep_reports_fresh`
- `complete_jobs_before_deadline`
- `maintain_gpu_utilization`
- `avoid_storage_collapse`
- `stay_under_cost_budget`
- `preserve_data_quality`
- `survive_until`

Core fields:

```jsonc
{
  "objectives": [
    { "type": "complete_jobs_before_deadline", "priority": "critical", "max_missed": 0 },
    { "type": "keep_reports_fresh", "targets": ["kusto-gpu-health-dashboard"], "max_stale_ticks": 300 },
    { "type": "cost_budget", "max_cost": 25000 }
  ],
  "failure_conditions": [
    { "type": "critical_dashboard_stale", "target": "kusto-gpu-health-dashboard", "grace_ticks": 120 },
    { "type": "scheduler_stalled", "max_stalled_ticks": 300 }
  ]
}
```

### 8. Agency

Agency declares what AI agents are allowed to see and do. This prevents omnipotent magic fixes.

Examples:

- Scheduler operator can prioritize jobs but cannot scale Kusto.
- Kusto operator can throttle dashboards but cannot move GPU jobs.
- Incident commander can override priorities but consumes trust/risk budget.
- Cost governor can block expensive actions unless incident mode is active.

Core fields:

```jsonc
{
  "agents": [
    {
      "id": "scheduler-operator",
      "role": "scheduler_operator",
      "interval_ticks": 60,
      "observation_scope": ["jobs", "queues", "compute_pools", "storage_pressure"],
      "allowed_actions": ["set_job_priority", "preempt_job", "retry_job", "drain_pool"],
      "objective_weights": { "deadline": 0.5, "utilization": 0.3, "cost": 0.2 }
    }
  ]
}
```

### 9. Observability

Observability is both a player input and a game output. Agents act from metrics, reports, traces, dashboards, and alerts; the viewer also needs those surfaces to understand drama.

Examples:

- Metro: station load, route congestion, overload countdown.
- Factory: blocked belts, starved machines, buffer trends.
- Data/HPC: GPU utilization, Kusto freshness, query queue, job failures, RDMA health, storage throughput, cost burn.

Core fields:

```jsonc
{
  "observability": {
    "dashboards": ["kusto-gpu-health-dashboard", "powerbi-gpu-uptime"],
    "metrics": ["gpu_utilization", "query_queue_depth", "storage_gbps", "job_deadline_risk"],
    "alerts": [
      { "id": "gpu-dashboard-stale", "when": "freshness > 300", "severity": "critical" }
    ]
  }
}
```

### 10. Narrative / replay markers

View-only AI games need visible arcs. JSON should let authors define which moments deserve emphasis in replay and DecisionTimeline.

Examples:

- First overload.
- First successful reroute.
- Dashboard went stale.
- Critical job recovered after eviction.
- Schema drift detected and fixed.

Core fields:

```jsonc
{
  "milestones": [
    { "id": "first-gpu-fault-storm", "trigger": "pressure.gpu_fault_storm.started", "label": "GPU fault storm begins" },
    { "id": "dashboard-recovered", "trigger": "dashboard.kusto-gpu-health-dashboard.fresh", "label": "Health dashboard recovered" }
  ]
}
```

### Design implication

The engine should not know about "Azure" or "Factorio" as special cases. It should know:

- places with capacity/state,
- links with flow/constraints,
- typed things,
- transforms,
- demand,
- pressure,
- outcomes,
- agents,
- observations,
- milestones.

Everything else is authored content and theme.

## Draft unified JSON schema shape

This is an author-facing draft, not an implementation contract yet. The goal is to make the shape concrete enough to reason about validation, engine state, protocol snapshots, and agent tools.

### Top-level scene shape

```jsonc
{
  "schema_version": 3,
  "name": "gpu-launch-week",
  "theme": {
    "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
    "background_index": 0,
    "font": "system-ui"
  },
  "catalog": {
    "slug": "gpu-launch-week",
    "title": "GPU Launch Week",
    "description": "Agents keep GPU jobs and health dashboards stable through HPC and data-platform pressure."
  },

  "places": [],
  "links": [],
  "things": [],
  "transforms": [],
  "demand": [],
  "pressure": [],
  "objectives": [],
  "failure_conditions": [],
  "agents": [],
  "observability": {},
  "milestones": []
}
```

### `places[]`

Places replace the overloaded current `pieces.nodes[]` concept for game-bearing worlds. A place is still renderable like a node, but now it has stateful semantics.

```jsonc
{
  "id": "kusto-gpu-health",
  "role": "kusto_cluster",
  "pos": [720, 260],
  "shape": "hexagon",
  "color": 4,
  "capacity": {
    "ingest_mb_per_tick": 200,
    "query_slots": 24,
    "cache_gb": 1024
  },
  "storage": {
    "gpu_uptime_fact": { "capacity": 100000, "initial": 0 },
    "gpu_fault_fact": { "capacity": 100000, "initial": 0 }
  },
  "accepts": ["gpu_uptime_fact", "gpu_fault_fact", "dashboard_query"],
  "produces": ["dashboard_result", "query_latency_metric"],
  "failure_domains": ["eastus", "az1", "data-platform"],
  "operating_states": {
    "strained": { "when": "query_slots.used_percent >= 80" },
    "overloaded": { "when": "query_slots.used_percent >= 95", "grace_ticks": 120 },
    "failed": { "when": "overloaded_ticks > 600" }
  }
}
```

Validation notes:

- `id` follows existing stable id rules.
- `role` is an open string with engine-known behavior only when paired with supported transform/action types.
- `capacity` keys are typed quantities but should be bounded and finite.
- `storage` is optional; absent means the place does not buffer things.
- `operating_states` should be declarative and limited to supported predicates, not arbitrary expressions in implementation.

### `links[]`

Links replace the overloaded current `pieces.paths[]` concept for game-bearing worlds.

```jsonc
{
  "id": "telemetry-to-normalizer",
  "type": "data_stream",
  "from": "mycroft-gpu-heartbeats",
  "to": "normalize-heartbeats",
  "direction": "forward",
  "capacity": { "events_per_tick": 120 },
  "travel_ticks": 1,
  "compatibility": ["gpu_heartbeat"],
  "queue_capacity": 1000,
  "backpressure": "block_upstream",
  "render": { "style": "flow", "color": 3 }
}
```

Validation notes:

- `from` and `to` reference existing places.
- `direction` is `forward`, `bidirectional`, or later omitted for dependency-only links.
- `compatibility` references `things[].id` or thing tags.
- Backpressure policies should be explicit: `block_upstream`, `drop_low_priority`, `spill_to_buffer`, or `degrade_quality`.

### `things[]`

Things are typed units. They can be discrete, batched, metric-like, data-like, physical, or abstract.

```jsonc
{
  "id": "gpu_heartbeat",
  "kind": "raw_signal",
  "tags": ["telemetry", "gpu", "host"],
  "schema_version": 12,
  "freshness_budget_ticks": 120,
  "quality_contract": {
    "max_drop_percent": 0.1,
    "max_late_ticks": 60,
    "required_fields": ["gpu_id", "cluster_id", "timestamp", "state"]
  },
  "render": { "glyph": "pulse", "color": 5 }
}
```

Validation notes:

- `kind` is author-facing; behavior comes from transforms/demand/objectives.
- `schema_version` is optional for non-data things.
- Quality contracts are optional but become game-bearing when referenced by objectives/failure conditions.

### `transforms[]`

Transforms are deterministic work rules. They are the shared form for recipes, data jobs, report refreshes, job execution, autoscale workflows, and repair workflows.

```jsonc
{
  "id": "build-gpu-uptime-facts",
  "type": "materialized_view",
  "runs_on": "kusto-gpu-health",
  "inputs": [
    { "thing": "normalized_gpu_heartbeat", "amount": 100 },
    { "thing": "cluster_inventory", "amount": 1 }
  ],
  "outputs": [
    { "thing": "gpu_uptime_fact", "amount": 1 }
  ],
  "cadence_ticks": 120,
  "duration_ticks": 40,
  "deadline_ticks": 60,
  "capacity_cost": { "query_slots": 4 },
  "failure_policy": "retry_then_warn",
  "max_attempts": 2
}
```

Validation notes:

- `runs_on` references a place.
- Inputs/outputs reference things.
- `duration_ticks`, `cadence_ticks`, and `deadline_ticks` are bounded positive integers.
- Failure policies should map to typed warnings/faults.

### `demand[]`

Demand declares requests the world generates.

```jsonc
{
  "id": "exec-dashboard-refresh",
  "kind": "report_refresh",
  "target": "kusto-gpu-health-dashboard",
  "requires": ["gpu_uptime_fact", "gpu_fault_fact"],
  "spawn_schedule": {
    "type": "fixed",
    "every_ticks": 300,
    "start_tick": 600
  },
  "deadline_ticks": 120,
  "priority": "critical",
  "value": 100,
  "penalty": { "score": -500, "warning": "critical_report_stale" }
}
```

Validation notes:

- Demand can target a place, transform, report, objective, or virtual sink.
- Schedules should be deterministic: fixed, wave, scripted, or seeded with explicit seed.
- Penalties should be bounded and typed.

### `pressure[]`

Pressure creates drama by mutating load, state, availability, or constraints.

```jsonc
{
  "id": "gpu-fault-storm",
  "type": "source_multiplier",
  "at_tick": 1500,
  "duration_ticks": 600,
  "target": "mycroft-gpu-heartbeats",
  "thing": "gpu_heartbeat",
  "multiplier": 4.0
}
```

Other supported pressure candidates:

- `schema_drift`
- `dashboard_storm`
- `spot_eviction_wave`
- `storage_metadata_storm`
- `cooling_degradation`
- `path_outage`
- `demand_growth`
- `quota_reduction`

Validation notes:

- Every pressure event has stable `id`, bounded time fields, and an explicit target.
- No hidden random event without seed/logging.

### `objectives[]` and `failure_conditions[]`

Objectives describe success/progress. Failure conditions describe death spirals or terminal loss.

```jsonc
{
  "objectives": [
    {
      "id": "fresh-gpu-dashboard",
      "type": "keep_fresh",
      "target": "kusto-gpu-health-dashboard",
      "max_stale_ticks": 300,
      "weight": 5
    },
    {
      "id": "critical-jobs",
      "type": "complete_jobs_before_deadline",
      "filter": { "priority": "critical" },
      "max_missed": 0,
      "weight": 5
    },
    {
      "id": "cost-budget",
      "type": "cost_budget",
      "max_cost": 25000,
      "weight": 2
    }
  ],
  "failure_conditions": [
    {
      "id": "dashboard-too-stale",
      "type": "stale_target",
      "target": "kusto-gpu-health-dashboard",
      "threshold_ticks": 600,
      "grace_ticks": 120
    },
    {
      "id": "scheduler-stalled",
      "type": "place_state",
      "target": "slurm-prod",
      "state": "failed",
      "grace_ticks": 0
    }
  ]
}
```

Validation notes:

- Objectives are nonterminal by default.
- Failure conditions can fault/end the run depending on scene policy.
- Objective/failure target references must resolve.

### `agents[]`

Agents define role, permission, observation, and cadence.

```jsonc
{
  "id": "kusto-operator",
  "kind": "llm",
  "role": "observability_operator",
  "interval_ticks": 120,
  "observation_scope": [
    "kusto-gpu-health",
    "kusto-gpu-health-dashboard",
    "query_queue_depth",
    "dashboard_freshness",
    "ingestion_lag"
  ],
  "allowed_actions": [
    "scale_place_capacity",
    "throttle_demand",
    "prioritize_transform",
    "warm_cache",
    "pause_report_refresh"
  ],
  "budgets": {
    "max_cost_per_decision": 1000,
    "cooldown_ticks": 60
  },
  "objective_weights": {
    "freshness": 0.45,
    "correctness": 0.35,
    "cost": 0.20
  }
}
```

Validation notes:

- `kind: "llm"` remains feature-gated/default-off for live model use.
- Built-in/mock agents should use the same role/action contract.
- Invalid actions always produce typed warnings.

### `observability`

Observability defines the dashboards, metrics, and alerts that both viewers and agents can see.

```jsonc
{
  "observability": {
    "metrics": [
      { "id": "gpu_utilization", "source": "gpu-dedicated-nd", "window_ticks": 300 },
      { "id": "query_queue_depth", "source": "kusto-gpu-health", "window_ticks": 60 },
      { "id": "dashboard_freshness", "source": "kusto-gpu-health-dashboard" }
    ],
    "dashboards": [
      {
        "id": "kusto-gpu-health-dashboard",
        "type": "kusto_dashboard",
        "depends_on": ["gpu_uptime_fact", "gpu_fault_fact"],
        "freshness_slo_ticks": 300,
        "query_cost": { "query_slots": 3 }
      }
    ],
    "alerts": [
      {
        "id": "gpu-dashboard-stale",
        "when": { "metric": "dashboard_freshness", "gt": 300 },
        "severity": "critical"
      }
    ]
  }
}
```

Validation notes:

- Observability should be derived from engine state, not independently mutating state.
- Dashboards can be consumers/demand targets when referenced by objectives.

### `milestones[]`

Milestones make the replay and viewer experience understandable.

```jsonc
{
  "milestones": [
    {
      "id": "fault-storm-started",
      "trigger": { "pressure": "gpu-fault-storm", "event": "started" },
      "label": "GPU fault storm begins",
      "camera_focus": ["mycroft-gpu-heartbeats", "kusto-gpu-health"]
    },
    {
      "id": "dashboard-recovered",
      "trigger": { "metric": "dashboard_freshness", "lte": 120 },
      "label": "GPU health dashboard recovered",
      "highlight": "kusto-gpu-health-dashboard"
    }
  ]
}
```

Validation notes:

- Milestones should not affect simulation outcome unless separately referenced by objectives.
- Labels are user-facing text and must be rendered safely.

### Compatibility with existing schema

- Existing `pieces.nodes`, `pieces.paths`, and `pieces.movers` can remain as v1/v2 visual/motion scenes.
- New game-bearing scenes should use schema v3-style `places`, `links`, `things`, and `transforms`.
- A migration shim can map simple `pieces.nodes` to places and `pieces.paths` to links for read-only compatibility, but it should not invent capacities/objectives silently.
- Existing `resources/producers/consumers` can become a subset of `things/transforms`, but global inventory should not be the long-term primary model for strategy-heavy scenes.

### Minimum implementation slice from this schema

The smallest useful implementation does not need every field above. It needs:

1. `places` with role, pos, capacity, storage, operating state, and nested semantic structs on existing `Node`/`World` types.
2. `links` with from/to, capacity, compatibility, explicit queue/backpressure behavior, and nested semantic structs on existing `Path`/`World` types.
3. `things` with ids/tags, typed inventories, freshness/quality contracts, and bounded counts.
4. `transforms` with inputs/outputs/duration/cadence/deadline and typed starved/blocked/late states.
5. `demand` with deadline/priority/value/penalty and visible lifecycle.
6. `objectives` + `failure_conditions` + first-class `GameOutcome`.
7. `agents` with target-scoped allowed actions, budgets, cooldowns, target-version preconditions, and structured rejection reasons.
8. `observability.metrics` with explicit `ok/no_data/stale/degraded/invalid` states for HUD, replay, and agent observations.
9. v3-specific `LoadError`, `Warning`, and `Fault` variants so invalid schema, silent starvation, stale actions, dashboard staleness, and terminal game loss are all named.

That is enough to build the GPU health dashboard showcase without hard-coding Azure as a special case.

## Next planning bundle: include all four tracks

The next planning work should include all of the following together, because each one pressure-tests the others.

### 1. Concrete GPU launch-week scene JSON

Draft a full example scene using the unified schema. It should include:

- Places:
    - Mycroft GPU heartbeat source.
    - Infinity Band cluster signal source.
    - Normalizer / deduper.
    - Kusto GPU health cluster.
    - Fabric/Lens analysis pipeline.
    - Slurm/CycleCloud-like scheduler.
    - Dedicated GPU pool.
    - Spot GPU pool.
    - RDMA MPI pool.
    - Lustre/NetApp-style storage.
    - Kusto GPU health dashboard.
    - Power BI uptime report.
- Links:
    - Telemetry streams.
    - Data lineage dependencies.
    - Scheduler-to-compute assignment paths.
    - Checkpoint-to-storage path.
    - Dashboard query path.
- Things:
    - `gpu_heartbeat`
    - `infinity_band_signal`
    - `gpu_fault_event`
    - `cluster_inventory`
    - `deployment_event`
    - `repair_action`
    - `gpu_uptime_fact`
    - `gpu_fault_fact`
    - `scheduler_queue_fact`
    - `dashboard_query`
    - `gpu_training_job`
    - `mpi_simulation_job`
    - `checkpoint_data`
    - Optional/follow-up: `research_hypothesis`, `experiment_candidate`, `metric_result`, `crash_report`.
- Transforms:
    - Normalize heartbeats.
    - Deduplicate faults.
    - Build uptime facts.
    - Build fault facts.
    - Refresh Kusto dashboard.
    - Refresh Power BI report.
    - Execute GPU training jobs.
    - Execute MPI simulation job.
    - Write checkpoints.
    - Optional/follow-up: run bounded autoresearch experiments against a trusted simulated evaluator.
- Pressure:
    - GPU job surge.
    - Fault storm.
    - Schema drift.
    - Dashboard storm.
    - Spot eviction wave.
    - Storage metadata storm.
    - RDMA placement constraint.
- Objectives and failure:
    - Keep dashboard fresh and correct.
    - Complete critical jobs before deadline.
    - Maintain GPU utilization.
    - Stay under cost budget.
    - Preserve data quality.
    - Avoid storage collapse and scheduler stall.
- Agents:
    - Incident commander.
    - Scheduler operator.
    - Capacity planner.
    - Storage operator.
    - Fabric/network operator.
    - Kusto/observability operator.
    - Data quality guardian.
    - Cost governor.

### 2. Validate and prune the minimum implementation slice

The first code slice should not implement every brainstormed mechanic. It should implement the least schema that can make the GPU launch-week scene playable:

- Place capacities and storage.
- Link queues/backpressure.
- Thing definitions and typed inventories.
- Deterministic transforms.
- Demand/deadlines.
- Pressure events.
- Objective/failure evaluation.
- Agent observations and a small action set.
- Observability metrics for HUD/replay.

Candidate deferrals:

- Arbitrary predicate language in `operating_states`.
- Full expression engine.
- Bidirectional path authoring sugar.
- Research/unlock tree.
- Complex governance/privacy mechanics.
- Hidden/revealed map information.
- Multi-region disaster recovery.
- Real Azure API integration.

### 3. Map all showcase ideas onto the grammar

Use the same grammar to prove breadth:

| Showcase | Places | Links | Things | Transforms | Pressure | Outcome |
| --- | --- | --- | --- | --- | --- | --- |
| Mini-Metro-like | stations, depots | tracks | passengers, trains | board/unboard/dispatch | demand growth, new station | survive overflow |
| Data factory | sources, Kusto, Fabric, reports | streams, lineage | telemetry, facts, queries | normalize, aggregate, refresh | schema drift, dashboard storm | fresh correct dashboards |
| HPC/GPU | scheduler, pools, storage | RDMA, checkpoint paths | jobs, checkpoints, telemetry | execute job, checkpoint, aggregate | spot eviction, quota, storage storm | complete jobs under budget |
| Datacenter | racks, zones, cooling, power | network, power, cooling | workloads, requests, replicas | place, migrate, repair | cooling loss, traffic surge | maintain SLO |

This mapping should identify reusable primitives and expose one-off mechanics that need pruning.

### 4. Prepare implementation plan for code changes

Once the schema and showcase are concrete, implementation should be staged:

1. Add schema v3 raw structs and validation for the minimum slice.
2. Add engine state for places, links, things, inventories/queues, transforms, demand, objectives, pressure, and run outcome.
3. Add deterministic systems in stable order:

pressure schedule,demand spawn,link movement/backpressure,transform execution,objective/failure evaluation,observability metric derivation,agent action application.

4. Extend protocol static/snapshot/event payloads for place/link state, thing counts, objective progress, warnings/failures, and metrics.
5. Extend frontend renderer/HUD to make pressure visible.
6. Extend agent observations and tool specs with a narrow action set.
7. Create the GPU launch-week scene.
8. Add determinism, loader, systems, protocol, frontend, and world-quality tests.

The implementation plan should stay vertical: first make one scene visibly winnable/losable, then generalize.

## Factorio-style inspiration without a spaceman

Factorio is a strong inspiration because almost all of its drama comes from rules and systems, not from the player character. If simetro removes the spaceman, the playable object becomes the factory/network itself. The AI agent or agents are not "walking around"; they are planners, builders, dispatchers, and operators acting through explicit tools and constraints.

Important Factorio-like mechanics to capture as JSON language:

1. Extraction nodes.

Resource patches become nodes or regions with finite quantity, extraction rate, richness, depletion, and required extractor type.The interesting question is not "does ore exist?" but "can the AI reach it, power it, and route its output before the current patch runs dry?"

2. Belts and directional flow.

Paths become more than edges: they are belts, pipes, cables, rails, roads, or channels with direction, lane count, throughput, item filters, and occupancy.Direction matters strategically because reversing or braiding flow can fix or create bottlenecks.

3. Inserters / transfer rules.

Movement between node inventories and path flow should be explicit.A transfer can have reach, rate, filter, power draw, cooldown, and source/target orientation.This prevents "magic teleport inventory" and gives the AI small tactical levers.

4. Machines and recipes.

Processor nodes run recipes with inputs, outputs, craft time, energy draw, module slots, and byproducts.Recipe chains create legible goals: iron ore -> plate -> gear -> science -> research -> unlock.

5. Power network.

Machines and movers need power; generators provide it; poles/cables transmit it with area/range/capacity.Brownouts should degrade throughput before total collapse, creating recoverable drama.

6. Build graph and construction queue.

The agent should not instantly reshape the world for free.JSON can define buildable entities, costs, placement constraints, construction time, prerequisites, and a queue.Viewer drama comes from watching the AI choose what to build next under scarcity.

7. Research / unlock tree.

Objectives can unlock new tools, recipes, capacities, or automation policies.Research gives long-run direction: survive the early factory, automate science, unlock trains/logistics, reach a launch-equivalent objective.

8. Pollution / external pressure.

A factory can emit heat, pollution, waste, noise, instability, or debt.Pressure can trigger attacks, failures, regulations, demand spikes, or environmental penalties.This is the "keep dying" loop for Factorio-like scenes: growth solves one problem while creating another.

9. Logistics layers.

Add tiers of transport: belts for local throughput, pipes for fluids, bots for flexible but energy-limited movement, trains for long-distance bulk, power lines for energy.JSON can model these as path/mover/link classes with different costs and constraints rather than hard-coded genres.

10. Factory health metrics.

The HUD should expose throughput, backlog, power satisfaction, idle machines, starved machines, blocked outputs, pollution pressure, research progress, and objective progress.These are the "scoreboard" that makes the AI's strategy readable.

11. Multi-agent factory roles.

One agent could own macro planning, another belt routing, another power, another defense/pressure, another production balancing.JSON declares responsibilities, permissions, observation scope, and objective weights.This supports the fantasy of watching an AI operations team run a living factory.

12. Victory and death arcs.

Win conditions can be launch-equivalent: complete a megaproject, sustain target science throughput, deliver a final complex product, stabilize a city/factory under peak demand.Loss conditions can be blackout, critical shortage, pollution-triggered collapse, missed contracts, overrun defenses, or unrecoverable debt.

### Factorio-to-simetro translation

The shared grammar still works if the names are generalized:

- Factorio **assemblers/refineries/furnaces** are `processor` nodes running `recipes`.
- Factorio **belts/pipes/rails/power lines** are constrained `paths` or `links` with flow semantics.
- Factorio **items/fluids/power/science** are `things` with tags and compatibility.
- Factorio **inserters/pumps/splitters** are `transfer` or `routing` rules.
- Factorio **pollution/biters/resource depletion** are `pressure` systems.
- Factorio **research/rocket launch** are `objectives`, `milestones`, and `unlocks`.

The big design point: avoid a hidden omnipotent god-agent. A factory AI is interesting only if JSON gives it limited tools, costs, delays, prerequisites, and consequences.

## Factory flow primitives: belts, inserters, splitters, buffers

These are the core Factorio-like flow atoms to brainstorm before narrowing. They should be expressible in generic JSON language, not hard-coded as one game mode.

1. Belts as link flow.

A belt is a directed `link` with `lanes`, `speed`, `capacity_per_lane`, `accepted_things`, and current occupancy.Items advance deterministically by tick; if the next slot is occupied, upstream flow blocks.Interesting JSON fields: `lanes`, `slot_count`, `ticks_per_slot`, `side_loading`, `lane_policy`, `backpressure`.

2. Belt lanes and side loading.

Two lanes on the same belt can carry different item mixes.Side loading is a rule, not a visual accident: it determines which lane receives inserted items.This creates subtle AI-routing strategy without requiring a spaceman.

3. Splitters as routing policy.

A splitter consumes one or more input links and emits onto one or more output links.JSON can declare `policy: "alternate" | "balance" | "priority_left" | "priority_right" | "filter" | "agent_controlled"`.Splitters create visible strategic choices: balance production, prioritize critical lines, or isolate scarce items.

4. Mergers and junctions.

Merging should have deterministic tie-breaking and fairness rules.Example fields: `merge_policy`, `priority_input`, `starvation_guard_ticks`.This prevents hidden nondeterminism and makes traffic jams explainable.

5. Inserters as explicit transfers.

Inserters move things between inventories, machines, belts, buffers, and paths.Example fields: `from`, `to`, `rate`, `hand_size`, `filter`, `cooldown_ticks`, `power_draw`, `enabled_when`.Inserters are agent-relevant because they are a compact way to tune throughput and priority.

6. Filters and item routing.

Flow components can accept only certain thing tags: `ore`, `plate`, `science_red`, `fluid`, `cold`, `urgent`.Filters can be static, unlocked, or agent-set.This lets one world use the same mechanics for factory items, data packets, hospital supplies, or cargo.

7. Buffers as pressure absorbers.

Buffers store things with capacity, input/output rates, and optional spoilage or priority.They make the system resilient, but too much buffering can hide downstream failure.HUD should distinguish healthy buffer, starving buffer, and blocked-full buffer.

8. Backpressure as a first-class signal.

If output is blocked, machines pause, inserters stall, belts fill, upstream extractors clog.JSON should allow backpressure to be observed by the agent and rendered as path/node strain.This is where factory drama becomes visible: the system is alive because congestion propagates.

9. Throughput contracts.

A line can be required to sustain N items per window, not just total production.Example objective: `deliver 60 gears per 600 ticks for 5 consecutive windows`.This is more dramatic than "eventually produce 300 gears" because stalls matter.

10. Flow diagnostics.

The engine can compute derived metrics: item age, average throughput, blocked ticks, idle ticks, starvation ticks, buffer fill trend.These should be visible to both the AI and the viewer.The AI should be able to say "line A is starved by iron plates" and then act.

11. Agent actions for flow.

Candidate actions: `place_link`, `remove_link`, `set_splitter_policy`, `set_filter`, `place_transfer`, `upgrade_link`, `toggle_machine`, `reserve_buffer`, `prioritize_output`.Each should cost time/resources and be scene-permission-gated.Invalid actions should produce typed warnings, not silent no-ops.

12. Minimal flow showcase.

Start with one extractor, one belt, one processor, one buffer, one sink, and one objective.Then add a second resource, a splitter, and a bottleneck so the AI must rebalance.Loss could be missed throughput windows, critical buffer starvation, or pollution/energy collapse.

## Azure datacenter dynamics brainstorm

Azure-style datacenter operations are a strong fit because they are already a systems game: finite capacity, strict SLOs, layered infrastructure, failures, maintenance, growth pressure, and specialized operators. The "player" can be a team of agents managing a living cloud region rather than an avatar.

### Datacenter-to-simetro translation

- **Regions / availability zones / datacenters / halls / rows / racks / servers** become nested places or grouped nodes.
- **Network links, power feeds, cooling loops, storage fabrics, and service dependencies** become constrained links with capacity, latency, loss, and failure modes.
- **VMs, containers, databases, model deployments, batch jobs, customer requests, replicas, and data flows** become things moving through or occupying places.
- **Schedulers, autoscalers, load balancers, repair systems, deployment rings, and incident mitigations** become transforms or agent actions.
- **SLOs, capacity commitments, latency targets, quota, cost, energy, carbon, water, and reliability** become objectives and pressure systems.

### Core datacenter mechanics

1. Capacity pools and placement.

Nodes can expose typed capacity: `cpu`, `gpu`, `memory`, `storage`, `iops`, `network_egress`, `power_kw`, `cooling_kw`.Workloads have requirements, affinity/anti-affinity, redundancy, region/zone constraints, and priority.The AI must place demand without overcommitting a hidden bottleneck.

2. Workload demand and SLOs.

Demand can be service traffic, batch jobs, AI inference, database replicas, backups, or customer deployments.Each demand unit can have latency, availability, deadline, durability, or throughput targets.Win/loss is legible: serve traffic within SLO, meet deadlines, avoid dropped requests, maintain redundancy.

3. Power as a hard shared constraint.

Racks and rooms consume power; power feeds, UPS, generators, and substations have finite capacity.Brownout states can throttle workloads before failure.AI choices: shed low-priority load, migrate jobs, delay batch, rebalance power domains, start backup generation.

4. Cooling and thermal runaway.

- Compute produces heat; cooling loops remove it with finite capacity and lag.Overheated nodes throttle, fail, or require evacuation.This creates visible drama: a hotspot spreads unless the AI drains workload or changes cooling policy.

Network topology and congestion.

- Links have bandwidth, latency, packet loss, and oversubscription.Cross-zone replication, customer traffic, storage sync, and model serving compete for bandwidth.AI choices: reroute traffic, move replicas closer, throttle replication, prioritize critical services.

Failure domains and redundancy.

- JSON can define zones, racks, power domains, network fabrics, and blast-radius groups.Workloads declare redundancy requirements: `n+1`, `zone_redundant`, `rack_anti_affinity`, `quorum`.The AI can "win" capacity but lose resilience if it packs too tightly.

Maintenance and draining.

- Scheduled maintenance creates deterministic pressure: drain rack, patch hosts, replace cooling unit, upgrade network fabric.Draining consumes spare capacity; if the region is too hot, maintenance becomes risky.Agent drama: defer, drain, migrate, or accept elevated risk.

Incident response.

- Shocks can include rack failure, ToR switch degradation, storage hot partition, cooling leak, power feed loss, noisy neighbor, bad deployment, traffic surge.Incidents should be deterministic and replayable, with warnings, escalation levels, and mitigations.The viewer should see "we are in SEV2; the agents are trying to stabilize."

Quota, reservations, and customer priority.

- Capacity is not just physical; it is promised to tenants or workloads.JSON can model quotas, reserved instances, premium customers, internal jobs, and eviction policies.Tradeoff: honor commitments vs maximize utilization vs preserve emergency headroom.

Cost, carbon, and water pressure.

- Datacenters can optimize beyond uptime: energy cost, carbon intensity by time/region, water use, cooling efficiency.This creates multi-objective strategy: cheap/green placement can conflict with latency or capacity.Great for agent personalities: reliability-first, cost-first, carbon-aware, or balanced.

Deployment rings and change risk.

- Services can roll out through rings; bad versions increase error rate or resource use.The AI can pause, rollback, continue, or isolate.This makes software change a game mechanic, not just hardware logistics.

Observability as a gameplay surface.

- Agents do not need perfect truth. They can observe metrics, alerts, traces, forecasts, and delayed signals.JSON can declare which metrics are available: `cpu`, `p95_latency`, `error_rate`, `temperature`, `queue_depth`, `power_headroom`, `replica_health`.The viewer can watch dashboards light up and agents reason from symptoms.Candidate agents: capacity planner, workload scheduler, network operator, power/cooling operator, incident commander, SRE, cost optimizer, carbon optimizer, deployment manager.JSON should declare each agent's tools, observation scope, cadence, authority, and objective weights.Coordination itself becomes part of the game: one agent's optimization can create another agent's incident.

Multi-agent operations team.

14. Win and loss arcs.

Win: survive a traffic surge, complete maintenance with no SLO breach, host a product launch, sustain GPU service throughput, recover from regional incident, hit cost/carbon target.Lose: SLO breach budget exhausted, quorum lost, power/cooling collapse, cascading network congestion, critical customer eviction, maintenance window missed.

### Example JSON language atoms for datacenters

```jsonc
{
  "places": [
    {
      "id": "rack-a1",
      "role": "rack",
      "region": "eastus",
      "zone": "az1",
      "capacity": { "cpu": 640, "memory_gb": 4096, "gpu": 8, "power_kw": 42, "cooling_kw": 45 },
      "failure_domain": ["row-a", "power-feed-1", "tor-a1"]
    }
  ],
  "links": [
    {
      "id": "tor-a1-spine-1",
      "type": "network",
      "from": "rack-a1",
      "to": "spine-1",
      "capacity": { "gbps": 400 },
      "latency_ticks": 1
    }
  ],
  "workloads": [
    {
      "id": "checkout-api",
      "requirements": { "cpu": 80, "memory_gb": 256 },
      "replicas": 6,
      "placement": { "anti_affinity": "rack", "zone_redundant": true },
      "slo": { "p95_latency_ticks": 4, "availability": "99.95" },
      "priority": "critical"
    }
  ],
  "pressure": [
    { "type": "traffic_surge", "target": "checkout-api", "at_tick": 1800, "multiplier": 2.4 },
    { "type": "cooling_degradation", "target": "row-a", "at_tick": 2400, "cooling_kw_delta": -35 }
  ],
  "objectives": [
    { "type": "survive_until", "tick": 7200 },
    { "type": "slo_error_budget", "max_breaches": 3 },
    { "type": "power_headroom", "min_percent": 8 }
  ]
}
```

### Why this belongs in the same game language

Datacenter worlds reuse the same primitives as Metro/Factorio:

- Capacity-constrained places.
- Directed links with throughput and failure.
- Demand units with deadlines and priority.
- Transform systems that consume resources and produce service.
- Pressure curves and incidents.
- Multi-agent decisions under scarcity.
- Visible win/loss conditions.

The difference is theme and scale, not the underlying game grammar.

## HPC / GPU datacenter Well-Architected research

Source-grounded notes from Microsoft Learn / Azure Architecture Center research:

- Azure Well-Architected Framework pillars: reliability, security, cost optimization, operational excellence, and performance efficiency.
- Azure HPC docs frame HPC as a composition of compute, orchestration, network, and storage resources.
- Azure CycleCloud is the main Azure tool for orchestrating familiar HPC schedulers, provisioning cluster infrastructure, autoscaling based on scheduler load, integrating file systems, and connecting with Azure Monitor and Cost Management.
- CycleCloud supports schedulers including Slurm, PBS Pro, LSF, Grid Engine, and HTCondor.
- Azure RDMA/InfiniBand guidance emphasizes RDMA-capable HB/HC/HX and selected N-series VMs, full fat-tree InfiniBand design, low latency/high bandwidth MPI communication, correct driver/MPI stack, and avoiding VNet overlap with the RDMA `172.16.0.0/16` address space.
- For MPI jobs that require RDMA, VMs must be placed in the same VM scale set or availability set. CycleCloud HB/HC best practices call out `Azure.SingleScaleset = true` for Slurm MPI jobs so autoscaled nodes land in an InfiniBand-compatible placement.
- HB/HC guidance emphasizes SKU fit: HB for memory-bandwidth-heavy workloads such as CFD/weather/finite element analysis; HC for compute-intensive molecular dynamics / implicit finite element workloads.
- HPC storage guidance commonly points at high-performance shared storage such as Azure NetApp Files and Azure Managed Lustre, plus local NVMe/SSD scratch for appropriate jobs.
- HPC landing zone patterns emphasize secure VNets, subnet segmentation for login/scheduler/compute/storage, private endpoints, NSGs/firewalls, monitoring, scheduler/controller resiliency, quota management, autoscale, IaC, and runbooks.

### WAF pillars translated into HPC game mechanics

1. Reliability.

Game primitive: job completion reliability, scheduler health, retry/reschedule policy, checkpointing, head-node/controller resilience, storage durability, failure-domain spread.Failure pressure: controller down, job lost after node eviction, storage outage, zone/rack failure, checkpoint missing, deadline missed.Agent choices: reschedule, drain nodes, checkpoint, replicate outputs, move job class, defer maintenance, preserve spare capacity.

2. Security.

Game primitive: identity, RBAC, tenant isolation, network segmentation, private endpoints, secret handling, audit logs, data sensitivity labels.Failure pressure: unauthorized job submission, data exfiltration path, over-broad scheduler permissions, exposed login node, secret leak.Agent choices: tighten policy, isolate subnet, rotate secret, quarantine workload, block egress, grant least privilege.

3. Cost optimization.

Game primitive: quota, budget, reserved capacity, spot/low-priority nodes, idle-node deallocation, right-sized SKUs, storage tiering.Failure pressure: budget burn, stranded quota, idle expensive GPUs, spot eviction, overprovisioned cache/storage, underutilized reservations.Agent choices: use spot for retryable jobs, scale down idle nodes, move cold outputs to cheap storage, right-size SKU, reserve critical capacity.

4. Operational excellence.

Game primitive: runbooks, alert routing, incident command, deployment templates, scheduler telemetry, job/accounting logs, maintenance windows.Failure pressure: noisy alerts, missing telemetry, stuck drain, failed autoscale, patch window collision, unresolved incident.Agent choices: execute runbook, silence noise, escalate, rollback config, patch after drain, update cluster template.

5. Performance efficiency.

Game primitive: SKU selection, RDMA topology, MPI placement, parallel filesystem throughput, data locality, queue policy, cache/scratch behavior.Failure pressure: MPI job spans incompatible placement, storage bottleneck, cache miss storm, network congestion, wrong GPU/CPU SKU, queue starvation.Agent choices: pack tightly coupled jobs into same scale set, choose HB/HC/ND SKU, move data closer, tune queue priority, allocate Lustre/NetApp throughput, split jobs.

### HPC-specific game objects

1. Scheduler / control plane.

Slurm or CycleCloud-like scheduler node with job queue, partitions, priorities, autoscale rules, accounting DB, and failure state.Interesting state: queued jobs, pending reason, running jobs, failed jobs, preemptible jobs, controller health, autoscale lag.

2. Job types.

MPI simulation, GPU training, inference batch, parameter sweep, data preprocessing, checkpointing, visualization, report generation.Fields: required SKU, node count, walltime, deadline, checkpoint interval, interruptible, data input, output, priority, tenant, cost budget.

3. Compute pools.

HB/HC/HX/ND/NC-like pools with CPU, GPU, memory bandwidth, RDMA support, placement group, quota, price, spot availability, failure rate.Strategic tension: the "fastest" pool may be expensive, quota-limited, spot-volatile, or storage-starved.

4. RDMA / InfiniBand fabric.

Special link layer for tightly coupled jobs.Constraints: same scale set / placement, driver health, MPI compatibility, topology capacity.Failure mode: MPI job runs but scales poorly because it violated placement/fabric assumptions.

5. Parallel storage.

Azure NetApp Files / Managed Lustre / Blob / local scratch equivalents.State: throughput, IOPS, metadata pressure, mount health, cache warmness, cost, data lifecycle.Bottleneck: GPUs idle because the filesystem cannot feed them.

6. Quota and capacity reservations.

Region/SKU quota is a hard limiter, not just a budget.Agents must plan around quota, request capacity, reserve critical nodes, or shift workload.

7. Checkpoints and recoverability.

Long jobs can checkpoint to durable storage.Tradeoff: checkpoint overhead vs lost work after eviction/failure.Great "drama" mechanic: a 12-hour run is almost done when spot nodes start evicting.

8. Autoscale lag.

Nodes do not appear instantly. Provisioning, image readiness, mount setup, and scheduler registration take time.This creates planning pressure: scale before the queue explodes, but avoid expensive idle capacity.

9. Data locality.

Jobs consume huge input datasets and emit outputs.Moving data is itself a scheduled, costly, capacity-constrained flow.The AI can lose by having compute but no data, or data but no nearby compute.

10. Observability / reports.

Kusto/Power BI/Lens dashboards become operational outputs of the HPC world: GPU utilization, queue depth, job failure reasons, fault domains, cost burn, storage throughput, fabric health.This links the HPC datacenter world back to the typed data-factory world.

### HPC / GPU cluster showcase idea

Scenario: "Keep the GPU research cluster productive through a launch week."

Raw pressures:

- Surge of GPU training jobs.
- One critical MPI simulation requiring RDMA placement.
- Storage metadata storm from checkpoint-heavy jobs.
- Spot eviction wave.
- Kusto dashboard storm from leadership watching GPU uptime/fault reports.
- Quota ceiling on the best GPU SKU.

Objectives:

- Complete critical jobs before deadlines.
- Keep GPU utilization above target without exceeding cost budget.
- Maintain Kusto/Power BI health dashboard freshness.
- Avoid more than N failed jobs.
- Preserve RDMA placement requirements for tightly coupled jobs.

Loss conditions:

- Critical job misses deadline.
- GPU health dashboard stale during incident.
- Cost budget exhausted.
- Storage bottleneck causes sustained GPU idle.
- Scheduler/control plane failure blocks queue progress.

Agent roles:

- Scheduler operator: prioritizes jobs, partitions, preemption, retries.
- Capacity planner: manages quota, reservations, spot vs dedicated pools.
- Storage operator: allocates Lustre/NetApp throughput, moves datasets, handles checkpoints.
- Fabric operator: protects RDMA placement and network topology.
- Observability/data operator: keeps Kusto/Power BI/Lens health reports fresh and trustworthy.
- Incident commander: coordinates tradeoffs during eviction/failure storms.

### Include all HPC design tracks

Do not split these into competing directions. The HPC/GPU datacenter game language should include all of the following as one coherent system:

1. Multi-agent operations across HPC and data factory.

The HPC cluster and the data-reporting factory are coupled.Compute agents keep jobs running; data agents keep the operational truth fresh; incident agents coordinate when those goals conflict.Example conflict: scheduler wants to spend Kusto capacity on job telemetry during a fault storm, while dashboard owner needs query slots for executive GPU uptime reports.

2. HPC/GPU cluster showcase win/loss loop.

A concrete scenario should prove the mechanics: job surge, RDMA placement requirement, storage bottleneck, spot eviction, quota ceiling, dashboard storm.Win means critical jobs complete, GPU utilization stays healthy, reports stay fresh, cost remains bounded, and no major SLO/fault visibility breach occurs.Lose means deadline miss, stale health dashboard, storage-induced GPU idle, cost overrun, scheduler/control-plane stall, or incorrect fault reporting.

3. JSON schema for scheduler, jobs, compute pools, and storage.

Add explicit schema concepts for scheduler/control plane, queues/partitions, jobs, compute pools, placement constraints, RDMA fabric, storage tiers, checkpoint policy, quota, and autoscale rules.These should be generic enough to express Slurm/CycleCloud-like behavior without making Azure-specific names mandatory in engine code.

4. Observability dashboards for HPC health and faults.

Kusto/Power BI/Lens-style dashboards are not just UI chrome; they are high-order deliverables produced by the typed data factory.Dashboard freshness, correctness, and query latency are part of win/loss.Observability data should include GPU utilization, queue depth, failed jobs, pending reasons, RDMA/fabric health, storage throughput, checkpoint backlog, cost burn, quota headroom, and fault-domain health.

### Multi-agent operations across HPC + data factory

Candidate agents and tensions:

- **Incident commander**
    - Sees high-level state, chooses incident mode, coordinates priorities, can override lower-priority agents.
    - Tension: every override can hurt cost, freshness, or throughput.
- **Scheduler operator**
    - Controls queues, partitions, priorities, retries, preemption, and job admission.
    - Tension: maximize utilization vs protect critical deadlines and RDMA placement.
- **Capacity planner**
    - Controls reserved vs spot pools, SKU selection, quota allocation, and scale-ahead decisions.
    - Tension: avoid idle expensive GPUs vs avoid being caught short during bursts.
- **Storage operator**
    - Controls Lustre/NetApp throughput, scratch placement, checkpoint cadence, data prefetch, lifecycle/archive.
    - Tension: checkpoint often enough to survive eviction vs avoid saturating storage and idling GPUs.
- **Fabric / network operator**
    - Protects RDMA placement, MPI topology, network partitions, and cross-zone data flow.
    - Tension: pack jobs for low latency vs preserve failure-domain diversity.
- **Kusto / observability operator**
    - Controls ingestion, query slots, materialized views, cache warming, dashboard throttling, and alert latency.
    - Tension: keep reports fresh vs avoid starving ingestion and alert pipelines.
- **Data quality guardian**
    - Watches schema drift, missing dimensions, late data, duplicates, and report correctness.
    - Tension: block bad data and lose freshness, or pass imperfect data and risk wrong dashboards.
- **Cost governor**
    - Enforces budgets, idle deallocation, spot use, storage tiering, and query/job spend.
    - Tension: save money vs preserve reliability and responsiveness.

JSON should declare each agent's:

- `role`
- `objective_weights`
- `allowed_actions`
- `observation_scope`
- `authority`
- `cadence_ticks`
- `escalation_policy`

### HPC showcase loop: "GPU launch week"

Opening state:

- GPU cluster is healthy but near capacity.
- Critical Kusto dashboard tracks GPU uptime and faults.
- Several data pipelines transform Mycroft / Infinity Band / GPU telemetry into uptime and fault facts.
- The scheduler has dedicated and spot GPU pools, an RDMA-capable MPI pool, and quota limits.

Pressure sequence:

1. Training-job surge fills GPU queues.
2. MPI simulation arrives requiring RDMA-compatible placement in one scale-set-like pool.
3. Checkpoint-heavy jobs create parallel storage pressure.
4. Spot eviction wave interrupts retryable jobs.
5. GPU fault storm increases telemetry volume and data quality risk.
6. Leadership opens a dashboard, creating Kusto query pressure.
7. Schema drift in raw telemetry threatens dashboard correctness.

Strategic decisions:

- Pre-scale dedicated nodes or gamble on spot.
- Preempt low-priority jobs or risk critical deadline miss.
- Increase checkpoint interval to protect storage or risk lost work.
- Reserve Kusto query slots for health dashboard or ingest all telemetry first.
- Rebuild materialized view after schema drift or patch report logic.
- Pack RDMA job tightly or preserve failure-domain spread.

Win:

- Critical MPI/training jobs complete on time.
- GPU utilization stays above target.
- GPU uptime/fault dashboard remains fresh and correct.
- Cost remains below budget.
- No uncontrolled queue or storage collapse.

Lose:

- Critical job deadline missed.
- Dashboard stale/incorrect during incident.
- Storage bottleneck idles GPUs for too long.
- Cost budget exhausted.
- RDMA job assigned to invalid topology and performance collapses.
- Scheduler/control-plane fault blocks job progress.

### Schema atoms for HPC scheduler/jobs/compute/storage

```jsonc
{
  "schedulers": [
    {
      "id": "slurm-prod",
      "role": "scheduler",
      "queues": ["critical", "training", "batch", "spot"],
      "capacity": { "queued_jobs": 10000, "dispatches_per_tick": 50 },
      "health": { "controller_redundancy": 2, "accounting_db": "slurmdb-prod" }
    }
  ],
  "compute_pools": [
    {
      "id": "gpu-dedicated-nd",
      "sku_class": "gpu",
      "capacity": { "nodes": 64, "gpu_per_node": 8, "memory_gb": 2048 },
      "quota": { "max_nodes": 80 },
      "cost_per_tick": 50,
      "preemptible": false
    },
    {
      "id": "mpi-hb-rdma",
      "sku_class": "hpc_cpu",
      "capacity": { "nodes": 96, "cores_per_node": 120 },
      "network": { "rdma": true, "placement_group": "single_scale_set" },
      "quota": { "max_nodes": 128 }
    }
  ],
  "storage_pools": [
    {
      "id": "lustre-scratch",
      "role": "parallel_scratch",
      "capacity": { "tb": 500, "gbps": 200, "metadata_ops_per_tick": 100000 },
      "cost_per_tick": 20
    }
  ],
  "jobs": [
    {
      "id": "mpi-weather-critical",
      "type": "mpi_simulation",
      "queue": "critical",
      "requires": { "nodes": 64, "rdma": true, "storage": "lustre-scratch" },
      "deadline_tick": 5400,
      "checkpoint": { "every_ticks": 600, "target": "lustre-scratch" },
      "priority": "critical"
    }
  ],
  "dashboards": [
    {
      "id": "kusto-gpu-hpc-health",
      "depends_on": ["gpu_uptime_fact", "gpu_fault_fact", "scheduler_queue_fact"],
      "freshness_slo_ticks": 300,
      "correctness_required": true
    }
  ]
}
```

### Observability dashboard mechanics

Dashboards and reports should consume the same data-factory products that the agents use:

- `gpu_utilization_fact`
- `scheduler_queue_fact`
- `job_failure_reason_fact`
- `rdma_fabric_health_fact`
- `storage_throughput_fact`
- `checkpoint_backlog_fact`
- `quota_headroom_fact`
- `cost_burn_fact`
- `fault_domain_health_fact`

Dashboard-specific pressure:

- Refresh storms consume query slots.
- Materialized views lag behind telemetry.
- Schema drift can make a report fresh but wrong.
- Alert pipelines can be starved by dashboard queries.
- Cost governor may throttle expensive reports.

Viewer drama:

- Green/amber/red dashboard freshness.
- Query queue meter.
- GPU utilization gauge.
- RDMA topology validity indicator.
- Storage bottleneck heatmap.
- Cost burn-down/burn-up.
- Incident timeline tied to DecisionTimeline entries.

### Sources to preserve for future implementation

- Azure Well-Architected Framework overview: `https://learn.microsoft.com/en-us/azure/architecture/framework/`
- Azure HPC documentation: `https://learn.microsoft.com/en-us/azure/high-performance-computing/`
- Azure CycleCloud overview: `https://learn.microsoft.com/en-us/azure/cyclecloud/overview`
- Azure InfiniBand/RDMA setup: `https://learn.microsoft.com/en-us/azure/virtual-machines/setup-infiniband`
- CycleCloud HB/HC best practices: `https://learn.microsoft.com/en-us/azure/cyclecloud/how-to/hb-hc-best-practices`

## Azure data platform dynamics brainstorm

A second Azure-inspired world can sit above the physical datacenter: the data/analytics control plane. Instead of racks and power feeds, the game pieces are telemetry streams, Kusto clusters, Databricks or Fabric jobs, report refreshes, dashboards, alert pipelines, query load, data freshness, and incident investigations. This is still a capacity-and-flow game, but the "things" are data, queries, jobs, reports, alerts, and decisions.

### Data-platform-to-simetro translation

- **Kusto / Azure Data Explorer clusters** become query-and-ingestion processor nodes with hot cache, storage, CPU, memory, concurrency, and ingestion capacity.
- **Event hubs, queues, logs, metrics, traces, and tables** become data streams or buffers moving through directed links.
- **Databricks / Spark / Fabric jobs** become batch transforms with cluster capacity, dependencies, retry policy, cost, and freshness deadlines.
- **Power BI reports, Kusto reports, dashboards, and Lens jobs** become downstream consumers with refresh cadence, query cost, cache behavior, and viewer-facing SLOs.
- **Datadog-like observability** can be modeled as an external monitoring plane: alerts, monitors, dashboards, anomaly detectors, and paging pressure.
- **Analysts, SREs, product teams, executives, and automated agents** become demand sources asking questions of the data platform.

### Core data platform mechanics

1. Ingestion pipelines.

- Data arrives as telemetry, customer events, logs, traces, metrics, billing records, incident signals, or product facts.Streams have volume, schema, priority, freshness budget, retention, and loss tolerance.Failure pressure: ingestion lag, dropped events, schema mismatch, hot partition, bad parser, or late-arriving data.

Kusto clusters as query/ingest factories.

- Kusto nodes can have separate ingestion, query, cache, materialized-view, and storage capacity.Queries compete with ingestion and with each other.AI choices: scale cluster, throttle expensive reports, create materialized views, reroute tenants, adjust retention, warm cache, pause noisy workloads.

Query load and dashboard storms.

- Reports can fan out into many queries; an executive dashboard or incident bridge can trigger synchronized refresh pressure.A single bad KQL query can consume cluster resources and starve critical alerts.This creates visible drama: dashboards turn red, query queues grow, freshness ages, and alert latency rises.

Databricks / Spark / Fabric job DAGs.

- Jobs are transforms with dependencies, input datasets, output datasets, run windows, cluster requirements, retries, and cost.Late upstream data can cascade into stale reports and missed SLAs.AI choices: reprioritize jobs, allocate larger clusters, skip noncritical outputs, retry failed stages, backfill, or accept stale data.

Microsoft Fabric / data mesh semantics.

- Fabric-style worlds can model lakehouses, warehouses, semantic models, notebooks, pipelines, OneLake shortcuts, and report artifacts.The useful mechanic is lineage: a broken source or slow transform propagates through downstream reports.The viewer can watch a dependency graph go from green to amber to red.

Power BI and report freshness.

- Reports have refresh cadence, semantic-model dependencies, query cost, cache TTL, viewer priority, and freshness SLO.Loss can be stale executive report, broken scorecard, missed regulatory dashboard, or degraded customer-facing analytics.AI choices: refresh now, defer, precompute, cache, isolate tenant, or shed low-priority reports.

Lens jobs / analytics lenses.

- Treat "Lens jobs" as recurring analytical perspectives over raw data: aggregations, anomaly scans, health reports, feature extraction, or incident lenses.Each lens has inputs, compute cost, cadence, freshness budget, and consumers.If Lens means a specific internal tool later, map it onto this generic `analysis_job` primitive.

Datadog-style monitors and alerts.

- Monitors consume metrics/logs/traces and emit alerts with severity, dedupe rules, thresholds, and noise.Too much alerting creates operator overload; too little misses failures.AI choices: tune thresholds, silence noisy alerts, escalate, correlate signals, or create temporary monitors during an incident.

Data quality and schema drift.

- Datasets have contracts: schema, completeness, null rate, uniqueness, freshness, range checks.Producers can ship schema drift or bad values.This creates a different kind of failure: the dashboard is green but wrong unless agents catch data quality degradation.

Lineage and blast radius.

- Every report or model should know its upstream dependencies.Incidents can propagate along lineage edges; AI agents can trace the blast radius and prioritize fixes by downstream importance.This is a strong view-only mechanic because dependency graphs are inherently watchable.

Cost and quota.

- Query clusters, Spark jobs, report refreshes, retention, cache warming, and backfills consume budget.Quota prevents the AI from scaling everything forever.Strategy: spend on freshness for critical reports while letting low-priority analytics degrade gracefully.

Privacy, governance, and access policy.

- Data products can have sensitivity labels, retention policies, tenant boundaries, and access controls.Invalid joins or exports can produce policy violations.This gives the AI non-performance constraints: correct, fresh, cheap, and compliant.

Incident investigation as gameplay.

- A data incident can begin as symptoms: dashboard stale, alert missing, Kusto queue spike, job failed, report wrong, customer complaint.Agents inspect lineage, metrics, logs, reports, job history, and recent deployments.Win: isolate root cause and restore freshness/SLO before breach budget expires.Candidate agents: ingestion operator, Kusto cluster manager, job scheduler, report owner, data quality guardian, observability/noise tuner, cost governor, incident commander.Each has different tools and may conflict: cost agent wants to defer jobs; report owner wants fresh dashboards; incident commander wants emergency capacity.

Multi-agent data ops team.

15. Data-world win and loss arcs.

Win: keep critical reports fresh through a launch, recover telemetry after schema drift, survive dashboard storm, complete regulatory close, maintain alert latency during incident.Lose: stale critical dashboard, missed SLA, alert arrives too late, runaway query cost, corrupted report, privacy policy breach, unrecoverable job backlog.

### Example JSON language atoms for data ops

```jsonc
{
  "data_products": [
    {
      "id": "regional-health-facts",
      "type": "table",
      "freshness_slo_ticks": 300,
      "quality_contract": { "max_null_percent": 0.5, "schema_version": 4 },
      "sensitivity": "internal"
    }
  ],
  "processors": [
    {
      "id": "kusto-prod-east",
      "role": "kusto_cluster",
      "capacity": { "ingest_mb_per_tick": 200, "query_slots": 32, "cache_gb": 2048 },
      "cost_per_tick": 12
    },
    {
      "id": "fabric-refresh-01",
      "role": "fabric_pipeline",
      "capacity": { "job_slots": 8 },
      "retry_policy": { "max_attempts": 2, "backoff_ticks": 60 }
    }
  ],
  "jobs": [
    {
      "id": "lens-regional-health",
      "type": "analysis_job",
      "inputs": ["raw-service-telemetry"],
      "outputs": ["regional-health-facts"],
      "cadence_ticks": 180,
      "deadline_ticks": 90,
      "compute": { "job_slots": 2 }
    }
  ],
  "reports": [
    {
      "id": "exec-powerbi-health",
      "type": "power_bi_report",
      "depends_on": ["regional-health-facts"],
      "refresh_cadence_ticks": 300,
      "freshness_slo_ticks": 360,
      "priority": "critical"
    }
  ],
  "pressure": [
    { "type": "dashboard_storm", "at_tick": 1200, "target": "exec-powerbi-health", "refresh_multiplier": 8 },
    { "type": "schema_drift", "at_tick": 1800, "target": "raw-service-telemetry", "new_schema_version": 5 }
  ],
  "objectives": [
    { "type": "keep_reports_fresh", "targets": ["exec-powerbi-health"], "max_stale_ticks": 360 },
    { "type": "cost_budget", "max_cost": 50000 },
    { "type": "data_quality", "max_contract_violations": 1 }
  ]
}
```

### Why this expands the game language

Azure data-platform worlds add mechanics that physical datacenter and factory worlds do not emphasize:

- Freshness as a resource.
- Query concurrency as path/node capacity.
- Lineage as a directed dependency graph.
- Reports as high-stakes consumers.
- Data quality as a failure mode.
- Observability as both input and output.
- Cost/governance as constraints on brute-force scaling.

This gives simetro a strong "AI operations game" direction: agents are not only moving dots; they are keeping truth, dashboards, alerts, and decisions alive under pressure.

## Autoresearch-inspired simulated research environment

`karpathy/autoresearch` adds a useful mental model: autonomous agents are not merely operating a fixed system; they are running a constrained research loop that proposes changes, spends compute, evaluates one trusted metric, and keeps or discards the result. The repository is intentionally tiny:

- `prepare.py` is fixed infrastructure: data prep, tokenizer, dataloader, evaluation, constants. The agent must not modify it.
- `train.py` is the single mutable artifact: model, optimizer, hyperparameters, training loop. The agent edits only this.
- `program.md` is the human-authored "research org code": instructions that define how the autonomous researcher should behave.
- Each experiment runs for a fixed 5-minute budget.
- The trusted metric is `val_bpb`; lower is better.
- Results are logged in `results.tsv` with commit, metric, memory, keep/discard/crash status, and description.
- The loop is autonomous: propose experiment, commit, run, evaluate, keep if better, reset/discard if worse/crashed, repeat.

### Fit with simetro

This is in scope as a **simulated environment pattern**, not as a live code-execution feature. It fits the existing plan because it maps directly onto the unified grammar:

| Autoresearch concept | simetro grammar |
| --- | --- |
| `program.md` | agent policy / operating doctrine artifact |
| `prepare.py` | immutable evaluation harness / trusted transform |
| `train.py` | mutable experiment candidate / research artifact |
| 5-minute training run | bounded transform job consuming compute |
| `val_bpb` | objective metric; lower is better |
| peak VRAM | capacity/cost pressure metric |
| crash/OOM | typed failure outcome |
| git commit | experiment version / lineage milestone |
| keep/discard/reset | decision state machine |
| `results.tsv` | experiment log / replay table / dashboard |

The important import is **discipline**, not literal Python training:

- one mutable artifact,
- one trusted evaluator,
- one fixed time budget,
- one objective metric,
- explicit crash handling,
- complete experiment log,
- autonomous loop with human-authored policy.

### Simulated research-loop mechanics

Autoresearch can become a nested loop inside GPU Launch Week or a follow-up showcase:

```text
agent policy
  │
  ▼
propose experiment
  │
  ▼
allocate GPU window ──capacity/budget/queue pressure──▶ run experiment
  │                                                   │
  │                                                   ├── crash/OOM ──▶ log crash + discard
  │                                                   ├── timeout ───▶ log failure + discard
  │                                                   └── metric ────▶ compare to best
  ▼
keep improvement / discard regression
  │
  ▼
update experiment log + dashboard + next hypothesis
```

State machine:

```text
hypothesis → queued → running → evaluating → kept
                         │          │
                         │          ├── worse_or_equal → discarded
                         │          └── crash/timeout ─▶ failed
                         └── resource_denied ─────────▶ blocked
```

### Why it improves the current plan

Autoresearch contributes a clearer **agent gameplay loop** than generic "multi-agent ops":

- The AI has a legible goal: improve the metric.
- The world has stakes: scarce GPU time, memory pressure, deadline windows, crash risk, and opportunity cost.
- The viewer can understand each step: "the agent tried a change, spent compute, got a result, kept or reverted."
- The result log becomes a natural replay/DecisionTimeline artifact.
- `program.md` suggests a user-authored policy surface that is safer and more focused than arbitrary agent prompts.

### Scope recommendation

This is **not required for the first v3 engine slice**, but it is easy to add to the plan and should influence the agent model immediately.

Recommended placement:

1. Include autoresearch principles in the **agent policy / agency** design.
2. Add experiment-loop primitives as a follow-up scenario once GPU Launch Week has jobs, metrics, dashboards, and outcomes.
3. Do not run real training, mutate repo files, execute arbitrary code, or call external package managers from the simulation engine.
4. If live autoresearch is ever explored, it must be a separate, human-run tool behind explicit feature gates, not part of deterministic CI or default scenes.

### Potential scene: "Overnight Research Swarm"

This could be a later showcase after GPU Launch Week:

- Places: research coordinator, GPU queue, evaluator, experiment log, model artifact, dashboard.
- Things: hypothesis, code\_patch, experiment\_run, metric\_result, crash\_report, kept\_candidate.
- Transforms: propose patch, run fixed-budget training, evaluate metric, compare best, keep/discard.
- Pressure: GPU queue contention, VRAM limit, flaky experiment, diminishing returns, deadline before morning review.
- Objectives: minimize `val_bpb`, keep crash rate below threshold, stay within GPU budget, produce a readable morning report.
- Agents: research lead, experiment proposer, evaluator, simplification critic, compute scheduler.

This would make agents themselves part of the simulated world: the viewer watches a research org learn, not just an ops team respond.

## Data-typed Factorio: telemetry as ore, dashboards as high-order products

This narrows the Factorio analogy: do not import generic iron/copper fantasy into simetro. The "raw materials" should be domain data types. A data factory is interesting when raw operational signals move through deterministic pipelines and become higher-order deliverables that people or agents rely on.

In this model:

- **Ore / copper equivalents** are raw data streams such as Mycroft events, Infinity Band signals, GPU cluster telemetry, host health pings, workload placement records, fault events, repair logs, deployment events, incident annotations, and capacity snapshots.
- **Plates / gears equivalents** are cleaned and joined data products: normalized GPU heartbeats, deduped faults, enriched cluster inventory, uptime windows, fault attribution tables, SLO burn-rate facts, and capacity headroom facts.
- **Science pack equivalents** are high-value deliverables: Kusto dashboards, Power BI reports, Lens reports, anomaly alerts, executive scorecards, incident summaries, and automated remediation recommendations.
- **The factory objective** is not "make arbitrary widgets"; it is "keep operational truth fresh, correct, cheap, and actionable under load."

### Data type language

Data types should be first-class JSON things, not comments on generic resources.

```jsonc
{
  "things": [
    {
      "id": "mycroft_event",
      "kind": "raw_signal",
      "schema_version": 12,
      "tags": ["telemetry", "gpu", "host"],
      "freshness_budget_ticks": 120,
      "quality": { "max_drop_percent": 0.1, "max_late_ticks": 60 }
    },
    {
      "id": "infinity_band_signal",
      "kind": "raw_signal",
      "schema_version": 7,
      "tags": ["capacity", "fabric", "cluster-health"],
      "freshness_budget_ticks": 180
    },
    {
      "id": "gpu_uptime_fact",
      "kind": "curated_fact",
      "inputs": ["mycroft_event", "infinity_band_signal"],
      "freshness_budget_ticks": 300,
      "quality": { "required_dimensions": ["cluster", "region", "fault_domain"] }
    },
    {
      "id": "gpu_fault_fact",
      "kind": "curated_fact",
      "inputs": ["mycroft_event", "repair_log", "deployment_event"],
      "freshness_budget_ticks": 300
    }
  ]
}
```

### Production chains as data lineage

The factory chain becomes data lineage:

1. Raw signal ingestion.

Mycroft / Infinity Band / host logs / GPU telemetry enter through source nodes.Pressure: volume spikes, late arrivals, schema drift, dropped events, hot partitions.

2. Normalization.

Raw signals are parsed, validated, deduped, timestamp-aligned, and shaped into canonical tables.Pressure: parser failures, version skew, duplicate storms, bad timestamps.

3. Enrichment and joins.

Telemetry joins with cluster inventory, deployment rings, fault domains, maintenance windows, and repair history.Pressure: missing dimensions, stale inventory, expensive joins, broken lineage.

4. Aggregation.

Facts roll up by time window, cluster, region, GPU SKU, tenant, fault domain, service, or incident.Pressure: materialized view lag, Kusto query saturation, Fabric job backlog, cache misses.

5. Report/dashboard production.

Kusto dashboards, Power BI reports, and Lens jobs consume curated facts.Pressure: freshness SLOs, query storms, executive refreshes, report correctness, alert latency.

### Example deliverable: GPU cluster uptime and faults dashboard

The first data-factory showcase could be a view-only AI operations game around keeping a GPU cluster health dashboard alive.

Raw data inputs:

- `gpu_heartbeat`: periodic host/GPU availability signal.
- `gpu_fault_event`: hardware/software fault signal.
- `cluster_inventory`: mapping of GPU to cluster, region, rack, fault domain, SKU.
- `deployment_event`: software rollout and driver version changes.
- `repair_action`: maintenance and remediation events.
- `workload_placement`: which jobs were affected by each cluster.

Intermediate products:

- `normalized_gpu_heartbeat`
- `deduped_gpu_fault`
- `gpu_inventory_snapshot`
- `gpu_fault_with_blast_radius`
- `gpu_uptime_window`
- `gpu_slo_burn_rate`

Final deliverables:

- `kusto_gpu_health_dashboard`
- `powerbi_gpu_uptime_report`
- `lens_gpu_fault_review`
- `sev_alert_gpu_slo_breach`
- `agent_remediation_brief`

### Example JSON atoms for this showcase

```jsonc
{
  "sources": [
    {
      "id": "mycroft-gpu-heartbeats",
      "emits": "gpu_heartbeat",
      "rate_per_tick": 80,
      "burst_schedule": [{ "at_tick": 1800, "multiplier": 3.0, "duration_ticks": 600 }]
    },
    {
      "id": "infinity-band-cluster-signals",
      "emits": "cluster_capacity_signal",
      "rate_per_tick": 12,
      "freshness_slo_ticks": 180
    }
  ],
  "processors": [
    {
      "id": "normalize-heartbeats",
      "role": "normalizer",
      "inputs": ["gpu_heartbeat"],
      "outputs": ["normalized_gpu_heartbeat"],
      "capacity": { "events_per_tick": 120 }
    },
    {
      "id": "kusto-gpu-health",
      "role": "kusto_cluster",
      "inputs": ["gpu_uptime_fact", "gpu_fault_fact"],
      "capacity": { "ingest_mb_per_tick": 200, "query_slots": 24, "cache_gb": 1024 }
    }
  ],
  "jobs": [
    {
      "id": "build-gpu-uptime-facts",
      "type": "materialized_view",
      "inputs": ["normalized_gpu_heartbeat", "cluster_inventory"],
      "outputs": ["gpu_uptime_fact"],
      "cadence_ticks": 120,
      "deadline_ticks": 60
    },
    {
      "id": "lens-gpu-fault-review",
      "type": "analysis_job",
      "inputs": ["gpu_fault_fact", "deployment_event", "repair_action"],
      "outputs": ["gpu_fault_review"],
      "cadence_ticks": 300,
      "deadline_ticks": 120
    }
  ],
  "reports": [
    {
      "id": "kusto-gpu-health-dashboard",
      "type": "kusto_dashboard",
      "depends_on": ["gpu_uptime_fact", "gpu_fault_fact"],
      "freshness_slo_ticks": 300,
      "priority": "critical"
    },
    {
      "id": "powerbi-gpu-uptime",
      "type": "power_bi_report",
      "depends_on": ["gpu_uptime_fact"],
      "refresh_cadence_ticks": 600,
      "freshness_slo_ticks": 900,
      "priority": "high"
    }
  ],
  "objectives": [
    { "type": "keep_reports_fresh", "targets": ["kusto-gpu-health-dashboard"], "max_stale_ticks": 300 },
    { "type": "data_quality", "target": "gpu_uptime_fact", "max_contract_violations": 0 },
    { "type": "query_latency", "target": "kusto-gpu-health", "p95_max_ticks": 5 },
    { "type": "cost_budget", "max_cost": 25000 }
  ],
  "pressure": [
    { "type": "gpu_fault_storm", "at_tick": 1500, "target": "mycroft-gpu-heartbeats", "multiplier": 4.0 },
    { "type": "schema_drift", "at_tick": 2100, "target": "gpu_heartbeat", "new_schema_version": 13 },
    { "type": "dashboard_storm", "at_tick": 2400, "target": "kusto-gpu-health-dashboard", "refresh_multiplier": 10 }
  ]
}
```

### Strategy and stakes

This game loop has real choices:

- Scale Kusto query capacity, or spend budget on ingestion.
- Refresh the executive dashboard now, or preserve query slots for alerting.
- Backfill late Mycroft events, or accept a temporary freshness gap.
- Rebuild a materialized view after schema drift, or patch downstream report logic.
- Prioritize GPU uptime facts over lower-priority exploratory Lens jobs.
- Cache expensive dashboard queries, or risk stale data.
- Tune Datadog-style monitors to reduce noise, or risk missing GPU fault spikes.

Loss conditions become concrete:

- Critical GPU health dashboard stale beyond SLO.
- Fault rate is underreported because raw signal quality degraded.
- Kusto query queue starves alert generation.
- Power BI report shows incorrect uptime after schema drift.
- Incident commander loses trust because the dashboard and Lens job disagree.
- Cost budget exhausted by brute-force scaling.

### Why this is better than generic factory resources

Generic `iron_ore -> plate -> gear` would prove mechanics but not product identity. Data-typed resources make the world feel like simetro's own lane:

- The things moving through the graph are operational truth.
- The high-order deliverables are what real operators use to make decisions.
- The AI agents are believable because they are acting like data/SRE/platform operators.
- Viewer drama is not only "belt jam"; it is "the GPU cluster is faulting and the dashboard may be stale."

## Additional mechanics brainstorm

These are not implementation commitments. They are candidate language atoms that could be mixed into JSON worlds when they create strategy, stakes, or viewer legibility.

1. Node moods / operating states.

Nodes could transition through `normal`, `busy`, `strained`, `overloaded`, `disabled`, `recovering`.JSON declares thresholds and recovery rules.Useful for stations, substations, hospitals, routers, marketplaces, and factories.

2. Service disciplines.

Queues could be `fifo`, `priority`, `nearest_deadline`, `largest_batch`, `critical_first`, or `agent_selected`.This makes node behavior interesting without adding new geometry.The AI can choose policies when allowed, not just move vehicles.

3. Multi-commodity compatibility.

Things in the world can have `kind`, `tags`, `temperature`, `fragility`, `voltage`, `shape`, `color`, or `priority`.Nodes/paths/processors declare what they accept.This creates strategy through mismatch: the route exists but cannot carry the thing.

4. Path wear and reliability.

Paths can degrade under load and recover when idle.Failure can be soft: slower travel, lower capacity, blocked admission, or random-but-seeded outages.This creates a reason to distribute traffic rather than always choose one optimal route.

5. Transfer friction.

Moving through certain nodes can cost time, capacity, money, energy, or risk.Interchange hubs become powerful but dangerous bottlenecks.This turns graph shape into strategy.

6. Budgets and build economics.

Worlds can grant finite `budget`, `materials`, `upgrade_points`, `energy`, or `crew`.Actions consume budget and may have cooldowns.This adds stakes to AI author actions: every fix has an opportunity cost.

7. Upgrades with tradeoffs.

Upgrades should not be pure improvements only.Example: increase station capacity but slow transfers; widen path but raise maintenance; speed trains but increase wear.JSON can define upgrade menus per scene.

8. Contracts and SLAs.

Instead of one global score, worlds can have contracts: "deliver 20 medicine units before tick 900" or "keep hospital wait under 8".Contracts can expire, renew, chain, or conflict.This gives the viewer short arcs inside a longer run.

9. Emergencies and shocks.

Timed or seeded disruptions: station fire, power spike, bridge outage, resource contamination, sudden festival demand.Shocks force adaptation and reveal whether the AI is robust or brittle.JSON should make these deterministic and replayable.

10. Hidden-but-revealed map information.

Some resources, hazards, or future demand can be unknown until scouting, time, or proximity reveals them.This is useful for exploration games but risky for determinism; reveal schedules must be seeded and logged.It creates "the AI learned something" moments.

11. Forecasts and imperfect predictions.

The scene can expose a forecast surface: likely demand, likely outages, predicted target changes.Forecasts can be exact, noisy, delayed, or partial.Strategy becomes planning under uncertainty instead of reacting to current queues only.

12. Local policies and constraints.

Nodes or regions can have rules: no heavy cargo, no night traffic, priority for emergency vehicles, one-way during peak.This supports city/logistics fantasies while staying graph-based.Constraints are more interesting than more node colors.

13. Regions and territories.

JSON can group nodes/paths into districts, factories, biomes, power zones, or service areas.Regions can have aggregate demand, tax, weather, hazards, or policy.This gives strategic map structure above individual nodes.

14. Energy / power as a universal limiter.

Actions and processors consume energy; generators produce it; grid paths transmit it with capacity/loss.Power shortage can slow everything rather than immediately fail.This creates an elegant cross-genre constraint for factories, metros, hospitals, and packet networks.

15. Storage, buffering, and spoilage.

Storage nodes can buffer resources, but capacity is finite and some things decay.Spoilage/deadline pressure makes overproduction dangerous.This adds factory drama: throughput is not enough if timing is wrong.

16. Routing ownership.

A mover/path can be controlled by the engine, the scene policy, or an agent.JSON can state whether the AI controls dispatch, full routing, construction, priorities, or upgrades.This helps each showcase feel like a different game without changing the engine boundary.

17. Multi-agent roles.

Separate AI agents can own different responsibilities: dispatcher, builder, forecaster, safety officer, optimizer.JSON declares agent role, action permissions, interval, and objective weight.The viewer can watch coordination or disagreement.

18. Objective tension / conflicting goals.

Worlds can score multiple objectives that pull against each other: throughput vs safety, speed vs wear, profit vs equity.JSON weights can make a scene feel cooperative, ruthless, resilient, or balanced.This gives the AI a personality surface.

19. Milestones and phase changes.

A world can move through phases: tutorial, growth, crisis, recovery, finale.Phase changes can unlock actions, introduce new demand, change target recipes, or raise failure thresholds.This creates a visible story arc instead of an endless flat loop.

20. Replay/story annotations.

JSON can define what moments deserve timeline markers: first overload, near loss, completed contract, emergency resolved, new bottleneck.These annotations help DecisionTimeline and replay UI become a "game film" rather than raw logs.Particularly important because the human is watching, not playing.

## Design guardrails for the expanded language

- Every mechanic should answer at least one of: "What is the AI trying to do?", "What can go wrong?", "What tradeoff exists?", or "How will the viewer see it?"
- Prefer composable primitives over genre-specific one-offs. `demand`, `capacity`, `deadline`, `transform`, and `failure` can express many fantasies.
- Avoid mechanics that only add nouns. A new resource, node role, or path type needs a behavior, constraint, or scoring consequence.
- Keep determinism visible in the schema: schedules, seeds, stable IDs, bounded queues, explicit tie-breaking.
- Make hidden state rare. If the viewer cannot understand why the AI acted, the mechanic may be too opaque for a view-only game.

## Suggested first vertical slice

Start with the **GPU Launch Week** vertical slice, not a generic Metro clone. It is still small, but it proves the unique product direction: AI agents operate a typed data/HPC system where operational truth is the thing being produced and protected.

- Places become source systems, processors, Kusto/Fabric/Lens/report nodes, scheduler/control-plane nodes, compute pools, storage pools, and dashboards.
- Links become telemetry streams, lineage dependencies, scheduler assignment paths, checkpoint/storage paths, and dashboard query paths.
- Things become typed operational data and jobs: Mycroft events, Infinity Band signals, GPU heartbeats, fault events, deployment records, repair records, uptime facts, fault facts, dashboard queries, GPU training jobs, MPI simulation jobs, and checkpoint data.
- Transforms normalize, dedupe, join, aggregate, refresh dashboards, execute jobs, and write checkpoints.
- Follow-up transforms run policy-search trials: same fixed scenario, one agent heuristic change, bounded run, trusted evaluator, keep/discard decision, and experiment log.
- Pressure introduces a GPU job surge, fault storm, schema drift, dashboard storm, spot eviction wave, storage metadata storm, and RDMA placement constraint.
- AI agents prioritize, throttle, scale simulated capacity, reserve query slots, adjust checkpoint pressure, preempt jobs, rebuild facts, and protect the launch-review dashboard.
- Win: critical jobs complete, GPU utilization stays healthy, Kusto/Power BI/Lens outputs remain fresh and correct, data quality holds, and cost remains bounded.
- Lose: dashboard becomes stale/incorrect during incident, a critical job misses deadline, storage pressure idles GPUs, RDMA placement collapses performance, query slots starve alerts, or cost budget is exhausted.

This slice must be scripted enough in the first 30 seconds for the viewer to learn the loop: calm system, visible pressure spike, AI intervention, consequence, second-order tradeoff, and clear trajectory toward win or loss.

After the single-run spectator version works, add **autoresearch policy-search mode** for the same scenario: run GPU Launch Week repeatedly while changing only one agent heuristic at a time, then compare outcomes and keep the best play policy. This makes simetro useful for sandboxed real-world roleplay: the world mechanics stay fixed, but the operating doctrine evolves.

## Todos

1. Audit and document current semantics for nodes, paths, movers, goals, and resource chains.
2. Design a shared game-language grammar for places, links, things, transforms, pressure, outcomes, and agency.
3. Design `stakes_v1` JSON schema additions for node roles/capacity, path capacity/cost, demand, objectives, and failure conditions.
4. Implement engine state for node queues, path occupancy, demand units, scoring, and run outcome.
5. Add deterministic systems for demand spawn, loading/unloading, path-capacity admission, scoring, and failure detection.
6. Extend protocol snapshots/events for score, objective progress, node load, path occupancy, and failure-risk overlays.
7. Extend agent observations and tool/actions so the AI can make strategic decisions.
8. Create the GPU Launch Week showcase world that can be won or lost and demonstrates visible operational tension.
9. Create policy-search mode for a fixed scenario, inspired by autoresearch: one heuristic change per trial, fixed seed/pressure/evaluator, keep/discard based on trusted outcome score.
10. Create or stub one factory/data-pressure follow-up world that proves resources/recipes/processors fit the same language.
11. Add tests for deterministic demand, capacity constraints, win/loss transitions, policy-search repeatability, and invalid action warnings.

## Notes and considerations

- Do not try to fully clone Mini Metro, shapez, Factorio, Azure, HPC, and autoresearch at once. The schema should be broad enough for all of them, but the first implementation should prove GPU Launch Week end-to-end.
- Keep all new systems deterministic: stable ID ordering, no wall-clock randomness, and all demand generated from scene config plus seeded RNG if randomness is needed.
- Preserve current visual scenes by making new fields opt-in under a new schema version or explicit feature shape.
- Prefer visible stakes over complex mechanics. A simple overload/failure loop is more valuable than a large taxonomy with no drama.
- In policy-search mode, the scenario must remain fixed. Only the agent heuristic/policy artifact changes between trials; otherwise comparisons are not meaningful.