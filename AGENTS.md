# Agent instructions for simetro

## Work style

- Prefer small, vertical changes that keep the app runnable.
- Reuse existing engine, protocol, Tauri, frontend, and test patterns before
  adding new abstractions.
- Keep behavior deterministic in the Rust engine. Use stable ordering for
  action application, production systems, hashing, and emitted events.
- Do not add live Copilot/provider integration unless explicitly requested.

## Safety rules

- Scene selection must be registry-backed by `scene_id`; never pass arbitrary
  paths from frontend to backend.
- Failed scene loading or switching must preserve the previous running scene
  unless a plan explicitly says otherwise.
- Render user-facing strings with safe text APIs. Avoid `innerHTML`.
- Invalid user/agent actions should produce typed warnings or faults, not
  silent no-ops.
- Keep resource, author-action, replay, WebSocket, and WASM plugin work separate
  from provider/LLM work.

## Validation shortcuts

- Engine/protocol changes: `cargo test --workspace --all-targets` and
  `cargo clippy --workspace --all-targets -- -D warnings`.
- Tauri scene switching or file watching: `cd src-tauri && cargo test --locked`.
- Frontend changes: `cd frontend && npm run typecheck && npm run lint &&
  npm test -- --run`.
- World/catalog changes: run the world quality checklist, frontend catalog
  tests, and Tauri scene registry tests.

## Documentation

Update nearby docs when changing:

- JSON schema or world conventions: `docs/schema.md`, `docs/world-quality.md`,
  `docs/world-template.jsonc`.
- Protocol or agent surfaces: `docs/protocol.md`, `docs/agents.md`.
- Tauri scene switching or file watching: `docs/tauri-bridge.md`,
  `docs/runbook.md`.
