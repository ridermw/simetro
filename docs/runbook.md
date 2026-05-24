# Runbook

When the engine or frontend shows a fault or warning, this is the
operational response.

## Faults (red, blocking overlay)

### `Fault::LoadError { field, message }`

**What it means.** The scene JSON failed validation. The `field`
points at the JSON path; the `message` is the typed reason
(`UnknownPieceId`, `BadColorIndex`, `MoverOnUnknownPath`, etc.).

**Action.** Edit `games/<scene>.json` and fix the field. The desktop
shell reloads the watched scene automatically after the file is stable;
↻ Reload is still available as a manual fallback. A failed reload does
**not** discard the current running scene; the driver keeps the old
world active until a replacement validates.

For future scene switching, an unknown `scene_id` is handled the same
way: show the typed fault, keep the old scene, and resolve the id via
the local scene registry rather than asking for a path.

### `Fault::AgentCrashed { agent_id, message }`

**What it means.** The bridge surfaced an unrecoverable
`LlmError` (transport, malformed response after retries, etc.).
The agent is removed from the active set; the engine keeps ticking.

**Action.**
1. Check `agent_log.jsonl` for the last few decisions; look for a
   pattern.
2. If a backend (Copilot, Claude, …) is failing, swap to
   `MockBackend` in the bridge config to keep the sim running.
3. File an issue with the agent_id and the message.

### `Fault::NumericDrift { tick, mover }`

**What it means.** The determinism gate caught a divergence between
two runs of the same scene + seed + agent trace. Bit-identical is
gone.

**Action.** **DO NOT** ship until resolved. Run:
```bash
cargo run -p simetro-headless --release -- hash games/<scene>.json
diff against tests/baselines/<scene>.hash
```
If a recent commit changed an event payload, an iteration order,
or a numeric formula, that's your culprit. Either revert or
regenerate the baseline (see `testing.md`) AND write an ADR.

### `Fault::ChannelSaturated { lag_frames }`

**What it means.** The transport ringbuffer is full; messages are
being dropped. Almost always a debug-mode artifact.

**Action.** Build with `--release`. If it persists, profile the
frontend with the `?perf=1` overlay and the browser DevTools
timeline.

### `Fault::SystemPanic { system, message }`

**What it means.** A Rust system panicked. The engine caught it
and reported. This is a bug.

**Action.** Capture the stack from the engine log, file an issue
with the scene JSON and the panic message.

### `Fault::SchemaMismatch { found, supported }`

**What it means.** The engine and the desktop shell were built
from different protocol versions.

**Action.** Rebuild both from the same commit:
```bash
cargo build --workspace --release
cd frontend && npm run build
```

## Warnings (amber, non-blocking pill)

### `Warning::InvalidAction { agent_id, reason }`

