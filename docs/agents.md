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

## Author actions in P1

Three `Action`s (`PlacePiece`, `ConnectPieces`, `RemovePiece`) are
**exposed** to the model in P1 even though the engine rejects them
with `Warning::InvalidAction`. This teaches the model the tool shape
ahead of P2 turning them on. The warning shows up in the
`WarningStrip` for transparency.

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
