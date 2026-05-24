# Wire protocol

The engine and the frontend exchange JSON messages over a transport
(Tauri IPC in production, in-process Mock in dev, WebSocket in P2).
Every message is wrapped in an `Envelope`:

```ts
interface Envelope<T> {
  schema_version: number;
  seq: number;
  payload: T;
}
```

`SCHEMA_VERSION` (currently `1`) is defined in
`frontend/src/protocol/messages.ts` and matches `SCHEMA_VERSION` in
`crates/protocol/src/lib.rs`. The frontend rejects any envelope
whose `schema_version` doesn't match, raising
`Fault::SchemaMismatch` and freezing the renderer.

## Engine → frontend

```ts
type SimMessage =
  | { type: "Static";   payload: StaticPayload }
  | { type: "Snapshot"; payload: SnapshotPayload }
  | { type: "Events";   payload: SimEvent[] }
  | { type: "AgentReport"; payload: AgentReport }
  | { type: "Fault";    payload: EngineFault }
  | { type: "Warning";  payload: EngineWarning };
```

- `Static` ships once per scene: theme, scene name, and the
  `id_map` (numeric handle → human-readable JSON id, used by the
  Inspector and hover tooltip).
- `Snapshot` at 20 Hz: `tick`, full positions of every node, path,
  and mover. The renderer interpolates between consecutive snapshots
  to hit 60+ fps.
- `Events` is a burst array of `SimEvent` variants (`MoverDeparted`,
  `MoverArrived`, `MoverSpeedChange`, `NodeHighlighted`, `PathPulsed`,
  `AgentDecided`, `Tick`). Each spawns an animation slot.
- `AgentReport` is the inspector payload: agent_id, tick,
  considered actions with confidences, chosen action, free-text
  rationale, top-level confidence.
- `Fault` is a hard problem (load error, agent crash, drift, panic,
  schema mismatch, backpressure saturation). Full-bleed overlay.
- `Warning` is a soft problem (`InvalidAction`, `Behind`,
  `TickOverBudget`, `AgentLogSlow`). Pill in the top-right strip.

## Frontend → engine (P2)

P1 only ships `ControlIntent` as a UI-internal type; routing it
over the wire is a Step 22 follow-up.

```ts
type ControlIntent =
  | { kind: "TogglePause" }
  | { kind: "Step" }
  | { kind: "Reload" }
  | { kind: "SetSpeed"; factor: number };
```

## Agent ↔ engine (via `agent-bridge`)

The bridge mediates between LLM tool calls and engine `Action`s:

```rust
enum Action {
    NoOp,
    SetSpeed { mover: u32, value: f32 },
    PlacePiece { ... },     // P2 — engine rejects with Warning::InvalidAction
    ConnectPieces { ... },  // P2
    RemovePiece { ... },    // P2
}
```

The bridge exposes one `ToolSpec` per `Action` (see
`crates/agent-bridge/src/tools.rs`); models call the tool, the
bridge parses the JSON arguments, and the result lands on the
engine's per-agent action queue at the next tick boundary.

## Versioning

This document is the canonical reference for the wire protocol.
Schema bumps require:

1. Bumping `SCHEMA_VERSION` in `crates/protocol/src/lib.rs` and in
   `frontend/src/protocol/messages.ts`.
2. Adding a new entry to `docs/adr/` describing the change.
3. Updating the determinism baseline (see `testing.md`).
