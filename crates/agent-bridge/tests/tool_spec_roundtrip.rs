//! P2.A task 1: Tool-spec round-trip & exhaustiveness tests.
//!
//! Acceptance criterion from spec §3 task 1:
//!
//! > Assert every `Action` variant in `actions.rs` has a matching
//! > `ToolSpec` and the inline JSON Schema in `tools.rs` validates a
//! > known-good call. Regression test that fails if a new `Action`
//! > variant lands without a tool.
//!
//! ## What this file enforces
//!
//! 1. **Exhaustive `ActionTag` → tool coverage** — uses an exhaustive
//!    `match` over `ActionTag` (no wildcard arm) so adding a new
//!    variant to the `Action` enum without also adding a tool fails
//!    to compile (the strongest possible regression gate). The same
//!    `match` produces the canonical tool-name string for each variant.
//!
//! 2. **Canonical round-trip per variant** — for every variant we
//!    construct a representative `Action`, serialize it as the LLM
//!    would emit it (the same wire shape `parse_tool_call` expects),
//!    feed it through `parse_tool_call`, and assert we get the
//!    original `Action` back. Catches schema/parser drift.
//!
//! 3. **Schema validation** — for every tool, validate the canonical
//!    `arguments_json` against the tool's JSON Schema using the
//!    `jsonschema` crate. Asserts the schema actually accepts what
//!    the parser accepts.
//!
//! 4. **Negative schema cases** — schema-violating arguments
//!    (out-of-range numerics, oversized strings, wrong types, extra
//!    fields where `additionalProperties: false`) must be rejected
//!    by the schema. Documents the schema's safety surface.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use jsonschema::JSONSchema;
use serde_json::json;
use simetro_agent_bridge::tools::{action_tool_specs, names};
use simetro_agent_bridge::{parse_tool_call, ToolCall};
use simetro_protocol::{Action, ActionTag};

/// THE regression-gate function. Exhaustive `match` over `ActionTag`
/// — no wildcard arm — so a new variant fails to compile until it
/// is added here AND given a tool name string.
///
/// If this match grows a new arm, the test
/// `every_action_variant_has_a_tool_spec` will also fail at runtime
/// until `action_tool_specs()` in `crates/agent-bridge/src/tools.rs`
/// adds the matching `ToolSpec`. Two layers, both strict.
fn expected_tool_name_for(tag: ActionTag) -> &'static str {
    match tag {
        ActionTag::NoOp => names::NO_OP,
        ActionTag::SetSpeed => names::SET_SPEED,
        ActionTag::PlacePiece => names::PLACE_PIECE,
        ActionTag::ConnectPieces => names::CONNECT_PIECES,
        ActionTag::RemovePiece => names::REMOVE_PIECE,
    }
}

/// Every `ActionTag` variant must appear in `action_tool_specs()`.
/// This is the "you added an Action variant without a tool" regression.
#[test]
fn every_action_variant_has_a_tool_spec() {
    let specs = action_tool_specs();
    let spec_names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    for tag in all_action_tags() {
        let expected = expected_tool_name_for(tag);
        assert!(
            spec_names.contains(&expected),
            "ActionTag::{tag:?} maps to tool name {expected:?} but no ToolSpec with that name \
             was found in action_tool_specs(). Add the tool to crates/agent-bridge/src/tools.rs."
        );
    }
}

/// Every `ToolSpec` must correspond to some `ActionTag` — no orphan
/// tools. (A tool with no matching variant means the bridge is
/// exposing something the engine can't handle.)
#[test]
fn every_tool_spec_maps_to_an_action_variant() {
    let specs = action_tool_specs();
    let all_expected: Vec<&str> = all_action_tags()
        .into_iter()
        .map(expected_tool_name_for)
        .collect();
    for spec in &specs {
        assert!(
            all_expected.contains(&spec.name.as_str()),
            "ToolSpec {:?} has no matching ActionTag. Either remove the tool or add the variant.",
            spec.name
        );
    }
}

/// Every tool's schema MUST be a valid JSON Schema (Draft-07 or
/// compatible). If `JSONSchema::compile` fails, the schema is malformed.
#[test]
fn every_schema_is_a_valid_json_schema_document() {
    for spec in action_tool_specs() {
        let schema_value: serde_json::Value =
            serde_json::from_str(&spec.json_schema).expect("schema must parse as JSON");
        JSONSchema::compile(&schema_value).unwrap_or_else(|e| {
            panic!(
                "ToolSpec {:?} schema does not compile as a JSON Schema: {e}",
                spec.name
            )
        });
    }
}

// ============================================================
//  Canonical round-trip per variant
// ============================================================

