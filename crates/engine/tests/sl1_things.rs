//! `scenario_language_v1` Things integration tests (PR 3).
//!
//! Exercises:
//!   - Every new `Sl1LoadError::Thing*` variant via a minimal scene.
//!   - Cross-validation: `PlaceStorageUnknownThing`,
//!     `PlaceUnknownThingReference` (accepts + produces),
//!     `LinkCompatibilityUnknownReference`.
//!   - Runtime initialization from `places[*].storage[*].initial`.
//!   - Freshness aging Ok→Stale across the budget boundary.
//!   - Absent freshness budget = no aging.
//!   - f64 canonicalization in hash (-0.0 and 0.0 produce same hash).
//!   - Reordering-independence: declaration order in JSON does not
//!     change canonical hash or in-memory order.
//!   - All five `FreshnessStateView` round trips through serde.
//!   - The `sl1-things.json` fixture ticks deterministically against
//!     `tests/baselines/sl1-things.hash`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{
    encode_snapshot, encode_static, hash_run, load_scene_str, FreshnessState, LoadError,
    Sl1LoadError, TickRunner,
};
use simetro_protocol::{FreshnessStateView, SnapshotPayload};

const THINGS_SCENE: &str = include_str!("fixtures/sl1-things.json");
const THINGS_BASELINE: &str = include_str!("../../../tests/baselines/sl1-things.hash");
const TICKS: u64 = 200;

fn scene_with(places_json: &str, links_json: &str, things_json: &str) -> String {
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-things-test",
            "theme": {{
                "palette": ["#000000"],
                "background_index": 0,
                "font": "system-ui"
            }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places_json},
                "links": {links_json},
                "things": {things_json}
            }}
        }}"##
    )
}

fn scene_with_things(things_json: &str) -> String {
    scene_with("[]", "[]", things_json)
}

fn expect_sl1_err(json: String) -> Sl1LoadError {
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(e) => e,
        other => panic!("expected LoadError::Sl1, got {other:?}"),
    }
}

// -------------------------------------------------------------------
// Per-variant Thing* error coverage
// -------------------------------------------------------------------

#[test]
fn thing_duplicate_id_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[
            {"id": "x", "kind": "k", "tags": []},
            {"id": "x", "kind": "k2", "tags": []}
        ]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingDuplicateId { ref id } if id == "x"),
        "{err:?}"
    );
}

#[test]
fn thing_invalid_id_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "", "kind": "k", "tags": []}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingInvalidId { .. }),
        "{err:?}"
    );
}

#[test]
fn thing_empty_kind_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "", "tags": []}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingEmptyKind { ref id } if id == "a"),
        "{err:?}"
    );
}

#[test]
fn thing_empty_tag_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [""]}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingEmptyTag { ref id } if id == "a"),
        "{err:?}"
    );
}

#[test]
fn thing_duplicate_tag_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": ["t", "t"]}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::ThingDuplicateTag { ref id, ref value }
                if id == "a" && value == "t"
        ),
        "{err:?}"
    );
}

#[test]
fn thing_schema_version_zero_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [], "schema_version": 0}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingSchemaVersionZero { ref id } if id == "a"),
        "{err:?}"
    );
}

#[test]
fn thing_freshness_budget_zero_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [], "freshness_budget_ticks": 0}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingFreshnessBudgetZero { ref id } if id == "a"),
        "{err:?}"
    );
}

#[test]
fn thing_freshness_budget_out_of_range_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [],
            "freshness_budget_ticks": 1000000001}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingFreshnessBudgetOutOfRange { .. }),
        "{err:?}"
    );
}

#[test]
fn thing_quality_max_drop_percent_out_of_range_above_one_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [],
            "quality_contract": {"max_drop_percent": 1.5}}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::ThingQualityMaxDropPercentOutOfRange { .. }
        ),
        "{err:?}"
    );
}

#[test]
fn thing_quality_max_drop_percent_negative_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [],
            "quality_contract": {"max_drop_percent": -0.1}}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::ThingQualityMaxDropPercentOutOfRange { .. }
        ),
        "{err:?}"
    );
}

