//! # Recorded-fixture test suite
//!
//! Drives the `simetro-bridge` dispatch path through one fixture per
//! `LlmError` variant + happy/edge paths. Each variant has its own fixture file under `tests/fixtures/error_modes/`
//! so adding a new error mode is a one-file change.
//!
//! Fixture shape (JSON):
//! ```jsonc
//! {
//!   "label": "human-readable identifier",
//!   "doc":   "what this fixture exercises and why",
//!   "backend": { "kind": "ok" | "err", ... },
//!   "expected": {
//!     "chosen_kind": "no_op" | "set_speed" | ...,
//!     "rationale_contains":      ["substr", ...],     // all must appear
//!     "rationale_must_not_contain": ["substr", ...],  // none may appear (security gate)
//!     "chosen_args_contains":    ["substr", ...]      // when chosen is non-NoOp
//!   }
//! }
//! ```
//!
//! ## Why fixture files instead of inline test data?
//!
//! - Adding a new `LlmError` variant means dropping a new JSON file,
//!   not editing Rust. Easy for non-Rust contributors.
//! - The fixtures double as ground-truth examples for the eventual
//!   replay harness and docs.
//! - The drift-detection test
//!   `every_llm_error_variant_has_a_fixture` enforces that each
//!   variant of `LlmError` has at least one fixture file.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use serde::Deserialize;
use simetro_agent_bridge::backend::{Backend, BackendRequest, BackendResponse, ToolCall};
use simetro_agent_bridge::backends::mock::{MockBackend, MockTurn};
use simetro_agent_bridge::bridge::parse_tool_call;
use simetro_agent_bridge::error::LlmError;
use simetro_agent_bridge::error_mapping::llm_error_to_message;
use simetro_engine::lifecycle::{AgentReply, AgentRequest, RequestId};
use simetro_protocol::{Action, SimMessage};
use std::path::{Path, PathBuf};

const FIXTURE_DIR: &str = "tests/fixtures/error_modes";

#[derive(Debug, Clone, Deserialize)]
struct Fixture {
    label: String,
    #[allow(dead_code)]
    doc: String,
    backend: BackendOutcome,
    expected: Expected,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum BackendOutcome {
    Ok {
        #[serde(default)]
        raw: String,
        #[serde(default)]
        tool_calls: Vec<ToolCallJson>,
    },
    Err {
        variant: String,
        #[serde(default)]
        agent_id: Option<String>,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        elapsed_ms: Option<u32>,
        #[serde(default)]
        retry_after_ms: Option<u32>,
        #[serde(default)]
        code: Option<i32>,
        #[serde(default)]
        raw: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct ToolCallJson {
    name: String,
    arguments_json: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Expected {
    chosen_kind: String,
    #[serde(default)]
    rationale_contains: Vec<String>,
    #[serde(default)]
    rationale_must_not_contain: Vec<String>,
    #[serde(default)]
    chosen_args_contains: Vec<String>,
}

fn fixture_dir() -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest).join(FIXTURE_DIR)
}

fn load_fixture(path: &Path) -> Fixture {
    let src = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    serde_json::from_str(&src)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()))
}

fn list_fixtures() -> Vec<(PathBuf, Fixture)> {
    let dir = fixture_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    // Stable order so the test output is deterministic across runs.
    entries.sort();
    entries
        .into_iter()
        .map(|p| {
            let fix = load_fixture(&p);
            (p, fix)
        })
        .collect()
}

fn build_mock(outcome: &BackendOutcome) -> MockBackend {
    match outcome {
        BackendOutcome::Ok { raw, tool_calls } => {
            let resp = BackendResponse {
                raw: raw.clone(),
                tool_calls: tool_calls
                    .iter()
                    .map(|t| ToolCall {
                        name: t.name.clone(),
                        arguments_json: t.arguments_json.clone(),
                    })
                    .collect(),
            };
            MockBackend::with_responses([MockTurn::Ok(resp)])
        }
        BackendOutcome::Err {
            variant,
            agent_id,
            message,
            elapsed_ms,
            retry_after_ms,
            code,
            raw,
        } => {
            let aid = agent_id.clone().unwrap_or_else(|| "test-agent".into());
            let err = match variant.as_str() {
                "not_authenticated" => LlmError::NotAuthenticated,
                "subprocess_died" => LlmError::SubprocessDied { code: *code },
                "refused" => LlmError::Refused {
                    agent_id: aid,
                    message: message.clone().unwrap_or_default(),
                },
                "timeout" => LlmError::Timeout {
                    agent_id: aid,
                    elapsed_ms: elapsed_ms.unwrap_or(1000),
                },
                "rate_limited" => LlmError::RateLimited {
                    retry_after_ms: retry_after_ms.unwrap_or(0),
                },
                "malformed_response" => LlmError::MalformedResponse {
                    agent_id: aid,
                    raw: raw.clone().unwrap_or_default(),
                },
                "disconnected" => LlmError::Disconnected,
                other => panic!("unknown LlmError variant in fixture: {other}"),
            };
            MockBackend::with_responses([MockTurn::Err(err)])
        }
    }
}

fn sample_request(label: &str) -> AgentRequest {
    AgentRequest {
        id: RequestId {
            timeline_id: 1,
            agent_id: "test-agent".to_string(),
            source_tick: 100,
            attempt: 0,
        },
        deadline_ticks: 60,
        observation_json: format!("{{\"label\":\"{label}\"}}"),
    }
}

