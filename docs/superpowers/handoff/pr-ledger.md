# PR ledger snapshot (2026-05-24)

This is a frozen export of the in-memory SQL `pr_ledger` table at the end
of the autonomous-week session. New sessions can re-derive this from
`gh pr list --state merged --base main` if needed; this file makes it
zero-lookup.

## Merged (23)

| # | Slug | Phase | Merge SHA | Notes |
|---|------|-------|-----------|-------|
| 4 | `p2a0-1-moratorium-lift` | P2.A0 | `0605eb1` | 7 review rounds; lifted moratorium + ratified spec v2 |
| 5 | `amendments-rubber-duck` | PRE | `013649e` | `hash_run` covers messages; stable agent host sort; `Warning::Behind.agent_id` |
| 6 | `pre-p2a-error-map` | PRE | `eb9b90f` | 24 KB Error & Rescue Map analysis doc |
| 7 | `pre-p2a-security-threat-model` | PRE | `c4600fa` | 903-line threat model; PR4-sec F3+F4+F5 |
| 8 | `p2a0-2-agentlog-v2` | P2.A0 | `06ee3c2` | AgentLog v2 schema; provenance fields; truncation marker |
| 9 | `p2a-1-tool-spec-roundtrip` | P2.A | `d73530d` | 20 tests; exhaustive `ActionTag` coverage + JSON-schema validation |
| 10 | `p2a-3-system-prompt` | P2.A | `2e23b20` | `prompts/system.md` + 13-substring regression test |
| 11 | `p2a-4-llm-error-mapping` | P2.A | `75be02b` | 7-variant `LlmError → SimMessage` mapping; 13 tests |
| 12 | `p2a-5-outbox-inbox` | P2.A | `a19f2c5` | `RequestLifecycle` per spec §10.2.1; 20 tests; Codex P1 deadline-rebase fix |
| 13 | `p2a-7-decision-timeline` | P2.A | `fb8c40f` | `DecisionTimeline` first-class addressable; 19 tests |
| 14 | `p2a-8-llm-agent-wrapper` | P2.A | `69cce86` | `AgentRuntime` orchestrator (engine half of task 8); 13 tests |
| 15 | `p2a0-6-redactor` | P2.A0 | `6c71f58` | 10-pattern secret-pattern redactor; **unblocks real ACP wiring**; 26 tests |
| 16 | `p2a-13-docs-actual` | P2.A | `15865cb` | `docs/agents.md` + `docs/runbook.md` for live LLM bridge |
| 17 | `(scene-browser)` | UI | (squashed) | Collapsible + scrollable scene browser (frontend) |
| 18 | `p2a-8-part2-llm-agent` | P2.A | `1456977` | `LlmAgent` Agent-trait impl (task 8 part 2); 6 tests |
| 19 | `p2a-10-scene-wiring-actual` | P2.A | `39fc71e` | `kind: "llm"` loader + `llm-live` feature gate; Codex P1 src-tauri fix |
| 20 | `p2a-9-author-tools-actual` | P2.A | `adcb372` | `DefineResource` / `AddProducer` / `AddConsumer` / `SetGoal` + 9 tests |
| 21 | `p2a-6-bridge-process-actual` | P2.A | `2b42f6a` | `simetro-bridge` NDJSON stdio loop; 8 wire + 4 subprocess tests; Codex P1×2 |
| 22 | `p2a-11-fixture-suite-actual` | P2.A | `6f74cd2` | 10 LlmError fixtures + drift detection; compile-time variant catalogue |
| 23 | `p2a-12-xtask-smoke-actual` | P2.A | `323d29a` | `cargo xtask copilot-smoke`; std-only PATH walk |
| 24 | `export-session-tar` | P2.B-prep | `5ccd776` | `--bundle` flag; hand-built tar headers for byte-exact reproducibility; 4 review rounds |

## Planned (4)

| Slug | Phase | Status | Reason ahead |
|------|-------|--------|--------------|
| `p2a-2real-acp-wiring` | P2.A | GATED | Needs `crates/agent-bridge/tests/fixtures/copilot-acp/captured-happy-path.jsonl` |
| `p2a-9-5-pending-mutations` | P2.A | speculative | NOT in spec; prior session added this. Consider dropping. |
| `p2b-1` | P2.B | multi-PR | ~4 PRs: bundle loader, scrubber UI, Tauri command, Playwright, docs |
| `p2c-1` | P2.C | multi-PR | ~4 PRs: bezier paths, fog/time-of-day, audio v2, theme system |

## §2.7 review-round statistics

| Stat | Value |
|------|-------|
| Total PRs merged in session | 23 |
| Total adversarial review rounds | ~50 |
| Average review rounds per PR | ~2 |
| Max review rounds on one PR | 4 (PR #22 fixture suite, PR #24 tar bundle) |
| 5-strikes-then-pivot rule tripped | 0 times |
| Codex bot P1 findings caught | ~6 (all real; all fixed in code) |
| Codex bot P2 findings caught | ~5 |
| My sub-agent code-review findings | dozens; most caught design issues early |
| My sub-agent security-review findings | 0 actionable (defensive design held) |

## Notable Codex P1 catches (all fixed)

1. **PR #12** — `drain_reply` deadline-rebase: re-issued requests were born already-overdue. Fixed in commit `a6b1071`.
2. **PR #19** — `src-tauri/` didn't enable the `llm-live` feature, so the real desktop binary would refuse `metro-pulse.json`. Fixed in commit `bc4a123`.
3. **PR #21** — bridge accepted mismatched `schema_version` (spec §10.1 says MUST reject); plus parse errors silently became NoOp. Both fixed.
4. **PR #24** — `tar::HeaderMode::Deterministic` in tar 0.4 doesn't zero mtime (only uid/gid/uname/gname). Required hand-built `tar::Header`.

These are the failure modes the next session should be primed for. Whenever
the adversarial reviewer raises a finding, **read the diff carefully** —
my own code-review sub-agent has missed real bugs that Codex caught.

## Workspace stats at handoff

- Engine: ~205 unit tests + ~6 integration
- Protocol: ~38 unit tests
- Agent-bridge: ~49 unit + 4 subprocess + 2 fixture-driver + 20 tool-spec round-trip
- Headless: 6 binary + 6 CLI integration
- Tauri shell: 1 sanity build
- Frontend: 89 vitest + 9 Playwright e2e
- **Total: ~280+ tests, all green on `main` at `5ccd776`**

All clippy + rustdoc warnings are denied via `RUSTFLAGS=-D warnings` /
`RUSTDOCFLAGS=-D warnings` in CI; local pre-commit hook enforces same.
