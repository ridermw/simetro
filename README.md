# simetro

A personal-use, JSON-driven, deterministic systems-game platform. A human
watches; AI agents operate, author, and improve gameplay policies inside
simulated worlds with visible stakes.

> **Current roadmap:** [`docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`](./docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md).
> The active direction is **scenario_language_v1**: move beyond kinetic dioramas toward
> winnable/losable AI-operated scenarios using one JSON grammar: places, links,
> typed things, transforms, demand, pressure, outcomes, agency, observability,
> and milestones.

## Quick start

Run the current shipped simulation stack first. `scenario_language_v1`
is the roadmap; today's runnable scenes are the existing gallery worlds
under `games/`.

### 1. Validate the Rust workspace

```bash
# Install the pre-commit hook (fmt + clippy on staged Rust changes)
git config core.hooksPath .githooks

# Rust workspace: check, test, lint
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### 2. Run a scene headlessly

```bash
# Print tick/event output for a short run
cargo run -p simetro-headless -- run games/demo-paths.json --ticks 1000

# Benchmark the deterministic engine loop
cargo run -p simetro-headless -- bench games/demo-paths.json

# Produce the determinism hash used by tests/review
cargo run -p simetro-headless -- hash games/demo-paths.json

# Export a replay/session bundle
cargo run -p simetro-headless -- export-session games/demo-paths.json --ticks 1000 --out /tmp/simetro-demo
```

### 3. Try a polished gallery world

```bash
cargo run -p simetro-headless -- run games/metro-pulse.json --ticks 1000
```

If that file does not exist in your checkout, list available scenes:

```bash
ls games/*.json
```

### 4. Run the frontend

```bash
cd frontend
npm ci
npm run dev
```

Open the Vite URL shown in the terminal. Browser-only mode uses
`MockTransport`; the desktop shell uses the Tauri driver.

### 5. Validate the frontend

```bash
cd frontend
npm run typecheck
npm run lint
npm test
npm run test:e2e
```

### 6. Run the Tauri desktop shell

```bash
cd frontend && npm run build
cd ../src-tauri && cargo build
cd .. && cargo tauri dev
```

`src-tauri` is intentionally outside the Rust workspace; see
[`docs/adr/0003-tauri-over-electron.md`](./docs/adr/0003-tauri-over-electron.md).

### 7. Optional bridge helpers

```bash
# Agent bridge stdio process, currently mock-first / live-provider gated
cargo run -p simetro-agent-bridge --bin simetro-bridge

# Project-specific helpers (xtask convention)
cargo xtask help
cargo xtask copilot-smoke   # human-run only; requires `copilot` CLI on PATH
```

## Examples

### Current scene JSON

Current runnable scenes use the legacy v1/v2 shape documented in
[`docs/schema.md`](./docs/schema.md):

```jsonc
{
  "schema_version": 1,
  "name": "demo-paths",
  "pieces": {
    "nodes": [
      { "id": "a", "pos": [120, 200], "shape": "circle", "color": 2 }
    ],
    "paths": [
      { "id": "ab", "from": "a", "to": "b", "color": 3 }
    ],
    "movers": [
      { "id": "m1", "on_path": "ab", "speed": 0.8 }
    ]
  },
  "goals": [{ "type": "loop_forever" }],
  "agents": [{ "kind": "speed_tuner", "interval_ticks": 30 }]
}
```

These scenes prove deterministic loading, motion, rendering, controls,
faults/warnings, and replay foundations.

### scenario_language_v1 target shape

`scenario_language_v1` is not implemented yet. It is the next roadmap:

```jsonc
{
  "scenario_language_v1": {
    "places": ["gpu-pool", "kusto-dashboard", "scheduler"],
    "links": ["telemetry-stream", "checkpoint-path"],
    "things": ["gpu_heartbeat", "gpu_fault_fact", "training_job"],
    "transforms": ["build_uptime_fact", "refresh_dashboard"],
    "pressure": ["fault_storm", "dashboard_storm", "spot_eviction"],
    "outcomes": ["dashboard_fresh", "critical_jobs_complete"],
    "agents": ["scheduler_operator", "data_quality_guardian"]
  }
}
```

The first planned showcase is **GPU Launch Week**: agents protect a
GPU health dashboard and critical HPC jobs while telemetry faults,
schema drift, query pressure, storage pressure, cost, and spot evictions
push the system toward failure.

The viewer litmus test: within 30 seconds, you should know what the AI
is trying to save, what is going wrong, and whether the latest action
helped.

## Layout

```
crates/
├── engine/          # pure sim core; no IO, no LLM deps
├── protocol/        # versioned wire types
├── agent-bridge/    # pluggable LLM backends (separate binary)
├── headless/        # CLI: bench, hash, run, replay, export-session
└── tauri-app/       # workspace stub (engine-facing helpers)
src-tauri/           # Tauri 2 desktop shell (built separately; see ADR-003)
frontend/            # Vite + TS + Canvas2D + Tone.js + Playwright
games/               # JSON scene files
docs/                # architecture, schema, protocol, agents, testing, runbook, ADRs
tests/baselines/     # determinism hashes + visual diff PNGs
```

## Docs

- [`docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`](./docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md) — active roadmap and design plan
- [`docs/tauri-bridge.md`](./docs/tauri-bridge.md) — Tauri engine driver architecture + message flow
- [`docs/architecture.md`](./docs/architecture.md) — system tour, crate map, determinism contract
- [`docs/schema.md`](./docs/schema.md) — JSON scene schema
- [`docs/world-quality.md`](./docs/world-quality.md) + [`docs/world-template.jsonc`](./docs/world-template.jsonc) — polished v1 world checklist and template
- [`docs/protocol.md`](./docs/protocol.md) — wire envelope reference
- [`docs/agents.md`](./docs/agents.md) — agent loop, backends, AgentLog
- [`docs/testing.md`](./docs/testing.md) — what every test layer guarantees
- [`docs/runbook.md`](./docs/runbook.md) — operational responses to faults
- [`docs/adr/`](./docs/adr/) — architectural decision records

## Plan

See [`PLAN.md`](./PLAN.md) for the active roadmap pointer and implementation
guardrails.

## License

MIT — see [`LICENSE`](./LICENSE).