#[test]
fn thing_quality_max_late_ticks_zero_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [],
            "quality_contract": {"max_late_ticks": 0}}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingQualityMaxLateTicksZero { ref id } if id == "a"),
        "{err:?}"
    );
}

#[test]
fn thing_quality_max_late_ticks_out_of_range_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [],
            "quality_contract": {"max_late_ticks": 1000000001}}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingQualityMaxLateTicksOutOfRange { .. }),
        "{err:?}"
    );
}

#[test]
fn thing_quality_required_field_empty_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [],
            "quality_contract": {"required_fields": [""]}}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingQualityRequiredFieldEmpty { ref id } if id == "a"),
        "{err:?}"
    );
}

#[test]
fn thing_quality_required_field_duplicate_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [],
            "quality_contract": {"required_fields": ["f", "f"]}}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::ThingQualityRequiredFieldDuplicate { ref id, ref value }
                if id == "a" && value == "f"
        ),
        "{err:?}"
    );
}

#[test]
fn thing_empty_render_glyph_rejected() {
    let err = expect_sl1_err(scene_with_things(
        r#"[{"id": "a", "kind": "k", "tags": [],
            "render": {"glyph": ""}}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::ThingEmptyRenderGlyph { ref id } if id == "a"),
        "{err:?}"
    );
}

#[test]
fn thing_unknown_nested_field_is_parse_error() {
    match load_scene_str(
        &scene_with_things(r#"[{"id": "a", "kind": "k", "tags": [], "bogus": 1}]"#),
        0,
    )
    .expect_err("expected load failure")
    {
        LoadError::Sl1(Sl1LoadError::Parse { .. }) => {}
        other => panic!("expected Sl1::Parse, got {other:?}"),
    }
}

#[test]
fn thing_unknown_quality_contract_field_is_parse_error() {
    match load_scene_str(
        &scene_with_things(
            r#"[{"id": "a", "kind": "k", "tags": [],
                 "quality_contract": {"required_fields": [], "bogus": 1}}]"#,
        ),
        0,
    )
    .expect_err("expected load failure")
    {
        LoadError::Sl1(Sl1LoadError::Parse { .. }) => {}
        other => panic!("expected Sl1::Parse, got {other:?}"),
    }
}

#[test]
fn thing_unknown_render_field_is_parse_error() {
    match load_scene_str(
        &scene_with_things(
            r#"[{"id": "a", "kind": "k", "tags": [],
                 "render": {"glyph": "W", "bogus": 1}}]"#,
        ),
        0,
    )
    .expect_err("expected load failure")
    {
        LoadError::Sl1(Sl1LoadError::Parse { .. }) => {}
        other => panic!("expected Sl1::Parse, got {other:?}"),
    }
}

// -------------------------------------------------------------------
// Cross-validation
// -------------------------------------------------------------------

#[test]
fn place_storage_references_undeclared_thing() {
    let err = expect_sl1_err(scene_with(
        r#"[{"id": "p", "role": "r", "pos": [0,0],
              "storage": {"ghost": {"capacity": 5, "initial": 0}}}]"#,
        "[]",
        r#"[{"id": "real", "kind": "k", "tags": []}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::PlaceStorageUnknownThing { ref place_id, ref thing_id }
                if place_id == "p" && thing_id == "ghost"
        ),
        "{err:?}"
    );
}

#[test]
fn place_accepts_references_undeclared_thing_or_tag() {
    let err = expect_sl1_err(scene_with(
        r#"[{"id": "p", "role": "r", "pos": [0,0], "accepts": ["ghost"]}]"#,
        "[]",
        r#"[{"id": "real", "kind": "k", "tags": ["t"]}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::PlaceUnknownThingReference { ref place_id, field, ref value }
                if place_id == "p" && field == "accepts" && value == "ghost"
        ),
        "{err:?}"
    );
}

#[test]
fn place_produces_references_undeclared_thing_or_tag() {
    let err = expect_sl1_err(scene_with(
        r#"[{"id": "p", "role": "r", "pos": [0,0], "produces": ["ghost"]}]"#,
        "[]",
        r#"[{"id": "real", "kind": "k", "tags": ["t"]}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::PlaceUnknownThingReference { ref place_id, field, ref value }
                if place_id == "p" && field == "produces" && value == "ghost"
        ),
        "{err:?}"
    );
}

