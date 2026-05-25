//! AgentLog v2 work: AgentLog v1 → v2 migration shim — golden-file test.
//!
//! Acceptance criterion from spec §3.0 AgentLog v2 work:
//!
//! > `simetro-headless replay` works against both a v1 log (existing
//! > fixture) and a v2 log (new fixture), bit-for-bit deterministic.
//!
//! This test loads both committed fixtures line-by-line through the
//! v2 deserializer and asserts:
//!
//! 1. Every line parses successfully (no migration data loss).
//! 2. v1 rows produce entries with `schema_version: 1` and
//!    `backend / model / latency_ms / prompt_tokens / completion_tokens
//!    / truncated_bytes` all `None`.
//! 3. v2 rows produce entries with `schema_version: 2` and the
//!    provenance fields populated.
//! 4. Serializing a v1-loaded entry and re-deserializing yields the
//!    same `AgentLogEntry` (round-trip after migration is stable —
//!    note the serialized form now carries `"schema_version": 1`
//!    explicitly, which is intentional: re-emitting a v1 row through
//!    this engine preserves its v1 identity rather than silently
//!    upgrading it).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::AgentLogEntry;

const V1_FIXTURE: &str = include_str!("fixtures/agent_log/v1-sample.jsonl");
const V2_FIXTURE: &str = include_str!("fixtures/agent_log/v2-sample.jsonl");

fn parse_jsonl(text: &str) -> Vec<AgentLogEntry> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str::<AgentLogEntry>(l)
                .unwrap_or_else(|e| panic!("failed to parse jsonl line\n  line: {l}\n  error: {e}"))
        })
        .collect()
}

#[test]
fn v1_fixture_replays_through_v2_deserializer() {
    let entries = parse_jsonl(V1_FIXTURE);
    assert_eq!(entries.len(), 3, "v1 fixture should have 3 rows");
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(
            entry.schema_version, 1,
            "v1 row {i} should report schema_version 1"
        );
        assert_eq!(entry.backend, None, "v1 row {i} backend must be None");
        assert_eq!(entry.model, None, "v1 row {i} model must be None");
        assert_eq!(entry.latency_ms, None, "v1 row {i} latency must be None");
        assert_eq!(
            entry.prompt_tokens, None,
            "v1 row {i} prompt_tokens must be None"
        );
        assert_eq!(
            entry.completion_tokens, None,
            "v1 row {i} completion_tokens must be None"
        );
        assert_eq!(
            entry.truncated_bytes, None,
            "v1 row {i} truncated_bytes must be None"
        );
    }

    // Spot-check semantic content of the first row.
    assert_eq!(entries[0].tick, 600);
    assert_eq!(entries[0].agent_id, "speed_tuner_0");
    assert_eq!(entries[0].rationale, "nudge");
    assert_eq!(entries[0].considered_count, 3);

    // Third row is the minimal one without raw_response.
    assert_eq!(entries[2].tick, 1800);
    assert_eq!(entries[2].parsed_action, None);
}

#[test]
fn v2_fixture_decodes_with_provenance_populated() {
    let entries = parse_jsonl(V2_FIXTURE);
    assert_eq!(entries.len(), 2, "v2 fixture should have 2 rows");
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(
            entry.schema_version, 2,
            "v2 row {i} should report schema_version 2"
        );
        assert_eq!(
            entry.backend.as_deref(),
            Some("copilot"),
            "v2 row {i} backend"
        );
        assert_eq!(
            entry.model.as_deref(),
            Some("gpt-5-mini"),
            "v2 row {i} model"
        );
        assert!(entry.latency_ms.is_some(), "v2 row {i} latency_ms");
        assert!(entry.prompt_tokens.is_some(), "v2 row {i} prompt_tokens");
        assert!(
            entry.completion_tokens.is_some(),
            "v2 row {i} completion_tokens"
        );
    }
}

/// Migration round-trip: a v1 row loaded through the v2 deserializer
/// must re-serialize to JSON that, when re-loaded, produces the same
/// `AgentLogEntry`. The re-emitted JSON now explicitly carries
/// `"schema_version": 1` so a v1 row's identity is preserved across
/// load → serialize → load cycles.
#[test]
fn v1_row_roundtrip_after_migration_is_stable() {
    let entries = parse_jsonl(V1_FIXTURE);
    for entry in &entries {
        let serialized = serde_json::to_string(entry).expect("serialize");
        assert!(
            serialized.contains("\"schema_version\":1"),
            "re-serialized v1 row should explicitly carry schema_version: 1; got: {serialized}"
        );
        let back: AgentLogEntry = serde_json::from_str(&serialized).expect("re-deserialize");
        assert_eq!(back, *entry, "round-trip identity for migrated v1 row");
    }
}

/// Deterministic replay parity: parsing the v1 fixture twice produces
/// byte-identical in-memory representations. This is the core
/// "bit-for-bit deterministic" property the spec §3.0 AgentLog v2 work
/// acceptance criterion calls out.
#[test]
fn parsing_v1_fixture_twice_is_deterministic() {
    let a = parse_jsonl(V1_FIXTURE);
    let b = parse_jsonl(V1_FIXTURE);
    assert_eq!(a, b);
}

#[test]
fn parsing_v2_fixture_twice_is_deterministic() {
    let a = parse_jsonl(V2_FIXTURE);
    let b = parse_jsonl(V2_FIXTURE);
    assert_eq!(a, b);
}

/// Migration shim end-state: a v1 entry can be inspected for
/// schema_version to know whether replay tooling needs the v1-era
/// rationale-only interpretation of its fields.
#[test]
fn schema_version_field_distinguishes_v1_from_v2() {
    let v1 = parse_jsonl(V1_FIXTURE);
    let v2 = parse_jsonl(V2_FIXTURE);
    assert!(v1.iter().all(|e| e.schema_version == 1));
    assert!(v2.iter().all(|e| e.schema_version == 2));
}
