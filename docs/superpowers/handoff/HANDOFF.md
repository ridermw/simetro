# Autonomous-week handoff (2026-05-24)

**Author:** prior Copilot CLI agent session (closed)
**For:** next agent picking up the autonomous PR queue
**Source of truth for the plan:** [`docs/superpowers/specs/2026-05-24-post-pr3-roadmap-design.md`](../specs/2026-05-24-post-pr3-roadmap-design.md)
**Source of truth for what shipped:** [`pr-ledger.md`](./pr-ledger.md) (also in this folder)

---

## 1-minute pitch

The simetro project has an autonomous-execution week underway. The user is
unavailable for extended periods and expects a Copilot CLI agent to advance
a defined PR queue, follow the §2.7 adversarial-review workflow on every PR,
commit + push frequently, and report progress as a burn-down between PRs.

The prior session merged **23 PRs into `main`** (`#4`–`#24`, excluding `#17`
which was a user-driven frontend fix). All P2.A engine + bridge work is done.
Only 4 items remain on the queue.

---

## 2. Operating contract (DO NOT DEVIATE)

These are the user's explicit rules. They were honored throughout the prior
session and produced clean, mergeable PRs.

### 2.1 Workflow per PR

1. **Worktree per PR**, not the main checkout:
   ```bash
   git worktree add ../copilot-worktrees/simetro/<slug> -b feat/<slug>
   ```
   Worktrees live under `~/git/copilot-worktrees/simetro/`. Per-user.

2. **Every PR targets `main` directly.** No long-running working branch.

3. **Local validation BEFORE push** (the pre-commit hook enforces fmt + clippy):
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   RUSTDOCFLAGS="-D warnings" cargo doc -p <crate> --no-deps
   cargo test --workspace --all-targets
   ```
   Frontend changes additionally need:
   ```bash
   cd frontend
   npm run typecheck && npm run lint && npm test && npm run format:check
   ```

4. **PR description format** (see the existing 23 merged PRs for templates):
   - Spec ref (e.g. "Implements spec §3 task N")
   - "What lands" section
   - Validation summary (test counts, lint/doc status)
   - "Scope NOT in this PR" so reviewers can short-circuit out-of-scope nits
   - Always end with `cc @copilot please review.`

5. **Adversarial review per §2.7** (see [spec §2.7](../specs/2026-05-24-post-pr3-roadmap-design.md#26-merge-policy)):
   - Dispatch BOTH `code-review` AND `security-review` sub-agents per HEAD
   - On every new HEAD (force-push / bot fix / fix commit), re-run BOTH
   - Wait for Codex bot review on each HEAD too (it usually arrives within 60s of CI completing)
   - Wait for Copilot Code Review when it posts (intermittent — sometimes it doesn't)
   - Resolve every review thread (reply + `resolveReviewThread` mutation)
   - **5-strikes-then-pivot rule** never tripped in 23 PRs but is in spec §2.7

6. **Merge gate** = `mergeStateStatus: CLEAN` (CI green + 0 unresolved threads + branch protection passes).
   Squash-merge: `gh pr merge <N> --squash`.

7. **After merge:**
   - `git fetch --prune origin`
   - `git checkout main && git pull --ff-only`
   - `git worktree remove -f <path>`
   - `git branch -D feat/<slug>`
   - Update the SQL `pr_ledger` row to `status='merged'` with the merge sha
   - Post a **burn-down** showing what's left (this is a user-stored
     workflow rule — see [§3.2](#32-burn-down-format) below).

### 2.2 Reviewer dispatch (background sub-agents)

```bash
# In parallel — both are independent:
#   task --agent_type code-review     --mode background --name pr<N>-code-review-r1
#   task --agent_type security-review --mode background --name pr<N>-security-review-r1
```

Prompt template (working pattern from prior session):

> Review on PR #N (HEAD `<sha>`, worktree `<path>`).
> [Spec ref + what changed]
> Verify [3–7 specific invariants].
> Output: ✅ "No blocking findings." or ⚠️ "N findings: ..."
> Terse. Do not modify code.

### 2.3 Common CI gotchas (caught the hard way)

- **`cargo doc -D warnings`** rejects `[`some/path`]` intra-doc links. Use plain backticks ``` `path` ``` for non-Rust paths.
- **Bot-authored commits trigger `action_required`** workflow approval. Push an empty commit from owner account to re-trigger:
  ```bash
  git commit --allow-empty -m "ci: re-trigger after bot-authored refactor"
  ```
- **`required_conversation_resolution: true`** in branch protection. Outdated threads must still be `resolveReviewThread`d explicitly.
- **GitHub secret-scanning push-protection** rejects test fixtures that look like real tokens. Use synthetic shapes (e.g. `AKIA0Z0Z0Z0Z0Z0Z0Z0Z`, `ghs_ZzZz...`) — never real-doc-canonical examples like `AKIAIOSFODNN7EXAMPLE`.
- **`HeaderMode::Deterministic` in tar 0.4** only zeroes uid/gid/uname/gname, NOT mtime. Use hand-built `tar::Header` for reproducibility (see PR #24).
- **macOS `cargo deny check` advisories** locally errors on CVSS 4.0 parse — local install too old. CI version handles it. Run `cargo deny check bans licenses sources` locally; let CI handle advisories.

### 2.4 Git commit trailer

Per the user's standing instruction, every commit ends with:
```
Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>
```

---

## 3. What's done, what's left

### 3.1 Done (23 PRs merged 2026-05-24)

See [`pr-ledger.md`](./pr-ledger.md) for the full table with shas + notes.

Phases complete:
- ✅ **P2.A0 prep**: 5 PRs (#4–#8) + P2.A0.6 redactor (#15)
- ✅ **P2.A engine tasks 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13**
- ✅ **P2.B foundation**: `export-session --bundle` produces reproducible tar (#24)

### 3.2 Burn-down format

The user explicitly requested: **after every PR merge, post a remaining-work
list as a markdown table.** Format:

```markdown
### Burn-down after PR #N

