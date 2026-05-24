# simetro — Post-PR-#3 Roadmap (Phase 2 + Phase 3) — Design Spec

**Date:** 2026-05-24
**Status:** Draft — awaiting user review (user is unavailable for ~6 hours)
**Authors:** Copilot CLI (Claude Opus 4.7), on behalf of @ridermw
**Supersedes:** session draft at
`/Users/mattheww/.copilot/session-state/664f56f2-47b6-44bf-8ed6-68f2180ac0f7/plan.md`

---

## 0. Process notes (brainstorming skill)

This spec was produced via the superpowers `brainstorming` skill. The user
provided design-level approvals through a structured form during the
session: `top_focus=llm_live`, `horizon=all_of_phase_2_and_3`,
`distribution=personal-use-only`, `scope_areas` (extra_backends, juice,
replay_ui, perf_webgl, world_quality, llm_live, editor, ops), and
`agent_thesis=copilot_sdk_only`. Those answers are folded into the
"Confirmed user decisions" block in §1.

The user is unavailable for ~6 hours and explicitly authorised continued
autonomous progress, stubbing out anything that requires a human in the
loop. This is captured in §2.5 as an enforceable constraint.

The brainstorming skill's terminal hand-off is the `writing-plans` skill.
Per skill discipline, that hand-off is deferred until the user has
reviewed this spec on return.

---

## 1. Problem statement

Phase 1 proved the engine, protocol, bridge scaffolding, world gallery,
and authoring/production primitives. Two product promises remain
**scaffolded but not lit up live** once PR #3 merges:

1. **Reason** — agents authoring/playing worlds end-to-end through a real
   LLM, with the inspector showing the decision movie.
2. **Watch** — scenes are visually pleasing but the renderer/theme/audio
   layer is "v1 polished" rather than "Mini-Metro-class motion." Replay
   exists only as a headless command.

Confirmed user decisions for this design:

| Decision | Value |
| --- | --- |
| Top focus | Live LLM agent end-to-end via the Copilot CLI SDK |
| Horizon | All of Phase 2 and Phase 3 |
| Distribution stance | Personal-use only; no signing / i18n / public-release track |
| Backend strategy | Copilot SDK is the only **active** live backend |
| In-scope tracks | Live LLM, replay UI, juice/theme/audio, perf + WebGL eval, world-quality, dev/ops, visual editor (P3) |
| Out-of-scope-by-choice | WebSocket external agents + WASM plugin agents (shelved, not active) |
| Sequencing | Marquee-first (Option A from the approaches list) |

### Success criteria for this design

- Live LLM agent observably decides on at least one scene without
  taking the engine down.
- Recorded sessions replay in the UI as scrubable "decision movies."
- Renderer/theme/audio v2 makes the first 30s of a showcase scene
  noticeably more pleasing than today's gallery.
- A measured WebGL decision (ADR-0009) exists, recommending switch or
  defer with numbers.
- Visual editor reaches a usable "edit a scene without leaving the app"
  state in P3.A.
- CI stays deterministic and `ci-ok`-gated; live-LLM work never runs in
  CI.

---

## 2. Approach

Treat the post-merge work as four sequential phases plus one parallel
opportunistic phase, then P3. The marquee phase (P2.A — live LLM) ships
first because every other phase either benefits from or is independent
of it.

```
  P2.A  Live LLM agent ──▶  P2.B  Replay UI + decision movies
                                          │
                                          ▼
                                P2.C  Juice / theme / audio v2
                                          │
                                          ▼
                                P2.D  Perf hardening + WebGL eval
                                          │
                                          ▼
   P3.A  Visual editor  ◀──  P3.B  Platform trajectory (deferred shelf)
                                  (extra backends, WS, WASM, multi-sim)

   P2.E  World quality + dev/ops ─── runs opportunistically in parallel
```

Each phase has acceptance criteria and a short list of PR-sized tasks.
Tasks become todos in the session SQL store so we can pick the
next-ready one without re-reading the spec.

---

## 2.5. Autonomous-window execution constraints

The user said, verbatim:

> "you will only have existing gh login to test the copilot sdk. i will
> be unavailable for the next 6 hours. if you cant do E2E testing
> without a human, you will have to stub out that part of the testing
> and move on."

Environment audit at design-time:

- `gh auth status` → logged in as `ridermw` (scopes: `gist`, `read:org`,
  `repo`, `workflow`).
