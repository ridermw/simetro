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
2. If a backend is failing, swap to
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
Expected while action policies are still narrow.

### `Warning::Behind { lag_frames, agent_id }`

**Meaning.** The frontend is rendering N frames behind the engine.
Usually a tab-switch artifact; clears on its own. When `agent_id` is
present, the lag is attributable to a specific agent (typically a
live LLM bridge that missed its reply deadline) rather than to
engine-wide pacing; investigate the named agent first.

### `Warning::TickOverBudget { ms }`

**Meaning.** A tick took longer than the 33 ms budget (tick-budget invariant).
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

## LLM bridge failure modes

These rows cover failure modes from the live LLM bridge (engine →
bridge → backend). All are mapped to engine `Fault` / `Warning`
surfaces by `simetro_agent_bridge::error_mapping::llm_error_to_message`
(see `crates/agent-bridge/src/error_mapping.rs`).

### `LlmError::NotAuthenticated` → `Fault::AgentCrashed { agent_id, message: "LLM not authenticated" }`

**Meaning.** The backend rejected the call because credentials are
missing or invalid.

**Action.** For Copilot: run `gh auth status` and re-authenticate if
needed. After fixing credentials, restart the bridge process. The
engine continues running; the agent just stops getting valid replies
until restart.

### `LlmError::RateLimited { retry_after_ms }` → `Warning::Behind { lag_frames, agent_id }`

**Meaning.** The backend told the bridge to slow down. The bridge
treats this as a transient delay; the request will be re-issued
after the deadline expires.

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
included in the warning (XPIA hardening).

**Action.** Check the AgentLog v2 row for this tick — `raw_response`
will be there (capped at 64 KiB and secret-redacted). If
this is reproducible, raise an issue with the redacted sample. The
simulation continues; the agent will try again.

### `LlmError::SubprocessDied { code }` → `Fault::AgentCrashed { agent_id: "<bridge>", message }`

**Meaning.** The `simetro-bridge` subprocess exited unexpectedly
(non-zero `code`, or `None` on signal).
The engine treats this as fatal for the bridge (the subprocess
boundary catches panics inside `Backend::invoke`, so exit-with-nonzero
typically means a stdio framing error or OS-level issue.

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
bridge crashed mid-write, restarted, and replayed). The request
lifecycle guarantees deduplication.

**Action.** None — the reply was correctly dropped. If this is
frequent, the bridge is over-eager about replay; check
`crates/agent-bridge/src/main.rs` retry logic.

### `Warning::Behind { lag_frames, agent_id }` (backpressure path)

**Meaning.** A second request for the same agent was emitted while
the first was still pending. Per the one-outstanding-per-agent
backpressure rule, the second was dropped and this warning
was emitted (`lag_frames = current_tick - source_tick`, clamped to
≥1; `agent_id` populated). The payload shape is identical to the
deadline-lag `Behind` variant — distinguish by context (a fresh
backpressure warning will fire on the same tick the second request
was attempted, before any deadline could elapse).

