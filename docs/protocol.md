# Wire protocol (v1)

simetro's engine, frontend, headless CLI, and agent-bridge all
exchange messages over a single versioned protocol (PLAN §6). The
canonical definition lives in
[`crates/protocol/src/lib.rs`](../crates/protocol/src/lib.rs); the TS
mirror is [`frontend/src/protocol/messages.ts`](../frontend/src/protocol/messages.ts).
Roundtrip JSON tests in the protocol crate guard the two against
drift.

## Envelope

Every message is wrapped in an envelope that carries the schema
version and a monotonic sequence number:

```jsonc
{
  "schema_version": 1,
  "seq": 42,
  "payload": { /* SimMessage | AgentMessage */ }
}
```

Frontends check `schema_version` on every inbound message; a mismatch
surfaces as a `Fault::SchemaMismatch` overlay and freezes the
renderer.

## Engine → consumers: `SimMessage`

Tagged union. The Rust source uses
`#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]`,
so every variant on the wire is `{ "kind": ..., "payload": ... }`:

| `kind`         | Payload type     | Cadence                                 |
| -------------- | ---------------- | --------------------------------------- |
| `static`       | `StaticPayload`  | Once on connect.                         |
| `snapshot`     | `SnapshotPayload`| ~20 Hz; mover positions only.            |
| `events`       | `SimEvent[]`     | Batched per tick.                        |
| `agent_report` | `AgentReport`    | Each time an agent acts.                 |
| `fault`        | `FaultPayload`   | Engine entered a fault state.            |
| `warning`      | `WarningPayload` | Non-fatal degradation.                   |

### `StaticPayload`

Sent once at connect-time; contains everything the renderer needs to
draw the scene's immutable structure:

```jsonc
{
  "name": "demo-paths",
  "palette": ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"],
  "background_index": 0,
  "nodes": [
    { "id": 1, "pos": [120, 200], "shape": "circle",   "color": 2 }
  ],
  "paths": [
    { "id": 4, "from_pos": [120, 200], "to_pos": [420, 120], "color": 3 }
  ],
  "node_names":  { "1": "a" },
  "path_names":  { "4": "ab" },
  "mover_names": { "7": "m1" }
}
```

* `nodes[i].shape` is one of `circle | square | triangle | diamond | hexagon`.
* `paths[i]` carries **baked endpoint positions** (not node refs) so
  the renderer can group all paths of one color into a single `Path2D`
  in one pass (PLAN §9).
* `*_names` are reverse maps from runtime numeric id to original
  JSON id, used by the Inspector and hover tooltip.

### `SnapshotPayload`

Mover positions only; nodes and paths are immutable for the scene's
lifetime so they don't ship per snapshot:

```jsonc
{
  "tick": 142,
  "movers": [
    { "id": 7, "pos": [180.5, 175.2], "speed": 0.8, "on_path": 4 }
  ]
}
```

### `SimEvent[]`

Flat tagged union (`#[serde(tag = "kind", rename_all = "snake_case")]`):

```jsonc
[
  { "kind": "tick", "tick": 142 },
  { "kind": "mover_departed", "mover": 7, "from_node": 1, "path": 4 },
  { "kind": "mover_arrived",  "mover": 7, "at_node": 2,   "path": 4 },
  { "kind": "mover_speed_change", "mover": 7, "old": 0.8, "new": 1.2 },
  { "kind": "node_highlighted", "node": 1, "reason": "agent_focus" },
  { "kind": "path_pulsed", "path": 4 },
  { "kind": "agent_decided", "agent_id": "speed_tuner_0", "action": "set_speed" }
]
```

`HighlightReason` is one of `agent_focus | bottleneck | goal_reached`.
The `action` field on `agent_decided` is an `ActionTag` (the
snake_case discriminant of `Action`, not the full action body — full
actions ship via `AgentReport.chosen`).

### `AgentReport`

