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

## AgentLog (PLAN §10.4 / Step 12; v2 per spec §3.0 P2.A0.5)

Every agent decision writes an append-only JSONL line to the
AgentLog. As of P2.A0.5 the schema is **v2** (additive on v1; v1
rows are still loaded by the v2 deserializer for replay).

```jsonc
{
  "schema_version": 2,
  "tick": 142,
  "agent_id": "trafficker",
  "observation_hash": 1234567890,        // FNV-1a u64 over observation
  "parsed_action": { "kind": "set_speed", "mover": 1, "speed": 1.5 },
  "considered_count": 2,
  "rationale": "crowded pickup, accelerate to clear backlog",

  // ---- v2 additions (all optional) ----------------------------
  "raw_response": "{...}",               // capped at 64 KiB
  "truncated_bytes": null,               // original size if capped
  "backend": "copilot",                  // e.g. "copilot", "mock"
  "model": "gpt-5-mini",                 // backend-specific
  "latency_ms": 742,                     // outbox → inbox round trip
  "prompt_tokens": 1024,                 // backend-reported
  "completion_tokens": 64                // backend-reported
}
```

### v1 → v2 migration

The v2 deserializer treats absent `schema_version` as `1` (so v1
jsonl rows load cleanly). v2 rows always carry `"schema_version": 2`
explicitly on the wire so replay tooling can distinguish them. The
migration is **lossless and additive**: v1 rows simply leave all v2
fields as `None`.

Fixtures in `crates/engine/tests/fixtures/agent_log/{v1-sample,v2-sample}.jsonl`
are golden-file inputs for the `agent_log_migration` test suite,
which asserts:
- v1 fixture rows decode as `schema_version: 1` with v2 fields `None`.
- v2 fixture rows decode as `schema_version: 2` with provenance populated.
- v1 rows round-trip through serialize/deserialize stably.

### Security controls (PR #4 sec Finding 4 + P2.A0.4 §5)

The v2 writer enforces:

| Control | Implementation |
| --- | --- |
| `raw_response` size cap | 64 KiB (`RAW_RESPONSE_MAX_BYTES`); excess bytes are dropped at a UTF-8 boundary and `truncated_bytes` records the original length. |
| Schema validation before persist | `validate_entry` runs before `serde_json::to_string`; invalid rows are dropped + emit `WarningPayload::AgentLogSlow` once per run. |
| File mode 0o600 on Unix | `AgentLog::open_for_scene` uses `OpenOptions::mode(0o600)`. Windows ACL hardening deferred to spec §13. |
| Path traversal prevention | `AgentLog::open_for_scene(scene_id)` validates `scene_id` against `^[A-Za-z0-9_-]{1,64}$` (`validate_scene_id`). Path is `data_dir()/simetro/<scene_id>/decisions-<ts>.jsonl`. Scene JSON name is NEVER used in the path. |
| Secret-pattern redaction | Deferred to a tight follow-up PR; the cap above ensures unbounded secret leak via raw_response is not possible today (and bridge code that emits raw_response does not exist yet). See `docs/superpowers/analysis/p2a-security-threat-model.md` §5.3 for the required minimum 10-pattern set. |

### Path / data dir

Default platform-appropriate data dirs:
- Linux: `$XDG_DATA_HOME/simetro` or `~/.local/share/simetro`.
- macOS: `~/Library/Application Support/simetro`.
- Windows: `%APPDATA%/simetro`.

Override with `SIMETRO_DATA_DIR=/custom/path` (used by tests).

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