#[test]
fn place_accepts_by_tag_succeeds() {
    let json = scene_with(
        r#"[{"id": "p", "role": "r", "pos": [0,0], "accepts": ["t"]}]"#,
        "[]",
        r#"[{"id": "real", "kind": "k", "tags": ["t"]}]"#,
    );
    load_scene_str(&json, 0).expect("scene with accepts-by-tag should load");
}

#[test]
fn link_compatibility_references_undeclared_thing_or_tag() {
    let err = expect_sl1_err(scene_with(
        r#"[
            {"id": "a", "role": "r", "pos": [0,0]},
            {"id": "b", "role": "r", "pos": [1,0]}
        ]"#,
        r#"[{
            "id": "l", "type": "t", "from": "a", "to": "b",
            "direction": "forward", "capacity": {},
            "travel_ticks": 1, "compatibility": ["ghost"],
            "queue_capacity": 1, "backpressure": "block_upstream"
        }]"#,
        r#"[{"id": "real", "kind": "k", "tags": []}]"#,
    ));
    assert!(
        matches!(
            err,
            Sl1LoadError::LinkCompatibilityUnknownReference { ref id, ref value }
                if id == "l" && value == "ghost"
        ),
        "{err:?}"
    );
}

// -------------------------------------------------------------------
// Runtime / freshness behavior
// -------------------------------------------------------------------

#[test]
fn runtime_initial_inventories_seed_freshness_state() {
    let json = scene_with(
        r#"[{
            "id": "p", "role": "r", "pos": [0,0],
            "storage": {
                "fresh": {"capacity": 10, "initial": 5},
                "empty": {"capacity": 10, "initial": 0}
            }
        }]"#,
        "[]",
        r#"[
            {"id": "fresh", "kind": "k", "tags": [], "freshness_budget_ticks": 100},
            {"id": "empty", "kind": "k", "tags": [], "freshness_budget_ticks": 100}
        ]"#,
    );
    let loaded = load_scene_str(&json, 0).expect("scene should load");
    let runtime = loaded
        .world
        .sl1_runtime
        .as_ref()
        .expect("runtime should be present");
    let fresh_state = runtime
        .freshness
        .get(&("p".to_string(), "fresh".to_string()))
        .expect("fresh entry");
    assert!(matches!(
        fresh_state,
        FreshnessState::Ok { last_set_tick: 0 }
    ));
    let empty_state = runtime
        .freshness
        .get(&("p".to_string(), "empty".to_string()))
        .expect("empty entry");
    assert!(matches!(empty_state, FreshnessState::NoData));
    assert_eq!(
        *runtime.inventories.get("p").unwrap().get("fresh").unwrap(),
        5
    );
}

#[test]
fn freshness_ages_ok_to_stale_at_budget_plus_one() {
    let json = scene_with(
        r#"[{
            "id": "p", "role": "r", "pos": [0,0],
            "storage": {"x": {"capacity": 10, "initial": 1}}
        }]"#,
        "[]",
        r#"[{"id": "x", "kind": "k", "tags": [], "freshness_budget_ticks": 3}]"#,
    );
    let mut loaded = load_scene_str(&json, 0).expect("scene should load");
    let mut runner = TickRunner::new();
    let key = ("p".to_string(), "x".to_string());
    // tick 0: just loaded, age = 0, Ok.
    {
        let state = loaded
            .world
            .sl1_runtime
            .as_ref()
            .unwrap()
            .freshness
            .get(&key)
            .unwrap();
        assert!(matches!(state, FreshnessState::Ok { last_set_tick: 0 }));
    }
    // After 3 ticks: tick is 3, age = 3, still Ok (3 is not > 3).
    for _ in 0..3 {
        runner.tick_once(&mut loaded.world);
    }
    {
        let state = loaded
            .world
            .sl1_runtime
            .as_ref()
            .unwrap()
            .freshness
            .get(&key)
            .unwrap();
        assert!(matches!(state, FreshnessState::Ok { last_set_tick: 0 }));
    }
    // After 4 ticks total: tick is 4, age = 4, now Stale.
    runner.tick_once(&mut loaded.world);
    {
        let state = loaded
            .world
            .sl1_runtime
            .as_ref()
            .unwrap()
            .freshness
            .get(&key)
            .unwrap();
        assert!(matches!(state, FreshnessState::Stale { last_set_tick: 0 }));
    }
}

