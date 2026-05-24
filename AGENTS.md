# Agent instructions for simetro

> **Authoritative roadmap:** see
> [`docs/superpowers/specs/2026-05-24-post-pr3-roadmap-design.md`](docs/superpowers/specs/2026-05-24-post-pr3-roadmap-design.md).
> That spec defines the current active scope (Phase 2.A → 2.C this week),
> autonomous-execution policy, PR workflow, and stop conditions.

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

## Live Copilot/provider direction

Live Copilot CLI SDK integration is the current marquee work
(Phase 2.A in the spec). It supersedes the previous "intentionally
deferred" stance.

- The Copilot backend uses `copilot --acp` (Agent Client Protocol)
  spawned as a subprocess from the `simetro-bridge` binary. No HTTP
  client, no API keys — `gh auth status` is the only credential.
- The bridge is a **separate process** from the engine (per
  PLAN-v4 §3.4 and the post-PR-#3 spec §10). The engine speaks the
  versioned wire protocol to it.
- Every `LlmError` variant maps to a typed `Fault` or `Warning`. No
  silent failures.

## Other backends (OpenAI, Anthropic, Codex, Ollama)

Out of active scope. Stay shelved unless the spec explicitly enables
them. The `Backend` trait keeps the door open.

## WebSocket external agents and WASM plugin agents

Out of active scope. The protocol foundation exists; do not extend it
into live wiring during the current working week.

## Safety rules

- Scene selection must be registry-backed by `scene_id`; never pass
  arbitrary paths from frontend to backend.
- Failed scene loading or switching must preserve the previous running
  scene unless a plan explicitly says otherwise.
- Render user-facing strings with safe text APIs. Avoid `innerHTML`.
- Invalid user/agent actions should produce typed warnings or faults,
  not silent no-ops.
- Live LLM scenes must be feature-gated; CI never invokes the real
  Copilot provider.

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
- LLM/agent runtime behavior, autonomous-execution policy, and PR
  workflow: the canonical spec.
