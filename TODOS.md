# TODOs (post-Phase-1)

Phase 1 captures plan-v4 §19. This file captures work explicitly deferred
beyond Phase 1, per plan §23 and §24.

---

## Delivery loop policy

Use `docs/testing.md` as the source of truth for the continuous PR delivery
loop. When pulling from this backlog, size work for one independently
reviewable PR per hour where feasible. If logic work also needs scene JSON,
baselines, generated assets, or demo-world churn, use compressed paired-world
mode: keep the logic PR focused and open a companion mechanical world PR in the
same delivery window when feasible.

New follow-up TODOs created from review should include the source PR, whether
they are blocking or non-blocking, and the validation command that would prove
completion. Non-blocking review ideas should be recorded here instead of
expanding the active PR.

---

## P1.5 (done)

- ✅ Animated MockTransport (3 movers, events, agent reports)
- ✅ Engine driver task in Tauri (60Hz tick, 20Hz snapshot, mpsc commands)
- ✅ TauriTransport in frontend (listen + subscribe handshake)
- ✅ Control intents bridge (pause/resume/speed/reload via invoke)
- ✅ Reload from disk (re-read JSON, rebuild world)
- ✅ Animation E2E test (pixel-diff + heartbeat, 9 E2E total)
- ✅ Docs + stale comment cleanup
- ✅ Continuous PR delivery loop documented (`PLAN.md` §18.5 and
  `docs/testing.md`)

---

## P2 (next after Phase 1 ships / fleet follow-ups)

### LLM agent end-to-end via Copilot SDK
- **What:** First live LLM agent through the bridge, using `crates/agent-bridge` Copilot backend.
- **Why:** P1's `SpeedTuner` proves the wiring; this proves the product thesis.
- **Context:** Bridge crate + mock backend ship in P1; flip feature flag, register tool specs corresponding to `Action` enum, handle `SessionHandler` events.
- **Effort:** M
- **Priority:** P1 (of P2 backlog)
- **Depends on:** Phase 1 complete

### Author actions (PlacePiece, ConnectPieces, RemovePiece)
- **What:** Implemented deterministic node placement, path connection,
  and safe removal for the authoring `Action` variants.
- **Why:** Unlocks "agents AUTHOR worlds" half of the vision.
- **Context:** Engine validates malformed/unsafe requests and surfaces
  `Warning::InvalidAction`; future work is richer policy/UX, not the
  core action surface.
- **Effort:** Done
- **Priority:** Done

### Resources / inventory / production chain
- **What:** Implemented schema additions: `resources`, `producers`,
  `consumers`, `inventory`, plus deterministic production/consumption
  systems.
- **Why:** Shapez-style and factory-style scenarios are impossible without these.
- **Context:** Schema versioning bumps to v2; loader supports v1 and v2 via auto-upgrade.
- **Effort:** Done
- **Priority:** Done

### Replay tool (scrub the AgentLog)
- **What:** Headless command replays a recorded AgentLog deterministically
  and can emit summary, JSON, or protocol JSONL.
- **Why:** "Decision movies" — the platform capability flagged in plan §10.
- **Context:** UI scrubber remains optional polish; do not involve live agents.
- **Effort:** Done
- **Priority:** Done

### File watcher for live JSON reload
- **What:** Watch the open JSON file with debouncing; reload on change.
- **Why:** Tightens the iteration loop for designing scenes.
- **Context:** Implemented in the Tauri desktop driver for the current
  scene path; manual reload remains as a fallback.
- **Effort:** S
- **Priority:** Done

### Scene registry + safe scene switching
- **What:** Added a local scene registry and switch scenes by stable
  `scene_id` instead of hard-coded or renderer-supplied paths.
- **Why:** Enables autonomous/non-interactive launch and picker flows
  without blocking on user input or weakening path safety.
- **Context:** Registry maps ids to repo-relative `games/*.json` files.
  Build the replacement world, runner, metadata, static payload, and
  first snapshot before swapping; on unknown id or load failure, emit
  the typed fault and preserve the old scene. Avoid new dependencies
  and binary assets. If scene JSON/baseline churn is large, pair it as
  a mechanical world PR. If UX details are blocked, fall back to tests,
  docs, protocol shape, or acceptance criteria updates.
- **Effort:** Done
- **Priority:** Done

### Theme expansion (renderer plugins)
- **What:** Bezier paths, shape variants, fog, time-of-day; theme becomes a structured object.
- **Why:** Mini Metro cities are genuinely distinct; palette+font can't capture that alone.
- **Context:** Renderer is already split into `drawNodes/drawPaths/drawMovers/drawOverlays`; theme exposes plug points.
- **Effort:** L
- **Priority:** P2

### Additional bridge backends (OpenAI, Anthropic, Codex, Ollama)
- **What:** Implement `Backend` for each provider.
- **Why:** Pluggable-provider promise.
- **Context:** Each is a new file in `crates/agent-bridge/src/backends/`; feature-flagged.
- **Effort:** M per backend
- **Priority:** P2

### WebGL renderer
- **What:** Swap Canvas2D for WebGL when 1k-mover stress drops below 60fps.
- **Why:** Raises aesthetic ceiling and entity-count ceiling.
- **Context:** Renderer is isolated; swap is contained.
- **Effort:** M
- **Priority:** P3

## P3 (later)

### Visual editor for JSON sims
- **Effort:** XL — **Priority:** P3

### Multi-sim tiled view
- **Effort:** L — **Priority:** P3

### External-language agents over WebSocket
- **Context:** Protocol already wire-format-versioned and language-neutral.
- **Effort:** M (mostly docs + examples) — **Priority:** P3

### Plugin agents (WASM)
- **What:** Add a sandboxed WASM plugin host that implements the existing
  `Agent` boundary without coupling to live Copilot/provider backends.
- **Protocol foundation:** Reuse v1 observation JSON, `AgentReport`, and
  `Action`; capability strings live in `simetro-protocol::capabilities`
  (`wasm-plugin-agent`, `observations-v1`, `actions-v1`,
  `author-actions-v1`, `agent-log-v1`).
- **ABI sketch:** Manifest declares `abi = "simetro.agent.wasm.v1"`,
  `agent_id`, `interval_ticks`, and requested capabilities; guest calls
  exchange UTF-8 JSON through WASM memory rather than Rust struct layouts.
- **Fault model:** Map traps, missing exports, instantiation errors, and
  fuel/memory/deadline exhaustion to `FaultPayload::AgentCrashed`; map
  denied capabilities and invalid/unsafe actions to
  `WarningPayload::InvalidAction`; use `TickOverBudget` / `Behind` for
  host lag.
- **Effort:** L — **Priority:** P3

### Apple/Windows code signing + auto-updater
- **Context:** Tauri's signing pipeline + updater plugin.
- **Effort:** M — **Priority:** P3 (only if distribution becomes a goal)
