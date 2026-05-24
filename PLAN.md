# simetro — Plan v4

> Authoritative plan after Mega Plan Review (EXPANSION mode).
> Folds in all 14 adopted issues: 1A, 2A, 3A, 4A, 5A, 6A, 7A, 8A, 9A, 10A, 11A, 12A, 13A, 14A.
> v3 superseded.

---

## 1. Vision

**simetro** is a personal-use, JSON-driven, top-down simulation platform with the visual sensibility of Mini Metro and the systemic clarity of Shapez. A human watches; AI agents author and play. Everything that matters is observable, deterministic-where-possible, and shippable as a "decision movie."

The product bet: **fast, responsive, visually pleasing → everything else falls into place.**

Three core capabilities:

1. **Watch** — desktop app renders simulations at 60fps with juice (Mini Metro-class motion, audio per event).
2. **Author** — JSON describes pieces, interactions, goals, theme. Agents can also produce/edit JSON. (Author actions stubbed in P1, live in P2.)
3. **Reason** — Inspector panel shows what each agent sees, considered, chose, and why; AgentLog enables replay.

This is for me. Not for distribution. That decision is reversible later; the architecture supports public release if I ever want it.

---

## 2. Three Runtime Modes

| Mode | Binary | Use |
| --- | --- | --- |
| **Visual** | `cargo tauri dev` | Watch and operate the sim. Default. |
| **Headless** | `simetro-headless …` | Benchmarks, determinism hash, agent replay, session export, CI. |
| **Playwright** | `npm run test:e2e` | Deterministic UI testing via `?seed=N&deterministic=true&tick=N` URL params. |

Same engine binary serves all three. No code paths diverge based on mode.

---

## 3. Architecture

### 3.1 Crate layout

```javascript
simetro/
├── crates/
│   ├── engine/          # pure sim core; no IO, no LLM deps
│   ├── protocol/        # versioned wire types shared by all consumers
│   ├── agent-bridge/    # pluggable LLM backends (separate binary)
│   ├── headless/        # CLI: bench, hash, run, replay, export-session
│   └── tauri-app/       # Tauri shell hosting frontend + engine
├── frontend/            # Vite + TS + Canvas2D + Tone.js + Playwright
├── games/               # JSON scene files (demo-paths.json, stress-1k.json)
├── docs/                # architecture, schema, protocol, agents, testing, runbook, ADRs
├── tests/
│   └── baselines/       # demo-paths.hash, visual diff PNGs (per OS)
└── TODOS.md
```

### 3.2 Why this shape (Issue 9A)

`engine` is pure — no `tokio`, no `reqwest`, no `github-copilot-sdk`. The engine speaks **one versioned protocol** over multiple transports (Tauri events / WebSocket / stdio). `agent-bridge` is a **separate process/binary** that connects to the engine over the protocol and to one of several backends (Copilot CLI SDK, Mock in P1; OpenAI/Anthropic/Codex/Ollama in P2). Swapping providers = swapping backend implementations behind a `Backend` trait.

### 3.3 Top-level data flow

```javascript
                   ┌──────────────────────────────────────────────┐
                    │                ENGINE (pure)                 │
                    │                                              │
JSON scene ─────────▶  loader ──▶ World ──▶ tick() ──▶ TickOutput  │
                    │                ▲                  │  │       │
                    │                │                  │  │       │
                    │       built-in Agent              ▼  ▼       │
                    │       (SpeedTuner)         snapshot events   │
                    │                                  │   │       │
                    └──────────────────────────────────│───│───────┘
                                                       │   │
                              ┌────────────────────────┘   │
                              │                            │
                              ▼                            ▼
                         protocol message  (versioned, schema_version: u32)
                              │
              ┌───────────────┼────────────────┐
              │               │                │
              ▼               ▼                ▼
       Tauri events     WebSocket          stdio
       (frontend)       (external          (agent-bridge,
                         agents, P3)        headless replay)
                                                │
                                                ▼
                                       ┌────────────────────┐
                                       │   agent-bridge     │
                                       │  ┌──────────────┐  │
                                       │  │ Backend trait│  │
                                       │  └──────────────┘  │
                                       │  Mock | Copilot |  │
                                       │  OpenAI(P2) | …    │
                                       └────────────────────┘
```

### 3.4 Bridge architecture (Issue 9A)

- `crates/protocol/` — `SimMessage`, `AgentMessage`, `schema_version: u32` on every envelope, action specs, observation shape.
- `crates/agent-bridge/` — separate binary `simetro-bridge`. Connects to engine (stdio/WS) and to one of:
    - `MockBackend` — scripted responses, used in P1 tests
    - `CopilotBackend` — uses `github-copilot-sdk` JSON-RPC client (`Session` + `SessionHandler`)
    - `OpenAiBackend`, `AnthropicBackend`, `CodexBackend`, `OllamaBackend` — P2, feature-flagged
