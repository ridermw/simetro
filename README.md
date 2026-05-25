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

```bash
# Install the pre-commit hook (fmt + clippy on staged Rust changes)
git config core.hooksPath .githooks

# Rust workspace: check, test, lint
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# Headless CLI
cargo run -p simetro-headless -- run games/demo-paths.json --ticks 1000
cargo run -p simetro-headless -- bench games/demo-paths.json
cargo run -p simetro-headless -- hash games/demo-paths.json

# Agent bridge stub
cargo run -p simetro-agent-bridge --bin simetro-bridge

# Project-specific helpers (xtask convention)
cargo xtask help
cargo xtask copilot-smoke   # human-run; requires `copilot` CLI on PATH

# Frontend (Vite dev server + Playwright)
cd frontend
npm ci
npm run typecheck
npm run lint
npm test          # 59 unit tests
npm run test:e2e  # 9 Playwright E2E tests (builds first)

# Tauri desktop shell (NOT in workspace — see ADR-003)
cd src-tauri && cargo build
# Then launch: cargo tauri dev  (from repo root, needs tauri-cli)
```

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
