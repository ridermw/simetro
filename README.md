# simetro

A personal-use, JSON-driven, top-down simulation platform with the visual
sensibility of Mini Metro and the systemic clarity of Shapez. A human watches;
AI agents author and play.

> **Status:** Phase 1 complete (22/22 steps). All 22 steps shipped per [`PLAN.md`](./PLAN.md) — Rust engine, protocol, headless CLI, agent-bridge with Mock + Copilot stub, full TypeScript frontend with Canvas2D renderer, animation engine, audio, inspector, UI shell, faults/warnings overlays, and Playwright E2E suite. Determinism gate green. See [`docs/`](./docs/) for architecture, schema, protocol, agents, testing, runbook, and ADRs.

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

# Frontend (Vite dev server + Playwright)
cd frontend
npm ci
npm run typecheck
npm run lint
npm test          # 52 unit tests
npm run test:e2e  # 7 Playwright smoke tests (builds first)

# Tauri desktop shell (NOT in workspace — see ADR-003)
cd src-tauri && cargo build
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

- [`docs/architecture.md`](./docs/architecture.md) — system tour, crate map, determinism contract
- [`docs/schema.md`](./docs/schema.md) — JSON scene schema
- [`docs/protocol.md`](./docs/protocol.md) — wire envelope reference
- [`docs/agents.md`](./docs/agents.md) — agent loop, backends, AgentLog
- [`docs/testing.md`](./docs/testing.md) — what every test layer guarantees
- [`docs/runbook.md`](./docs/runbook.md) — operational responses to faults
- [`docs/adr/`](./docs/adr/) — architectural decision records

## Plan

See [`PLAN.md`](./PLAN.md) for the full Phase 1 plan (post mega-review),
including architecture, schema, observability, deployment, and the 22-step
implementation sequence.

## License

MIT — see [`LICENSE`](./LICENSE).