- `Backend` trait: `async fn invoke(prompt, tools, context) -> Result<BackendResponse, LlmError>`
- Engine never imports any LLM crate. Engine only sees the in-process `Agent` trait.
- The bridge is the boundary where prompts, tools, retries, and refusals live.

### 3.5 Mover state machine

```javascript
       ┌─────────┐  spawn   ┌─────────┐  enter path  ┌──────────┐
        │  empty  │─────────▶│ waiting │─────────────▶│ traveling │
        └─────────┘          └─────────┘              └──────────┘
                                  ▲                         │
                                  │ arrive at node          │
                                  └─────────────────────────┘

Invalid transitions: `waiting → empty` (no implicit removal),
                     `traveling → empty` (requires `Despawn` action).
Prevention: state changes are functions on a typed state field;
            no `Option<&mut Mover>` access from outside the system.
```

### 3.6 Engine run-state

```javascript
  ┌──────┐  load   ┌────────┐  start  ┌─────────┐
   │ idle │────────▶│ loaded │────────▶│ running │
   └──────┘         └────────┘         └─────────┘
       ▲                ▲                   │
       │                │ stop              │ pause
       │                └───────────────────┤
       │ fault                              ▼
       │                              ┌──────────┐
       └──────────────────────────────│ paused   │
                                      └──────────┘
```

---

## 4. Project Structure (file-level)

```javascript
crates/engine/src/
├── lib.rs                    # public API + top-level ASCII diagram
├── world.rs                  # World, TickOutput
├── components.rs             # Node, Path, Mover, etc. (renamed from ecs.rs per 8A)
├── systems/
│   ├── mod.rs
│   ├── movement.rs
│   ├── interaction.rs
│   └── lifecycle.rs
├── agent/
│   ├── mod.rs                # Agent trait + ASCII diagram (split per 8A)
│   ├── observation.rs        # Observation type
│   ├── report.rs             # AgentReport (rationale, considered, chosen)
│   └── speed_tuner.rs        # SpeedTuner built-in agent
├── loader.rs                 # JSON → World; typed LoadError
├── snapshot.rs               # encoded state for renderer
├── event.rs                  # SimEvent enum (renamed from Event per 8A)
├── error.rs                  # LoadError, AgentError, EngineFault enums (3A)
├── rng.rs                    # seeded Pcg64Mcg
└── tick.rs                   # fixed-timestep tick loop

crates/protocol/src/
├── lib.rs                    # SimMessage, AgentMessage, ASCII diagram
├── version.rs                # schema_version constants + migration map
└── tools.rs                  # action tool-specs for LLM backends

crates/agent-bridge/src/
├── lib.rs                    # bridge harness + ASCII diagram
├── backend.rs                # Backend trait
├── backends/
│   ├── mod.rs
│   ├── mock.rs               # P1
│   └── copilot.rs            # P1 stub; P2 live
└── error.rs                  # LlmError enum

crates/headless/src/
└── main.rs                   # subcommands: bench, hash, run, replay, export-session

crates/tauri-app/src/
├── main.rs                   # Tauri shell, default-deny allowlist
└── bridge.rs                 # engine ↔ Tauri events

frontend/src/
├── main.ts                   # entry + ASCII diagram
├── transport/
│   ├── tauri.ts              # Tauri event transport
│   └── mock.ts               # browser-dev transport
├── store/
│   ├── snapshots.ts          # interpolation buffer + ASCII diagram
│   └── events.ts             # event queue
├── renderer/
│   ├── canvas.ts             # Path2D batching by color
│   ├── animations.ts         # event → animation map + ASCII diagram (HMR target)
│   └── theme.ts              # palette, typography, eased curves
├── audio/
│   ├── engine.ts             # Tone.js voice pool
│   └── mappings.ts           # shape → tone
├── inspector/
│   ├── panel.ts              # decision timeline UI
│   └── hover.ts              # hover-to-explain
├── ui/
│   ├── shell.ts              # pause/speed/step/reload
│   ├── error_overlay.ts      # in-canvas JSON error rendering
│   ├── heartbeat.ts          # pulse indicator
│   ├── timeline.ts           # event-marker strip
│   └── perf_overlay.ts       # toggleable F12-style metrics
└── tests/
    └── e2e/                  # Playwright specs
```

---

## 5. JSON Schema (v1)

Every JSON game file has `schema_version: 1` at root.

```json
{
  "schema_version": 1,
  "name": "demo-paths",
  "theme": {
    "palette": ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"],
    "background_index": 0,
    "font": "system-ui"
  },
  "pieces": {
    "nodes": [ { "id": "a", "pos": [100, 100], "shape": "circle", "color": 2 } ],
    "paths": [ { "id": "ab", "from": "a", "to": "b", "color": 3 } ],
    "movers": [ { "id": "m1", "on_path": "ab", "speed": 1.0 } ]
  },
  "goals": [ { "type": "loop_forever" } ],
  "agents": [ { "kind": "speed_tuner", "interval_ticks": 30 } ]
}
```

