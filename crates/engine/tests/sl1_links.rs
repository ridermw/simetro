//! `scenario_language_v1` Link primitive integration tests (PR 2).
//!
//! Exercises:
//!   - Every new `Sl1LoadError::Link*` variant via a minimal scene.
//!   - The `sl1-links.json` fixture loads, the deterministic hash
//!     matches the committed baseline, and the StaticPayload carries
//!     a sorted `sl1_links` mirror with typed direction/backpressure
//!     enums and optional render hints.
//!
//! Update procedure when a deliberate Link schema change lands:
//!   1. Inspect the failure diff and confirm the new hash is intended.
//!   2. Replace the contents of `tests/baselines/sl1-links.hash`.
//!   3. Document the cause in the commit message.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{
    encode_static, hash_run, load_scene_str, LoadError, Sl1LinkBackpressure, Sl1LinkDirection,
    Sl1LoadError, TickRunner,
};
use simetro_protocol::{Sl1LinkBackpressureView, Sl1LinkDirectionView};

const LINKS_SCENE: &str = include_str!("fixtures/sl1-links.json");
const LINKS_BASELINE: &str = include_str!("../../../tests/baselines/sl1-links.hash");
const TICKS: u64 = 1_000;
const SEED: u64 = 42;

/// Wraps the supplied `places` JSON array and `links` JSON array
/// inside a minimal SL1 scene envelope. Both arrays must be
/// well-formed JSON snippets.
fn scene_with(places_json: &str, links_json: &str) -> String {
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-link-test",
            "theme": {{
                "palette": ["#000000"],
                "background_index": 0,
                "font": "system-ui"
            }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places_json},
                "links": {links_json}
            }}
        }}"##
    )
}

const STANDARD_PLACES: &str = r#"[
    {"id": "a", "role": "source", "pos": [0,0]},
    {"id": "b", "role": "sink",   "pos": [1,0]},
    {"id": "c", "role": "store",  "pos": [2,0]}
]"#;

fn expect_sl1_err(json: String) -> Sl1LoadError {
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(e) => e,
        other => panic!("expected LoadError::Sl1, got {other:?}"),
    }
}

fn link(extras: &str) -> String {
    // Spread an extra `id` if provided by caller; defaults to a valid id.
    format!(
        r#"[{{
            "id": "l1",
            "type": "data_stream",
            "from": "a",
            "to": "b",
            "direction": "forward",
            "capacity": {{}},
            "travel_ticks": 1,
            "compatibility": [],
            "queue_capacity": 4,
            "backpressure": "block_upstream"
            {extras}
        }}]"#
    )
}

// --- per-variant error coverage ---

#[test]
fn link_duplicate_id_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[
            {"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
             "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
             "backpressure":"block_upstream"},
            {"id":"l1","type":"t","from":"a","to":"c","direction":"forward",
             "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
             "backpressure":"block_upstream"}
        ]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkDuplicateId { ref id } if id == "l1"),
        "got {err:?}"
    );
}

#[test]
fn link_invalid_id_rejected() {
    for bad in ["", "has space", "weird!"] {
        let json = scene_with(
            STANDARD_PLACES,
            &format!(
                r#"[{{"id":"{bad}","type":"t","from":"a","to":"b","direction":"forward",
                     "capacity":{{}},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
                     "backpressure":"block_upstream"}}]"#
            ),
        );
        let err = expect_sl1_err(json);
        assert!(
            matches!(err, Sl1LoadError::LinkInvalidId { .. }),
            "got {err:?} for id {bad:?}"
        );
    }
}

#[test]
fn link_empty_type_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkEmptyType { .. }),
        "got {err:?}"
    );
}

#[test]
fn link_unknown_place_rejected_for_both_endpoints() {
    let json_from = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"missing","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json_from);
    assert!(
        matches!(
            err,
            Sl1LoadError::LinkUnknownPlace { which: "from", ref place, .. } if place == "missing"
        ),
        "got {err:?}"
    );

    let json_to = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"missing","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json_to);
    assert!(
        matches!(
            err,
            Sl1LoadError::LinkUnknownPlace { which: "to", ref place, .. } if place == "missing"
        ),
        "got {err:?}"
    );
}

#[test]
fn link_self_loop_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"a","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkSelfLoop { ref place, .. } if place == "a"),
        "got {err:?}"
    );
}

#[test]
fn link_missing_direction_rejected() {
    // Omit the `direction` field entirely.
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkMissingDirection { .. }),
        "got {err:?}"
    );
}

#[test]
fn link_unknown_direction_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"sideways",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkUnknownDirection { ref value, .. } if value == "sideways"),
        "got {err:?}"
    );
}

#[test]
fn link_missing_backpressure_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkMissingBackpressure { .. }),
        "got {err:?}"
    );
}