**Meaning.** An agent tool-called an `Action` the engine rejects, such
as a malformed or unsafe author action (Place/Connect/Remove; see
[`agents.md` § Author actions](agents.md#author-actions)).
Expected during P1 while policies are still narrow.

### `Warning::Behind { lag_frames, agent_id }`

**Meaning.** The frontend is rendering N frames behind the engine.
Usually a tab-switch artifact; clears on its own. When `agent_id` is
present, the lag is attributable to a specific agent (typically a
live LLM bridge that missed its reply deadline) rather than to
engine-wide pacing; investigate the named agent first.

### `Warning::TickOverBudget { ms }`

**Meaning.** A tick took longer than the 33 ms budget (PLAN §13 #6).
Single-tick spikes are harmless; sustained means an algorithmic
regression. Profile with `cargo run -p simetro-headless --release
-- bench games/<scene>.json`.

### `Warning::AgentLogSlow`

**Meaning.** The agent-log writer fell behind and degraded to the
in-memory ring buffer. No data lost yet, but the next batch may be.

**Action.** Check disk space and inode quota. The log file is at
the path supplied to `attach_agent_log`.

## Heartbeat states

| Color  | State | Meaning                                            |
| ------ | ----- | -------------------------------------------------- |
| green  | ok    | Snapshot received within the last second.          |
| amber  | stale | No snapshot for 1–3 seconds.                       |
| red    | dead  | No snapshot for ≥ 3 seconds; engine has stopped.   |

A dead heartbeat with no fault is a transport bug. Restart the
desktop shell.

## LLM bridge failure modes (P2.A)

These rows cover failure modes from the live LLM bridge (engine →
bridge → backend). All are mapped to engine `Fault` / `Warning`
surfaces by `simetro_agent_bridge::error_mapping::llm_error_to_message`
(see `crates/agent-bridge/src/error_mapping.rs`).

### `LlmError::NotAuthenticated` → `Fault::AgentCrashed { agent_id, message: "LLM not authenticated" }`

**Meaning.** The backend (Copilot SDK / Anthropic / OpenAI / etc.)
rejected the call because credentials are missing or invalid.

**Action.** For Copilot: run `gh auth status` and re-authenticate
with `gh auth login --scopes copilot`. For other backends, check the
relevant env var or OS keychain entry. After fixing credentials,
restart the bridge process (the engine continues running; the agent
just stops getting valid replies until restart).

### `LlmError::RateLimited { retry_after_ms }` → `Warning::Behind { lag_frames, agent_id }`

**Meaning.** The backend told the bridge to slow down. The bridge
treats this as a transient delay; the request will be re-issued
after the deadline expires per spec §10.2.1.

**Action.** Usually no action — the simulation continues. If the
warning sustains, you may be hitting your account-level rate limit;
upgrade tier or use a different backend. The retry-after delay is
emitted into the AgentLog row's `latency_ms`.

### `LlmError::Timeout { elapsed_ms }` → `Warning::Behind { lag_frames, agent_id }`

**Meaning.** The bridge did not receive a reply within the
configured deadline (default 1 tick @ 60 Hz ≈ 16 ms, configurable
per agent via `deadline_ticks`).

**Action.** First, check the backend's status page. If chronic, raise
`deadline_ticks` for the affected agent in the scene JSON. If
re-issue runs out of attempts (`MAX_ATTEMPTS = 2`, so 3 total tries),
the runtime gives up on that decision and emits
`Warning::InvalidAction { agent_id, reason: "max attempts (2) exceeded" }`.

### `LlmError::Refused { agent_id, message }` → `Warning::InvalidAction { agent_id, reason }`

**Meaning.** The model declined the request (safety classifier,
policy filter, etc.). The bridge forwards the model's full `message`
into the warning's `reason` field. There is no explicit length cap
today — keep this in mind if you alert on warning payload size.

**Action.** Usually scene-level: the prompt or observation may be
triggering a content filter. Check the recent observation in the
inspector. The simulation continues; the agent will try again on
its next `interval_ticks`.

### `LlmError::MalformedResponse { agent_id, raw }` → `Warning::InvalidAction { agent_id, reason: "LLM returned malformed response" }`

**Meaning.** The model returned text that did not parse as a valid
tool call (e.g. invalid JSON, schema mismatch). The reason string is
a hardcoded constant — by design, the `raw` field is **never**
included in the warning (XPIA hardening, spec §7.1).

**Action.** Check the AgentLog v2 row for this tick — `raw_response`
will be there (capped at 64 KiB, secret-redacted per spec §5.3). If
this is reproducible, raise an issue with the redacted sample. The
simulation continues; the agent will try again.

### `LlmError::SubprocessDied { code }` → `Fault::AgentCrashed { agent_id: "<bridge>", message }`

**Meaning.** The `simetro-bridge` subprocess exited unexpectedly
(non-zero `code`, or `None` on signal).
The engine treats this as fatal for the bridge (the subprocess
boundary catches panics inside `Backend::invoke` per spec §3.1, so
exit-with-nonzero typically means a stdio framing error or OS-level
issue).

**Action.** Check `~/Library/Logs/simetro/bridge.log` (macOS) or
`~/.local/state/simetro/bridge.log` (Linux). The bridge is restarted
automatically by the parent (Tauri shell or headless runner) within
~1 second; transient crashes self-heal. If chronic, capture the log
and the engine's `AgentLog` for the surrounding ticks before filing
a bug.

### `LlmError::Disconnected` → `Fault::AgentCrashed { agent_id: "<bridge>", message: "backend disconnected" }`

**Meaning.** The bridge lost its connection to the backend
(e.g. WebSocket dropped, stdio pipe broken without subprocess exit).
The engine treats this as a bridge fault.

**Action.** Same as `SubprocessDied`. The parent runner reconnects
within ~1 second.

### `Warning::InvalidAction { agent_id, reason: "duplicate reply" }`

**Meaning.** A reply arrived for a request whose ID is already in
the `completed` ring. This is a bridge-replay artifact (e.g. the
bridge crashed mid-write, restarted, and replayed). Spec §10.2.1
guarantees deduplication.

**Action.** None — the reply was correctly dropped. If this is
frequent, the bridge is over-eager about replay; check
`crates/agent-bridge/src/main.rs` retry logic.

### `Warning::Behind { lag_frames, agent_id }` (backpressure path)

**Meaning.** A second request for the same agent was emitted while
the first was still pending. Per spec §10.2.1 "one-outstanding-per-
agent backpressure" rule, the second was dropped and this warning
was emitted (`lag_frames = current_tick - source_tick`, clamped to
≥1; `agent_id` populated). The payload shape is identical to the
deadline-lag `Behind` variant — distinguish by context (a fresh
backpressure warning will fire on the same tick the second request
was attempted, before any deadline could elapse).

**Action.** Increase `interval_ticks` for the agent (it's firing
faster than the bridge can keep up), or `deadline_ticks` (the bridge
can't meet the current deadline).

## Routine recovery
| Symptom                              | First action                     |
| ------------------------------------ | -------------------------------- |
| Sim looks frozen                     | Click ▶/⏸; check heartbeat       |
| Animations stuttering                | `?perf=1`, watch fps             |
| No audio                             | Click anywhere (autoplay consent) |
| Inspector empty                      | Verify at least one agent loaded |
| Scene won't load                     | Open `Fault::LoadError` overlay; fix JSON |