**Action.** Increase `interval_ticks` for the agent (it's firing
faster than the bridge can keep up), or `deadline_ticks` (the bridge
can't meet the current deadline).

## scenario_language_v1 (SL1) taxonomy

PR 14 (hardening) added an explicit operator-facing summary of every
SL1 error, warning, fault, and game outcome variant. The full
authoritative source remains the Rust enums (`Sl1LoadError`,
`Sl1Warning`, `Sl1Fault`, `GameOutcome` in
`crates/engine/src/scenario_language_v1.rs`) — this section groups
them by primitive so on-call has a quick lookup map.

### `Sl1LoadError` — load-time rejections

SL1 enforces strict-schema rejection: any unknown behavior-bearing
field, any out-of-range value, any reference to an undeclared id, or
any unsupported predicate kind produces a typed `Sl1LoadError` and
the scene fails to load. The previous running scene is preserved.

| Section | Variant prefix | Examples |
|---|---|---|
| Top-level | `UnsupportedSchema`, `UnknownField`, `ExpectedObject`, `Parse`, `PrimitiveNotImplemented`, `TooManyItems` | Schema version mismatch; unknown SL1 root key; SL1 block is not an object; serde shape error; primitive used before its PR landed; section exceeds bounded size. |
| Places (PR 1) | `Place*` (15 variants) | Duplicate id; invalid id chars; empty role; non-finite pos; storage initial > capacity; zero capacity; unsupported predicate; percent threshold > 100. |
| Links (PR 2) | `Link*` (17 variants) | Duplicate id; unknown place ref; self-loop; missing/unknown direction; missing/unknown backpressure; travel/queue ticks out of range; unknown compatibility ref. |
| Things (PR 3) | `Thing*` (14 variants) + `PlaceStorageUnknownThing` / `PlaceUnknownThingReference` | Duplicate id; empty kind/tag; freshness budget zero/out of range; quality drop percent out of range; required quality field empty/dup; storage references undeclared thing. |
| Transforms (PR 4) | `Transform*` (21 variants) | Duplicate id; unknown place/thing ref; empty outputs; zero/out-of-range cadence/duration/deadline; deadline < duration; unknown capacity key; invalid failure policy; max_attempts zero/out of range. |
| Demand (PR 5) | `Demand*` (27 variants) | Duplicate id; unknown target kind; unknown thing ref in `requires`; unsupported schedule kind; schedule field zero/out-of-range; scripted ticks not strictly increasing; invalid priority; penalty score positive (must be ≤0); penalty score out of range. |
| Pressure (PR 7) | `Pressure*` (16 variants) | Duplicate id; unknown pressure type; missing/unexpected per-variant fields; duration zero; at_tick + duration overflow; unknown target / thing / capacity bucket; multiplier out of range. |
| Objectives / failure / victory (PR 8) | `Objective*` (11), `FailureCondition*` (13), `VictoryCondition*` (6) — 30 variants combined | Duplicate id; unknown type; missing/unexpected per-variant fields; weight out of `[0, MAX_OBJECTIVE_WEIGHT]`; unknown target/thing/place state; unsupported place-state predicate; unknown referenced objective. |
| Observability (PR 9) | `Metric*` (10), `Dashboard*` (6), `Alert*` (5) — 21 variants combined | Too many items; duplicate id; unsupported metric source; unknown place/thing/dashboard ref; freshness SLO zero; alert range inverted; unsupported severity. |
| Agents (PR 10) | `Agent*` (17 variants) | Duplicate id; unknown kind; empty role; interval ticks zero/out-of-range; observation scope malformed/unknown id; allowed actions unknown kind; max_cost_per_decision zero; cooldown out of range; objective weight non-finite / out of range / unknown. |
| Milestones (PR 11) | `Milestone*` (14 variants) | Duplicate id; empty label/highlight/camera focus; unknown camera focus / highlight / pressure / metric / dashboard / dashboard state; unknown trigger predicate. |

**On-call action.** All `Sl1LoadError` variants surface as
`Fault::LoadError { field, message }`. The previous scene keeps
running. Fix the JSON, save, and the file watcher will reload
deterministically. See `docs/scenario-language-v1.md` for the
authoring template.

### `Sl1Warning` — non-blocking runtime warnings

These are amber pills in the HUD; the run continues. Operators
should investigate but the simulation has not failed.

| Variant | Meaning | Trigger |
|---|---|---|
| `TransformStarved { transform_id, tick }` | A cadence slot fired but required inputs were missing. | Upstream producer cannot keep up with consumer; capacity mismatch. |
| `TransformBlocked { transform_id, tick }` | A cadence slot fired but typed capacity / output storage was full. | Bottleneck downstream; tune `capacity_cost` or output storage. |
| `TransformLate { transform_id, tick }` | A running instance exceeded `scheduled_at + deadline_ticks`. | Duration too tight for the contention; raise `deadline_ticks` or reduce contention. |
| `TransformFailed { transform_id, tick, attempt }` | A running instance exhausted `max_attempts`. | Persistent block/late under retry policy; raise attempts or change `failure_policy`. |
| `TransformSlotMissed { transform_id, tick }` | A new cadence slot arrived while the previous one was still `Running`. | Cadence faster than duration + contention; raise cadence or reduce duration. |
| `DemandDropped { demand_id, sequence, tick, value, penalty_score }` | A demand instance was not fulfilled by its deadline. | Upstream pipeline cannot meet demand; affects score per declared `penalty.score`. |
| `DemandBacklogOverflow { demand_id, tick }` | Pending instances reached `MAX_DEMAND_OUTSTANDING`. | New spawn slot suppressed; fix the throughput hole or reduce spawn rate. |
| `PressureUnsupportedInThisPr { pressure_id, kind, tick }` | A scheduled pressure activated but its variant has no runtime effect in this build. | Pressure type is recognized at load time but its execution is not yet wired (`schema_drift`, `dashboard_storm`, etc.). |
| `ObjectiveUnsupportedInThisPr { objective_id, kind, tick }` | A recognized objective kind (`cost_budget`, `data_quality`, `query_latency`) has no runtime evaluator yet. | Objective parses fine but its progress is `Unknown` for the whole run. |

**On-call action.** Warnings are designed to never silently degrade
the run. If a scene relies on a `PressureUnsupportedInThisPr` or
`ObjectiveUnsupportedInThisPr` to stay winnable, that scene cannot
be winnable in this build — the author must remove the dependency
or wait for the implementing PR.

### `Sl1Fault` — fatal SL1 engine faults

PR 14 retains the `#[non_exhaustive]` reserved placeholder. SL1
engine faults bubble up as the existing top-level `Fault` variants
documented above (`LoadError`, `SystemPanic`, etc.). Future PRs
may add SL1-specific fault variants here.

### `GameOutcome` — terminal game state

| Variant | Wire string | Meaning |
|---|---|---|
| `InProgress` | `"in_progress"` | Game is still running. |
| `Won` | `"won"` | A victory condition fired this tick. Terminal — outcome is sticky for the rest of the replay. |
| `Lost { reason }` | `"lost"` | A failure condition fired (or an objective breach exceeded its `max_count`). Terminal — outcome is sticky. `reason` is a typed lower_snake_case label suitable for log filtering, never user-controlled HTML. |

**Monotone progression.** Once a run is `Won` or `Lost`, it stays
that way for the remainder of the deterministic tick budget.
Replay carries the outcome unchanged.

**Stable wire strings.** The `variant_str()` helper is the source
of truth for snapshot/protocol/hash baseline encoding. Don't read
`Display` output from logs and assume it's stable — match on the
enum or call `variant_str()` instead.

## Routine recovery
| Symptom                              | First action                     |
| ------------------------------------ | -------------------------------- |
| Sim looks frozen                     | Click ▶/⏸; check heartbeat       |
| Animations stuttering                | `?perf=1`, watch fps             |
| No audio                             | Click anywhere (autoplay consent) |
| Inspector empty                      | Verify at least one agent loaded |
| Scene won't load                     | Open `Fault::LoadError` overlay; fix JSON |