### 5.1 Validation table (Issue 5A — security)

| Field | Type | Bound | On violation |
| --- | --- | --- | --- |
| `schema_version` | u32 | exact `1` in P1 | `LoadError::UnsupportedVersion` |
| `name` | string | ≤200 chars, no control chars | `LoadError::InvalidName` |
| `theme.palette` | array | ≤32 entries | `LoadError::PaletteTooLarge` |
| `theme.palette[i]` | string | matches `^#[0-9a-fA-F]{6}$` | `LoadError::InvalidColor` |
| `theme.background_index` | u8 | `< palette.len()` | `LoadError::PaletteIndexOOB` |
| `pieces.nodes` | array | ≤100,000 | `LoadError::TooManyPieces` |
| `pieces.paths` | array | ≤100,000 | `LoadError::TooManyPieces` |
| `pieces.movers` | array | ≤100,000 | `LoadError::TooManyPieces` |
| any `id` | string | ≤64 chars, `[a-zA-Z0-9_-]+`, unique within section | `LoadError::DuplicateId` / `InvalidId` |
| `pos` | `[f32; 2]` | finite (no NaN/Inf), within ±1e6 | `LoadError::NonFiniteCoord` |
| `speed` | f32 | finite, `0.0..=100.0` | `LoadError::SpeedOutOfRange` |
| `agents[i].interval_ticks` | u32 | `1..=10_000` | `LoadError::IntervalOOB` |

All string fields that surface in UI go through `textContent`, **never** `innerHTML` (Issue 5A).

### 5.2 IDs

JSON has string IDs for readability. Loader interns them to numeric handles (`u32`) at parse time. Engine and protocol carry numeric IDs only. Inspector translates back for display via a small `IdMap` (Issue 8A — explicit over clever).

---

## 6. Wire Protocol (Issue 1A, 4A)

`crates/protocol/src/lib.rs`:

```rust
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct Envelope<T> {
    pub schema_version: u32,
    pub seq: u64,
    pub payload: T,
}

pub enum SimMessage {
    Static(StaticPayload),           // sent once on connect: theme, ID map, palette
    Snapshot(SnapshotPayload),       // 20Hz; deltas
    Events(Vec<SimEvent>),           // semantic events per tick
    AgentReport(AgentReport),        // produced when an agent acts
    Fault(EngineFault),              // typed engine fault
    Warning(EngineWarning),          // non-fatal (tick budget, channel saturation)
}

pub enum AgentMessage {              // bridge → engine (P2 live, P1 scaffolded)
    Connect { agent_id: String, capabilities: Vec<String> },
    Action(Action),
    Heartbeat,
    Disconnect { reason: String },
}
```

Frontend on receipt checks `schema_version`; mismatch → fatal banner, do not animate (Issue 4A).

---

## 7. Semantic Events (Issue 8A — renamed)

```rust
pub enum SimEvent {
    MoverDeparted    { mover: u32, from_node: u32, path: u32 },
    MoverArrived     { mover: u32, at_node: u32, path: u32 },
    MoverSpeedChange { mover: u32, old: f32, new: f32 },
    NodeHighlighted  { node: u32, reason: HighlightReason },
    PathPulsed       { path: u32 },
    AgentDecided     { agent_id: u32, action: ActionTag },
    Tick             { tick: u64 },
}
```

Engine emits events; frontend animates from them. Snapshots carry positions for interpolation but **events drive juice**.

---

## 8. Agent Trait + Inspector Contract

```rust
pub trait Agent: Send {
    fn observe(&mut self, world: &World) -> Observation;
    fn act(&mut self, obs: &Observation) -> Result<AgentReport, AgentError>;
}

pub struct AgentReport {
    pub considered: Vec<ConsideredAction>,   // capped at 1000 per Issue 7A
    pub chosen: Option<Action>,
    pub rationale: String,                    // ≤512 chars
    pub confidence: f32,                      // 0.0–1.0
}
```

- Built-in agents (P1: `SpeedTuner`) implement `Agent` directly.
- LLM agents (P2): `LlmAgent` in engine is a thin wrapper that sends an `AgentMessage` over the protocol to the bridge, awaits, and surfaces the response as an `AgentReport`. Behind a feature flag in P1 (Issue 9A).
- `catch_unwind` around `agent.act()`; panic → `AgentReport` with `chosen: None` and `rationale: "agent panicked: <message>"`, plus a `Fault::AgentCrashed` event (Issue 2A).

---

## 9. Frontend Animation Layer

The HMR-target file. Hot-reloads in <300ms.