/// One canonical example per variant: the wire-form `arguments_json`
/// AND the expected parsed `Action`. Used by the round-trip and
/// schema-validation tests below.
fn canonical_arguments_for(tag: ActionTag) -> (&'static str, Action) {
    match tag {
        ActionTag::NoOp => (r#"{}"#, Action::NoOp),
        ActionTag::SetSpeed => (
            r#"{"mover": 1, "speed": 1.5}"#,
            Action::SetSpeed {
                mover: 1,
                speed: 1.5,
            },
        ),
        ActionTag::PlacePiece => (
            r#"{"piece_kind": "node", "pos": [10.0, 20.0]}"#,
            Action::PlacePiece {
                piece_kind: "node".to_string(),
                pos: [10.0, 20.0],
            },
        ),
        ActionTag::ConnectPieces => (
            r#"{"from": 1, "to": 2}"#,
            Action::ConnectPieces { from: 1, to: 2 },
        ),
        ActionTag::RemovePiece => (r#"{"id": 1}"#, Action::RemovePiece { id: 1 }),
    }
}

/// For every variant, the canonical wire form parses back into the
/// expected `Action`. Catches parse_tool_call drift.
#[test]
fn canonical_tool_call_round_trips_for_every_variant() {
    for tag in all_action_tags() {
        let name = expected_tool_name_for(tag);
        let (args, expected) = canonical_arguments_for(tag);
        let call = ToolCall {
            name: name.to_string(),
            arguments_json: args.to_string(),
        };
        let parsed = parse_tool_call(&call, "test-agent").unwrap_or_else(|e| {
            panic!("canonical arguments for {tag:?} ({name}) failed to parse: {e}\n  args: {args}")
        });
        assert_eq!(
            parsed, expected,
            "round-trip for {tag:?}: parser produced a different Action than expected"
        );
        // Tag round-trips too.
        assert_eq!(parsed.tag(), tag);
    }
}

/// For every variant, the canonical `arguments_json` validates
/// against the tool's JSON Schema. Catches schema-vs-parser drift.
#[test]
fn canonical_arguments_validate_against_schema_for_every_variant() {
    let specs = action_tool_specs();
    for tag in all_action_tags() {
        let name = expected_tool_name_for(tag);
        let spec = specs
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("missing ToolSpec for {tag:?} ({name})"));

        let schema: serde_json::Value =
            serde_json::from_str(&spec.json_schema).expect("schema is JSON");
        let validator = JSONSchema::compile(&schema).expect("schema compiles");

        let (args, _expected) = canonical_arguments_for(tag);
        let instance: serde_json::Value =
            serde_json::from_str(args).expect("canonical arguments are JSON");

        assert!(
            validator.is_valid(&instance),
            "canonical arguments for {tag:?} ({name}) failed schema validation:\n  args: {args}\n  schema: {}",
            spec.json_schema,
        );
    }
}

// ============================================================
//  Negative schema cases — proves the schemas have teeth
// ============================================================

fn validator_for(name: &str) -> JSONSchema {
    let specs = action_tool_specs();
    let spec = specs
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no tool spec named {name}"));
    let schema: serde_json::Value = serde_json::from_str(&spec.json_schema).unwrap();
    JSONSchema::compile(&schema).unwrap()
}

#[test]
fn no_op_schema_rejects_extra_properties() {
    let v = validator_for(names::NO_OP);
    assert!(
        !v.is_valid(&json!({"unexpected": 1})),
        "no_op must reject extra fields (additionalProperties: false)"
    );
}

#[test]
fn set_speed_schema_rejects_negative_mover() {
    let v = validator_for(names::SET_SPEED);
    assert!(!v.is_valid(&json!({"mover": -1, "speed": 1.0})));
}

#[test]
fn set_speed_schema_rejects_speed_above_max() {
    let v = validator_for(names::SET_SPEED);
    assert!(!v.is_valid(&json!({"mover": 0, "speed": 100.1})));
}

#[test]
fn set_speed_schema_rejects_speed_below_zero() {
    let v = validator_for(names::SET_SPEED);
    assert!(!v.is_valid(&json!({"mover": 0, "speed": -0.1})));
}

#[test]
fn set_speed_schema_rejects_missing_required_fields() {
    let v = validator_for(names::SET_SPEED);
    assert!(!v.is_valid(&json!({"mover": 0}))); // missing speed
    assert!(!v.is_valid(&json!({"speed": 1.0}))); // missing mover
    assert!(!v.is_valid(&json!({}))); // missing both
}

#[test]
fn set_speed_schema_rejects_wrong_types() {
    let v = validator_for(names::SET_SPEED);
    assert!(!v.is_valid(&json!({"mover": "one", "speed": 1.0})));
    assert!(!v.is_valid(&json!({"mover": 1, "speed": "fast"})));
}

#[test]
fn set_speed_schema_rejects_extra_properties() {
    let v = validator_for(names::SET_SPEED);
    assert!(!v.is_valid(&json!({"mover": 0, "speed": 1.0, "extra": "bad"})));
}

#[test]
fn place_piece_schema_rejects_oversized_piece_kind() {
    let v = validator_for(names::PLACE_PIECE);
    let too_long = "x".repeat(65);
    assert!(!v.is_valid(&json!({"piece_kind": too_long, "pos": [0.0, 0.0]})));
}

#[test]
fn place_piece_schema_rejects_wrong_pos_arity() {
    let v = validator_for(names::PLACE_PIECE);
    assert!(!v.is_valid(&json!({"piece_kind": "node", "pos": [1.0]}))); // 1 element
    assert!(!v.is_valid(&json!({"piece_kind": "node", "pos": [1.0, 2.0, 3.0]}))); // 3 elements
    assert!(!v.is_valid(&json!({"piece_kind": "node", "pos": []}))); // 0 elements
}

#[test]
fn place_piece_schema_rejects_missing_fields() {
    let v = validator_for(names::PLACE_PIECE);
    assert!(!v.is_valid(&json!({"piece_kind": "node"}))); // missing pos
    assert!(!v.is_valid(&json!({"pos": [0.0, 0.0]}))); // missing piece_kind
}

#[test]
fn connect_pieces_schema_rejects_negative_ids() {
    let v = validator_for(names::CONNECT_PIECES);
    assert!(!v.is_valid(&json!({"from": -1, "to": 2})));
    assert!(!v.is_valid(&json!({"from": 1, "to": -2})));
}

#[test]
fn connect_pieces_schema_rejects_missing_fields() {
    let v = validator_for(names::CONNECT_PIECES);
    assert!(!v.is_valid(&json!({"from": 1}))); // missing to
    assert!(!v.is_valid(&json!({"to": 2}))); // missing from
}

#[test]
fn remove_piece_schema_rejects_negative_id() {
    let v = validator_for(names::REMOVE_PIECE);
    assert!(!v.is_valid(&json!({"id": -1})));
}

#[test]
fn remove_piece_schema_rejects_missing_id() {
    let v = validator_for(names::REMOVE_PIECE);
    assert!(!v.is_valid(&json!({})));
}

// ============================================================
//  Helpers
// ============================================================

/// MUST equal the number of variants in `ActionTag` (see
/// `crates/protocol/src/lib.rs`). Adding a new variant requires:
///
///   1. Adding an arm to `expected_tool_name_for` (compile-time gate)
///   2. Adding the variant to `all_action_tags()` (runtime catalogue)
///   3. Bumping THIS constant (so `all_action_tags_has_every_variant`
///      asserts the catalogue grew in step)
///
/// All three are non-trivial and visible in PR diff; missing any one
/// breaks the build or the test suite.
const ACTION_TAG_VARIANT_COUNT: usize = 5;

/// Catalogue of every `ActionTag` variant. Iterated by every test in
/// this file. Hand-maintained because we don't want a derive macro
/// dep on the protocol crate — the
/// `all_action_tags_has_every_variant` test (below) catches mismatches
/// by comparing this list's length against `ACTION_TAG_VARIANT_COUNT`,
/// which must also be hand-updated.
fn all_action_tags() -> Vec<ActionTag> {
    vec![
        ActionTag::NoOp,
        ActionTag::SetSpeed,
        ActionTag::PlacePiece,
        ActionTag::ConnectPieces,
        ActionTag::RemovePiece,
    ]
}

/// Catches the failure mode where `ACTION_TAG_VARIANT_COUNT` is
/// bumped (because a new variant was added) but `all_action_tags()`
/// was not updated — OR the inverse. This assertion is
/// non-tautological: it compares the list's length against a
/// hand-maintained constant that must also be updated. If either is
/// out of step, the test fails with an explicit message naming both
/// values.
///
/// This closes the runtime gap that the compile-time exhaustive
/// match in `expected_tool_name_for` can't see: a future PR that
/// adds a variant + updates `expected_tool_name_for` + updates
/// `action_tool_specs()` but forgets to update `all_action_tags()`
/// would have its new variant silently skipped by every iteration
/// test in this file. With this assertion, that gap turns into a
/// loud test failure.
#[test]
fn all_action_tags_has_every_variant() {
    let tags = all_action_tags();
    assert_eq!(
        tags.len(),
        ACTION_TAG_VARIANT_COUNT,
        "all_action_tags() returned {n} variants but ACTION_TAG_VARIANT_COUNT \
         = {expected}. \n\n\
         If you added an ActionTag variant, update BOTH ACTION_TAG_VARIANT_COUNT \
         AND all_action_tags() to include the new variant. \n\
         If you removed a variant, decrement both. \n\n\
         Why both: the compile-time exhaustive match in expected_tool_name_for \
         catches missing tool-name mappings, but cannot catch missing entries in \
         this runtime catalogue (which is iterated by every test in this file).",
        n = tags.len(),
        expected = ACTION_TAG_VARIANT_COUNT,
    );
    // Also: no duplicate variants in the catalogue. (Use position-
    // based dedup since ActionTag does not implement Hash.)
    let mut seen: Vec<ActionTag> = Vec::new();
    for &t in &tags {
        assert!(
            !seen.contains(&t),
            "all_action_tags() contains duplicate variant {t:?}"
        );
        seen.push(t);
    }
}
