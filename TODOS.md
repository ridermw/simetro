# TODOs (post-Phase-1)

Phase 1 captures plan-v4 §19. This file captures work explicitly deferred
beyond Phase 1, per plan §23 and §24.

---

## P1.5 (done)

- ✅ Animated MockTransport (3 movers, events, agent reports)
- ✅ Engine driver task in Tauri (60Hz tick, 20Hz snapshot, mpsc commands)
- ✅ TauriTransport in frontend (listen + subscribe handshake)
- ✅ Control intents bridge (pause/resume/speed/reload via invoke)
- ✅ Reload from disk (re-read JSON, rebuild world)
- ✅ Animation E2E test (pixel-diff + heartbeat, 9 E2E total)
- ✅ Docs + stale comment cleanup

---

## P2 (next after Phase 1 ships)

### LLM agent end-to-end via Copilot SDK
- **What:** First live LLM agent through the bridge, using `crates/agent-bridge` Copilot backend.
- **Why:** P1's `SpeedTuner` proves the wiring; this proves the product thesis.
- **Context:** Bridge crate + mock backend ship in P1; flip feature flag, register tool specs corresponding to `Action` enum, handle `SessionHandler` events.
- **Effort:** M
- **Priority:** P1 (of P2 backlog)
- **Depends on:** Phase 1 complete

### Author actions (PlacePiece, ConnectPieces, RemovePiece)
- **What:** Implement the action variants stubbed in `Action`.
- **Why:** Unlocks "agents AUTHOR worlds" half of the vision.
- **Context:** Engine needs entity-creation, path-attachment, removal systems; loader needs to accept partial worlds that agents fill in.
- **Effort:** L
- **Priority:** P1 (of P2 backlog)

### Resources / inventory / production chain
- **What:** Schema additions: `resources`, `producers`, `consumers`, `inventory`. Engine production/consumption systems.
- **Why:** Shapez-style and factory-style scenarios are impossible without these.
- **Context:** Schema versioning bumps to v2; loader supports v1 and v2 via auto-upgrade.
- **Effort:** L
- **Priority:** P2

### Replay tool (scrub the AgentLog)
- **What:** Headless + UI command that replays a recorded AgentLog deterministically.
- **Why:** "Decision movies" — the platform capability flagged in plan §10.
- **Context:** AgentLog format captured in P1; this is the consumer.
- **Effort:** M
- **Priority:** P2

### File watcher for live JSON reload
- **What:** Watch the open JSON file with debouncing; reload on change.
- **Why:** Tightens the iteration loop for designing scenes.
- **Context:** P1 uses manual reload.
- **Effort:** S
- **Priority:** P2

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
- **Effort:** L — **Priority:** P3

### Apple/Windows code signing + auto-updater
- **Context:** Tauri's signing pipeline + updater plugin.
- **Effort:** M — **Priority:** P3 (only if distribution becomes a goal)
