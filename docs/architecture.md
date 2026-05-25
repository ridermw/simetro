# simetro architecture

> One-page tour. For the active roadmap, see [`../PLAN.md`](../PLAN.md).

simetro is a **single-binary Rust + Canvas2D desktop app** (via Tauri)
plus a **separate process** for LLM agents (the `agent-bridge`), built
around a deterministic, fixed-tick simulation engine. The frontend
talks to the engine via a typed JSON protocol; agents talk to the
engine via the same protocol's `Action` shapes via the bridge.

```
  +----------------------+        +---------------------+
  |  Tauri Desktop Shell |        |   agent-bridge      |
  |  + WebView (TS UI)   |<-----> |   (separate proc)   |
  |                      |  JSON  |   - MockBackend     |
  +----------+-----------+        |   - CopilotBackend  |
             ^                    |   - future backends  |
             | StaticPayload      +----------+----------+
             | Snapshot @ 20Hz               ^
             | Events                        | Observation
             | AgentReport                   v Action
             | Fault / Warning      +--------+----------+
             |                      |  Engine (Rust)    |
             +----------------------+  fixed 30Hz tick  |
                                    |  systems pipeline |
                                    +--------+----------+
                                             ^
                                             | JSON scene
                                             +----------- games/*.json
```

## Workspace layout

| Crate              | Role                                                          |
| ------------------ | ------------------------------------------------------------- |
| `crates/engine`    | World, systems, tick loop, deterministic event emission.      |
| `crates/protocol`  | Wire types: `SimMessage`, `AgentMessage`, `Action`, faults.   |
| `crates/loader`    | JSON → world graph; bounds checks; typed `LoadError`.         |
| `crates/headless`  | CLI: `run`, `bench`, `hash`, `replay`, `export-session`.      |
| `crates/agent-bridge` | LLM connector boundary: `Backend` trait, Mock, Copilot stub. |
| `frontend/`        | Vite + TS + Canvas2D + Tone.js renderer, inspector, UI shell. |
| `src-tauri/`       | Tauri wrapper around `frontend/dist`.                         |

## Determinism contract

The engine is **bit-identical** for a given (scene + seed + agent
trace). The headless `hash` subcommand SHA-256s the world state + the
full event stream; `tests/baselines/demo-paths.hash` is the committed
baseline and `crates/engine/tests/determinism_baseline.rs` asserts
it on every CI run. This is the foundation of every other guarantee.

Sources of non-determinism are forbidden:
- No floating-point time accumulation (we use fixed-rate u64 ticks).
- BTreeMap ordering, never HashMap, for any iteration that emits events.
- Agent calls happen at deterministic tick boundaries; the bridge
  serializes observation → action with timeouts but never lets the
  engine drift on `await`.

## Protocol envelopes

Every message has `{schema_version, seq, payload}`. The frontend
guards every inbound message with `isCurrentSchema`. Mismatch raises
`Fault::SchemaMismatch` and freezes the renderer (see
[`runbook.md`](./runbook.md)).

## Frontend pipeline

```
transport ──▶ store ──▶ renderer (canvas + animations) + audio + inspector + UI
```

The renderer is pure: `(theme, snapshot, movers) -> pixels`. Animation
state lives in a ring buffer of slots. All UI text goes through
`textContent`; the `no-unsanitized/method+property` lint rule and
the `XSS-shaped rationale` unit test enforce this.

See:
- [`schema.md`](./schema.md) — JSON scene authoring.
- [`protocol.md`](./protocol.md) — wire envelope reference.
- [`agents.md`](./agents.md) — agent loop and AgentLog.
- [`testing.md`](./testing.md) — what every test layer guarantees.
- [`runbook.md`](./runbook.md) — operational responses to faults.
- [`adr/`](./adr/) — durable architectural decisions.
