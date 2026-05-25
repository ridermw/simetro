//! `scenario_language_v1` Place primitive integration tests (PR 1).
//!
//! Exercises:
//!   - Every new `Sl1LoadError::Place*` variant via a minimal scene.
//!   - The `sl1-places.json` fixture loads, the deterministic hash
//!     matches the committed baseline, and the StaticPayload carries
//!     a sorted `sl1_places` mirror.
//!
//! Update procedure when a deliberate Place schema change lands:
//!   1. Inspect the failure diff and confirm the new hash is intended.
//!   2. Replace the contents of `tests/baselines/sl1-places.hash`.
//!   3. Document the cause in the commit message.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{
    encode_static, hash_run, load_scene_str, LoadError, Sl1LoadError, TickRunner,
};
use simetro_protocol::Sl1OperatingPredicateView;

const PLACES_SCENE: &str = include_str!("fixtures/sl1-places.json");
const PLACES_BASELINE: &str = include_str!("../../../tests/baselines/sl1-places.hash");
const TICKS: u64 = 1_000;
const SEED: u64 = 42;

fn scene_with_places(places_json: &str) -> String {
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-place-test",
            "theme": {{
                "palette": ["#000000"],
                "background_index": 0,
                "font": "system-ui"
            }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places_json}
            }}
        }}"##
    )
}

fn expect_sl1_err(json: String) -> Sl1LoadError {
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(e) => e,
        other => panic!("expected LoadError::Sl1, got {other:?}"),
    }
}

#[test]
fn place_duplicate_id_rejected() {
    let err = expect_sl1_err(scene_with_places(
        r#"[
            {"id": "p1", "role": "x", "pos": [0,0]},
            {"id": "p1", "role": "y", "pos": [1,1]}
        ]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::PlaceDuplicateId { ref id } if id == "p1"),
        "got {err:?}"
    );
}

#[test]
fn place_invalid_id_rejected() {
    for bad in ["", "has space", "weird!"] {
        let err = expect_sl1_err(scene_with_places(&format!(
            r#"[{{"id":"{bad}","role":"x","pos":[0,0]}}]"#
        )));
        assert!(
            matches!(err, Sl1LoadError::PlaceInvalidId { .. }),
            "got {err:?} for id {bad:?}"
        );
    }
}

#[test]
fn place_empty_role_rejected() {
    let err = expect_sl1_err(scene_with_places(r#"[{"id":"p1","role":"","pos":[0,0]}]"#));
    assert!(
        matches!(err, Sl1LoadError::PlaceEmptyRole { .. }),
        "got {err:?}"
    );
}

#[test]
fn place_invalid_pos_rejected() {
    // Out-of-bounds and non-finite both rejected.
    for pos in ["[1e7,0]", "[0,1e8]"] {
        let err = expect_sl1_err(scene_with_places(&format!(
            r#"[{{"id":"p1","role":"r","pos":{pos}}}]"#
        )));
        assert!(
            matches!(err, Sl1LoadError::PlaceInvalidPos { .. }),
            "got {err:?}"
        );
    }
}

#[test]
fn place_storage_capacity_zero_rejected() {
    let err = expect_sl1_err(scene_with_places(
        r#"[{
            "id":"p1","role":"r","pos":[0,0],
            "storage": {"bin": {"capacity": 0, "initial": 0}}
        }]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::PlaceStorageCapacityZero { .. }),
        "got {err:?}"
    );
}

