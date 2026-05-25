# Agent instructions for simetro

> **Authoritative roadmap:** see
> [`docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`](docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md).
> That spec supersedes previous roadmap drafts and archived plans.

## Work style

- Prefer small, vertical changes that keep the app runnable.
- Reuse existing engine, protocol, Tauri, frontend, and test patterns
  before adding new abstractions.
- Keep behavior deterministic in the Rust engine. Use stable ordering
  for action application, production systems, hashing, and emitted
  events.
- The engine's tick loop is synchronous; live LLM agents must use the
  outbox/inbox pattern (the agent emits a request to a bounded outbox,
  the bridge fulfils asynchronously, the engine drains the inbox at
  tick boundaries). The deterministic non-LLM world must never block
  on an LLM call.

## Current product direction

The active product direction is **scenario_language_v1**:

- Move from kinetic scenes to winnable/losable AI-operated simulations.
- Use one JSON grammar: places, links, typed things, transforms, demand,
  pressure, outcomes, agency, observability, and milestones.
- First vertical slice: **GPU Launch Week**, a simulated HPC/data-ops
  scenario with visible dashboard freshness, job scheduling, storage,
  quota/cost, telemetry quality, and multi-agent policy pressure.
- Autoresearch-style loops are in scope as simulated policy search over
  a fixed scenario: one heuristic change per trial, same seed/pressure,
  trusted evaluator, keep/discard based on outcome.

## Live Copilot/provider direction

Live Copilot integration remains supported by the existing bridge
direction, but it is not the product marquee for `scenario_language_v1`. Do not let
live-provider work distract from the simulated game-language slice.

- The bridge remains a separate process from the engine.
- No live cloud/provider calls run in CI.
- Live LLM scenes stay feature-gated/default-off.
- Every `LlmError` variant maps to a typed `Fault` or `Warning`. No
  silent failures.

## WebSocket external agents and WASM plugin agents

Out of active scope for `scenario_language_v1` implementation unless the roadmap
explicitly promotes them.

## Safety rules

- Scene selection must be registry-backed by `scene_id`; never pass
  arbitrary paths from frontend to backend. **Reject any
  frontend-supplied file path; the backend registry is the only
  allowed source of scene file locations.**
- Failed scene loading or switching must preserve the previous running
  scene unless a plan explicitly says otherwise.
- **All user-facing text must render via `textContent` (or an
  equivalent safe API), never `innerHTML`.** This explicitly includes
  LLM-produced strings (rationale, raw_response, refusal messages,
  faulted-agent context) — those are the highest-risk XSS vectors
  because they can be prompt-injected to emit `<script>` payloads.
- Invalid user/agent actions should produce typed warnings or faults,
  not silent no-ops.
- Live LLM scenes must be feature-gated; CI never invokes the real
  Copilot provider.
- For `scenario_language_v1` scenes, unknown behavior-bearing schema
  fields must fail load. Only `catalog`/metadata may remain permissive.
- Use typed predicates and bounded declarative policies. Do not add an
  expression language or script-like scene behavior.
- Keep Azure/Kusto/Fabric/Power BI/HPC/autoresearch concepts simulated
  unless a later spec explicitly authorizes a live integration.

## Review workflow

- **Every PR gets Copilot Code Review.** After opening a PR, request
  review by commenting `@copilot review` on the PR (or by adding
  `copilot-pull-request-reviewer` as a reviewer). Branch protection
  requires conversations resolved before merge.
- Branch protection on `main` requires `ci-ok` green, conversations
  resolved, and is enforced on admins. Self-merge is permitted **only**
  when both gates are satisfied and the change is within the active
  spec scope.

## Validation shortcuts

- Engine/protocol changes: `cargo test --workspace --all-targets` and
  `cargo clippy --workspace --all-targets -- -D warnings`.
- Tauri scene switching or file watching:
  `cd src-tauri && cargo test --locked`.
- Frontend changes:
  `cd frontend && npm run typecheck && npm run lint && npm test -- --run`.
- World/catalog changes: run the world quality checklist, frontend
  catalog tests, and Tauri scene registry tests.

## Documentation

Update nearby docs when changing:

- JSON schema or world conventions: `docs/schema.md`,
  `docs/world-quality.md`, `docs/world-template.jsonc`.
- Protocol or agent surfaces: `docs/protocol.md`, `docs/agents.md`.
- Tauri scene switching or file watching: `docs/tauri-bridge.md`,
  `docs/runbook.md`.
- `scenario_language_v1` roadmap, schema decisions, policy search, and PR workflow:
  the canonical spec.