#[test]
fn absent_freshness_budget_never_ages() {
    let json = scene_with(
        r#"[{
            "id": "p", "role": "r", "pos": [0,0],
            "storage": {"x": {"capacity": 10, "initial": 1}}
        }]"#,
        "[]",
        r#"[{"id": "x", "kind": "k", "tags": []}]"#,
    );
    let mut loaded = load_scene_str(&json, 0).expect("scene should load");
    let mut runner = TickRunner::new();
    for _ in 0..100 {
        runner.tick_once(&mut loaded.world);
    }
    let state = loaded
        .world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .freshness
        .get(&("p".to_string(), "x".to_string()))
        .unwrap();
    assert!(matches!(state, FreshnessState::Ok { last_set_tick: 0 }));
}

// -------------------------------------------------------------------
// f64 canonicalization (-0.0 == 0.0 in hash)
// -------------------------------------------------------------------

#[test]
fn negative_zero_max_drop_percent_hashes_same_as_positive_zero() {
    let scene_pos_zero = scene_with(
        "[]",
        "[]",
        r#"[{"id": "x", "kind": "k", "tags": [],
            "quality_contract": {"max_drop_percent": 0.0}}]"#,
    );
    let scene_neg_zero = scene_with(
        "[]",
        "[]",
        r#"[{"id": "x", "kind": "k", "tags": [],
            "quality_contract": {"max_drop_percent": -0.0}}]"#,
    );
    let mut runner_a = TickRunner::new();
    let mut loaded_a = load_scene_str(&scene_pos_zero, 0).expect("pos zero loads");
    let hash_a = hash_run(&mut loaded_a.world, &mut runner_a, 10);

    let mut runner_b = TickRunner::new();
    let mut loaded_b = load_scene_str(&scene_neg_zero, 0).expect("neg zero loads");
    let hash_b = hash_run(&mut loaded_b.world, &mut runner_b, 10);

    assert_eq!(
        hash_a, hash_b,
        "+0.0 and -0.0 must hash identically (canonicalized)"
    );
}

// -------------------------------------------------------------------
// Reordering-independence
// -------------------------------------------------------------------

#[test]
fn things_declaration_order_does_not_affect_hash() {
    let order_1 = scene_with(
        "[]",
        "[]",
        r#"[
            {"id": "a", "kind": "k", "tags": ["t1", "t2"]},
            {"id": "b", "kind": "k", "tags": []},
            {"id": "c", "kind": "k", "tags": []}
        ]"#,
    );
    let order_2 = scene_with(
        "[]",
        "[]",
        r#"[
            {"id": "c", "kind": "k", "tags": []},
            {"id": "a", "kind": "k", "tags": ["t1", "t2"]},
            {"id": "b", "kind": "k", "tags": []}
        ]"#,
    );
    let mut runner_a = TickRunner::new();
    let mut loaded_a = load_scene_str(&order_1, 0).expect("order 1 loads");
    let hash_a = hash_run(&mut loaded_a.world, &mut runner_a, 5);

    let mut runner_b = TickRunner::new();
    let mut loaded_b = load_scene_str(&order_2, 0).expect("order 2 loads");
    let hash_b = hash_run(&mut loaded_b.world, &mut runner_b, 5);
    assert_eq!(hash_a, hash_b);
}

#[test]
fn tags_declaration_order_does_not_affect_hash() {
    let order_1 = scene_with(
        "[]",
        "[]",
        r#"[{"id": "x", "kind": "k", "tags": ["alpha", "beta", "gamma"]}]"#,
    );
    let order_2 = scene_with(
        "[]",
        "[]",
        r#"[{"id": "x", "kind": "k", "tags": ["gamma", "alpha", "beta"]}]"#,
    );
    let mut runner_a = TickRunner::new();
    let mut loaded_a = load_scene_str(&order_1, 0).expect("order 1 loads");
    let hash_a = hash_run(&mut loaded_a.world, &mut runner_a, 5);

    let mut runner_b = TickRunner::new();
    let mut loaded_b = load_scene_str(&order_2, 0).expect("order 2 loads");
    let hash_b = hash_run(&mut loaded_b.world, &mut runner_b, 5);
    assert_eq!(hash_a, hash_b);
}

