# simetro — Post-PR-#3 Roadmap (Phase 2 + Phase 3) — Design Spec

**Date:** 2026-05-24
**Status:** Pending — to become Active upon merge of PR #4 (P2.A0.1)
**Authors:** Copilot CLI (Claude Opus 4.7), on behalf of @ridermw
**Supersedes:** session-state brainstorming draft (not checked in)

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

Confirmed user decisions for this design (in chronological order):

| Decision | Value |
| --- | --- |
| Top focus | Live LLM agent end-to-end via the Copilot CLI SDK |
| Horizon | All of Phase 2 and Phase 3 |
| Distribution stance | Personal-use only; no signing / i18n / public-release track |
| Backend strategy | Copilot SDK is the only **active** live backend |
| In-scope tracks | Live LLM, replay UI, juice/theme/audio, perf + WebGL eval, world-quality, dev/ops, visual editor (P3) |
| Out-of-scope-by-choice | WebSocket external agents + WASM plugin agents (shelved, not active) |
| Sequencing | Marquee-first (Option A from the approaches list) |
| AgentLog v2 split | **Split out of P2.A** into a prep phase P2.A0 (per AGENTS.md "keep replay separate from provider") |
| Moratorium lift | **First** PR of P2.A0 must lift the live-LLM moratorium in `AGENTS.md` and `.github/copilot-instructions.md` so subsequent PRs aren't blocked by Copilot Code Review's standing instructions |
| Mega-review mode | **EXPANSION** — cathedral posture: pressure-test everything + push scope up where it raises the ceiling |
| Async boundary (§10) | **Outbox/inbox** (Issue 1A) — engine emits an `AgentRequest`, bridge fulfils async, engine drains at tick boundary. Determinism preserved for the non-LLM world. |
| Bridge process model | **Separate process from P2.A day 1** (Issue 2A) — matches PLAN-v4 §3.4; isolates blast radius |
| DecisionTimeline | **First-class engine object** (Issue 3) — addressable, version-pinned; replay/editor/bundle consume it; lives in `crates/protocol/` |
| LLM-as-author | **In P2.A** (Issue 4A) — add `define_resource`, `add_producer`, `add_consumer`, `set_goal` tool specs alongside the existing five |
| Merge policy this week | **Self-merge with guardrails** (see §2.6) — `ci-ok` green, no open conversations, in-scope, feature-flag default-off |
| Active-week scope | **P2.A → P2.C only** — P2.D / P2.E / P3.A queue for after user return |
| Branch strategy | **Every PR targets `main` directly** (decided post-spec via rubber-duck pass) — feature branches cut from `origin/main`, PR'd to `main`, merged, branch removed. The long-running working-branch model is **superseded**. |
| Stop conditions | **External dep unreachable** only — everything else gets retried with autonomy, captured in PR body, or queued as a follow-up |
| PR review policy | **Always request Copilot Code Review** (`@copilot review`) on every PR opened during the week |

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

## 2.6. Week-of-autonomous-execution policy

This spec is the schedule for a full working week with **no human
intervention** until the user returns. The constraints below override
any conflicting workflow heuristic; they are the actual operating
contract.

### 2.6.1 Merge policy — "self-merge with adversarial review"

The agent **may** merge its own PRs into `main` **iff all** of the
following hold simultaneously:

1. `ci-ok` is green on the PR head SHA. **(Mechanically enforced by
   branch protection.)**
2. There are **zero unresolved review conversations** on the PR.
   **(Mechanically enforced by branch protection, but see §2.7 below
   — the author can technically resolve their own conversations, so
   this gate is only as strong as the adversarial-review discipline
   that precedes it.)**
3. The change is in scope per this spec (§3-5, i.e. P2.A through
   P2.C). **(Honor-system; author judgment.)**
4. Every new capability is feature-flag-gated **default-off** so a
   regression does not corrupt the deterministic default world.
   **(Honor-system; reviewable in diff.)**
5. The PR body cites the spec section it implements and lists the
   tests added. **(Honor-system; reviewable in PR description.)**