- `copilot` CLI installed at `/opt/homebrew/bin/copilot` (v1.0.53-2).
- Non-interactive: `copilot -p '<prompt>' --allow-all-tools`.
- Agent Client Protocol server mode: `copilot --acp`. **This is the
  integration surface** the bridge will use as the "Copilot CLI SDK"
  backend.

**Enforceable constraints on autonomous work:**

1. **No autonomous live LLM calls.** The bridge MUST be testable
   end-to-end without ever spawning the real `copilot` subprocess. Any
   test that would burn quota or risk content policy is gated behind a
   `--features copilot-live-smoke` flag and only runs when a human
   explicitly invokes it. Mock backend is the default for CI, local
   loops, and dev.
2. **Recorded-fixture testing.** Capture synthetic ACP exchanges that
   mirror the real protocol shape and commit them under
   `crates/agent-bridge/tests/fixtures/copilot-acp/`. The Copilot
   adapter is tested via a `MockTransport` that replays the fixture
   bytes. Every error path (timeout, malformed JSON, refusal, rate
   limit, subprocess death, disconnect) gets its own fixture file.
3. **Live smoke = human-run only.** Add a `cargo xtask copilot-smoke`
   target the user runs after returning. It spawns the real
   `copilot --acp` once, sends a fixed prompt, asserts the response
   round-trips into a valid `AgentReport`, and exits. Total cost: a
   single chat completion.
4. **Stubs are explicit.** Every live-call stub gets a
   `// LIVE-CALL STUB: see spec §2.5` comment plus a row in
   `docs/agents.md`'s "Live testing" table so the user can grep on
   return.
5. **Continue past blockers.** If a task cannot progress without the
   user (e.g., Copilot entitlement issue), record it in `TODOS.md` with
   the exact repro, and switch to the next-ready task from P2.B–E. Do
   not idle.

Phases P2.B (replay UI), P2.C (juice), P2.D (perf/WebGL), P2.E (world
quality + ops), and P3.A (editor) have **zero** human-only steps.

---

## 3. Phase P2.A — Live LLM agent via Copilot SDK (marquee)

**Goal:** A Copilot-CLI-SDK-backed agent appears in the Inspector,
makes a visible decision every few hundred ticks on `metro-pulse` and
`emergency-dispatch`, and its decisions are captured in AgentLog for
replay.

### Acceptance criteria

- [ ] `cargo run -p simetro-bridge -- --backend copilot` connects to the
  engine and produces an `AgentReport` with `considered`, `chosen`,
  `rationale`, and `confidence ∈ [0,1]` from a real Copilot session
  (verified manually via the human-run smoke).
- [ ] Tool calls round-trip cleanly for every `Action` variant currently
  exposed in `crates/agent-bridge/src/tools.rs` (`no_op`, `set_speed`,
  `place_piece`, `connect_pieces`, `remove_piece`).
- [ ] Refusals, malformed JSON, timeouts, rate limits, subprocess death,
  and disconnects each surface as the correct `LlmError` variant; the
  engine emits the matching `Fault`/`Warning`.
- [ ] `gh auth status` is the only credential prerequisite. No API keys
  in config or env.
- [ ] AgentLog jsonl captures the full
  `(observation, raw_response, parsed_action)` tuple for every live
  decision; `simetro-headless replay` replays the log deterministically
  without re-invoking Copilot.
- [ ] One scene (`metro-pulse`) is wired to use the live LLM agent
  behind a feature flag; default scenes keep `SpeedTuner` so CI stays
  deterministic.
- [ ] Bridge has `catch_unwind` around `Backend::invoke` so a bad
  provider can't take down the engine.
- [ ] Determinism gate stays green: CI never invokes Copilot. Mock
  backend is used in CI; real backend lives behind `--features
  copilot-live`.

### PR-sized tasks (P2.A)

1. **Bridge: tool-spec round-trip tests** — assert every `Action`
   variant in `actions.rs` has a matching `ToolSpec` and the inline
   JSON Schema in `tools.rs` validates a known-good call. Regression
   test that fails if a new `Action` variant lands without a tool.
   **Effort:** S
2. **CopilotBackend: ACP subprocess wiring** — replace the
   `NotAuthenticated` stub with a real ACP client that spawns
   `copilot --acp` and exchanges framed JSON-RPC. Feature-flag the
   subprocess so the bridge still builds with mock-only.
   **Effort:** M
3. **Prompt + system message** — write the system prompt explaining
   the tools, observation shape, and AgentReport contract. Store in
   `crates/agent-bridge/prompts/system.md`, embedded via
   `include_str!`. **Effort:** S