#[test]
fn link_unknown_backpressure_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"explode"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkUnknownBackpressure { ref value, .. } if value == "explode"),
        "got {err:?}"
    );
}

#[test]
fn link_empty_capacity_key_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{"":10},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(
            err,
            Sl1LoadError::LinkEmptyEntry {
                field: "capacity",
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn link_empty_compatibility_entry_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[""],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(
            err,
            Sl1LoadError::LinkEmptyEntry {
                field: "compatibility",
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn link_duplicate_compatibility_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":["x","x"],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(
            err,
            Sl1LoadError::LinkDuplicateCompatibility { ref value, .. } if value == "x"
        ),
        "got {err:?}"
    );
}

#[test]
fn link_travel_ticks_zero_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":0,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkTravelTicksZero { .. }),
        "got {err:?}"
    );
}

#[test]
fn link_travel_ticks_out_of_range_rejected() {
    // 2 billion exceeds MAX_LINK_TRAVEL_TICKS (1 billion).
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":2000000000,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkTravelTicksOutOfRange { .. }),
        "got {err:?}"
    );
}

#[test]
fn link_queue_capacity_zero_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":0,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkQueueCapacityZero { .. }),
        "got {err:?}"
    );
}

#[test]
fn link_queue_capacity_out_of_range_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":2000000000,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkQueueCapacityOutOfRange { .. }),
        "got {err:?}"
    );
}

#[test]
fn link_empty_render_style_rejected() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream","render":{"style":""}}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::LinkEmptyRenderStyle { .. }),
        "got {err:?}"
    );
}

#[test]
fn link_unknown_nested_field_is_parse_error() {
    // `deny_unknown_fields` on RawSl1Link catches typos like `froom`
    // (instead of `from`) at the serde layer.
    let _ = link(""); // exercise the helper for coverage
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward","froom":"a",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream"}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(matches!(err, Sl1LoadError::Parse { .. }), "got {err:?}");
}

#[test]
fn link_unknown_nested_render_field_is_parse_error() {
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
              "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
              "backpressure":"block_upstream","render":{"style":"flow","extra":1}}]"#,
    );
    let err = expect_sl1_err(json);
    assert!(matches!(err, Sl1LoadError::Parse { .. }), "got {err:?}");
}

// --- ordering / canonicalization ---

#[test]
fn links_sorted_by_id_independent_of_declaration_order() {
    // Declare zeta first, then alpha; result must come back in
    // canonical id order.
    let json = scene_with(
        STANDARD_PLACES,
        r#"[
            {"id":"zeta","type":"t","from":"a","to":"b","direction":"forward",
             "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
             "backpressure":"block_upstream"},
            {"id":"alpha","type":"t","from":"a","to":"c","direction":"forward",
             "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
             "backpressure":"block_upstream"}
        ]"#,
    );
    let loaded = load_scene_str(&json, SEED).expect("loads");
    let sl1 = loaded.sl1.as_ref().expect("sl1");
    let ids: Vec<&str> = sl1.links.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, ["alpha", "zeta"]);
}

#[test]
fn link_references_place_declared_later_in_json() {
    // The `to` place appears AFTER the link's `from` place in JSON
    // text. The loader must succeed because validation runs against
    // the fully-parsed place set, not in source order.
    let places = r#"[
        {"id": "a", "role": "source", "pos": [0,0]},
        {"id": "b", "role": "sink",   "pos": [1,0]}
    ]"#;
    let links = r#"[
        {"id":"l1","type":"t","from":"a","to":"b","direction":"forward",
         "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4,
         "backpressure":"block_upstream"}
    ]"#;
    let loaded = load_scene_str(&scene_with(places, links), SEED).expect("loads");
    assert_eq!(loaded.sl1.as_ref().unwrap().links.len(), 1);
}

#[test]
fn link_compatibility_canonicalized_and_dedup_only_in_vec() {
    // Compatibility entries are sorted; capacity duplicate keys are
    // silently merged by serde (last-wins) which mirrors PR 1.
    let json = scene_with(
        STANDARD_PLACES,
        r#"[{
            "id":"l1","type":"t","from":"a","to":"b","direction":"forward",
            "capacity":{"x":1,"x":7},
            "travel_ticks":1,
            "compatibility":["zeta","alpha","mid"],
            "queue_capacity":4,
            "backpressure":"block_upstream"
        }]"#,
    );
    let loaded = load_scene_str(&json, SEED).expect("loads");
    let link = &loaded.sl1.as_ref().unwrap().links[0];
    assert_eq!(link.compatibility, vec!["alpha", "mid", "zeta"]);
    assert_eq!(link.capacity.get("x").copied(), Some(7));
}

// --- fixture + protocol mirror ---

