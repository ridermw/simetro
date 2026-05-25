//! `scenario_language_v1` skeleton integration test (PR 0).
//!
//! Loads the empty SL1 fixture, ticks the engine for a fixed number of
//! steps, and asserts:
//!   - the SL1 block validates and reaches the engine via
//!     `LoadedScene.sl1` and `World.sl1`;
//!   - the engine ticks deterministically (hash matches the committed
//!     baseline at `tests/baselines/sl1-empty.hash`);
//!   - `World::sl1_outcome()` reports `GameOutcome::InProgress`
//!     throughout (PR 8 introduces real terminal outcomes).
//!
//! Update procedure when a deliberate change to the SL1 skeleton lands:
//!   1. Inspect the failure diff and confirm the new hash is intended.
//!   2. Replace the contents of `tests/baselines/sl1-empty.hash`.
//!   3. Document the cause in the commit message.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{
    hash_run, load_scene_str, GameOutcome, LoadError, Sl1LoadError, TickRunner, SL1_SCHEMA_VERSION,
};

const SCENE: &str = include_str!("fixtures/sl1-empty.json");
const BASELINE: &str = include_str!("../../../tests/baselines/sl1-empty.hash");
const TICKS: u64 = 1_000;
const SEED: u64 = 42;

#[test]
fn sl1_empty_scene_loads_with_sl1_block() {
    let loaded = load_scene_str(SCENE, SEED).expect("sl1-empty scene should load");

    let sl1 = loaded
        .sl1
        .as_ref()
        .expect("scene declares scenario_language_v1; loader should attach it");
    assert_eq!(sl1.schema_version, SL1_SCHEMA_VERSION);
    assert!(sl1.places.is_empty());
    assert!(sl1.links.is_empty());
    assert!(sl1.things.is_empty());
    assert!(sl1.transforms.is_empty());
    assert!(sl1.demand.is_empty());
    assert!(sl1.pressure.is_empty());
    assert!(sl1.objectives.is_empty());
    assert!(sl1.failure_conditions.is_empty());
    assert!(sl1.agents.is_empty());
    assert!(sl1.observability.is_none());
    assert!(sl1.milestones.is_empty());

    assert!(
        loaded.world.sl1.is_some(),
        "World should carry the SL1 block alongside LoadedScene"
    );
}

#[test]
fn sl1_empty_scene_ticks_deterministically() {
    let mut loaded = load_scene_str(SCENE, SEED).expect("sl1-empty scene should load");
    let mut runner = TickRunner::new();
    let actual = hash_run(&mut loaded.world, &mut runner, TICKS);
    let expected = BASELINE.trim();
    assert_eq!(
        actual, expected,
        "scenario_language_v1 skeleton determinism baseline drift detected.\n\
         expected: {expected}\n\
         actual:   {actual}\n\
         If this drift is intentional, refresh tests/baselines/sl1-empty.hash."
    );
}

#[test]
fn sl1_empty_scene_remains_in_progress_after_ticks() {
    let mut loaded = load_scene_str(SCENE, SEED).expect("sl1-empty scene should load");
    let mut runner = TickRunner::new();

    assert_eq!(loaded.world.sl1_outcome(), GameOutcome::InProgress);

    // Drive the engine forward; PR 0 has no objective evaluator, so
    // the outcome must remain InProgress.
    for _ in 0..TICKS {
        let _ = runner.tick_once(&mut loaded.world);
        assert_eq!(loaded.world.sl1_outcome(), GameOutcome::InProgress);
    }
}

#[test]
fn loader_rejects_unknown_field_inside_sl1_block() {
    let json = r##"{
        "schema_version": 1,
        "name": "sl1-bad-field",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": {
            "mystery_field": 42
        }
    }"##;

    let err = load_scene_str(json, 0).expect_err("unknown SL1 field must be rejected");
    match err {
        LoadError::Sl1(Sl1LoadError::UnknownField { ref field }) => {
            assert_eq!(field, "mystery_field");
        }
        other => panic!("expected LoadError::Sl1(UnknownField), got {other:?}"),
    }
}