4. **`LlmError` → `Fault`/`Warning` mapping** — table-driven mapping
   with a unit test per variant. **Effort:** S
5. **AgentLog v2** — extend the jsonl row with `backend`, `model`,
   `latency_ms`, `prompt_tokens`, `completion_tokens` when the backend
   reports them. Bump AgentLog `schema_version` v1 → v2 with a
   migration shim. **Effort:** S
6. **Engine `LlmAgent` wrapper** — thin in-engine `Agent` impl that
   sends `AgentMessage::Action` to the bridge and awaits the report.
   Behind feature `llm-live`. **Effort:** M
7. **Scene wiring** — `metro-pulse.json` adds an `agents` entry with
   `kind: "llm"` and `interval_ticks: 600`. Loader rejects
   `kind: "llm"` unless the runtime is built with the feature.
   **Effort:** S
8. **Recorded-fixture test suite** — capture synthetic ACP exchanges
   per §2.5 and drive the bridge through them. **Effort:** M
9. **`cargo xtask copilot-smoke`** — human-run smoke that spawns real
   `copilot --acp` once. **Effort:** S
10. **Docs** — `docs/agents.md` gets "Running with a live LLM";
    `docs/runbook.md` gets `NotAuthenticated`, `RateLimited`,
    `Timeout`, `Refused` rows. **Effort:** S

---

## 4. Phase P2.B — Replay UI scrubber (decision movies)

**Goal:** A session bundle from `simetro-headless export-session` opens
in the desktop app and plays back as a scrubable movie, with the
Inspector showing each historical agent decision in context.

### Acceptance criteria

- [ ] Frontend opens a `session-<timestamp>.tar` and treats it as a
  read-only transport (no live engine, no Copilot calls).
- [ ] Scrubber across the bottom shows event-marker density;
  click/drag seeks. Keys: `←/→` step, space pause/resume, `,`/`.`
  0.5×/2×.
- [ ] Inspector at any seek point reflects decisions known by that
  tick: considered list, chosen action, rationale.
- [ ] Existing hover-to-explain works in replay mode.
- [ ] Replay is hermetic: no network, no fs writes, no Tauri commands
  outside the existing allowlist.
- [ ] Corrupt or version-mismatched bundle → typed `LoadError` in the
  in-canvas overlay; the live scene (if any) is preserved.

### PR-sized tasks (P2.B)

1. **Session-bundle reader (TS)** — parse the tar in the renderer,
   validate manifest, expose `ReplayTransport`. **Effort:** M
2. **Scrubber UI** — `frontend/src/ui/scrubber.ts` with keyboard
   shortcuts and event-density heatmap. Paired-world for fixtures.
   **Effort:** M
3. **Inspector replay-mode** — refactor inspector to read from a
   generic `DecisionTimeline` source. **Effort:** S
4. **Tauri command: `open-session-bundle`** — gated, validates path
   and tarball, returns parsed manifest + virtual file handle.
   **Effort:** S
5. **Playwright spec** — open committed fixture bundle, scrub to tick
   N, assert visible state matches baseline screenshot. **Effort:** S
6. **Docs** — new `docs/replay.md`; update `docs/runbook.md`.
   **Effort:** S

---

## 5. Phase P2.C — Juice / theme / audio v2

**Goal:** First 30s with `metro-pulse` or `night-market-runners` looks
and feels distinctly better than today — bezier paths, time-of-day
shading, layered audio, screen-glow on big events.

### Acceptance criteria

- [ ] Theme schema bumps to v3 (additive only; v1 + v2 still load) with
  optional `path_style: "bezier" | "polyline"`, `fog`, `time_of_day`,
  `audio_layers`.
- [ ] Renderer supports bezier paths derived from control points;
  falls back to straight segments when not provided.
- [ ] Audio engine layers a slow pad + per-shape tones; voice pool ≥
  16.
- [ ] HMR target (`animations.ts`) still reloads in <300ms with the new
  effects.
- [ ] 1000-mover stress still ≥ 30fps; bezier doesn't regress headless
  tps by more than 5%.
- [ ] Two scenes (`metro-pulse`, `night-market-runners`) showcase the
  new theme. Other scenes opt in via JSON only.

### PR-sized tasks (P2.C)

1. **Theme schema v3 (additive)** — loader auto-upgrades v1/v2; world
   quality checklist updated. Paired-world. **Effort:** M