```typescript
// frontend/src/renderer/animations.ts
// Event → animation binding table. Edit this file to retune juice.
export const animations: Record<SimEventTag, AnimationSpec> = {
  MoverDeparted:    { duration: 200, ease: easeOutCubic, render: drawDepartFlare },
  MoverArrived:     { duration: 300, ease: easeInOutQuad, render: drawArriveRing },
  MoverSpeedChange: { duration: 150, ease: easeOutCubic, render: drawSpeedHint },
  NodeHighlighted:  { duration: 600, ease: easeOutCubic, render: drawNodePulse },
  PathPulsed:       { duration: 400, ease: easeInOutCubic, render: drawPathPulse },
  AgentDecided:     { duration: 250, ease: easeOutQuad, render: drawDecisionPulse },
  Tick:             { duration: 0, ease: linear, render: noop },
};
```

Renderer (Issue 11A):

- Single Canvas2D context.
- Batch all paths of the same color into one `Path2D`, draw once per color → \~6 draw calls for the whole scene instead of \~1000.
- No per-frame allocations: ring buffers for active animations; pre-allocated scratch vectors.

---

## 10. Agent Inspector Panel

Right-rail panel, always visible in Phase 1:

```javascript
┌─ Inspector ───────────────────────────┐
│ Agent: SpeedTuner (tick 1421)         │
│                                       │
│ ▸ Considered (3)                      │
│   · SetSpeed(m1, 1.5)  conf 0.82  ◀ chosen
│   · SetSpeed(m1, 1.0)  conf 0.61      │
│   · NoOp               conf 0.34      │
│                                       │
│ ▸ Rationale                           │
│   "m1 has been waiting at b for 200t; │
│    speeding it up should clear the    │
│    backlog at a."                     │
│                                       │
│ ▸ Recent decisions (timeline)         │
│   ●───●──●─────●──●─●─────●─── now    │
│   t1100  t1200  t1300  t1400          │
└───────────────────────────────────────┘
```