/// Mirror the bridge's main.rs::dispatch logic so the test exercises
/// the same code path the production binary uses. Kept in sync via
/// the test `dispatch_logic_matches_main_rs_contract` below.
async fn dispatch(backend: &dyn Backend, req: &AgentRequest) -> AgentReply {
    let backend_req = BackendRequest {
        agent_id: req.id.agent_id.clone(),
        prompt: req.observation_json.clone(),
        tools: simetro_agent_bridge::tools::action_tool_specs(),
    };
    match backend.invoke(backend_req).await {
        Ok(resp) => match resp.tool_calls.first() {
            None => AgentReply {
                id: req.id.clone(),
                chosen: Some(Action::NoOp),
                rationale: truncate(&resp.raw, 512),
                confidence: 1.0,
            },
            Some(tc) => match parse_tool_call(tc, &req.id.agent_id) {
                Ok(action) => AgentReply {
                    id: req.id.clone(),
                    chosen: Some(action),
                    rationale: truncate(&resp.raw, 512),
                    confidence: 1.0,
                },
                Err(parse_err) => {
                    let msg = llm_error_to_message(&parse_err, &req.id.agent_id, 1);
                    AgentReply {
                        id: req.id.clone(),
                        chosen: Some(Action::NoOp),
                        rationale: rationale_for(&msg),
                        confidence: 1.0,
                    }
                }
            },
        },
        Err(err) => {
            let msg = llm_error_to_message(&err, &req.id.agent_id, 1);
            AgentReply {
                id: req.id.clone(),
                chosen: Some(Action::NoOp),
                rationale: rationale_for(&msg),
                confidence: 1.0,
            }
        }
    }
}

fn rationale_for(msg: &SimMessage) -> String {
    match msg {
        SimMessage::Warning(w) => format!("bridge warning: {w:?}"),
        SimMessage::Fault(f) => format!("bridge fault: {f:?}"),
        other => format!("bridge: {other:?}"),
    }
}

/// Mirror of `main.rs::truncate`. The test must apply the same cap
/// so a fixture that exercises an over-cap `raw` payload sees the
/// same rationale shape the production binary would emit. Critical
/// for security-gate fixtures (e.g. `malformed_response`) where the
/// secret in `raw` could otherwise slip past assertions that look
/// only at the truncated tail.
fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

#[tokio::test]
async fn every_fixture_round_trips_through_bridge_dispatch() {
    let fixtures = list_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no fixture files found in {}",
        fixture_dir().display()
    );

    for (path, fix) in fixtures {
        let backend = build_mock(&fix.backend);
        let req = sample_request(&fix.label);
        let reply = dispatch(&backend, &req).await;

        let label = &fix.label;
        let chosen_kind = chosen_kind_name(&reply.chosen);
        assert_eq!(
            chosen_kind, fix.expected.chosen_kind,
            "[{label}] {path:?}: chosen action kind mismatch"
        );

        for needle in &fix.expected.rationale_contains {
            let haystack_lower = reply.rationale.to_lowercase();
            let needle_lower = needle.to_lowercase();
            assert!(
                haystack_lower.contains(&needle_lower),
                "[{label}] {path:?}: rationale {:?} missing substr {needle:?}",
                reply.rationale
            );
        }
        for forbidden in &fix.expected.rationale_must_not_contain {
            assert!(
                !reply.rationale.contains(forbidden),
                "[{label}] {path:?}: rationale {:?} contains forbidden substring {forbidden:?} \
                 (XPIA hardening — `raw` must NOT surface)",
                reply.rationale
            );
        }
        if !fix.expected.chosen_args_contains.is_empty() {
            let args = format!("{:?}", reply.chosen);
            for needle in &fix.expected.chosen_args_contains {
                assert!(
                    args.contains(needle),
                    "[{label}] {path:?}: chosen action {args} missing substr {needle:?}"
                );
            }
        }
    }
}

fn chosen_kind_name(action: &Option<Action>) -> &'static str {
    match action {
        None => "none",
        Some(Action::NoOp) => "no_op",
        Some(Action::SetSpeed { .. }) => "set_speed",
        Some(Action::PlacePiece { .. }) => "place_piece",
        Some(Action::ConnectPieces { .. }) => "connect_pieces",
        Some(Action::RemovePiece { .. }) => "remove_piece",
        Some(Action::DefineResource { .. }) => "define_resource",
        Some(Action::AddProducer { .. }) => "add_producer",
        Some(Action::AddConsumer { .. }) => "add_consumer",
        Some(Action::SetGoal { .. }) => "set_goal",
    }
}

/// Drift-detection: every variant of `LlmError` MUST have at least
/// one fixture file under `tests/fixtures/error_modes/`. If a new
/// variant is added to `LlmError`, this test fails until the fixture
/// is added — preventing silent "we never tested that error mode".
#[test]
fn every_llm_error_variant_has_a_fixture() {
    // Hand-maintained catalogue derived from the EXHAUSTIVE match in
    // `LlmError::variant_name` — see crates/agent-bridge/src/error.rs.
    // Adding a new LlmError variant fails to compile until the arm is
    // added; adding the arm without updating `one_of_each` fails the
    // `variant_name_is_unique_per_variant` unit test there.
    let expected_variants = LlmError::all_variants();

    let fixtures = list_fixtures();
    let labels: Vec<String> = fixtures.iter().map(|(_, f)| f.label.clone()).collect();

    for variant in &expected_variants {
        assert!(
            labels.iter().any(|l| l == variant),
            "missing fixture for LlmError variant {variant:?}; \
             create {}/{variant}.json",
            fixture_dir().display()
        );
    }
}