```jsonc
{
  "tick": 142,
  "agent_id": "speed_tuner_0",
  "considered": [
    {
      "action": { "kind": "set_speed", "mover": 7, "speed": 1.6 },
      "confidence": 0.83
    },
    { "action": { "kind": "no_op" }, "confidence": 0.42 }
  ],
  "chosen": { "kind": "set_speed", "mover": 7, "speed": 1.6 },
  "rationale": "m1 has been waiting; speed up to clear backlog",
  "confidence": 0.83
}
```

`chosen` is nullable (an agent may decline to act). `considered` is
capped at 1000 entries per report (PLAN §7).

### `Action`

Tagged union shared with `AgentMessage`:

| `kind`            | Fields                              |
| ----------------- | ----------------------------------- |
| `no_op`           | (none)                              |
| `set_speed`       | `mover: u32`, `speed: f32`          |
| `place_piece`     | `piece_kind: string`, `pos: [f32;2]`|
| `connect_pieces`  | `from: u32`, `to: u32`              |
| `remove_piece`    | `id: u32`                           |

`place_piece` / `connect_pieces` / `remove_piece` are stubbed in P1 —
the engine accepts them but the validator rejects with
`Warning::InvalidAction` until P2.

### `FaultPayload`

Tagged with `kind`:

| `kind`                    | Fields                                       |
| ------------------------- | -------------------------------------------- |
| `load_error`              | `message`, `line: u32?`, `col: u32?`         |
| `agent_crashed`           | `agent_id: string`, `message`                |
| `numeric_drift`           | `tick: u64`                                  |
| `engine_fault`            | `message`                                    |
| `baseline_hash_mismatch`  | `expected`, `found`                          |
| `schema_mismatch`         | `expected: u32`, `found: u32`                |
| `transport_lost`          | (none)                                       |

### `WarningPayload`

| `kind`              | Fields                                  |
| ------------------- | --------------------------------------- |
| `invalid_action`    | `agent_id`, `reason`                    |
| `behind`            | `lag_frames: u32`                       |
| `tick_over_budget`  | `ms: f32`                               |
| `agent_log_slow`    | (none)                                  |

## Agent → engine: `AgentMessage`

Same tagging convention as `SimMessage` (`kind` + `payload`):

```jsonc
{ "kind": "connect", "payload": { "agent_id": "openai_0", "capabilities": ["set_speed"] } }
{ "kind": "action",  "payload": { "kind": "set_speed", "mover": 7, "speed": 1.2 } }
{ "kind": "heartbeat", "payload": null }
{ "kind": "disconnect", "payload": { "reason": "client closed" } }
```

P1 scaffolds the message types; P2 wires them end-to-end through
`agent-bridge` (see [agents.md](agents.md)).

## Versioning rules

* `SCHEMA_VERSION` is a `u32` constant in
  [`crates/protocol/src/lib.rs`](../crates/protocol/src/lib.rs) and
  the matching constant in `frontend/src/protocol/messages.ts`. Both
  must change in lockstep.
* Any additive change to an existing variant must keep the variant
  backward-compatible at v1 (extra fields default-on-deserialize) OR
  bump `SCHEMA_VERSION` and add a migration in
  `crates/protocol/src/version.rs`.
* Any breaking change (renamed field, removed variant) requires a
  version bump and updates to both Rust and TS mirrors.
* CI catches drift via the protocol crate's roundtrip JSON tests and
  the determinism baseline gate.

## Transports

The same versioned protocol flows over multiple transports
(PLAN §3.3):

* **Tauri events** — primary frontend transport. Each `SimMessage`
  is emitted on a named event channel.
* **stdio** — used by `simetro-headless replay` and by the
  agent-bridge subprocess.
* **WebSocket** — reserved for P2 (external-language agents and
  remote inspector clients).

All three transports speak the identical envelope shape; no
transport-specific encoding lives above the framing layer.