#[test]
fn loader_rejects_explicit_null_sl1_block() {
    // `scenario_language_v1: null` is not the same as omitting the
    // block — it must be rejected so a scene cannot bypass SL1
    // validation by writing an explicit null.
    let json = r##"{
        "schema_version": 1,
        "name": "sl1-null-block",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": null
    }"##;
    let err = load_scene_str(json, 0).expect_err("explicit null SL1 block must be rejected");
    match err {
        LoadError::Sl1(Sl1LoadError::ExpectedObject { found }) => {
            assert_eq!(found, "null");
        }
        other => panic!("expected LoadError::Sl1(ExpectedObject), got {other:?}"),
    }
}

#[test]
fn legacy_top_level_agents_still_loads_but_nested_agents_does_not() {
    // `agents` is intentionally excluded from the reserved-top-level
    // guard because legacy v1/v2 scenes use it at the top level
    // alongside `pieces`. This test documents both halves of that
    // intentional collision: top-level `agents` still loads, while a
    // nested SL1 `agents` block fails with PrimitiveNotImplemented
    // until PR 10 lands.

    let with_legacy_agents = r##"{
        "schema_version": 1,
        "name": "legacy-top-level-agents",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "agents": []
    }"##;
    load_scene_str(with_legacy_agents, 0)
        .expect("legacy top-level `agents` must still load (intentional collision)");

    let with_nested_agents = r##"{
        "schema_version": 1,
        "name": "sl1-nested-agents",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": {
            "agents": [{}]
        }
    }"##;
    let err = load_scene_str(with_nested_agents, 0)
        .expect_err("nested SL1 `agents` must fail until PR 10");
    match err {
        LoadError::Sl1(Sl1LoadError::PrimitiveNotImplemented { section }) => {
            assert_eq!(section, "agents");
        }
        other => panic!("expected LoadError::Sl1(PrimitiveNotImplemented), got {other:?}"),
    }
}

#[test]
fn loader_rejects_misplaced_top_level_sl1_primitive() {
    // A common authoring mistake: putting `places` at the scene's top
    // level instead of inside `scenario_language_v1`. Must surface as a
    // typed `Sl1ReservedKeyAtTopLevel` rather than silently ignoring it.
    let json = r##"{
        "schema_version": 1,
        "name": "sl1-misplaced-primitive",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "places": [{"id": "stranded"}]
    }"##;

    let err =
        load_scene_str(json, 0).expect_err("misplaced top-level SL1 primitive must be rejected");
    match err {
        LoadError::Sl1ReservedKeyAtTopLevel { name } => {
            assert_eq!(name, "places");
        }
        other => panic!("expected Sl1ReservedKeyAtTopLevel, got {other:?}"),
    }
}

#[test]
fn loader_rejects_non_empty_primitive_until_pr_lands() {
    // PRs 2–11 have no behavior for their primitive — even a
    // well-formed entry must fail load so authors cannot build
    // proto-SL1 scenes that silently no-op. PR 1 promoted `places`
    // to a typed primitive, so this regression test uses `links`
    // instead (still a placeholder until PR 2).
    let json = r##"{
        "schema_version": 1,
        "name": "sl1-non-empty-link",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": {
            "links": [{}]
        }
    }"##;
    let err =
        load_scene_str(json, 0).expect_err("non-empty SL1 placeholder primitive must be rejected");
    match err {
        LoadError::Sl1(Sl1LoadError::PrimitiveNotImplemented { section }) => {
            assert_eq!(section, "links");
        }
        other => panic!("expected LoadError::Sl1(PrimitiveNotImplemented), got {other:?}"),
    }
}

#[test]
fn loader_rejects_unsupported_sl1_schema_version() {
    let json = r##"{
        "schema_version": 1,
        "name": "sl1-bad-version",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": {
            "schema_version": 99
        }
    }"##;

    let err = load_scene_str(json, 0).expect_err("unsupported SL1 version must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("scenario_language_v1.schema_version"),
        "expected error to mention scenario_language_v1.schema_version, got: {msg}"
    );
}

#[test]
fn legacy_scene_without_sl1_block_still_loads() {
    // Borrowed from existing demo-paths scene; verifies adding the SL1
    // module is additive and that scenes omitting the block work.
    let json = r##"{
        "schema_version": 1,
        "name": "legacy-no-sl1",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": {
            "nodes": [{"id": "a", "pos": [0,0], "shape": "circle", "color": 1}],
            "paths": [],
            "movers": []
        },
        "goals": [{"type": "loop_forever"}]
    }"##;
    let loaded = load_scene_str(json, 0).expect("legacy scene should still load");
    assert!(loaded.sl1.is_none());
    assert!(loaded.world.sl1.is_none());
    assert_eq!(loaded.world.sl1_outcome(), GameOutcome::InProgress);
}

