# Copilot instructions for simetro

> **Authoritative roadmap:** see
> [`docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`](../docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md).
> That spec supersedes previous roadmap drafts and archived plans.

## Project shape

simetro is a JSON-driven simulation platform with a pure Rust engine,
a versioned protocol, a Tauri desktop shell, and a TypeScript Canvas2D
frontend. Keep engine logic deterministic. Provider/network logic
lives in the separate `simetro-bridge` process, not in the engine.

## Current product direction

The deterministic engine, protocol, Tauri shell, frontend, gallery,
author actions, global resources/production, replay foundations, file
watcher, and scene registry are already shipped.

The **active marquee** is now `scenario_language_v1`: make simetro a
view-only, AI-operated systems-game platform with visible objectives,
pressure, win/loss, observability, and policy search.

In-scope tracks:

- Unified v3 grammar: places, links, typed things, transforms, demand,
  pressure, outcomes, agency, observability, and milestones.
- GPU Launch Week as the first vertical slice.
- Strict v3 behavior schema, typed predicates, explicit v3
  LoadError/Warning/Fault/GameOutcome taxonomy, metric states, and
  scoped agent actions.
- Simulated autoresearch-style policy search over fixed scenarios.

Out of active scope unless a later spec explicitly promotes it:

- Live Azure/Kusto/Fabric/Power BI/HPC integrations.
- Arbitrary expression languages in scene JSON.
- Live LLM calls in CI.
- WebSocket/WASM/live-provider expansion not needed for `scenario_language_v1`.

## Review priorities

When reviewing PRs, focus on:

1. **Determinism.** Engine mutations must be deterministic and ordered
   by stable IDs. The async LLM boundary must not contaminate the
   deterministic world (outbox/inbox; LLM decisions land at known tick
   boundaries).
2. **Typed failure surfaces.** Every `LlmError` variant must map to a
   `Fault` or `Warning` with the user-visible behavior documented. No
   `unwrap`, no silent swallow, no `rescue StandardError`-equivalents.
3. **Scene safety.** Scene selection uses stable `scene_id` via the
   registry. **Reject any frontend-supplied file path** — the backend
   registry is the only allowed source of scene file locations. Failed
   loads preserve the previous scene. Live-LLM scenes are
   feature-gated and excluded from CI.
4. **Process boundary.** The bridge is a separate process. The engine
   never imports an LLM crate or `tokio::net`. Wire protocol carries
   `schema_version: u32` on every envelope.
5. **Safe text.** **All user-facing text** must render via
   `textContent` (or an equivalent safe API). This explicitly includes
   LLM-produced strings (rationale, raw_response, refusal messages),
   which are the highest-risk XSS vector — never `innerHTML` for any
   string that ultimately came from JSON, user input, or an LLM.
6. **Catalog alignment.** Frontend catalog entries, Tauri scene
   registry entries, and `games/*.json` files must stay aligned.
7. **File watcher determinism.** Debounce file edits and reuse the
   existing reload/load-error path.
8. **AgentLog v2 schema.** Schema bumps are additive and the loader
   keeps an explicit migration shim. Replay reads from AgentLog and
   never re-invokes the live model.
9. **Test ambition.** Recorded ACP fixtures cover every error mode.
   Live smoke is human-run only via `cargo xtask copilot-smoke`.
10. **Copilot Code Review on every PR.** After opening a PR, request
    Copilot Code Review by commenting `@copilot review`. This is a
    repo-wide policy, not a per-task preference.

For `scenario_language_v1` reviews, also require:

11. **Visible stakes.** A new v3 scene must be winnable/losable and must
    show what the AI is trying to save, what is going wrong, and whether
    the latest action helped within 30 seconds.
12. **Strict v3 schema.** Unknown behavior-bearing fields fail load.
    Typed predicates only; no string expression evaluator.
13. **No silent game failures.** Backpressure, starvation, stale
    dashboards, invalid policies, data-quality violations, objective
    breaches, and terminal losses must surface as typed warnings,
    faults, outcomes, metrics, or milestones.

## Validation expectations

Use the smallest relevant validation for the change, then widen when
touching shared engine/protocol/frontend surfaces:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cd src-tauri && cargo test --locked
cd frontend && npm run typecheck && npm run lint && npm test -- --run && npm run build
```

For world/catalog changes, also run the world quality checklist,
frontend catalog tests, and Tauri scene registry tests.

For live-LLM bridge changes, also run the recorded-fixture test suite
under `crates/agent-bridge/tests/fixtures/copilot-acp/`. Do NOT run
the live smoke target in CI.

## Explicit non-goals

- No arbitrary local scene browser in the first gallery flow.
- No external screenshots/assets or new UI dependencies unless
  explicitly approved in the spec.
- No live Copilot calls from CI. Ever.
- No silent fallbacks: invalid input or failed loading should be
  visible and test-covered.
- No additional live provider backends during `scenario_language_v1` unless the
  roadmap explicitly promotes them.
