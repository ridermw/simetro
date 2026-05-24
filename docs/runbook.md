# Runbook

When the engine or frontend shows a fault or warning, this is the
operational response.

## Faults (red, blocking overlay)

### `Fault::LoadError { field, message }`

**What it means.** The scene JSON failed validation. The `field`
points at the JSON path; the `message` is the typed reason
(`UnknownPieceId`, `BadColorIndex`, `MoverOnUnknownPath`, etc.).

**Action.** Edit `games/<scene>.json`, fix the field. Click ↻
Reload (P2). In P1, restart the binary.

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

**Meaning.** An agent tool-called an `Action` the engine rejects
(currently any author action — Place/Connect/Remove — in P1).
Expected during P1.

### `Warning::Behind { lag_frames }`

**Meaning.** The frontend is rendering N frames behind the engine.
Usually a tab-switch artifact; clears on its own.

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

## Routine recovery

| Symptom                              | First action                     |
| ------------------------------------ | -------------------------------- |
| Sim looks frozen                     | Click ▶/⏸; check heartbeat       |
| Animations stuttering                | `?perf=1`, watch fps             |
| No audio                             | Click anywhere (autoplay consent) |
| Inspector empty                      | Verify at least one agent loaded |
| Scene won't load                     | Open `Fault::LoadError` overlay; fix JSON |