2. **Bezier path renderer** — Path2D quadratic-curve builder; perf
   test verifies no per-frame alloc. **Effort:** M
3. **Time-of-day shader** — Canvas2D composite-mode pass tied to a
   `tick` parameter (determinism preserved). **Effort:** S
4. **Audio layer system** — layered voice pool; mappings per
   `SimEvent` tag. **Effort:** M
5. **Decision-pulse v2** — `AgentDecided` gets a screen-glow accent +
   subtle audio sting; inspector pulses the row. **Effort:** S
6. **Stress gate** — bench: 1000-mover scene at ≥30fps on macOS CI
   runner with the new theme on. **Effort:** S
7. **Docs** — new `docs/theme.md` with v3 examples. **Effort:** S

---

## 6. Phase P2.D — Perf hardening + WebGL evaluation

**Goal:** A measured WebGL decision (ADR-0009) plus tightened perf
gates so future regressions are caught at PR time.

### Acceptance criteria

- [ ] `cargo bench` extended: 5000-mover headless stress; per-tick
  alloc count gate (`dhat`) wired as a tracked CI metric (alert
  at +10%).
- [ ] Frontend perf Playwright spec asserts <16ms median frame time
  on the 1000-mover scene; runs on every PR.
- [ ] WebGL spike (feature branch) renders `metro-pulse` and
  `night-market-runners` and produces a written ADR with fps,
  draw-call counts, complexity cost, and a recommendation.
- [ ] If ADR says "switch," a follow-up phase is created; if "defer,"
  Canvas2D stays.

### PR-sized tasks (P2.D)

1. **5000-mover stress fixture** — `games/stress-5k.json` + baseline
   hash. Paired-world. **Effort:** S
2. **Allocation regression gate** — `dhat` in `simetro-headless bench
   --check-allocs`; CI parses and posts a comment. **Effort:** M
3. **Frontend perf Playwright spec** — `?deterministic=true&tick=N`
   to run a fixed scenario, measure frame time over 1000 frames.
   **Effort:** M
4. **WebGL spike (branch)** — single PR introducing a WebGL renderer
   behind a runtime toggle, only for the two showcase scenes. Not
   merged unless ADR says yes. **Effort:** L
5. **ADR-0009: WebGL decision** — written from spike measurements.
   **Effort:** S

---

## 7. Phase P2.E — World quality + dev/ops polish (parallel)

Runs opportunistically alongside P2.A–D — each PR is small and
independent. Pulled from the gallery work after PR #3.

### Acceptance criteria

- [ ] World-quality checklist has a green row for every scene in
  `games/`.
- [ ] Five new scenes showcase author actions and production chains
  (e.g., `factory-line-seeds-v2`, `power-grid-balancer-hard`).
- [ ] Per-OS visual-diff baselines committed for at least one Linux +
  one macOS run.
- [ ] `.devcontainer/devcontainer.json` lands; fresh clone is one
  "Reopen in Container" away from a working environment.
- [ ] Repo hygiene: `CONTRIBUTING.md`, issue + PR templates, a labels
  manifest, `CODEOWNERS` → `@ridermw`.

### PR-sized tasks (P2.E)

1. **World-quality CI gate** — promote the existing test from
   "checklist" to "fails build if any scene regresses." **Effort:** S
2. **5 new scenes** — paired-world; each adds one gallery row + one
   checklist row. **Effort:** S each
3. **Per-OS baselines** — commit Linux baselines, document refresh
   recipe in `docs/testing.md`. **Effort:** S
4. **Devcontainer** — Ubuntu 24.04 + Rust stable + Node 20 +
   Playwright. **Effort:** S
5. **CONTRIBUTING + templates + CODEOWNERS** — single hygiene PR.
   **Effort:** S
6. **Branch protection codification** — `.github/branch-protection.md`
   documents the current ruleset so it's reviewable. **Effort:** S

---

## 8. Phase P3.A — Visual JSON editor

**Goal:** Author a scene without leaving the app. Drag nodes, connect
paths, edit production chains, save to `games/*.json`.

### Acceptance criteria

- [ ] "Edit" mode toggle splits the canvas: read-only running scene on
  the left, editable working copy on the right.
- [ ] Edits produce valid v3 JSON; loader round-trip is lossless.
- [ ] Undo/redo ≥ 100 steps; no per-edit allocation in the running
  scene.
- [ ] Save writes only inside `games/` (Tauri allowlist scoped).
- [ ] Editor cannot author scenes that fail validation (live linting
  via `validate-only` Tauri command using the engine's `LoadError`
  taxonomy).

