# P2.A Error & Rescue Map

> **Status:** Pre-P2.A analysis doc (P2.A0.3 per spec §3.0).
> **Authoritative:** This document is a CONSTRAINT SET on every P2.A
> implementation PR. Each P2.A PR's adversarial review (§2.7) must
> verify that any new fallible codepath the PR introduces is present
> in this map OR that the PR extends the map atomically.
> **No code changes** — this is mega-review §2 (Error & Rescue Map)
> applied specifically to the P2.A surface.
>
> **Author:** Copilot CLI on behalf of @ridermw.
> **Sources:**
> - `crates/agent-bridge/src/error.rs` (current `LlmError` taxonomy).
> - `crates/engine/src/error.rs` (`AgentError`, `EngineFault`,
>   `LoadError`).
> - `crates/protocol/src/lib.rs` (`FaultPayload`, `WarningPayload`).
> - Roadmap spec sections §2.5, §3, §3.0, §10.1, §10.2, §10.2.1,
>   §10.4, §10.5.
> - PR #4 security-review deferred MEDIUMs Findings 3 / 4 / 5.

---

## 0. Operating principles

The spec's prime directives apply to every row below:

1. **Zero silent failures.** Every variant either emits a typed
   `Fault`/`Warning` to the wire AND adds an AgentLog row, OR has
   an explicit doc note explaining why suppression is correct (rare).
2. **Every error has a name.** No `Box<dyn Error>`, no
   `anyhow::Result` in the bridge or engine. Every variant lives in
   a typed enum so callers can pattern-match.
3. **Every rescue has a test.** Each row in the tables below MUST
   have at least one unit test or fixture-driven test. The test
   column is the canary that the rescue actually executes.