// -------------------------------------------------------------------
// Protocol round trips
// -------------------------------------------------------------------

#[test]
fn freshness_state_view_round_trips_all_variants() {
    let variants = [
        FreshnessStateView::NoData,
        FreshnessStateView::Ok { last_set_tick: 7 },
        FreshnessStateView::Stale { last_set_tick: 42 },
        FreshnessStateView::Degraded,
        FreshnessStateView::Invalid,
    ];
    for v in variants {
        let s = serde_json::to_string(&v).expect("serialize");
        let back: FreshnessStateView = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(back, v, "round trip for {v:?}");
    }
}

#[test]
fn things_fixture_loads_and_static_payload_carries_them() {
    let loaded = load_scene_str(THINGS_SCENE, 0).expect("fixture should load");
    let stat = encode_static(&loaded);
    assert_eq!(stat.sl1_things.len(), 3, "fixture declares 3 things");
    assert_eq!(stat.sl1_things[0].id, "gadget");
    assert_eq!(stat.sl1_things[1].id, "raw_material");
    assert_eq!(stat.sl1_things[2].id, "widget");
    // Sorted by id.
    let widget = &stat.sl1_things[2];
    assert_eq!(widget.tags, vec!["consumable"]);
    assert_eq!(widget.schema_version, Some(2));
    assert_eq!(widget.freshness_budget_ticks, Some(100));
    let qc = widget.quality_contract.as_ref().expect("quality contract");
    assert_eq!(qc.max_drop_percent, Some(0.05));
    assert_eq!(qc.max_late_ticks, Some(30));
    assert_eq!(qc.required_fields, vec!["batch", "serial"]);
    let render = widget.render.as_ref().expect("render hint");
    assert_eq!(render.glyph, "W");
    assert_eq!(render.color, Some(1));
}

#[test]
fn snapshot_carries_inventories_with_freshness() {
    let mut loaded = load_scene_str(THINGS_SCENE, 0).expect("fixture should load");
    let mut snap = SnapshotPayload::default();
    encode_snapshot(&loaded.world, &mut snap);
    assert!(!snap.sl1_place_inventories.is_empty());
    // Each (place, thing) storage slot is present every tick.
    let slot = snap
        .sl1_place_inventories
        .iter()
        .find(|s| s.place_id == "warehouse" && s.thing_id == "widget")
        .expect("warehouse/widget slot present");
    assert_eq!(slot.count, 10);
    assert!(matches!(
        slot.freshness,
        FreshnessStateView::Ok { last_set_tick: 0 }
    ));
    let empty = snap
        .sl1_place_inventories
        .iter()
        .find(|s| s.place_id == "warehouse" && s.thing_id == "gadget")
        .expect("warehouse/gadget slot present");
    assert_eq!(empty.count, 0);
    assert!(matches!(empty.freshness, FreshnessStateView::NoData));
    // Snapshot re-encodes deterministically: clear+populate every call.
    let len_before = snap.sl1_place_inventories.len();
    // Advance the world; gadget budget=50, widget budget=100 — neither
    // should stale at tick 10. But we still expect the same slot count.
    let mut runner = TickRunner::new();
    for _ in 0..10 {
        runner.tick_once(&mut loaded.world);
    }
    encode_snapshot(&loaded.world, &mut snap);
    assert_eq!(snap.sl1_place_inventories.len(), len_before);
}

#[test]
fn things_fixture_ticks_deterministically_against_baseline() {
    let mut loaded = load_scene_str(THINGS_SCENE, 0).expect("fixture should load");
    let mut runner = TickRunner::new();
    let actual = hash_run(&mut loaded.world, &mut runner, TICKS);
    let expected = THINGS_BASELINE.trim();
    assert_eq!(
        actual, expected,
        "sl1-things determinism baseline drift detected.\n\
         expected: {expected}\n\
         actual:   {actual}\n\
         If this drift is intentional, refresh tests/baselines/sl1-things.hash."
    );
}