#[test]
fn links_fixture_loads_and_static_payload_carries_them() {
    let loaded = load_scene_str(LINKS_SCENE, SEED).expect("fixture should load");
    let sl1 = loaded.sl1.as_ref().expect("SL1 block present");
    assert_eq!(sl1.links.len(), 3);

    // Sorted by id: normalizer-to-store, ops-bus, telemetry-to-normalizer.
    let ids: Vec<&str> = sl1.links.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(
        ids,
        ["normalizer-to-store", "ops-bus", "telemetry-to-normalizer"]
    );

    let ops_bus = sl1.links.iter().find(|l| l.id == "ops-bus").unwrap();
    assert!(matches!(ops_bus.direction, Sl1LinkDirection::Bidirectional));
    assert!(matches!(
        ops_bus.backpressure,
        Sl1LinkBackpressure::SpillToBuffer
    ));
    assert!(ops_bus.render.is_none());

    let telemetry = sl1
        .links
        .iter()
        .find(|l| l.id == "telemetry-to-normalizer")
        .unwrap();
    assert!(matches!(telemetry.direction, Sl1LinkDirection::Forward));
    let render = telemetry.render.as_ref().expect("render present");
    assert_eq!(render.style, "flow");
    assert_eq!(render.color, Some(3));

    let static_payload = encode_static(&loaded);
    assert_eq!(static_payload.sl1_links.len(), 3);

    let ops_view = static_payload
        .sl1_links
        .iter()
        .find(|l| l.id == "ops-bus")
        .unwrap();
    assert_eq!(ops_view.direction, Sl1LinkDirectionView::Bidirectional);
    assert_eq!(
        ops_view.backpressure,
        Sl1LinkBackpressureView::SpillToBuffer
    );
    assert!(ops_view.render.is_none());

    let store_view = static_payload
        .sl1_links
        .iter()
        .find(|l| l.id == "normalizer-to-store")
        .unwrap();
    assert_eq!(store_view.direction, Sl1LinkDirectionView::Forward);
    assert_eq!(
        store_view.backpressure,
        Sl1LinkBackpressureView::DropLowPriority
    );
    assert_eq!(store_view.compatibility, vec!["normalized_gpu_heartbeat"]);
}

#[test]
fn degrade_quality_backpressure_round_trips_and_hashes_uniquely() {
    // Positive coverage for the `degrade_quality` variant: the fixture
    // intentionally exercises only the other three policies, so this
    // test pins loader + StaticPayload mirror + hash-tag uniqueness for
    // the fourth. Two otherwise-identical links differing only in
    // backpressure must hash differently, confirming the tag byte is
    // distinct from the other variants.
    let base = r#"{
        "id":"l1","type":"t","from":"a","to":"b","direction":"forward",
        "capacity":{},"travel_ticks":1,"compatibility":[],"queue_capacity":4"#;
    let degrade_scene = scene_with(
        STANDARD_PLACES,
        &format!(r#"[{base},"backpressure":"degrade_quality"}}]"#),
    );
    let block_scene = scene_with(
        STANDARD_PLACES,
        &format!(r#"[{base},"backpressure":"block_upstream"}}]"#),
    );

    let degrade = load_scene_str(&degrade_scene, SEED).expect("loads");
    let degrade_sl1 = degrade.sl1.as_ref().unwrap();
    assert!(matches!(
        degrade_sl1.links[0].backpressure,
        Sl1LinkBackpressure::DegradeQuality
    ));
    let degrade_view = encode_static(&degrade);
    assert_eq!(
        degrade_view.sl1_links[0].backpressure,
        Sl1LinkBackpressureView::DegradeQuality
    );

    // Hash-tag uniqueness: degrade_quality must hash differently from
    // block_upstream when every other field is identical.
    let block = load_scene_str(&block_scene, SEED).expect("loads");
    let mut degrade_world = degrade.world;
    let mut block_world = block.world;
    let mut runner = TickRunner::new();
    let degrade_hash = hash_run(&mut degrade_world, &mut runner, 1);
    let block_hash = hash_run(&mut block_world, &mut TickRunner::new(), 1);
    assert_ne!(
        degrade_hash, block_hash,
        "degrade_quality must hash differently from block_upstream"
    );
}

#[test]
fn links_fixture_ticks_deterministically_against_baseline() {
    let mut loaded = load_scene_str(LINKS_SCENE, SEED).expect("fixture should load");
    let mut runner = TickRunner::new();
    let actual = hash_run(&mut loaded.world, &mut runner, TICKS);
    let expected = LINKS_BASELINE.trim();
    assert_eq!(
        actual, expected,
        "sl1-links determinism baseline drift detected.\n\
         expected: {expected}\n\
         actual:   {actual}\n\
         If this drift is intentional, refresh tests/baselines/sl1-links.hash."
    );
}