### PR-sized tasks (P3.A — sketch; refined when P3.A starts)

1. Editor shell + mode toggle.
2. Node placement + drag.
3. Path connect with bezier control handles.
4. Production-chain inspector with sliders.
5. Save + atomic rename.
6. Validation overlay (live, debounced).
7. Playwright E2E happy-path.

---

## 9. Phase P3.B — Platform trajectory (deferred shelf)

Documented so entry points are visible. Not active work for this
roadmap.

| Track | Notes |
| --- | --- |
| Extra bridge backends (OpenAI/Anthropic/Codex/Ollama) | Drop-in `Backend` impls; per-backend OS-keychain via `keyring`. Out of active scope per `agent_thesis=copilot_sdk_only`. |
| External-language agents over WebSocket | `crates/protocol/src/websocket.rs` is the foundation; needs reference Python + TS client. Out of active scope per user. |
| WASM plugin agents | `simetro-protocol::capabilities` already reserves strings; needs sandbox host (e.g. `wasmtime` w/ epoch interrupt + fuel). Out of active scope per user. |
| Multi-sim tiled view | Reuses replay transport + Canvas2D batching; mostly UI work. |
| Code signing + auto-updater | Only if distribution stance changes; out of scope per user. |
| Sharable session bundles as first-class artifact | Layered on top of P2.B replay UI. |

---

## 10. Architecture, components, data flow

### 10.1 Component map (new + changed)

```
                    ┌───────────────────────────────────────┐
                    │              ENGINE (pure)            │
                    │  ┌──────────┐    ┌──────────────────┐ │
                    │  │ tick.rs  │───▶│   LlmAgent (NEW) │ │  feature: llm-live
                    │  └──────────┘    └────────┬─────────┘ │
                    └──────────────────────────│────────────┘
                                               │ AgentMessage
                                               ▼
                              ┌───────────────────────────────┐
                              │      simetro-agent-bridge     │
                              │                               │
                              │   Backend trait               │
                              │   ┌──────────┐ ┌──────────┐   │
                              │   │ Mock     │ │ Copilot  │   │  CHANGED
                              │   │ Backend  │ │ Backend  │   │  (real ACP)
                              │   └──────────┘ └────┬─────┘   │
                              └───────────────────│───────────┘
                                                  │ subprocess (only when
                                                  │  feature copilot-live)
                                                  ▼
                                       copilot --acp
                                       (gh-auth-gated)

   ┌─────────────────────────────────────────────────────────────┐
   │                    FRONTEND (TS)                            │
   │                                                             │
   │  TauriTransport (existing)        ReplayTransport (NEW)     │  P2.B
   │           │                                │                │
   │           └─────────────┬──────────────────┘                │
   │                         ▼                                   │
   │                  decision timeline                          │
   │                         │                                   │
   │                         ▼                                   │
   │     inspector  +  scrubber (NEW, P2.B)  +  renderer v2      │  P2.C
   │                                              (bezier, fog,  │
   │                                               time-of-day)  │
   └─────────────────────────────────────────────────────────────┘
```

### 10.2 Data flow for a live decision

1. Engine reaches `LlmAgent::act` (every `interval_ticks`).
2. Engine builds `Observation`; emits `AgentMessage::Action(Observation)`
   to the bridge.
3. Bridge `CopilotBackend::invoke`:
   - Catches the inbound message under `catch_unwind`.
   - Sends a JSON-RPC `tools/call` payload over the ACP stdio of the
     `copilot --acp` subprocess.
   - Awaits a response, validates it against the tool's JSON Schema.
4. Bridge returns `BackendResponse` to engine; engine constructs an
   `AgentReport`.
5. AgentLog v2 writer appends a row with the full tuple.
6. Frontend (live mode): inspector renders the report; events drive
   juice. Frontend (replay mode): scrubber consumes the same jsonl.

### 10.3 Schema versioning