Hover any decision in the timeline → that piece pulses in the scene (delight #4).
Hover any piece in the scene → Inspector scrolls to most recent decision involving it (delight #4).

---

## 11. Error Handling (Issues 2A, 3A)

### 11.1 Typed error enums (no `anyhow` in core)

```rust
pub enum LoadError {
    Parse { line: u32, col: u32, message: String },
    UnsupportedVersion { found: u32, supported: u32 },
    InvalidName(String),
    PaletteTooLarge { size: usize, max: usize },
    InvalidColor { field: String, value: String },
    PaletteIndexOOB { field: String, index: usize, max: usize },
    TooManyPieces { section: &'static str, count: usize, max: usize },
    DuplicateId { section: &'static str, id: String },
    InvalidId { section: &'static str, id: String },
    NonFiniteCoord { id: String },
    SpeedOutOfRange { id: String, value: f32 },
    IntervalOOB { agent_index: usize, value: u32 },
    UnknownReference { from: String, to: String },
}

pub enum AgentError {
    Panicked { agent_id: String, message: String },
    InvalidAction { agent_id: String, reason: String },
    Timeout { agent_id: String, budget_ms: u32 },
}

pub enum EngineFault {
    NumericDrift { tick: u64, mover: u32 },
    BaselineHashMismatch { expected: String, found: String },
    ChannelSaturated { lag_frames: u32 },
    SystemPanic { system: &'static str, message: String },
}

pub enum LlmError {                          // lives in agent-bridge
    NotAuthenticated,
    SubprocessDied { code: Option<i32> },
    Refused { agent_id: String, message: String },
    Timeout { agent_id: String, elapsed_ms: u32 },
    RateLimited { retry_after_ms: u32 },
    MalformedResponse { agent_id: String, raw: String },
    Disconnected,
}
```

### 11.2 Typed failure events (Issue 2A)

Every error path surfaces a `SimMessage::Fault` or `SimMessage::Warning` so the UI **never silently fails**:

| Error | Surfaces as | User sees |
| --- | --- | --- |
| `LoadError::*` | `Fault::LoadError` | In-canvas overlay with field path / line:col |
| `AgentError::Panicked` | `Fault::AgentCrashed` | Inspector red row + banner "agent panicked, paused" |
| `AgentError::InvalidAction` | `Warning::InvalidAction` | Inspector yellow row, sim continues |
| `EngineFault::NumericDrift` | `Fault::NumericDrift` | Banner "numeric drift at tick N", sim pauses |
| `EngineFault::ChannelSaturated` | `Warning::Behind` | Banner "behind N frames" |
| `EngineFault::SystemPanic` | `Fault::EngineFault` | Banner "engine fault, paused"; export-session prompt |
| transport drop (Tauri/WS) | n/a (frontend-side) | Banner "transport lost, reconnecting" |
| LLM bridge errors (P2) | `Fault::AgentCrashed` w/ subtype | Inspector + banner |

### 11.3 Lint enforcement (Issue 8A)

`#![deny(clippy::unwrap_used, clippy::expect_used)]` at the crate root of `engine`, `protocol`, `agent-bridge`. Tests excepted. Enforced in CI.

### 11.4 catch\_unwind boundaries

- `Agent::act()` — wrapped in `catch_unwind`.
- Each `System::run()` — wrapped in `catch_unwind` at the tick loop level.
- Bridge backend `invoke()` — wrapped at the bridge level.

---

## 12. Security (Issues 5A, 6A)

- Tauri config: `"allowlist": { "all": false }`. No `fs`, no `shell`, no `http` for the renderer. Engine talks to disk/network on the renderer's behalf via explicit Tauri commands.
- All JSON-derived UI strings → `textContent` only. ESLint rule `no-inner-html` enforced.
- Input validation table (§5.1) is the contract.
- No PII in logs. Tracing fields are typed; we don't dump arbitrary objects.
- LLM prompts (P2): tool specs constrain output structure; refusals + malformed JSON are typed errors, not crashes.
- CI gates: `cargo audit`, `cargo deny check`, `npm audit --production`. Any High/Critical → fail.
- Secret storage: **not needed in P1** (Copilot SDK uses `gh auth`). Per-backend in `agent-bridge` for OpenAI/Anthropic/etc. when those ship in P2 (via `keyring` crate, OS keychain).

### 12.1 Trust boundaries

```javascript
  ┌─────────────────┐   trusted   ┌──────────────┐
   │ user-authored   │────────────▶│   engine     │
   │ JSON in games/  │  validated  │   (Rust)     │
   └─────────────────┘             └──────────────┘
                                          │  protocol  (versioned)
                                          ▼
   ┌──────────────────────────────────────────────┐
   │              renderer (TS)                   │
   │  - textContent for all JSON strings          │
   │  - no innerHTML                              │
   │  - no eval                                   │
   └──────────────────────────────────────────────┘

   ┌──────────────────────────────────────────────┐
   │     agent-bridge (P2 live)                   │
   │  - tool specs constrain LLM output           │
   │  - validates Action before forwarding        │
   │  - LlmError taxonomy                         │
   └──────────────────────────────────────────────┘
```

---

## 13. Data Flow Edge Cases (Issue 7A)

Each handled explicitly:

| # | Edge case | Behavior |
| --- | --- | --- |
| 1 | Empty `actions` from agent | Treat as `NoOp`; emit `AgentDecided { action: NoOp }`; Inspector shows green "no action" row |
| 2 | First snapshot before first event | Frontend renders snapshot statically; no animations until first event arrives |
| 3 | Dropped Fault banner reappears on reconnect | Banner state lives in frontend, restored from latest `Fault` seq number |
| 4 | Inspector entries unbounded | Ring buffer cap 1000 entries; UI shows "… N earlier decisions trimmed" |
| 5 | Tab backgrounded then refocused | Snapshot buffer holds last 2s; on refocus, jump-cut to latest rather than catching up animations |
| 6 | Tick budget exceeded | `Warning::TickOverBudget { ms }` emitted when tick > 16ms; perf overlay turns red |

---

## 14. Performance Targets (Issue 11A)

| Metric | P1 target | Stretch |
| --- | --- | --- |
| Demo scene (3 movers) frontend fps | 60 | — |
| Demo scene headless tps (single thread) | ≥ 50,000 | ≥ 100,000 |
| 1000-mover stress scene frontend fps | ≥ 30 | 60 |
| 1000-mover stress headless tps | ≥ 1,000 | ≥ 5,000 |
| Per-tick allocations after load | **0** (benchmark-enforced via `dhat`) | — |
| Snapshot encode (1000 movers) | < 1ms | — |
| Frontend frame time (1000 movers) | < 16ms | — |
| Tauri event channel | bounded 256; oldest-drop policy | — |
| HMR animation reload | < 300ms | — |

### 14.1 Perf budget

```javascript
  16.7ms total per frame budget
   ├── engine tick (when in-frame)   ~2ms
   ├── snapshot encode               ~0.5ms
   ├── transport hop                 ~0.5ms
   ├── interpolation + draw         ~8ms
   ├── inspector + UI               ~1ms
   ├── audio                        ~0.5ms
   └── slack                        ~4ms
```

---

## 15. Observability (Issue 12A)

- `tracing` everywhere. Structured fields. Levels: TRACE (per-tick), DEBUG (per-event), INFO (lifecycle), WARN (recoverable), ERROR (faults).
- Tracing subscriber writes to `~/.local/share/simetro/logs/run-<timestamp>.jsonl`.
- Perf overlay (toggle with backtick): tps, tick\_ms, fps, frame\_ms, draws, voices, channel\_lag, allocs (debug builds only).
- Heartbeat indicator: small pulsing dot bottom-right; pulse rate = tick rate.
- Timeline strip: bottom of window, event markers scrubable in P2.
- `simetro-headless export-session` produces a tarball:

```javascript
 session-<timestamp>/
  ├── scene.json
  ├── agent-log.jsonl
  ├── tracing.jsonl
  ├── baseline.hash
  └── manifest.json   # version, build, OS, seed, tick count
```

- AgentLog format (jsonl):

```json
 {"tick": 1421, "agent_id": "speed_tuner_0", "observation_hash": "...",
   "raw_response": "...", "parsed_action": {...}, "considered_count": 3, "rationale": "..."}
```

### 15.1 Runbook (operator response for each fault)

| Fault | Response |
| --- | --- |
| `LoadError::*` | Read field path in overlay; fix JSON; reload (Cmd-R) |
| `Fault::AgentCrashed` | Inspector shows panic message; `export-session` for repro; disable agent in JSON; reload |
| `Fault::NumericDrift` | Determinism violated; `export-session`; check seed reproducibility |
| `Fault::ChannelSaturated` | Reduce sim speed; check perf overlay for bottleneck |
| `Fault::EngineFault` | `export-session`; restart; check tracing log for system panic |
| Transport lost | Auto-reconnect; if persistent, restart app |

---

## 16. Determinism (Issue 13A)

- Engine: seeded `rand_pcg::Pcg64Mcg`, fixed-timestep, ordered system execution, no `HashMap` iteration that affects sim state (use `BTreeMap` or sorted vec where iteration order matters).
- Baseline:

```javascript
 cargo run -p simetro-headless -- hash \
    --scene games/demo-paths.json --ticks 10000 --seed 42
```

  Emits a sha256 of `(world_state_snapshot, event_stream)`. Committed to `tests/baselines/demo-paths.hash`. CI diffs against this — any drift fails the build.

- LLM determinism is **not achievable**. Instead, `AgentLog` captures `(observation, raw_response, parsed_action)` tuples; replay reads from the log, doesn't re-invoke the model.

---

## 17. Testing (Issue 10A)

### 17.1 Inventory

| Layer | Approx count | Examples |
| --- | --- | --- |
| Unit (engine) | \\~50 | loader validation per field; tick math; agent observation; SimEvent emission |
| Unit (protocol) | \\~10 | envelope encode/decode; schema\\_version mismatch; tool spec roundtrip |
| Unit (agent-bridge) | \\~10 | Backend trait mock; LlmError mapping; refusal handling |
| Unit (frontend) | \\~10 | interpolation buffer; animation table lookup; theme resolution |
| Integration (engine + headless) | \\~15 | full scene load → run N ticks → baseline hash |
| Integration (engine + bridge mock) | \\~10 | end-to-end agent loop with scripted backend |
| E2E (Playwright) | \\~8 | render scene; pause/speed/step; reload; JSON error overlay; inspector hover; audio prompt; perf overlay toggle; deterministic replay |
| Chaos | 5 | panic mid-tick; saturated channel; slow disk on AgentLog; corrupted JSON mid-load; transport drop |
| Stress | 2 | 1M tick stability smoke; 1000-mover frame stability |
| Bench | 4 | demo-paths tps; 1000-mover tps; snapshot encode; allocation count |

Total: \~125 tests.

### 17.2 Playwright determinism

URL params: `?seed=42&deterministic=true&tick=N`

- `deterministic=true` swaps real RNG for seeded.
- `tick=N` runs exactly N ticks then freezes.
- Visual diff baselines committed per OS (macOS Darwin first, others as needed).

### 17.3 Chaos tests

| Test | Inject | Expect |
| --- | --- | --- |
| `panic_mid_tick` | system that panics on tick 500 | `Fault::SystemPanic`; engine paused; no crash; export-session works |
| `saturated_channel` | flood snapshots with consumer blocked | `Warning::Behind`; oldest-drop policy active; no OOM |
| `slow_agent_log_disk` | mock filesystem with 500ms write latency | `Warning::AgentLogSlow`; logging downgrades to in-memory ring; banner |
| `corrupted_json` | truncate JSON file mid-load | `LoadError::Parse` with line/col; engine stays in `idle` state |
| `transport_drop` | kill Tauri event channel | frontend shows "transport lost" banner; auto-reconnect attempt |

---

## 18. CI / Build / Deploy (Issue 13A)

### 18.1 Pipeline

```yaml
jobs:
  rust:
    steps:
      - cargo fmt --check
      - cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
      - cargo test --workspace
      - cargo audit
      - cargo deny check
      - cargo bench --workspace (regression-tracked, not gated on absolute)
      - determinism gate:
          cargo run -p simetro-headless -- hash \
            --scene games/demo-paths.json --ticks 10000 --seed 42
          diff vs tests/baselines/demo-paths.hash → fail on mismatch

  frontend:
    steps:
      - npm ci
      - npm run lint
      - npm run typecheck
      - npm run test (unit)
      - npm audit --production
      - npm run build
      - npx playwright install --with-deps
      - npm run test:e2e (with visual diff)

  tauri:
    steps:
      - cargo tauri build (dev profile for CI sanity)
```

### 18.2 Schema versioning rules

Three schemas at v1 in Phase 1: JSON game files, wire protocol, AgentLog format. Each has its own `schema_version: u32`. Migration map in `crates/protocol/src/version.rs` (empty in P1; tested for v1→v1 identity).

### 18.3 Rollback posture

This is a desktop personal-use app. "Rollback" = `git revert` + rebuild. No DB, no migrations, no users to communicate with. Disaster scenarios:

| Scenario | Action |
| --- | --- |
| New build crashes on launch | `git checkout previous-tag && cargo tauri dev` |
| Baseline hash changed unintentionally | Bisect via determinism gate; revert offending commit |
| User-authored JSON corrupted | `LoadError` surfaces in overlay; user fixes JSON |

### 18.4 Step-by-step runnability

Each of the 22 implementation steps below produces a runnable artifact (test, binary, or visible behavior). Don't move on if the previous step doesn't run.

---

## 19. Phase 1 Implementation Order (22 Steps)

Each step ends with a passing test or a visible behavior.

1. **Workspace + crate skeletons** — `cargo check` passes across all 5 crates
2. **Lint/format config** — rustfmt, clippy lints, prettier, eslint, pre-commit; `cargo clippy -- -D warnings` clean
3. **CI workflow YAML** — push to branch runs all jobs (initially all green stubs)
4. **`crates/protocol/`** — Envelope, SimMessage, AgentMessage, SCHEMA\_VERSION; \~10 unit tests; ASCII diagram in `lib.rs`
5. **Engine core** — `components.rs`, seeded RNG, `tick.rs` with fixed timestep; empty world ticks cleanly
6. **JSON loader** — full validation table (§5.1) wired; `LoadError` typed; loader unit tests cover every error variant
7. **Movement + interaction systems** — movers traverse paths; arrival emits `MoverArrived`; bench: 50k tps demo
8. **Zero-alloc invariant** — `dhat`-based test that `tick()` makes 0 allocations after load
9. **Agent trait + SpeedTuner** — `catch_unwind` boundary; AgentReport with rationale; unit tests
10. **Snapshot + events encoding** — `serde_json` for now; per-color path batching prepared; encode tests
11. **Typed Fault/Warning events** — every error type maps to a `SimMessage::Fault` or `Warning`
12. **AgentLog writer** — jsonl append; backpressure → in-memory ring fallback
13. **Headless binary** — subcommands: `run`, `bench`, `hash`, `replay` (P2 placeholder), `export-session`
14. **Baseline hash** — commit `tests/baselines/demo-paths.hash`; CI gate active
15. **`crates/agent-bridge/`** — Backend trait, Mock backend, CopilotBackend stub; \~10 unit tests; bridge harness
16. **Frontend scaffold** — Vite + TS, mock transport for browser dev; renders single static frame
17. **Renderer** — Canvas2D + Path2D batching by color, dark theme, geometric shapes
18. **Animation layer** — easing registry; event→animation table; HMR verified <300ms
19. **Audio** — Tone.js voice pool; one-time autoplay consent; tone per shape
20. **Inspector panel** — observation, considered, chosen, rationale, timeline, hover-to-explain
21. **UI shell + overlays** — pause/speed/step/reload + JSON-error in-canvas + heartbeat + timeline strip + perf overlay
22. **Tauri integration + docs** — wrap frontend, default-deny allowlist, write 8 docs + ADRs + inline diagrams; Playwright E2E suite green; smoke test all 17 DoD items

---

## 20. Phase 1 Definition of Done

1. ✅ `cargo tauri dev` opens window; demo scene renders; movers loop smoothly
2. ✅ Every \~0.5s `SpeedTuner` makes a visible decision; Inspector updates
3. ✅ Heartbeat dot pulses; timeline strip shows event markers
4. ✅ Pause / speed (1×/2×/8×/max) / step / reload work; JSON errors render in-canvas
5. ✅ Editing `animations.ts` updates running app in <300ms
6. ✅ Editing `games/demo-paths.json` + reload updates scene
7. ✅ `headless bench --ticks 100000 --seed 42` ≥ 50k tps and matches baseline hash
8. ✅ `headless export-session` produces tarball
9. ✅ Playwright E2E (8 scenarios) passes with visual diffs
10. ✅ `cargo test --workspace && cargo clippy -- -D warnings -D clippy::unwrap_used && cargo audit && cargo deny check && npm audit --production` all green
11. ✅ Determinism baseline hash test passes in CI
12. ✅ 8 docs present (README, architecture, schema, protocol, agents, testing, runbook, ADRs)
13. ✅ 1000-mover stress at ≥30fps
14. ✅ Zero per-tick allocations after load
15. ✅ Perf overlay shows engine/transport/frontend/agent/audio metrics
16. ✅ Audio plays after consent; tone per shape on arrival
17. ✅ JSON-error overlay renders in canvas (no popup)

---

## 21. Delight Opportunities (all 8 in Phase 1)

| # | What | Where |
| --- | --- | --- |
| 1 | JSON errors in-canvas (typewriter red) | `ui/error_overlay.ts` |
| 2 | Heartbeat indicator | `ui/heartbeat.ts` |
| 3 | Timeline strip with event markers | `ui/timeline.ts` |
| 4 | Hover-to-explain (scene ↔ Inspector) | `inspector/hover.ts` |
| 5 | Decision pulse on chosen mover | `renderer/animations.ts` |
| 6 | Speed-as-time-dilation trails | `renderer/canvas.ts` |
| 7 | Tone per shape on arrival | `audio/mappings.ts` |
| 8 | Perf overlay aesthetic (same theme) | `ui/perf_overlay.ts` |

---

## 22. Documentation Deliverables (Issue 14A)

Written **during** Phase 1, not at the end:

| Doc | Owner step | Purpose |
| --- | --- | --- |
| `README.md` | Step 1 (stub) → Step 22 (complete) | Vision, run instructions, system diagram |
| `docs/architecture.md` | Step 5 → 22 | Crate map, diagrams, protocol overview |
| `docs/schema.md` | Step 6 | JSON schema reference with examples |
| `docs/protocol.md` | Step 4 | Wire format spec, versioning rules |
| `docs/agents.md` | Step 9 + 15 | How to write Agents and Backends |
| `docs/testing.md` | Step 14 | Conventions, determinism gate, baseline updates |
| `docs/runbook.md` | Step 11 | Fault → action table |
| `docs/ADRs.md` | Step 22 (rolling) | One-paragraph ADRs for: Tauri, hecs, Canvas2D, bridge split, schema versioning, Pcg64Mcg, Tone.js |

**Inline ASCII diagrams** in: `engine/lib.rs`, `engine/agent/mod.rs`, `engine/loader.rs`, `engine/components.rs`, `protocol/lib.rs`, `agent-bridge/lib.rs`, `agent-bridge/backend.rs`, `frontend/main.ts`, `frontend/renderer/animations.ts`, `frontend/store/snapshots.ts`.

---

## 23. Deferred (NOT in Phase 1)

| Item | Why deferred |
| --- | --- |
| LLM agent live end-to-end | Bridge ready; live in P2 |
| Author actions (PlacePiece/ConnectPieces/RemovePiece) | Action stubs in P1; live in P2 |
| Resources / inventory / producers / consumers | Schema v2 in P2 |
| WebGL renderer | Canvas2D suffices; switch when stress demands |
| Live JSON file watcher | Manual reload in P1 |
| Replay/scrubbing UI | AgentLog captured P1; tool in P2 |
| Visual editor | P3 |
| Multi-sim tiled view | P3 |
| External-language agents over WebSocket | Protocol ready; P3 |
| Apple/Windows code signing + auto-updater | P2 if distribution becomes a goal |
| Bezier paths, fog, weather | P2 theme expansion |
| Custom font | P2 polish |
| OpenAI/Anthropic/Codex/Ollama backends | Bridge ready; P2 |
| i18n | Never relevant unless distribution changes |

---

## 24. Phase 2 / Phase 3 Trajectory

**Phase 2** (1-3 months after P1): LLM agent end-to-end via Copilot SDK; author actions; resources + production chain; scored goals; replay tool; live JSON watcher; theme expansion; expanded audio; additional bridge backends; optional WebGL.

**Phase 3**: visual editor; multi-sim tiled view; external-language agents; WASM plugin agents; procedural scenario generation; sharable session bundles as first-class artifact.

**Phase 4+**: local model backends; multiplayer; public distribution (if ever).

---

## 25. Mega Review Adoptions (Reference)

All 14 issues adopted:

| # | Title | Section |
| --- | --- | --- |
| 1A | Versioned protocol, multiple transports | §3, §6 |
| 2A | Typed failure events | §11.2 |
| 3A | Typed error enums; no anyhow in core | §11.1 |
| 4A | schema\\_version on wire messages | §6 |
| 5A | JSON validation table + default-deny allowlist + textContent | §5.1, §12 |
| 6A | cargo audit + cargo deny + npm audit in CI; no keyring P1 | §12, §18 |
| 7A | Six edge cases explicitly handled | §13 |
| 8A | Rename SimEvent + components.rs + agent/ split + drop WorldView + clippy lints | throughout |
| 9A | agent-bridge + protocol crates as separate process/binary | §3.4 |
| 10A | Full test inventory \\~125 tests including chaos + stress | §17 |
| 11A | Zero-alloc benchmark, Path2D batching, perf overlay, bounded channels | §9, §14 |
| 12A | tracing everywhere, metrics overlay, session export, timeline+heartbeat | §15 |
| 13A | Full CI workflow, schema v1 for all three schemas, runnable-artifact-per-step | §18, §19 |
| 14A | 8 docs + ADRs + inline diagrams during P1 + P2/P3 trajectory | §22, §24 |

---

## 26. Status

- ✅ Plan v4 written
- ⏸️ Awaiting user approval to begin scaffolding (user said: do not begin)
- Next action when greenlit: Step 1 — workspace + crate skeletons