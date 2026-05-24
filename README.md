# simetro

A personal-use, JSON-driven, top-down simulation platform with the visual
sensibility of Mini Metro and the systemic clarity of Shapez. A human watches;
AI agents author and play.

> **Status:** Phase 1, Step 1 — workspace scaffolded. See [`PLAN.md`](./PLAN.md).

## Quick start

```bash
# Install the pre-commit hook (fmt + clippy on staged Rust changes)
git config core.hooksPath .githooks

# Sanity-check the Rust workspace
cargo check --workspace

# Headless stub
cargo run -p simetro-headless -- run

# Agent bridge stub
cargo run -p simetro-agent-bridge --bin simetro-bridge

# Tauri shell stub (full UI wired up in Step 22)
cargo run -p simetro-tauri-app
```

## Layout

```
crates/
├── engine/          # pure sim core; no IO, no LLM deps
├── protocol/        # versioned wire types
├── agent-bridge/    # pluggable LLM backends (separate binary)
├── headless/        # CLI: bench, hash, run, replay, export-session
└── tauri-app/       # Tauri desktop shell
frontend/            # Vite + TS + Canvas2D + Tone.js + Playwright
games/               # JSON scene files
docs/                # architecture, schema, protocol, agents, testing, runbook, ADRs
tests/baselines/     # determinism hashes + visual diff PNGs
```

## Plan

See [`PLAN.md`](./PLAN.md) for the full Phase 1 plan (post mega-review),
including architecture, schema, observability, deployment, and the 22-step
implementation sequence.

## License

MIT — see [`LICENSE`](./LICENSE).
