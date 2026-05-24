# ADR-003: Tauri over Electron

**Status:** Accepted.

## Context

We ship a desktop binary that bundles the frontend and talks to
local Rust code (the engine and the agent-bridge process).
Candidates: Electron, Tauri, native (egui / iced), web-only.

## Decision

Tauri.

- Native code is Rust — same language as the engine, so no FFI
  layer and no double JSON-encoding to the engine.
- The binary is < 20 MB vs Electron's ~150 MB, which matters for
  a single-engineer side project.
- Default-deny allowlist (PLAN §12) makes the security posture
  explicit: every native capability the frontend can call is
  enumerated in `tauri.conf.json`.
- Tauri 2's IPC is a typed pub/sub channel that maps cleanly onto
  our `SimMessage` envelopes.

## Consequences

- (+) Engine + frontend live in one process; no cross-process JSON
  for the hot loop (we keep cross-process JSON only for the
  agent-bridge, which is intentional — see ADR-005).
- (+) `cargo tauri build` produces signed binaries for macOS,
  Windows, and Linux from the same source.
- (-) Tauri requires platform WebKit/WebView2; this means the
  `src-tauri` crate is **not** part of the default `cargo
  --workspace` build to avoid breaking CI on minimal containers.
  It is built explicitly via `cd src-tauri && cargo build`.
- (-) Tauri's docs are thinner than Electron's. We accept this.