4. **Determinism preserved.** When a failure occurs, the resulting
   `Warning`/`Fault` is deterministically keyed to tick + agent_id
   + request_id (per spec §10.2.1) — never to wall-clock arrival
   order. This is enforced by `hash_run` covering
   `runner.messages()` (PR #5).
5. **User-visible degradation is also deterministic.** Inspector
   row renders the same text on every run that produced the same
   underlying `Fault`/`Warning`.

---

## 1. `LlmError` (bridge side) → engine surface

`crates/agent-bridge/src/error.rs` defines seven variants today.
P2.A may extend the enum; any new variant gets a row added here
**in the same PR**.

| `LlmError` variant | Triggered by | Engine `Fault`/`Warning` surface | Rescue action (bridge) | Rescue action (engine) | User-visible behavior | Required test |
| --- | --- | --- | --- | --- | --- | --- |
| `NotAuthenticated` | `copilot --acp` exits with auth error during handshake; `gh auth status` token missing Copilot entitlement; `copilot` binary not on PATH. | `Fault::AgentCrashed { agent_id, message: "not authenticated" }` | Mark backend permanently failed for this engine run. Stop sending requests to it. | Pause the running scene (engine `RunState::Faulted`) **only if** the failing agent is the sole agent in the scene; otherwise emit `Fault::AgentCrashed` and continue without this agent. | Inspector overlay: red banner "Copilot agent unavailable: not authenticated. Run `gh auth login` and reload scene." | Fixture: `crates/agent-bridge/tests/fixtures/copilot-acp/not-authenticated.jsonl` simulates the handshake reject; bridge integration test asserts `LlmError::NotAuthenticated`. Engine unit test asserts the resulting `Fault::AgentCrashed` is emitted and (when agent is sole) `RunState::Faulted` is set. |
| `SubprocessDied { code }` | `copilot --acp` process exits unexpectedly mid-session (SIGSEGV, SIGKILL, panic in subprocess, etc.). | `Fault::AgentCrashed { agent_id, message: "subprocess died (code: …)" }` | One re-spawn attempt with exponential backoff (cap at one retry). If second spawn dies before first reply, escalate. | Same as `NotAuthenticated`: pause iff sole agent, otherwise continue without. | Inspector overlay: red banner "Copilot subprocess died (code: N). Auto-retry attempted; see runbook." | Fixture-driven test: simulate process exit after partial handshake; assert one retry attempt + final `Fault::AgentCrashed`. |
| `Refused { agent_id, message }` | LLM reply contains an explicit refusal cue (e.g. "I cannot do that", policy violation). Bridge's `refusal_classifier` detects it. | `Warning::InvalidAction { agent_id, reason: "refused: <message>" }` | Do not retry; log the refusal in AgentLog v2 with `was_refused: true`. | Apply `Action::NoOp` for this tick. Continue normal operation. | Inspector: yellow row "Agent <id> declined this decision." Optional rationale snippet (textContent, ≤200 chars). | Fixture: `refused.jsonl` with known refusal phrasings. Unit test that the classifier matches each one + emits the `Warning`. |
| `Timeout { agent_id, elapsed_ms }` | LLM did not produce a parseable reply within the per-request budget (default 60s; configurable per scene). | `Warning::Behind { lag_frames: deadline_ticks_late, agent_id: Some(agent_id) }` | Cancel the outstanding ACP request. If `attempt < MAX_ATTEMPTS` (per spec §10.2.1), re-issue with `attempt += 1`; else mark request expired and stop retrying for this decision. | Per spec §10.2.1: the original request transitions `pending → expired`. The retry is enqueued at the **next tick boundary**, not immediately. | Inspector: yellow "Agent <id> timed out (Nms); retrying" row. After max attempts: yellow "Agent <id> giving up on this decision." | Fixture: stalled-reply fixture (sleeps past the deadline). Test that `Warning::Behind` carries the correct `agent_id` and that `MAX_ATTEMPTS` re-issues happen at correct tick boundaries. |
| `RateLimited { retry_after_ms }` | 429 or quota response from `copilot --acp` (ACP carries this as a structured error in the JSON-RPC envelope). | `Warning::Behind { lag_frames: retry_after_in_ticks, agent_id: Some(agent_id) }` | Pause the outbox for this agent for `retry_after_ms`. Other agents continue. Do NOT retry within the cooldown. | Engine pauses LlmAgent firing until the cooldown elapses (computed in ticks at the current `world.dt`). | Inspector: yellow "Agent <id> rate-limited; resumes in Ns." | Fixture: rate-limit reply; test that subsequent outbox requests are gated by the cooldown duration. |
| `MalformedResponse { agent_id, raw }` | LLM reply is not valid JSON; or is valid JSON but does not validate against the tool's schema; or `chosen` field references a non-existent tool. | `Warning::InvalidAction { agent_id, reason: "malformed response: <reason>" }` | One re-issue with `attempt += 1` (per §10.2.1). If second reply also malformed, give up on this decision. Log `raw` to AgentLog v2 with **size cap** (per security-review Finding 4: 64 KiB). | Apply `Action::NoOp` for this tick when the response is unparseable. | Inspector: yellow "Agent <id> returned malformed JSON; retried." | Three fixtures: invalid JSON, valid JSON but wrong schema, valid JSON but unknown tool name. Each must produce the correct `Warning::InvalidAction` text. Test the size cap by feeding a 1 MB `raw` and asserting the AgentLog row is truncated with a marker. |
| `Disconnected` | stdio EOF mid-session (different from `SubprocessDied` — process is alive but its stdout/stdin closed). | `Fault::AgentCrashed { agent_id, message: "ACP stdio disconnected" }` | Mark backend connection broken. Do not attempt to re-use the existing subprocess; if re-spawn is allowed, treat as `SubprocessDied`. | Same as `SubprocessDied`: pause iff sole agent, otherwise continue. | Inspector: red "ACP connection lost; subprocess re-spawning." | Test: deliberately close the stdio pipe; assert the bridge detects EOF and emits `LlmError::Disconnected` (not `Timeout`). |

### 1.1 P2.A0.3 *new* `LlmError` variants required

These variants do NOT exist yet; they must be added in P2.A task 2
(ACP wiring) and have rows added to this table at the same time.

| Proposed variant | Reason | Distinct from |
| --- | --- | --- |
| `ExecutableMissing { path: String }` | The `copilot` binary cannot be resolved (PATH lookup failed, absolute path doesn't exist, not executable). Per security-review Finding 3, we resolve to an absolute path at startup; this variant fires when that resolution fails. | `NotAuthenticated` (which is auth failure, not missing-binary). The current code conflates them. |
| `EntitlementMissing { account: Option<String> }` | `gh auth status` token is valid but lacks Copilot subscription/entitlement. ACP handshake returns a specific entitlement error code. | `NotAuthenticated` (no token at all). User-facing remediation differs. |
| `ProtocolMismatch { expected_version: String, found_version: String }` | The `ACP version probe` (spec §12 risk 6) detects a wire-format change between what the bridge expects and what `copilot --acp` speaks. | `MalformedResponse` (response is well-formed for the wrong protocol version). |

Each new variant gets a row in §1 above and a fixture+test in §5.

---

## 2. Subprocess spawn / lifecycle failures

These are operational failures around the `copilot --acp` subprocess
itself, distinct from response-content failures in §1. Per
security-review Finding 3 (PR #4), the bridge must harden the
subprocess interface.

| Failure | Detection point | `LlmError` mapped to | Rescue | User-visible | Required test |
| --- | --- | --- | --- | --- | --- |
| `copilot` binary not on PATH | Bridge startup: `which copilot` or absolute-path canonicalization fails. | `ExecutableMissing { path }` → `Fault::AgentCrashed` | Bridge enters "permanently-unavailable" state for the run. No retries. | Inspector: red "Copilot CLI not installed. Install via `brew install gh-copilot` (or similar) and reload." | Test that overrides `PATH` to be empty and asserts the bridge surfaces `ExecutableMissing` without panicking. |
| `gh auth status` not logged in | Bridge startup: handshake probe to ACP returns an auth-required error. | `NotAuthenticated` | Same as above. | Inspector: red banner with `gh auth login` instruction. | Fixture: handshake-rejects-with-auth-required. Test that the bridge surfaces `NotAuthenticated`. |
| Copilot entitlement missing | ACP handshake returns entitlement-required error. | `EntitlementMissing` | Permanent failure for run. | Inspector: red "Copilot subscription required. See https://github.com/features/copilot." | Fixture for handshake-rejects-with-entitlement-required. |
| Subprocess spawn fails (out-of-FDs, ENOMEM, sandbox blocks exec) | `std::process::Command::spawn()` returns `io::Error`. | `LlmError::NotAuthenticated` (best-effort fallback) → `Fault::AgentCrashed` | Bridge logs the io::Error with context and surfaces a typed fault. | Inspector: red "Could not start Copilot subprocess (OS error: …)." | Test that wraps `spawn()` in a mock that returns `io::ErrorKind::Other` and asserts the typed surfacing. |
| Subprocess crashes mid-session | Process group monitor detects exit while a request is `pending`. | `SubprocessDied { code }` | Per §1: one re-spawn attempt with backoff. | Per §1. | Spawn a subprocess that exits after the first reply; assert the bridge surfaces `SubprocessDied` with the correct exit code. |
| Subprocess hangs past wall-clock subprocess timeout | Hard subprocess-level timeout (independent of per-message timeout per security Finding 3) trips. | `SubprocessDied { code: None }` (the kill produced no exit code we trust). | Bridge SIGTERMs the subprocess, waits 5s, then SIGKILLs. Re-spawn allowed once. | Inspector: red "Copilot subprocess unresponsive; killed and restarted." | Test that spawns a sleep-forever subprocess + asserts the wall-clock timeout fires + kill happens + typed error surfaces. |
| `PATH`-traversal attack via `copilot` binary | Mitigated: bridge resolves to absolute path at startup, then pins it. New `copilot` later in `PATH` is ignored. | N/A (prevented). | N/A. | N/A. | Test that registers two `copilot` binaries in PATH and asserts only the canonicalized-at-startup one is used. |
| Subprocess inherits sensitive env vars | Mitigated: bridge spawns with **scrubbed env** (only `HOME`, `PATH`, `GH_TOKEN`/`GITHUB_TOKEN` if explicitly required). | N/A. | N/A. | N/A. | Test that verifies the spawned subprocess does NOT see arbitrary parent-process env vars (e.g. inject `SECRET_FOO=bar` and assert subprocess `env` doesn't contain it). |
| Subprocess outlives bridge | Mitigated: bridge spawns the subprocess as the **session leader** of a new process group; `Drop` impl on the bridge handle issues SIGTERM to the entire group, then SIGKILL after 2s. | N/A. | N/A. | N/A. | Test that drops the bridge handle and asserts the subprocess (PID) is no longer alive within 3s. (Linux/macOS only; document Windows behavior as TODO.) |

---

## 3. JSON parse failures

Multiple JSON-parse surfaces in P2.A. Each has different rescue
semantics depending on **where** the parse fails.

| Parse site | Input source | Failure mode | `LlmError`/error | Rescue | User-visible | Required test |
| --- | --- | --- | --- | --- | --- | --- |
| ACP envelope (JSON-RPC framing) | `copilot --acp` stdout, framed length-prefixed JSON. | Malformed envelope (length header doesn't match body length; envelope is not JSON; missing `jsonrpc`/`id`/`method`). | `LlmError::MalformedResponse { agent_id: "<unknown — pre-route>", raw }` | Drop the bad frame, log to AgentLog v2 with `frame_index`. Do not desynchronize the stream — if the length header is corrupted, the bridge cannot recover and must treat this as `Disconnected`. | Inspector: yellow "ACP framing error; some replies dropped." | Test: feed bytes representing a half-frame, an over-length frame, a body that is not valid JSON; assert correct error and that the bridge does not panic. |
| ACP tool-call result payload | The `result` field of a JSON-RPC reply to `tools/call`. | Valid JSON envelope, but `result` is not the expected shape (missing `chosen`, `confidence` out of range, `considered` not an array). | `LlmError::MalformedResponse { agent_id, raw }` | One re-issue per §1; if persistent, give up on this decision. | Per §1. | One fixture per known malformed shape. Assert the correct `reason` string. |
| LLM-emitted tool call arguments | A tool call inside the LLM's response, e.g. `place_piece({"piece_kind": ..., "pos": ...})`. | Arguments don't validate against the tool's JSON Schema (wrong type, missing field, out-of-range numeric, unknown enum value). | `LlmError::MalformedResponse` with `reason` describing the schema violation. | One re-issue with the schema description re-included in the prompt as a corrective hint (optional; default off until tested). | Per §1. | Schema-violation fixtures (one per common violation class): missing field, wrong type, OOB numeric. |
| AgentLog v1 → v2 migration (loader path) | jsonl file from disk, one line at a time. | A line is unparseable JSON; or a line is parseable but missing required v1 fields; or a line claims a `schema_version` we don't recognize. | `LoadError::Parse { line, col, message }` (re-use existing enum). | Skip the line, emit a `Warning::AgentLogSlow` once if any line is skipped, continue replay. Replay test asserts that a partially-corrupt log still produces a deterministic hash for the parseable subset. | Inspector (replay mode): yellow "AgentLog: N corrupt rows skipped." | Test fixture: AgentLog with one bad row in the middle; assert replay completes + warning surfaces + good rows replay deterministically. |
| Scene JSON (existing) | `games/*.json` files, loaded via `crates/engine/src/loader.rs`. | Already handled by existing `LoadError` taxonomy (see `crates/engine/src/error.rs` lines 14-77). New P2.A scenes (`metro-pulse` with `agents: [{kind: "llm"}]`) must extend the loader to reject `kind: "llm"` when the runtime is built without the `llm-live` feature. | `LoadError::UnsupportedVersion`-class error (consider new variant `LoadError::FeatureRequired { feature: &'static str }`). | Scene fails to load with the typed `LoadError`; previous scene preserved per AGENTS.md "Failed scene loading must preserve the previous running scene." | Inspector: red banner with the typed message. | Test: load a scene with `agents.kind == "llm"` against a runtime built without `--features llm-live`; assert `LoadError::FeatureRequired` and that the previously-running scene is preserved. |

---

## 4. AgentLog v2 write failures

Per security-review Finding 4 (PR #4) and spec §10.5, the AgentLog
v2 file is a security-sensitive surface. P2.A0.5 (AgentLog v2 schema)
will land the writer; this section sets the constraints.

| Write-path failure | Detection | Rescue | User-visible | Required test |
| --- | --- | --- | --- | --- |
| Disk full (`io::ErrorKind::WriteZero` or `StorageFull`) | Writer's `write_all` returns the io error. | Degrade to in-memory ring buffer (existing P1.5 behavior via `AgentLog::with_capacity`). Emit `WarningPayload::AgentLogSlow` **once** per run (existing single-warning semantics — test in `tick::tests::agent_log_failure_emits_slow_warning_once`). | Inspector: yellow "AgentLog: disk full; degraded to ring buffer (last N decisions retained)." | Existing test covers ring-buffer fallback; new test for the specific disk-full error path. |
| Permission denied (file or directory not writable) | Writer's `open` or `write_all` returns `io::ErrorKind::PermissionDenied`. | Engine does NOT attempt to chmod or escalate. Emit `Fault::EngineFault { message: "AgentLog write permission denied" }` and disable agent-log entirely for this run. | Inspector: red "AgentLog disabled (permission denied at <path>)." | Test that constructs `AgentLog` with a writer that returns `PermissionDenied`; assert the fault is emitted and no further writes are attempted. |
| **Row fails schema validation before persist** (new — required by PR #4 sec Finding 4 control 3) | Writer validates each AgentLog row against the v2 JSON schema (`schema_version`, required fields present, types correct, optional fields well-formed) BEFORE writing to disk. A row that fails validation is dropped. | Drop the offending row, emit `WarningPayload::AgentLogSlow` once per run for the first validation failure (same single-warning semantics as disk-full degradation), and **do not** propagate the malformed row to disk. Maintain a per-run counter of dropped rows. | Inspector: yellow "AgentLog: N rows failed schema validation; dropped." (only when count > 0) | New `agent_log_v2.rs` test: pass a row with `schema_version: 99` AND a row with a malformed `chosen` field; assert both are dropped + warning emitted + counter increments. Belt-and-suspenders versus the parse-time check at §3's `LLM-emitted tool call arguments` row — that one runs on the wire; this one runs at persistence-time so a downstream change can't bypass it. |
| Path traversal via scene name | Mitigated: AgentLog filename is derived from `scene_id` AFTER registry validation, never from raw scene JSON or user input. Path is `dirs::data_dir() + "simetro" + scene_id + "decisions-YYYY-MM-DD-HHMMSS.jsonl"`. | N/A (prevented). | N/A. | Test: register a scene whose JSON-declared name contains `../etc/passwd`; assert the AgentLog path is the registry-validated `scene_id`, not the JSON name. |
| File mode is world-readable | Mitigated: AgentLog files are created with mode `0o600` on Unix (owner read/write only). Windows: deferred to per-platform discussion in spec §13. | N/A. | N/A. | Test (Unix only, gated by `#[cfg(unix)]`): stat the created AgentLog file, assert mode is `0o600`. |
| `raw_response` field overflows | Mitigated: writer truncates at 64 KiB and appends a `{"truncated_bytes": N}` marker. | Inspector: no user-visible signal for truncation — the AgentLog row itself encodes it. | Test: write an AgentLog row with a 1 MiB `raw_response`; assert the persisted row is truncated and includes the marker. |
| Secret pattern in `raw_response` | Mitigated: pre-write redactor scans for known patterns. Matches replaced with `<redacted: <pattern-name>>`. Minimum required pattern set (non-exhaustive — see note below): **GitHub modern tokens** (`ghs_…`, `ghu_…`, `ghr_…`, `ghp_…`, `github_pat_…`), **legacy GitHub PAT** (40-hex pattern with `gho_` prefix), **Copilot session tokens** as exposed by `copilot --acp` envelopes (verify exact pattern during P2.A0.5; this is the highest-priority surface since the bridge talks directly to Copilot), **OpenAI keys** (`sk-…`, `sk-ant-…`), **AWS access keys** (`AKIA…`, `ASIA…`), **Google API keys** (`AIza…`), **Azure OpenAI** (resource-prefixed UUIDs), **JWT shape** (3-segment base64 `eyJ…`), **PEM private key blocks** (`-----BEGIN … PRIVATE KEY-----`). Inspector: no signal; redaction is silent. AgentLog persists redacted text. | Test: write a `raw_response` containing **each** pattern listed above; assert the persisted text contains the redaction marker, not the original secret. Negative tests for shapes that look-like-but-aren't (e.g. `sk_` without dash, 40-hex non-prefix strings) to confirm no false-positive over-redaction of legitimate scene content. Note: P2.A0.4 may extend this list; the writer reads the pattern set from a single module so additions are one-place changes. |
| Write across process restart (concurrent writers) | The bridge process and the engine process both write AgentLog. Concurrent appends to the same jsonl file are SAFE on Unix (atomic for small writes), but unsafe across restarts. | Use a per-process file (the bridge's AgentLog is a separate file from the engine's), OR file-locking. P2.A0.5 PR must decide and document. | Inspector: no signal unless write fails. | Multi-process integration test: spawn two writers, assert no row corruption. |

---

## 5. Test-suite mapping

Every row in §1-§4 cites a "Required test." This section consolidates
them into the test files where they live.

| Test file | Rows from §1-§4 covered |
| --- | --- |
| `crates/agent-bridge/tests/fixtures/copilot-acp/` | §1: all 7 `LlmError` variants (one fixture each) + the 3 proposed new variants. §3: ACP envelope / tool-call result / tool-call arguments parse failures. |
| `crates/agent-bridge/tests/lifecycle.rs` (new) | §2: spawn / kill / timeout / env-scrub / process-group tests. |
| `crates/engine/tests/agent_log_v2.rs` (new in P2.A0.5) | §4: disk-full, permission-denied, **schema-validation-before-persist**, path-traversal, mode 0600, truncation, redaction (all minimum patterns + negative tests), concurrent-writer tests. |
| `crates/engine/src/tick.rs` tests module | Existing `agent_log_failure_emits_slow_warning_once`; extend with one-shot AgentLogSlow semantics. |
| `crates/engine/tests/llm_stalled_determinism.rs` (new in P2.A task 5) | Per spec §10.5: same scene + seed + stalled bridge → deterministic world hash AND deterministic warning sequence. |
| `crates/engine/src/loader.rs` tests module | §3 scene-loader extension: `LoadError::FeatureRequired` for `kind: "llm"` agent. |
| `cargo xtask copilot-smoke` | Human-run only. Verifies §1 happy path against a real `copilot --acp`. Not in CI. |

---

## 6. CI gating

The required CI surface is unchanged from current state: `ci-ok` gates
on the existing checks. P2.A0.3 (this PR) adds no new CI jobs. P2.A
implementation PRs MAY add specific test jobs (e.g. a fixture-suite
job that runs the recorded ACP fixtures against the bridge) but
those are scoped per-PR, not by this analysis.

The determinism baseline (`tests/baselines/demo-paths.hash`) covers
no LLM scenes today; per spec §10.5 it must NOT be extended to cover
live-LLM scenes (those are non-deterministic by nature). Replay-mode
testing of recorded AgentLog files IS deterministic and IS gated.

---

## 7. Open questions deferred to follow-up PRs

These are explicitly out of scope for P2.A0.3 (analysis-only) and
will be answered in the PR that implements the relevant rescue.

1. **Re-spawn backoff curve** (§1 `SubprocessDied`, `Disconnected`):
   constant 1s? Exponential 1s→2s→4s? **Decision deferred to
   P2.A task 2 (ACP wiring).**
2. **Refusal classifier scope** (§1 `Refused`): substring match? Regex?
   ML-detector? Initial PR will use substring matching for a fixed
   phrase list (PR #3 already wired refusal cues in
   `crates/agent-bridge/src/bridge.rs`); extension deferred.
3. **MAX_ATTEMPTS value** (§1 `Timeout`, `MalformedResponse`): 2?
   3? Per-scene-configurable? **Decision deferred to P2.A task 5
   (outbox/inbox).** Default proposed: 2 attempts.
4. **AgentLog Windows file-mode equivalent** (§4): no `chmod 0o600`
   equivalent. Use NTFS ACLs? Deny "Everyone" read? **Decision
   deferred; Windows is post-week scope per spec.**
5. **Bridge re-spawn cap per run** (§2): if subprocess crashes 10x in
   a single engine run, do we eventually give up? **Decision
   deferred; propose 3 cap per agent per run.**
6. **`raw_response` size cap value** (§4): 64 KiB proposed; verify
   against real `copilot --acp` reply sizes during P2.A0.5.

---

## 8. Mapping to spec sections

| This doc § | Spec § | What the spec says | What this doc adds |
| --- | --- | --- | --- |
| §1 | §10.4 | Error mapping table for 7 `LlmError` variants. | Adds rescue actions + user-visible behavior + test requirement per row. Adds 3 new proposed variants. |
| §2 | §10.1, §3 task 6 | Bridge is a separate process. | Concrete subprocess-hardening test set (PATH resolution, env scrub, process-group kill, wall-clock timeout) per PR #4 security-review Finding 3. |
| §3 | §10.5 row "Bridge `CopilotBackend` ACP framing", row "AgentLog v2 migration" | Fixture-based testing + migration shim. | Specific parse-failure rows for each parse site. |
| §4 | §3.0 P2.A0.4 (forward reference) + PR #4 sec Finding 4 | Mentions raw_response controls. | Concrete control matrix (size cap value, redaction patterns, file mode, path derivation, concurrent-writer behavior). |
| §5 | §10.5 | Testing strategy table. | Maps every error row to the file where its test lives. |
| §6 | §10.5 | "Live-LLM scenes excluded from determinism gate." | Clarifies replay-mode is gated; live-mode is not. |
| §7 | — | — | Lists six open-question items deferred to specific implementation PRs. |

---

## 9. Acceptance criteria (this PR)

- [x] §1 enumerates every existing `LlmError` variant with rescue + UX + test.
- [x] §1.1 proposes the new variants needed by P2.A wiring.
- [x] §2 covers subprocess lifecycle hardening (matches PR #4 sec Finding 3 — all 4 controls present: PATH-absolute-resolution, env scrubbing, process-group kill, wall-clock timeout).
- [x] §3 covers all four distinct JSON-parse sites with separate rescue semantics.
- [x] §4 covers AgentLog v2 write-path failures and **all 6 controls from PR #4 sec Finding 4**:
  - (1) byte-size cap → "raw_response field overflows" row
  - (2) secret-pattern redaction → "Secret pattern in raw_response" row
  - (3) **JSON-schema validation before persistence → "Row fails schema validation before persist" row** (added in PR #6 R1 security-review fix)
  - (4) Write location pinned to `dirs::data_dir()` → "Path traversal via scene name" row
  - (5) File mode 0o600 → "File mode is world-readable" row
  - (6) Filename derived from validated scene_id → "Path traversal via scene name" row
- [x] §5 consolidates test ownership.
- [x] §7 lists open questions with explicit "decision deferred to PR X" notes.
- [x] §8 cross-references back to the spec so the analysis is anchored.
- [x] No code changes; doc-only PR per spec §3.0 P2.A0.3.

The next prep PR is **P2.A0.4 — Security & Threat Model** which
covers the same surface from the security angle (XPIA, subprocess
hardening details, AgentLog redaction patterns). The two docs are
intentionally complementary: this one answers "what goes wrong and
how do we recover?"; P2.A0.4 answers "what does an attacker do and
how do we prevent it?"