#[test]
fn place_storage_initial_exceeds_capacity_rejected() {
    let err = expect_sl1_err(scene_with_places(
        r#"[{
            "id":"p1","role":"r","pos":[0,0],
            "storage": {"bin": {"capacity": 5, "initial": 10}}
        }]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::PlaceStorageInitialExceedsCapacity {
                initial: 10,
                capacity: 5,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn place_duplicate_set_entry_rejected() {
    for (field, list) in [
        ("accepts", "[\"a\",\"a\"]"),
        ("produces", "[\"x\",\"x\"]"),
        ("failure_domains", "[\"d\",\"d\"]"),
    ] {
        let err = expect_sl1_err(scene_with_places(&format!(
            r#"[{{"id":"p1","role":"r","pos":[0,0],"{field}":{list}}}]"#
        )));
        assert!(
            matches!(err, Sl1LoadError::PlaceDuplicateEntry { field: f, .. } if f == field),
            "got {err:?} for field {field}"
        );
    }
}

#[test]
fn place_empty_entry_rejected() {
    let err = expect_sl1_err(scene_with_places(
        r#"[{"id":"p1","role":"r","pos":[0,0],"accepts":[""]}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::PlaceEmptyEntry {
                field: "accepts",
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn place_unsupported_predicate_rejected() {
    let err = expect_sl1_err(scene_with_places(
        r#"[{
            "id":"p1","role":"r","pos":[0,0],
            "operating_states": {"bad": {"when": "this is not a predicate"}}
        }]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::PlaceUnsupportedPredicate { .. }),
        "got {err:?}"
    );
}

#[test]
fn place_percent_threshold_out_of_range_rejected() {
    let err = expect_sl1_err(scene_with_places(
        r#"[{
            "id":"p1","role":"r","pos":[0,0],
            "operating_states": {"bad": {"when": "slots.used_percent >= 150"}}
        }]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::PlacePercentThresholdOutOfRange { threshold: 150, .. }
        ),
        "got {err:?}"
    );
}

#[test]
fn place_empty_operating_state_name_rejected() {
    let err = expect_sl1_err(scene_with_places(
        r#"[{
            "id":"p1","role":"r","pos":[0,0],
            "operating_states": {"": {"when": "slots.used_percent >= 50"}}
        }]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::PlaceEmptyOperatingStateName { .. }),
        "got {err:?}"
    );
}

#[test]
fn place_unknown_nested_field_is_parse_error() {
    // `deny_unknown_fields` on RawSl1Place catches typos like `pso`
    // (instead of `pos`) at the serde layer, so the loader surfaces
    // a typed `Sl1LoadError::Parse`.
    let err = expect_sl1_err(scene_with_places(
        r#"[{"id":"p1","role":"r","pos":[0,0],"pso":[1,1]}]"#,
    ));
    assert!(matches!(err, Sl1LoadError::Parse { .. }), "got {err:?}");
}

#[test]
fn places_fixture_loads_and_static_payload_carries_them() {
    let loaded = load_scene_str(PLACES_SCENE, SEED).expect("fixture should load");
    let sl1 = loaded.sl1.as_ref().expect("SL1 block present");
    assert_eq!(sl1.places.len(), 3);

    // Places are sorted by id, so order is dashboard, gpu-fleet,
    // kusto-cluster.
    let ids: Vec<&str> = sl1.places.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["dashboard", "gpu-fleet", "kusto-cluster"]);

    // Set-like fields are canonicalized.
    let kusto = sl1.places.iter().find(|p| p.id == "kusto-cluster").unwrap();
    assert_eq!(kusto.accepts, vec!["ingestion".to_string(), "query".into()]);
    assert_eq!(
        kusto.failure_domains,
        vec!["az1".to_string(), "data-platform".into(), "eastus".into()]
    );

    let static_payload = encode_static(&loaded);
    assert_eq!(static_payload.sl1_places.len(), 3);
    assert_eq!(
        static_payload
            .sl1_places
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["dashboard", "gpu-fleet", "kusto-cluster"]
    );

    let kusto_view = &static_payload.sl1_places[2];
    let strained = kusto_view
        .operating_states
        .get("strained")
        .expect("strained state");
    match &strained.predicate {
        Sl1OperatingPredicateView::UsedPercentGte { metric, threshold } => {
            assert_eq!(metric, "query_slots");
            assert_eq!(*threshold, 80);
        }
        other => panic!("unexpected predicate: {other:?}"),
    }
    let failed = kusto_view
        .operating_states
        .get("failed")
        .expect("failed state");
    match &failed.predicate {
        Sl1OperatingPredicateView::OverloadedTicksGt { ticks } => assert_eq!(*ticks, 600),
        other => panic!("unexpected predicate: {other:?}"),
    }
}

#[test]
fn places_fixture_ticks_deterministically_against_baseline() {
    let mut loaded = load_scene_str(PLACES_SCENE, SEED).expect("fixture should load");
    let mut runner = TickRunner::new();
    let actual = hash_run(&mut loaded.world, &mut runner, TICKS);
    let expected = PLACES_BASELINE.trim();
    assert_eq!(
        actual, expected,
        "sl1-places determinism baseline drift detected.\n\
         expected: {expected}\n\
         actual:   {actual}\n\
         If this drift is intentional, refresh tests/baselines/sl1-places.hash."
    );
}