#[test]
fn loader_rejects_misspelled_scenario_language_top_level_key() {
    // Typo'd key (e → a swap) must NOT silently fall back to legacy and
    // drop the SL1 block. Without this guard the scene loads as legacy
    // and the author thinks SL1 objectives are active when they are not.
    let json = r##"{
        "schema_version": 1,
        "name": "typo-sl1",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_langauge_v1": {
            "schema_version": 1
        }
    }"##;

    let err = load_scene_str(json, 0).expect_err("typo'd SL1 key must be rejected");
    match err {
        LoadError::Sl1MisspelledTopLevelKey { name } => {
            assert_eq!(name, "scenario_langauge_v1");
        }
        other => panic!("expected Sl1MisspelledTopLevelKey, got {other:?}"),
    }
}

#[test]
fn loader_rejects_future_scenario_language_version_at_top_level() {
    // A future-versioned key must also be rejected rather than silently
    // dropped — surfacing as a typed error keeps tooling and authors in
    // sync with what the engine actually supports.
    let json = r##"{
        "schema_version": 1,
        "name": "future-sl1",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v2": {}
    }"##;

    let err = load_scene_str(json, 0).expect_err("future SL key must be rejected");
    match err {
        LoadError::Sl1MisspelledTopLevelKey { name } => {
            assert_eq!(name, "scenario_language_v2");
        }
        other => panic!("expected Sl1MisspelledTopLevelKey, got {other:?}"),
    }
}

#[test]
fn loader_rejects_array_sl1_block() {
    // `"scenario_language_v1": []` must surface as a typed
    // ExpectedObject error, not silently load as legacy.
    let json = r##"{
        "schema_version": 1,
        "name": "array-sl1",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": []
    }"##;
    let err = load_scene_str(json, 0).expect_err("array SL1 block must be rejected");
    match err {
        LoadError::Sl1(Sl1LoadError::ExpectedObject { found }) => {
            assert_eq!(found, "array");
        }
        other => panic!("expected Sl1(ExpectedObject{{found:\"array\"}}), got {other:?}"),
    }
}

#[test]
fn loader_rejects_string_sl1_block() {
    let json = r##"{
        "schema_version": 1,
        "name": "string-sl1",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": "foo"
    }"##;
    let err = load_scene_str(json, 0).expect_err("string SL1 block must be rejected");
    match err {
        LoadError::Sl1(Sl1LoadError::ExpectedObject { found }) => {
            assert_eq!(found, "string");
        }
        other => panic!("expected Sl1(ExpectedObject{{found:\"string\"}}), got {other:?}"),
    }
}

#[test]
fn loader_rejects_null_inside_sl1_primitive() {
    // `"places": null` inside the SL1 block is a type-shape error
    // (primitives must be arrays) and must surface as Sl1(Parse).
    let json = r##"{
        "schema_version": 1,
        "name": "null-primitive",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": {
            "schema_version": 1,
            "places": null
        }
    }"##;
    let err = load_scene_str(json, 0).expect_err("places:null must be rejected");
    match err {
        LoadError::Sl1(Sl1LoadError::Parse { message }) => {
            // serde's exact wording is "invalid type: null, expected a
            // sequence" — we don't pin the prose, just confirm we got
            // a typed Parse error rather than a silent acceptance.
            assert!(
                message.to_ascii_lowercase().contains("null")
                    || message.to_ascii_lowercase().contains("sequence"),
                "expected serde type-error message, got: {message}"
            );
        }
        other => panic!("expected Sl1(Parse), got {other:?}"),
    }
}

#[test]
fn loader_accepts_null_observability_as_omitted() {
    // Explicit `"observability": null` is documented as equivalent to
    // omitting the block — the example in
    // docs/scenario-language-v1.md uses exactly this form. Lock the
    // contract so a future refactor cannot silently regress it.
    let json = r##"{
        "schema_version": 1,
        "name": "null-observability",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": { "nodes": [], "paths": [], "movers": [] },
        "scenario_language_v1": {
            "schema_version": 1,
            "observability": null
        }
    }"##;
    let loaded = load_scene_str(json, 0).expect("null observability must be accepted");
    let sl1 = loaded.sl1.expect("SL1 block should be attached");
    assert!(sl1.observability.is_none());
}