**<count> PRs merged this autonomous session.** Remaining:

| Slug | What | Why ahead |
|---|---|---|
| ... | ... | ... |

Next up: `<slug>` — [why this one is the right next step].
```

This is a **standing workflow rule**, not a one-time request. Do it after every merge.

### 3.3 Remaining work (4 items)

| Slug | What | Status |
|---|---|---|
| `p2a-2real-acp-wiring` | Real `copilot --acp` ACP client | **GATED** on `crates/agent-bridge/tests/fixtures/copilot-acp/captured-happy-path.jsonl` (user must capture) |
| `p2a-9-5-pending-mutations` | Engine N+1 mutation queue for author actions | Speculative — added to ledger by prior session; NOT in spec. May want to drop. |
| `p2b-1` | P2.B replay UI scrubber (multi-PR phase, ~4 PRs) | Foundation in place (#24 tar bundle). TS bundle reader + scrubber UI + Tauri command + Playwright spec + docs. |
| `p2c-1` | P2.C juice/theme/audio v2 (multi-PR phase, ~4 PRs) | Bezier paths, fog/time-of-day, audio v2, theme system. Needs design direction. |

---

## 4. Where things live

### 4.1 Engine (Rust, deterministic core)
- `crates/protocol/` — wire types, `DecisionTimeline` (P2.A task 7)
- `crates/engine/src/lifecycle.rs` — `RequestLifecycle` (P2.A task 5)
- `crates/engine/src/agent_runtime.rs` — `AgentRuntime` orchestrator (P2.A task 8 part 1)
- `crates/engine/src/llm_agent.rs` — `LlmAgent` Agent-trait wrapper (P2.A task 8 part 2)
- `crates/engine/src/redactor.rs` — secret-pattern redactor (P2.A0.6); 10-pattern list with drift-detection test
- `crates/engine/src/agent_log.rs` — AgentLog v2 schema (P2.A0.5)
- `crates/engine/src/actions.rs` — author tools `DefineResource` / `AddProducer` / `AddConsumer` / `SetGoal` (P2.A task 9)

### 4.2 Bridge (separate process, NDJSON stdio)
- `crates/agent-bridge/src/wire.rs` — `BridgeMessage` enum + NDJSON framing
- `crates/agent-bridge/src/main.rs` — stdio loop; `SIMETRO_BRIDGE_BACKEND=mock|copilot`
- `crates/agent-bridge/src/tools.rs` — 9 ToolSpecs (5 movement + 4 author)
- `crates/agent-bridge/src/backends/{mock,copilot}.rs` — `MockBackend` works; `CopilotBackend` is a stub returning `NotAuthenticated` until task 2-real lands
- `crates/agent-bridge/tests/fixtures/error_modes/` — 10 per-error-mode fixtures (P2.A task 11)
- `crates/agent-bridge/src/error.rs::LlmError::all_variants()` — compile-time-enforced variant catalogue

### 4.3 Frontend (TS, Vite + Playwright)
- `frontend/src/ui/scene_browser.ts` — collapsible scene panel (`setCollapsed` / `isCollapsed` API)
- `frontend/src/catalog/scenes.ts` — 11 scene entries

### 4.4 Headless CLI
- `crates/headless/src/main.rs` — `simetro-headless run|bench|hash|replay|export-session [--bundle]`
- Bundle export: `simetro-headless export-session --scene games/metro-pulse.json --ticks 1000 --seed 42 --out /tmp/bundle --bundle` → `/tmp/bundle.tar` byte-for-byte reproducible.

### 4.5 xtask
- `xtask/src/copilot_smoke.rs` — `cargo xtask copilot-smoke` (human-run; checks `copilot --acp` spawns cleanly)

### 4.6 Cargo features
- `simetro-engine/llm-live` — enables `kind: "llm"` agents in scenes. Enabled by default on `crates/headless`, `crates/tauri-app`, and `src-tauri/`. **Disable** to refuse LLM scenes in production-only builds.

---

## 5. Spec + analysis docs

The user's spec and analysis live in `docs/superpowers/`:
- [`specs/2026-05-24-post-pr3-roadmap-design.md`](../specs/2026-05-24-post-pr3-roadmap-design.md) — the canonical roadmap. §2.6 merge policy, §2.7 adversarial review, §3.0 prep enumeration, §10.2.1 request lifecycle, §14 plan-mode decisions.
- [`analysis/p2a-error-map.md`](../analysis/p2a-error-map.md) — error mapping + §4 redactor pattern list (must stay in lock-step with `redactor.rs::PATTERN_DEFINITIONS`).
- [`analysis/p2a-security-threat-model.md`](../analysis/p2a-security-threat-model.md) — security threat model; §5.3 redactor authoritative regexes; §7.1 XPIA framing.

---

## 6. Hard rules from prior sessions

These came from the user as standing directives and are stored in the
agent's memory system, but they're listed here so a fresh session sees them
without depending on memory retrieval:

1. **Commit and push after every step** in multi-step implementations.
2. **Scratchpad doc is single source of truth.** When the user maintains a
   plan in their local scratchpad, don't duplicate or diverge into the
   session's `plan.md`.
3. **Burn-down after every PR merge** (see §3.2 above).
4. **Branch protection cannot be relaxed.** PRs must reach `mergeStateStatus: CLEAN` — no `--admin` overrides.
5. **Real `copilot --acp` wiring is gated** on the user committing
   `crates/agent-bridge/tests/fixtures/copilot-acp/captured-happy-path.jsonl`.
   Do NOT speculate the wire format; do NOT open `p2a-2real-acp-wiring`
   without that file present.

---

## 7. Resume prompt

Open a new Copilot CLI session in `/Users/mattheww/git/simetro` and paste:

> Resume the autonomous-week PR queue. Start by reading
> `docs/superpowers/handoff/HANDOFF.md` + `docs/superpowers/handoff/pr-ledger.md`
> + the spec at `docs/superpowers/specs/2026-05-24-post-pr3-roadmap-design.md`.
> Follow the operating contract in HANDOFF.md §2 exactly. Pick the next
> PR from HANDOFF.md §3.3 (skip any that are GATED or speculative unless
> I've explicitly cleared them). After each merge, post a burn-down per
> HANDOFF.md §3.2.

That prompt is sufficient — the agent will discover everything else from
the linked docs.

---

## 8. State snapshot at handoff time

- `main` HEAD: **`5ccd776`** (PR #24)
- All worktrees cleaned up; no in-flight work.
- All planned items in `pr_ledger` are status=`planned` (none `open`).
- No test failures, no clippy warnings, no doc-link breaks across the workspace.
- ACP fixture: **NOT** present in repo.