- JSON game files: currently v2 (PR #3 adds resources/inventory). P2.C
  bumps to v3 additively.
- Wire protocol: v1; no bump expected this roadmap.
- AgentLog: v1 → v2 in P2.A (adds `backend`, `model`, `latency_ms`,
  token counts).
- Loader keeps explicit migration shims for each (`crates/engine/src/loader.rs`
  and `crates/protocol/src/version.rs`).

### 10.4 Error handling

No new top-level error enums. Reuse existing `LlmError` taxonomy
(already in `crates/agent-bridge/src/error.rs`). Mapping table for
live calls:

| Source | `LlmError` variant | Surface |
| --- | --- | --- |
| `copilot --acp` not on PATH | `NotAuthenticated` (best-effort) | `Fault::AgentCrashed` |
| `gh auth status` token has no Copilot entitlement | `NotAuthenticated` | `Fault::AgentCrashed` |
| Subprocess exits before responding | `SubprocessDied { code }` | `Fault::AgentCrashed` |
| Refusal in tool response | `Refused { agent_id, message }` | `Warning::InvalidAction` |
| > 60s no response | `Timeout { agent_id, elapsed_ms }` | `Warning::InvalidAction` |
| 429 / quota | `RateLimited { retry_after_ms }` | `Warning::Behind` |
| Non-JSON response | `MalformedResponse { agent_id, raw }` | `Warning::InvalidAction` |
| stdio EOF mid-session | `Disconnected` | `Fault::AgentCrashed` |

### 10.5 Testing strategy

| Layer | Approach |
| --- | --- |
| Engine `LlmAgent` wrapper | Unit tests with a `MockBackend` that returns scripted responses. |
| Bridge `CopilotBackend` ACP framing | Recorded fixtures under `crates/agent-bridge/tests/fixtures/copilot-acp/` — one per error-mode + one happy path. |
| Tool-spec round-trip | Property-style test that every `Action` variant has a tool and that the JSON Schema validates the canonical instance. |
| AgentLog v2 migration | Read v1 fixture, assert it upgrades cleanly; write v2, assert read-back is identity. |
| Replay UI | Playwright spec opens a committed `session-fixture.tar`, scrubs, asserts visible state. |
| Live smoke | `cargo xtask copilot-smoke` — human-run only, not in CI. |
| Determinism | Live-LLM scenes are excluded from the `demo-paths.hash` gate via scene-list allow/deny. |

---

## 11. Cross-cutting practices (carried forward)

- **PR cadence:** one independently reviewable PR per hour where
  feasible; CI must stay <8 minutes for that to hold.
- **Paired-world mode:** any scene/baseline/asset churn ships in a
  companion mechanical PR opened in the same delivery window.
- **`ci-ok` gating:** branch protection requires `ci-ok` to pass and
  all PR conversations resolved.
- **Determinism:** `tests/baselines/demo-paths.hash` and any new
  baselines diffed in CI; LLM-driven scenes excluded by feature flag.
- **Tracing + AgentLog:** every new failure mode adds a typed
  `LlmError` / `EngineFault` / `Warning` variant and a
  `docs/runbook.md` row.
- **Security:** `cargo audit`, `cargo deny check`, `npm audit
  --omit=dev` remain CI-gated; default-deny Tauri allowlist holds.

---

## 12. Open questions / risks

1. **Copilot CLI SDK API surface stability** — the CLI is young; pin a
   version and document upgrade procedure in `docs/agents.md`. *Action:*
   pin to v1.0.53 in dev docs; bump intentionally with a regression
   pass.
2. **LLM cost/quota in development** — first task in P2.A includes a
   token-cost measurement on `metro-pulse`. If per-session cost is
   uncomfortable, raise `interval_ticks` and reduce prompt size before
   going further.
3. **Determinism with live LLM** — explicitly accepted as
   non-deterministic. Replay reads the AgentLog rather than re-invoking
   the model; covered by `replay-from-fixture` test from day one.
4. **WebGL spike could swing into a full rewrite** — gated on ADR-0009
   with measurements, not enthusiasm.
5. **Editor scope creep** — most likely phase to balloon. The 7-PR
   breakdown + strict acceptance criteria are the defense.
6. **ACP protocol drift** — `copilot --acp` is still labeled preview by
   `gh copilot`. Risk that the message shape changes. *Mitigation:* an
   "ACP version probe" runs once on startup and emits a `Warning` if
   the framing differs from the recorded fixtures.

---

## 13. Status

- ✅ Spec written and committed under `docs/superpowers/specs/`.
- ⏸️ **Awaiting user review on return.** Per brainstorming-skill
  discipline, `writing-plans` is not invoked until the user reviews
  this doc.
- During the autonomous window, the existing P2.E (world quality +
  dev/ops) and the deterministic pieces of P2.A (items 1, 3, 4, 5, 7
  from §3) are eligible to start *only after PR #3 merges to main*.
  Until then, the working environment is paused on this branch.