6. The adversarial-review workflow defined in §2.7 has run on the
   final PR head SHA, with all CRITICAL and HIGH findings either
   fixed in code or documented as deliberate deferrals in the PR
   body. **(Honor-system; logged in the SQL ledger and PR body.)**

Any PR that fails any one of these gates **must NOT be merged** —
queue the failure into the PR description, post a comment, and move on
to the next ready task. Branch protection on `main` (already
configured: `ci-ok` required, conversations required,
`enforce_admins=true`, no force pushes) is the safety net behind
guardrails 1–2 only; guardrails 3–6 rely on the agent's own
discipline.

### 2.6.2 Active-week scope: P2.A → P2.C only

The week works **P2.A → P2.B → P2.C** in that order. Tasks may run in
parallel within a phase when they touch disjoint files (per the
PR-sized task lists). Do **not** start P2.D / P2.E / P3.A this week;
they wait for user return. The "do not idle" rule from §2.5 means:
within scope, pick the next ready task from the same phase or the next
phase; it does NOT mean drift into out-of-scope phases.

### 2.6.3 Branch strategy: every PR targets `main` directly

All week-of-autonomy work cuts **fresh feature branches from
`origin/main`** for each PR. The previously-proposed long-running
working branch (`ridermw/post-pr3-roadmap-spec`) is **retired** after
PR #4 (the one carrying these amendments) merges, because that model
silently bypassed branch protection on sub-PRs into the working
branch (rubber-duck CRITICAL #2 identified the gap).

Per-PR sequence:

1. From the main checkout: `git fetch origin main`.
2. `git worktree add ~/git/copilot-worktrees/simetro/<slug> -b feat/<slug> origin/main`.
3. Implement → adversarial review (§2.7) → commit → push → open PR
   against `main` → Copilot Code Review → CI → merge → remove
   worktree.

Strict serial merge: at most one code PR open at a time, with at most
one doc-only PR open in parallel (per the workspace operating contract
captured outside this spec).

### 2.6.4 Stop conditions — "external dep unreachable" only

The agent halts and posts a status comment **only** when an external
dependency it depends on is unreachable. Examples:

- `copilot --acp` subprocess refuses to start because the user's
  GitHub Copilot entitlement lapsed or `gh auth status` is logged out.
- GitHub API rate-limit exhaustion blocking PR operations.
- `npm install` / `cargo fetch` failing due to registry outage.

All other classes of failure — failing tests, flaky CI, ambiguous
requirements, design ambiguity, lint warnings, dependency conflicts —
are handled with autonomy: try alternative approaches, write the bug
into the PR body, capture the open question in
`docs/superpowers/specs/.../open-questions.md` (or as a PR comment),
and **keep moving**. The user prefers more progress with a few flagged
questions over zero progress while waiting for clarification.

### 2.6.5 Pull request review policy

**Every PR opened during this week requests Copilot Code Review.**
After `gh pr create`, post a comment containing `@copilot review` (or
add `copilot-pull-request-reviewer` as a reviewer via
`gh pr edit --add-reviewer`). When Copilot Code Review posts comments,
respond to each comment with the fix (or a justification + the
"resolve" action). Because branch protection requires conversations
resolved before merge, this is also a **technical** prerequisite, not
just a style preference.

### 2.6.6 Per-PR checklist (agent must satisfy each)

Before opening a PR:

- Build green locally (the narrowest applicable command from
  `docs/testing.md`).
- New behavior gated behind a feature flag default-off if it touches
  the engine or the bridge.
- Spec section cited in the PR body.
- Tests added: at minimum one unit + one path covered.

After opening a PR (in order):

1. Request Copilot Code Review (`@copilot review`).
2. **Run the adversarial-review workflow defined in §2.7 against the
   PR diff** before responding to any Copilot Code Review comment.
3. Fix all CRITICAL and HIGH findings from §2.7 + Copilot Code Review.
4. Watch `ci-ok` to green on the post-fix HEAD SHA.
5. Resolve all conversations Copilot Code Review opens (each must have
   a code fix or a justified TODO comment — never silent resolve).
6. Self-merge iff §2.6.1 guardrails pass.

---

## 2.7. Adversarial review workflow (mandatory per PR)

Because §2.6.1 guardrail 2 ("zero unresolved conversations") is
technically resolvable by the author themselves — i.e. Copilot Code
Review is not an independent reviewer in the same sense a separate
human is — the merge gate is only as strong as the adversarial-review
discipline that precedes it. Every PR opened during the
autonomous-execution week runs the following two-pass review locally
before any conversation is marked resolved:

### 2.7.1 Required passes

**Pass A — `code-review` sub-agent** runs on every PR regardless of
content. Inputs: the PR diff range, the PR body, the spec section
cited. The pass surfaces findings categorized
CRITICAL / HIGH / MEDIUM / LOW.

**Pass B — `security-review` sub-agent** runs on every PR that
touches any of:

- `crates/engine/`
- `crates/protocol/`
- `crates/agent-bridge/`
- `src-tauri/`
- `crates/engine/src/agent_log.rs` specifically
- any code path that spawns a subprocess, parses untrusted JSON,
  reads/writes files, or speaks to the network
- any change to `.github/copilot-instructions.md` or `AGENTS.md`
  (because those steer every future Copilot Code Review pass and
  drift in either weakens the entire week's review signal)

Pure doc / fixture / test-only PRs may skip Pass B with a one-line
justification in the PR body; standing-instructions changes never
skip it.

### 2.7.2 Disposition of findings

For each finding from passes A and B:

- **CRITICAL** — must be fixed in code on the same branch before
  merge. No deferral.
- **HIGH** — must be fixed in code OR explicitly logged as a
  deliberate deferral in the PR body, with rationale and a tracking
  entry in the SQL review-findings ledger.
- **MEDIUM** — fix or defer; deferred mediums get a TODO comment
  near the affected code citing the finding.
- **LOW / nit** — fix at author discretion; record either way.

A finding categorized "user-decision-needed" (e.g. token scope
reduction, repo settings change) gets logged as such and surfaced in
PR #4's review trail rather than treated as a blocker.

### 2.7.3 Ordering rule

The adversarial-review passes run on the **final HEAD SHA** that will
be merged. If new commits land after the passes ran, the passes must
re-run on the new HEAD. The merge button does not get pressed until
the SHA the passes ran against matches the SHA being merged.

### 2.7.4 Adversarial review of the standing instructions themselves

Any PR that modifies `AGENTS.md`, `.github/copilot-instructions.md`,
or `docs/superpowers/specs/2026-05-24-post-pr3-roadmap-design.md`
**must** include a security-review pass specifically checking whether
the change weakens any prior safety rule — for example, narrowing a
"never X" to a conditional "never X unless Y," removing a sentence
that previously guided Copilot Code Review against a specific bug
class, or shrinking the scope of a textContent/escaping rule. This
rule exists because standing-instructions drift compounds across the
entire week's PRs.

---

## 3. Phase P2.A — Live LLM agent via Copilot SDK (marquee)

### 3.0 Prep phase — P2.A0 (must merge before P2.A starts)

Two pieces have to land **before** any P2.A live-LLM PR opens. They
are small, low-risk, and unblock the marquee. Each is a separate PR.

**P2.A0.1 — Lift the live-LLM moratorium in standing instructions.**

- `.github/copilot-instructions.md` and `AGENTS.md` previously said
  "live Copilot/provider integration is intentionally deferred." That
  language conflicts with this spec's marquee. Until it is lifted,
  Copilot Code Review will block every subsequent P2.A PR.
- This PR replaces those instructions with the current direction (see
  the new `AGENTS.md` / `.github/copilot-instructions.md` that point
  to this spec), archives `PLAN.md` (Phase 1) into
  `docs/historical/`, and archives the post-Phase-1 `TODOS.md` to
  `docs/historical/`. Top-level `PLAN.md` and `TODOS.md` become
  pointers to this spec.
- **Acceptance:** `docs/superpowers/specs/2026-05-24-post-pr3-roadmap-design.md`
  is referenced from `AGENTS.md`, `.github/copilot-instructions.md`,
  `PLAN.md`, and `TODOS.md`. Phase 1 plan still readable under
  `docs/historical/PLAN-v4-phase1.md`.

**P2.A0.2 — AgentLog v2 schema bump (no provider work).**

- Bump the AgentLog jsonl `schema_version` v1 → v2.
- New fields are **all optional**: `backend`, `model`, `latency_ms`,
  `prompt_tokens`, `completion_tokens`, `raw_response` (when the
  backend exposes it). Existing v1 rows continue to load via an
  explicit migration shim that maps to v2 defaults.
- This PR adds the schema, the loader, the migration shim, the
  golden-file test (replay an existing v1 jsonl), and a v2 sample
  fixture. **No provider/network code in this PR.**
- **Acceptance:** `simetro-headless replay` works against both a v1
  log (existing fixture) and a v2 log (new fixture), bit-for-bit
  deterministic.

### 3.1 Acceptance criteria (P2.A proper)

**Goal:** A Copilot-CLI-SDK-backed agent appears in the Inspector,
makes a visible decision every few hundred ticks on `metro-pulse` and
`emergency-dispatch`, and its decisions are captured in AgentLog for
replay.

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

> P2.A0.1 (instructions lift) and P2.A0.2 (AgentLog v2 schema split)
> from §3.0 must merge **before** any of the tasks below.

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
5. **Outbox / inbox boundary** — introduce `AgentRequest` / `AgentReply`
   queues on the engine side (bounded, deterministic ordering, stable
   IDs). Engine emits to outbox at tick N and drains the inbox at tick
   N + k where k is the configured deadline (default 1; bridge can
   pre-empt with a request-marker). Documented in §10.2; backed by a
   determinism test where a stalled bridge does not perturb the
   non-LLM world's hash. **Effort:** M
6. **`simetro-bridge` as a separate process** — split bridge into a
   standalone binary spawned by the Tauri shell (or by
   `simetro-headless`). Wire protocol uses framed JSON over stdio
   with `schema_version: u32` per envelope. Reuse `crates/protocol/`
   types. **Effort:** M
7. **DecisionTimeline as first-class** — promote DecisionTimeline to a
   versioned engine object with stable ID, addressable from replay,
   editor, and bundle export. Lives in `crates/protocol/` (or a new
   tiny crate `crates/decision-timeline/`). Schema versioned the same
   way as AgentLog. **Effort:** M
8. **Engine `LlmAgent` wrapper** — thin in-engine `Agent` impl that
   sends `AgentMessage::Action` to the bridge through the outbox and
   awaits the reply from the inbox. Behind feature `llm-live`.
   **Driver.rs taste guard:** any new wiring goes through a new
   `agent_runtime` module — do not grow `src-tauri/src/driver.rs`
   further. **Effort:** M
9. **LLM-as-author tool surface** — add `define_resource`,
   `add_producer`, `add_consumer`, `set_goal` to `tools.rs` and
   `actions.rs`. The new actions mutate the world's resource/production
   graph (the existing one shipped in PR #3) via the same deterministic
   action-application pipeline. Feature-flag the new tools so they are
   only exposed in author-mode scenes. **Effort:** M
10. **Scene wiring** — `metro-pulse.json` adds an `agents` entry with
    `kind: "llm"` and `interval_ticks: 600`. Loader rejects
    `kind: "llm"` unless the runtime is built with the feature.
    **Effort:** S
11. **Recorded-fixture test suite** — capture synthetic ACP exchanges
    per §2.5 and drive the bridge through them. Every `LlmError`
    variant has its own fixture file. **Effort:** M
12. **`cargo xtask copilot-smoke`** — human-run smoke that spawns real
    `copilot --acp` once. **Effort:** S
13. **Docs** — `docs/agents.md` gets "Running with a live LLM" section
    (outbox/inbox, process model, fixtures); `docs/runbook.md` gets
    `NotAuthenticated`, `RateLimited`, `Timeout`, `Refused`,
    `MalformedJson`, `SubprocessCrash` rows. **Effort:** S

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

## 7. Phase P2.E — World quality + dev/ops polish (DEFERRED)

> **DEFERRED to user return.** §2.6.2 explicitly excludes P2.E from
> the autonomous-execution week. The content below is the planned
> shape; nothing in this section is in-scope for autonomous PRs this
> week. Previously the section header described P2.E as running
> "opportunistically alongside P2.A–D"; that wording is superseded by
> §2.6.2.

Pulled from the gallery work after PR #3. Each PR is small and
independent and will run after the user returns to active oversight.

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
                    ┌────────────────────────────────────────────┐
                    │              ENGINE (pure)                 │
                    │  ┌──────────┐    ┌──────────────────────┐  │
                    │  │ tick.rs  │───▶│   LlmAgent (NEW)      │ │  feature: llm-live
                    │  └──────────┘    └─────────┬─────────────┘  │
                    │                            │ enqueue        │
                    │                  ┌─────────▼──────────┐     │
                    │                  │  outbox (bounded,  │     │  NEW (P2.A task 5)
                    │                  │  deterministic     │     │
                    │                  │  order, stable IDs)│     │
                    │                  └─────────┬──────────┘     │
                    │                            │                │
                    │                  ┌─────────▼──────────┐     │
                    │                  │  inbox (drained at │     │
                    │                  │  tick boundary)    │     │
                    │                  └─────────▲──────────┘     │
                    └────────────────────────────│───────────────┘
                                                 │ wire protocol
                                                 │ (framed JSON over stdio,
                                                 │  schema_version: u32)
                                                 ▼
                          ┌──────────────────────────────────────────┐
                          │      simetro-bridge  (SEPARATE PROCESS)  │  NEW (P2.A task 6)
                          │                                          │
                          │   Backend trait                          │
                          │   ┌──────────┐ ┌──────────┐              │
                          │   │ Mock     │ │ Copilot  │              │  CHANGED
                          │   │ Backend  │ │ Backend  │              │  (real ACP)
                          │   └──────────┘ └────┬─────┘              │
                          └────────────────────│─────────────────────┘
                                               │ subprocess (only when
                                               │  feature copilot-live)
                                               ▼
                                    copilot --acp
                                    (gh-auth-gated)

   ┌─────────────────────────────────────────────────────────────────┐
   │                    FRONTEND (TS)                                │
   │                                                                 │
   │  TauriTransport (existing)        ReplayTransport (NEW)         │  P2.B
   │           │                                │                    │
   │           └─────────────┬──────────────────┘                    │
   │                         ▼                                       │
   │              DecisionTimeline (NEW: first-class, addressable)   │  P2.A task 7
   │                         │                                       │
   │                         ▼                                       │
   │     inspector  +  scrubber (NEW, P2.B)  +  renderer v2          │  P2.C
   │                                              (bezier, fog,      │
   │                                               time-of-day)      │
   └─────────────────────────────────────────────────────────────────┘
```

### 10.2 Data flow for a live decision (outbox/inbox boundary)

The engine never blocks on an LLM call. The async boundary is the
outbox/inbox queue pair.

```
   Tick N           Tick N+1                  Tick N+k (k ≤ deadline)
     │                 │                            │
     ▼                 ▼                            ▼
 ┌────────┐       ┌────────┐                  ┌────────────┐
 │  emit  │       │ engine │   bridge fulfils │  drain     │
 │ Agent  │──┐    │ ticks  │   in background  │ inbox →    │
 │Request │  │    │ on the │ ─ ─ ─ ─ ─ ─ ─ ─▶ │ apply      │
 │ to     │  │    │ deter- │                  │ Action     │
 │ outbox │  │    │ minist │                  │ via tools/ │
 └────────┘  │    │ ic     │                  │ actions    │
             │    │ path   │                  └────────────┘
             ▼    └────────┘                        │
       (bounded queue,                              ▼
        stable id order)                       AgentLog v2
                                                  row
```

Deterministic-world invariants:

- Non-LLM agents (e.g. `SpeedTuner`) still produce a bit-for-bit
  identical hash across runs even when the LLM bridge is stalled or
  killed. Test: `tests/determinism/llm_stalled.rs`.
- The LLM's reply is applied at a **known** tick boundary. If the
  reply arrives later than the deadline, the engine emits
  `Warning::Behind { agent_id, ticks_late }` and re-issues the
  request. No "apply at unpredictable tick" race.
- The outbox uses stable agent-IDs for ordering, never wall-clock or
  request-arrival order.

Sequence per decision:

1. Engine reaches `LlmAgent::act` (every `interval_ticks`); builds
   `Observation`; enqueues `AgentRequest` into the bounded outbox.
2. Bridge process reads the outbox over the stdio wire protocol.
3. Bridge `CopilotBackend::invoke`:
   - Catches the inbound message under `catch_unwind`.
   - Sends a JSON-RPC `tools/call` payload over the ACP stdio of the
     `copilot --acp` subprocess.
   - Awaits a response, validates it against the tool's JSON Schema.
4. Bridge writes `AgentReply` back into the wire protocol; engine's
   inbox-drainer applies it on the next tick boundary as an
   `AgentReport` + `Action` pair.
5. AgentLog v2 writer appends a row with the full
   `(observation, raw_response, parsed_action, latency_ms, model)`
   tuple, indexed by DecisionTimeline ID.
6. Frontend (live mode): inspector renders the report and the
   DecisionTimeline entry; events drive juice. Frontend (replay
   mode): scrubber consumes the same jsonl, addressed by the
   DecisionTimeline ID.

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

- ✅ Spec written, mega-reviewed (Section 0 + Section 1), committed.
- ✅ Standing instructions (`AGENTS.md`,
  `.github/copilot-instructions.md`) refreshed to point at this spec
  and to require Copilot Code Review on every PR.
- ✅ Phase 1 plan archived to `docs/historical/PLAN-v4-phase1.md`;
  post-Phase-1 backlog archived to
  `docs/historical/TODOS-post-phase1.md`. Top-level `PLAN.md` and
  `TODOS.md` are now pointers.
- ✅ PR #3 merged; branch protection (`ci-ok` + conversations
  resolved + admins enforced) active on `main`.
- ▶️ **PR #4 is the final PR cut from `ridermw/post-pr3-roadmap-spec`.**
  After PR #4 merges, the branch is **retired** (deleted on remote
  + local) per §2.6.3, and every subsequent PR cuts a fresh feature
  branch from `origin/main`. The "single long-running working branch"
  language that previously appeared in this section is **superseded**
  by §2.6.3.
- ⏳ Mega plan review Sections 2–10 are scheduled but **deferred to
  user return** so they do not block week-of-autonomy execution.
  Section 1 decisions (outbox/inbox, separate bridge process,
  DecisionTimeline first-class, LLM-as-author in P2.A) are
  authoritative for week-of-autonomy work.
- ▶️ **Next concrete action (after PR #4 merges):** retire the
  `ridermw/post-pr3-roadmap-spec` branch (delete on remote + local),
  sync the main checkout to the post-merge `origin/main`, then cut a
  **fresh feature branch from `origin/main`** for the next PR. The
  next PR carries the rubber-duck and adversarial-review derived
  prerequisites the spec does not yet enumerate in §3.0:
  - the `hash_run` widening to cover `runner.messages()` (so stalled
    bridges cannot leak nondeterminism past the determinism gate),
  - sorting `TickRunner` agent hosts by stable `agent_id` (so multi-
    agent same-tick ordering is reproducible across registration
    order changes),
  - extending `WarningPayload::Behind` with an optional `agent_id`
    field (additive, no schema bump),
  - the spec amendments that formalize the §10.2 request lifecycle
    `{timeline_id, agent_id, source_tick, attempt}` and add §3.0
    entries for itself plus two pre-P2.A analysis PRs derived from
    the user's plan-mode decisions during the rubber-duck pass:
    mega-review §2 (Error & Rescue Map) and mega-review §3 (Security
    & Threat Model) promoted from "deferred" to "pre-P2.A blocking"
    (see §14 "Post-rubber-duck plan-mode decisions" below).

  That PR runs the full §2.7 adversarial-review workflow and merges
  per §2.6.1. After it lands, §3.0 will list **five** prep PRs
  (P2.A0.1 ← this PR, the amendments PR, the two pre-P2.A analysis
  PRs, and P2.A0.2 AgentLog v2 schema), all of which must merge
  before any P2.A implementation PR opens.

---

## 14. Mega plan review trail

Decisions resolved (committed into the body above):

| § | Decision | Outcome |
| --- | --- | --- |
| Step 0 | Mode | EXPANSION (user pick) |
| Step 0 | 10x | LLM-as-author tool surface (folded into §3) |
| Step 0 | Delight | 5+ identified; tracked under §3 task 9 + §5 |
| Step 0 | Taste | `crates/engine/src/actions.rs` chosen as style reference; `src-tauri/src/driver.rs` flagged as anti-pattern (§3 task 8 guard) |
| §1 | Async boundary | Outbox/inbox (1A) — §10.2 |
| §1 | Process model | Separate bridge process day 1 (2A) — §10.1, §3 task 6 |
| §1 | DecisionTimeline | First-class object (3) — §3 task 7 |
| §1 | LLM-as-author | In P2.A (4A) — §3 task 9 |
| §2.5 | Autonomy live-call policy | Confirmed: recorded fixtures + human smoke |
| §2.6 | Merge policy | Self-merge with adversarial review (§2.6.1 + §2.7) |
| §2.6 | Scope this week | P2.A → P2.C only |
| §2.6 | Branch strategy | Every PR targets `main` directly (superseded original "single long-running working branch" model — see §2.6.3) |
| §2.6 | Stop conditions | External dependency unreachable only |
| §2.6 | PR review policy | Copilot Code Review on every PR |

### Post-rubber-duck plan-mode decisions (record only — body amendments tracked in the next PR)

The following decisions came from the user's plan-mode answers
during/after the rubber-duck pass and are documented here so this
PR's §13 / §3.0 references stay coherent. They are NOT yet
structurally encoded in §3.0; the next PR (amendments PR — see §13)
will add §3.0 entries for them.

| § | Decision | User pick | Effect |
| --- | --- | --- | --- |
| §2.6.1 | Merge gate stance | `self_merge_with_adversarial_review` | Codified in §2.6.1 + §2.7 in this PR. |
| Mega-review §2 + §3 | Pre-P2.A security/error gate | `run_min_security_and_error_map_before_p2a` | **PROMOTED** out of "deferred to user return"; now pre-P2.A blocking. Two analysis PRs queued (one per mega-review section). |
| §3 / §4 / §5 | PR granularity | `phase_sized_prs` | ~25 PRs across P2.A → P2.C; per §3 task list. |
| §2.6.3 | Branch model | `every_pr_targets_main` | Codified in §2.6.3 in this PR. |
| §2.7 | Independent-PR exception | `doc_only_pr_can_overlap` | Process detail tracked outside spec (workspace contract). |
| §2.7 | Wait-idle policy | `prep_next_pr_readonly` | Process detail tracked outside spec. |
| §2.7 | Hard-stuck protocol | 5 strikes (a strike = issue count unchanged round-over-round; decreasing is acceptable) | Process detail tracked outside spec. |
| §2.5 | ACP ground truth | User capturing one real `copilot --acp` happy-path exchange | Required at `crates/agent-bridge/tests/fixtures/copilot-acp/captured-happy-path.jsonl` before P2.A task 2 (real ACP wiring) starts. |

Sections still deferred to user return (not blocking execution):

- §4 Data Flow & Interaction Edge Cases (mega-review)
- §5 Code Quality Review (mega-review)
- §6 Test Review (test pyramid + chaos scenarios)
- §7 Performance Review (mega-review)
- §8 Observability & Debuggability Review ("joy to operate" pass)
- §9 Deployment & Rollout Review (mega-review)
- §10 Long-Term Trajectory Review (mega-review)

Mega-review §2 and §3 were originally on this deferred list but
were **promoted to pre-P2.A blocking** by the user's plan-mode
decision above; they now have analysis PRs queued before any P2.A
implementation work begins.

Each PR opened during the autonomous week will include its own
section-by-section mini-review in the PR body so the user can pick up
the mega-review thread on return without losing context.
