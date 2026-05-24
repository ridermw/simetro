# Copilot instructions for simetro

## Project shape

simetro is a JSON-driven simulation platform with a pure Rust engine, a
versioned protocol, a Tauri desktop shell, and a TypeScript Canvas2D frontend.
Keep engine logic deterministic and provider/network logic out of the engine.

## Current product direction

Prioritize non-Copilot product capabilities first:

- Safe local scene gallery and registry-backed scene switching.
- Polished JSON worlds that load through the existing scene model.
- Author actions, resources/production, replay, and protocol foundations.
- Frontend cockpit clarity and visual quality.

Live Copilot/provider integration is intentionally deferred. Do not suggest
live LLM backend work unless explicitly asked.

## Review priorities

When reviewing changes, focus on:

1. Scene selection must use stable `scene_id` values and a backend whitelist.
   Never accept arbitrary frontend-provided file paths.
2. Failed scene switches must preserve the previous running scene and avoid
   stale catalog metadata, snapshots, hover/inspector state, warnings, or faults.
3. Frontend catalog entries, Tauri scene registry entries, and `games/*.json`
   files must stay aligned.
4. User-facing text must be rendered via `textContent` or an equivalent safe
   API, not `innerHTML`.
5. Engine mutations must be deterministic, ordered by stable IDs where order
   matters, and surface invalid requests as typed warnings or faults.
6. File watching must debounce edits and reuse the existing reload/load-error
   path.
7. Protocol additions must reject schema-version mismatches before processing
   payloads.
8. New worlds are visual/gallery data. Check JSON validity, catalog metadata,
   registry alignment, and visual distinctness; do not expand the scope to
   schema v2, resources UI, or editor work in the same PR.

## Validation expectations

Use the smallest relevant validation for the change, then widen when touching
shared engine/protocol/frontend surfaces:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cd src-tauri && cargo test --locked
cd frontend && npm run typecheck && npm run lint && npm test -- --run && npm run build
```

For world/catalog changes, also make sure the world quality checklist, frontend
catalog tests, and Tauri scene registry tests pass.

## Explicit non-goals

- No arbitrary local scene browser in the first gallery flow.
- No external screenshots/assets or new UI dependencies unless explicitly
  approved.
- No live Copilot/provider work while that roadmap item is deferred.
- No silent fallbacks: invalid input or failed loading should be visible and
  test-covered.
