# Agents

simetro agents are **out-of-process** LLM-driven decision makers. The
engine never imports an HTTP or LLM crate; instead the
`agent-bridge` crate sits between the two and offers a `Backend`
trait that any concrete LLM connector can implement.

```
+----------------+   observation   +-------------------+   tool_call   +----------+
|     Engine     | --------------> |  agent-bridge     | <-----------> |  Model   |
|  (Rust crate)  |                 |  (separate proc)  |               |  (LLM)   |
|                | <-------------- |                   |               +----------+
+----------------+     Action      +-------------------+
                                          |
                                          v
                                   tools.rs (5 JSON Schemas:
                                   no_op, set_speed, place_piece,
                                   connect_pieces, remove_piece)
```

## Bridge

`crates/agent-bridge/src/bridge.rs` exposes:

```rust
pub struct Bridge { /* Arc<dyn Backend> */ }

impl Bridge {
    pub async fn decide(&self, agent_id: &str, prompt: &str) -> Result<Action, LlmError>;
}
```

`decide` calls the backend with the action tool specs from
`tools.rs`, takes the first `ToolCall` from the response, and parses
it into a typed `Action`. Empty `tool_calls` + a refusal cue in the
raw text (`"refuse"`, `"can't help"`, `"won't help"`, `"cannot help"`,
`"i'm sorry"`) is reported as `LlmError::Refused`; empty + no cue is
`LlmError::MalformedResponse`.

## Backends

| Backend            | Status     | Use                                      |
| ------------------ | ---------- | ---------------------------------------- |
| `MockBackend`      | P1 (live)  | Scripted responses for tests + replay.   |
| `CopilotBackend`   | P1 (stub)  | Returns `NotAuthenticated`; wiring in P2.|
| Claude API         | P2         | Drop-in `Backend` implementor.           |
| Codex / OpenAI     | P2         | Drop-in `Backend` implementor.           |

## External-language agents over WebSocket

The protocol crate now exposes a focused WebSocket foundation for agents
written outside Rust. It is deliberately separate from live
Copilot/provider backend work: `simetro-protocol::websocket` only
encodes and decodes one JSON `Envelope` per WebSocket text message,
advertises subprotocol `simetro.v1`, and rejects schema mismatches.

Expected P2/P3 flow:

1. External agent connects with `AgentMessage::Connect` and capabilities
   `external-agent`, `actions-v1`.
2. Host sends observations using existing protocol envelopes; no
   WebSocket-specific payload shape is introduced.
3. Agent replies with `AgentMessage::Action`; author actions are validated
   by the engine and invalid requests surface as `Warning::InvalidAction`.
4. Host maps disconnects/timeouts to the existing transport fault or
   agent crash warnings.

Open implementation decisions before a live host: which process owns the
WebSocket listener, auth/origin policy, heartbeat timeout, and how
observations are scheduled without awaiting network IO in the engine tick.

## WASM plugin agents (P3 design stub)

WASM plugin agents are a separate extension path from live Copilot or any
provider backend. They should run under a small plugin host that implements
the existing engine `Agent` trait boundary; the engine still sees only
`Observation -> AgentReport -> Action`.

### ABI and capability model

The first ABI should be JSON-over-linear-memory rather than Rust structs:
guests exchange UTF-8 JSON for the same observation, report, and action
contracts used by built-in and WebSocket agents. A plugin manifest declares:

| Field | Purpose |
| ----- | ------- |
| `abi` | Must be `simetro.agent.wasm.v1`. |
| `agent_id` | Stable id surfaced in Inspector, AgentLog, faults, and warnings. |
| `interval_ticks` | Same scheduling contract as built-in agents. |
| `capabilities` | Requested contracts and permissions. |

Capability strings live in `simetro-protocol::capabilities` so WebSocket,
WASM, and tests do not drift:

| Capability | Meaning |
| ---------- | ------- |
| `wasm-plugin-agent` | Guest is a sandboxed WASM plugin. |
| `observations-v1` | Guest accepts the v1 observation JSON contract. |
| `actions-v1` | Guest can emit the v1 `Action` JSON contract. |
| `author-actions-v1` | Guest requests `place_piece`, `connect_pieces`, and `remove_piece`; host policy may still deny them. |
| `agent-log-v1` | Guest can provide deterministic decision metadata for AgentLog. |

Capabilities are not ambient authority. The host intersects requested
capabilities with scene policy and local allowlists; denied or unsupported
capabilities fail closed before the plugin is scheduled.

### Sandbox failure mapping

Do not add WASM-only wire variants in v1. Map sandbox failures into the
existing typed protocol so frontend and replay tooling keep working:

| Sandbox condition | Protocol surface |
| ----------------- | ---------------- |
| Manifest ABI/schema mismatch | `FaultPayload::SchemaMismatch` when it is a protocol version mismatch; otherwise `FaultPayload::AgentCrashed`. |
| Missing exports, instantiation failure, trap, fuel/deadline/memory exhaustion | `FaultPayload::AgentCrashed { agent_id, message }`; disable that plugin instance. |
| Capability denied or action outside granted set | `WarningPayload::InvalidAction { agent_id, reason }`; sim continues. |
| Valid capability but invalid world mutation | Existing `WarningPayload::InvalidAction` from `crates/engine/src/actions.rs`. |
| Host falls behind while servicing plugin | `WarningPayload::TickOverBudget` or `WarningPayload::Behind`. |
| AgentLog sink degradation | Existing `WarningPayload::AgentLogSlow`. |

The plugin host must catch traps the same way `AgentHost` catches panics:
one bad guest can produce a typed fault or warning, but it must not unwind
through the engine tick or bypass deterministic scheduling.

## Author actions

Three `Action`s (`PlacePiece`, `ConnectPieces`, `RemovePiece`) are
exposed to the model and handled by the engine deterministically:
placing creates a node, connecting creates a path between nodes, and
removal deletes a safe node plus any unused incident paths. Invalid
requests are rejected with `Warning::InvalidAction`, which shows up in
the `WarningStrip` for transparency.

## AgentLog (PLAN §10.4 / Step 12)

Every agent decision writes an append-only JSONL line to the
AgentLog:

```jsonc
{
  "tick": 142,
  "agent_id": "trafficker",
  "observation_hash": "a1b2c3d4",   // FNV-1a over the observation bytes
  "considered": [
    { "action": "SetSpeed(1.6)", "confidence": 0.83, "chosen": true },
    { "action": "NoOp",           "confidence": 0.42, "chosen": false }
  ],
  "rationale": "crowded pickup, accelerate to clear backlog",
  "confidence": 0.83
}
```

The log is captured via `TickRunner::attach_agent_log` /
`take_agent_log`. If the sink errors, the log falls back to a
bounded in-memory ring and emits `Warning::AgentLogSlow` — slow logs
never block ticks.

`AgentHost::observe_only()` captures the exact observation that
would be sent to a backend; tests use it to assert the
`observation_hash` is stable across runs.

## Determinism

Agents are deterministic by design:
- Observation is built from world state at a known tick boundary.
- Backends are either local (Mock) or, in P2, called with
  `temperature=0` and a logged seed.
- The bridge never `await`s on a long network call inside the tick
  hot path; it returns a deferred decision that lands on the next
  agent boundary.

See `crates/engine/tests/determinism_baseline.rs` and
`tests/baselines/demo-paths.hash` for how the determinism gate
catches drift.
