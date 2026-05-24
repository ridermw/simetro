# ADR-005: Out-of-process agent bridge with a Backend trait

**Status:** Accepted.

## Context

simetro's interesting bit is watching LLM-driven agents make
decisions. The user wants the LLM connection itself to be
**plug-n-play**: today Copilot CLI SDK, tomorrow Claude API, then
Codex, then a local model.

If the engine imports any LLM SDK directly, swapping providers is
invasive and the engine takes on async runtimes, HTTP clients,
auth flows, retry policies — none of which belong in a
deterministic tick loop.

## Decision

A separate `crates/agent-bridge` crate that:

1. Defines a `Backend` trait: `async fn complete(&self, prompt,
   tools) -> Result<BackendResponse, LlmError>`.
2. Ships `MockBackend` (queue of canned responses, P1) and a
   `CopilotBackend` stub (P1 surface, P2 live).
3. Owns tool specs (5 JSON Schemas in `tools.rs`).
4. Parses tool calls into typed engine `Action`s and reports
   `LlmError::Refused` / `LlmError::MalformedResponse` for the
   nonsense cases.
5. Runs as its own process; talks to the engine through the same
   protocol envelopes used by the frontend.

The engine talks to a thin `Bridge` handle; the handle talks to a
real backend, a mock for tests, or a remote process over the
protocol.

## Consequences

- (+) Backends are interchangeable. Adding Claude or Codex is a
  new `impl Backend` and nothing else.
- (+) The engine stays pure — no `tokio`, no `reqwest`, no API
  keys in its dependency tree.
- (+) Determinism gate is unaffected by LLM choice: the test bench
  uses `MockBackend` with a scripted response trace.
- (+) Bad LLM behavior (refusals, malformed JSON, auth failures)
  has a single, typed funnel: `LlmError`.
- (-) Two processes to start. The desktop shell launches both;
  CI runs the engine with `MockBackend` and never spawns the
  bridge subprocess.
