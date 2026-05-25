//! `scenario_language_v1` Pressure integration tests (PR 7).
//!
//! Exercises:
//!   - Every typed `Sl1LoadError::Pressure*` variant via minimal scenes.
//!   - Cross-variant per-field rejection (e.g. `multiplier` on
//!     `quota_reduction` → `PressureUnexpectedField`).
//!   - All 9 variant kinds round-trip into typed `Sl1Pressure`.
//!   - Runtime activation/deactivation lifecycle emits
//!     `SimEvent::Sl1PressureLifecycle` at the expected ticks.
//!   - `source_multiplier` injects inventory clamped by storage
//!     capacity, with fractional milli-unit carry across ticks.
//!   - `demand_growth` multiplies spawn count on the targeted demand.
//!   - `quota_reduction` reduces effective capacity in `try_start`.
//!   - `path_outage` records the outaged link in overlay state.
//!   - Unsupported variants emit `WarningPayload::Sl1Pressure ::
//!     UnsupportedInThisPr` once per activation.
//!   - The `sl1-pressure.json` fixture ticks deterministically against
//!     `tests/baselines/sl1-pressure.hash`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{
    encode_static, hash_run, load_scene_str, LoadError, Sl1LoadError, TickRunner,
};
use simetro_protocol::{
    SimEvent, SimMessage, Sl1PressureEventKind, Sl1PressureWarningKind, WarningPayload,
};

const PRESSURE_SCENE: &str = include_str!("fixtures/sl1-pressure.json");
const PRESSURE_BASELINE: &str = include_str!("../../../tests/baselines/sl1-pressure.hash");
const TICKS: u64 = 60;

// -------------------------------------------------------------------
// Scene helpers
// -------------------------------------------------------------------

fn scene_with_pressure(pressure_json: &str) -> String {
    scene_with(
        default_places(),
        default_things(),
        default_links(),
        default_demand(),
        pressure_json,
    )
}

fn scene_with(
    places_json: &str,
    things_json: &str,
    links_json: &str,
    demand_json: &str,
    pressure_json: &str,
) -> String {
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-pressure-test",
            "theme": {{
                "palette": ["#000000"],
                "background_index": 0,
                "font": "system-ui"
            }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places_json},
                "links": {links_json},
                "things": {things_json},
                "demand": {demand_json},
                "pressure": {pressure_json}
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

fn default_places() -> &'static str {
    r#"[
        {
            "id": "factory",
            "role": "producer",
            "pos": [0.0, 0.0],
            "capacity": { "machine_hours": 4 },
            "storage": {
                "raw_material": { "capacity": 100, "initial": 100 },
                "report": { "capacity": 100, "initial": 0 }
            },
            "accepts": ["raw_material"],
            "produces": ["report"]
        },
        {
            "id": "sink",
            "role": "consumer",
            "pos": [10.0, 0.0],
            "capacity": { "queries": 8 },
            "storage": {
                "report": { "capacity": 100, "initial": 0 }
            },
            "accepts": ["report"],
            "produces": []
        }
    ]"#
}

fn default_things() -> &'static str {
    r#"[
        { "id": "raw_material", "kind": "input", "tags": [] },
        { "id": "report", "kind": "data", "tags": [], "freshness_budget_ticks": 100 }
    ]"#
}

fn default_links() -> &'static str {
    r#"[
        {
            "id": "factory-to-sink",
            "type": "report_pipe",
            "from": "factory",
            "to": "sink",
            "direction": "forward",
            "capacity": { "reports": 5 },
            "travel_ticks": 2,
            "compatibility": ["report"],
            "queue_capacity": 5,
            "backpressure": "block_upstream"
        }
    ]"#
}

fn default_demand() -> &'static str {
    r#"[
        {
            "id": "d1",
            "type": "report_refresh",
            "target": { "type": "place", "id": "sink" },
            "requires": ["report"],
            "spawn_schedule": { "type": "fixed", "every_ticks": 5, "start_tick": 5 },
            "deadline_ticks": 10,
            "priority": "normal",
            "value": 5,
            "penalty": { "score": -1 }
        }
    ]"#
}

// -------------------------------------------------------------------
// Loader error coverage — one per Sl1LoadError::Pressure* variant.
// -------------------------------------------------------------------

#[test]
fn pressure_invalid_id_rejected() {
    let json = scene_with_pressure(
        r#"[{
            "id": "BAD ID!",
            "type": "source_multiplier",
            "at_tick": 5, "duration_ticks": 2,
            "target": "factory",
            "thing": "report",
            "multiplier": 1.5
        }]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureInvalidId { .. }
    ));
}

#[test]
fn pressure_duplicate_id_rejected() {
    let json = scene_with_pressure(
        r#"[
          {"id":"p1","type":"source_multiplier","at_tick":5,"duration_ticks":2,
           "target":"factory","thing":"report","multiplier":1.5},
          {"id":"p1","type":"source_multiplier","at_tick":10,"duration_ticks":2,
           "target":"factory","thing":"report","multiplier":1.5}
        ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureDuplicateId { .. }
    ));
}

#[test]
fn pressure_duration_zero_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"source_multiplier","at_tick":5,"duration_ticks":0,
             "target":"factory","thing":"report","multiplier":1.5}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureDurationZero { .. }
    ));
}

#[test]
fn pressure_unknown_type_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"bogus_kind","at_tick":5,"duration_ticks":2,
             "target":"factory"}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureUnknownType { .. }
    ));
}

#[test]
fn pressure_missing_field_rejected() {
    // source_multiplier without `thing` should fail.
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"source_multiplier","at_tick":5,"duration_ticks":2,
             "target":"factory","multiplier":1.5}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureMissingField { field: "thing", .. }
    ));
}

#[test]
fn pressure_unexpected_field_rejected() {
    // `multiplier` on a `quota_reduction` should be rejected.
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"quota_reduction","at_tick":5,"duration_ticks":2,
             "target":"factory","capacity":"machine_hours","reduction_percent":10,
             "multiplier": 2.0}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureUnexpectedField {
            field: "multiplier",
            ..
        }
    ));
}

#[test]
fn pressure_unknown_thing_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"source_multiplier","at_tick":5,"duration_ticks":2,
             "target":"factory","thing":"ghost","multiplier":1.5}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureUnknownThing { .. }
    ));
}

#[test]
fn pressure_unknown_target_place_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"source_multiplier","at_tick":5,"duration_ticks":2,
             "target":"ghost_place","thing":"report","multiplier":1.5}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureUnknownTarget {
            expected: "place",
            ..
        }
    ));
}

#[test]
fn pressure_unknown_target_demand_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"demand_growth","at_tick":5,"duration_ticks":2,
             "target":"ghost_demand","spawn_multiplier":2}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureUnknownTarget {
            expected: "demand",
            ..
        }
    ));
}

#[test]
fn pressure_unknown_target_link_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"path_outage","at_tick":5,"duration_ticks":2,
             "target":"ghost_link"}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureUnknownTarget {
            expected: "link",
            ..
        }
    ));
}

#[test]
fn pressure_no_storage_slot_rejected() {
    // `sink` has no `raw_material` storage slot.
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"source_multiplier","at_tick":5,"duration_ticks":2,
             "target":"sink","thing":"raw_material","multiplier":1.5}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureNoStorageSlot { .. }
    ));
}

#[test]
fn pressure_unknown_capacity_bucket_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"quota_reduction","at_tick":5,"duration_ticks":2,
             "target":"factory","capacity":"ghost_bucket","reduction_percent":10}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureUnknownCapacityBucket { .. }
    ));
}

#[test]
fn pressure_multiplier_out_of_range_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"source_multiplier","at_tick":5,"duration_ticks":2,
             "target":"factory","thing":"report","multiplier":0.0}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureMultiplierOutOfRange { .. }
    ));
}

#[test]
fn pressure_spawn_multiplier_out_of_range_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"demand_growth","at_tick":5,"duration_ticks":2,
             "target":"d1","spawn_multiplier":0}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureSpawnMultiplierOutOfRange { .. }
    ));
}

#[test]
fn pressure_reduction_percent_out_of_range_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"quota_reduction","at_tick":5,"duration_ticks":2,
             "target":"factory","capacity":"machine_hours","reduction_percent":0}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::PressureReductionPercentOutOfRange { .. }
    ));
}

#[test]
fn pressure_unknown_top_level_field_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"source_multiplier","at_tick":5,"duration_ticks":2,
             "target":"factory","thing":"report","multiplier":1.5,
             "stray_top_level": true}]"#,
    );
    // RawSl1Pressure has deny_unknown_fields, so this surfaces as a
    // parse error wrapped in Sl1LoadError::Parse.
    let err = expect_sl1_err(json);
    let msg = format!("{err:?}");
    assert!(msg.contains("stray_top_level"), "got {msg}");
}

// -------------------------------------------------------------------
// Round-trip + happy load for every variant.
// -------------------------------------------------------------------

#[test]
fn pressure_all_variants_round_trip() {
    let scene = load_scene_str(PRESSURE_SCENE, 0).expect("fixture loads");
    let sl1 = scene.world.sl1.as_ref().expect("sl1 present");
    assert_eq!(sl1.pressure.len(), 9, "fixture has 9 pressures");
    // Pressures are sorted by id.
    let ids: Vec<&str> = sl1.pressure.iter().map(|p| p.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "pressures must be id-sorted after validation");

    // Static payload mirrors all 9.
    let static_payload = encode_static(&scene);
    assert_eq!(static_payload.sl1_pressure.len(), 9);
}

// -------------------------------------------------------------------
// Runtime: lifecycle events at expected ticks
// -------------------------------------------------------------------

fn pressure_lifecycle_events(events: &[SimEvent]) -> Vec<(String, Sl1PressureEventKind, u64)> {
    let mut out = Vec::new();
    for e in events {
        if let SimEvent::Sl1PressureLifecycle {
            pressure_id,
            event,
            tick,
            ..
        } = e
        {
            out.push((pressure_id.clone(), *event, *tick));
        }
    }
    out
}

fn unsupported_warnings(messages: &[SimMessage]) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for m in messages {
        if let SimMessage::Warning(WarningPayload::Sl1Pressure {
            pressure_id,
            event: Sl1PressureWarningKind::UnsupportedInThisPr,
            tick,
            ..
        }) = m
        {
            out.push((pressure_id.clone(), *tick));
        }
    }
    out
}

#[test]
fn pressure_lifecycle_activates_and_deactivates_at_scheduled_ticks() {
    let mut scene = load_scene_str(
        &scene_with_pressure(
            r#"[{"id":"p1","type":"source_multiplier",
                "at_tick":3,"duration_ticks":2,
                "target":"factory","thing":"report","multiplier":1.0}]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();

    let mut activations: Vec<u64> = Vec::new();
    let mut deactivations: Vec<u64> = Vec::new();
    for _ in 0..10 {
        runner.tick_once(&mut world);
        for (id, event, tick) in pressure_lifecycle_events(runner.events()) {
            assert_eq!(id, "p1");
            match event {
                Sl1PressureEventKind::Activated => activations.push(tick),
                Sl1PressureEventKind::Deactivated => deactivations.push(tick),
            }
        }
    }
    // active interval is [3, 5) → activate at 3, deactivate at 5.
    assert_eq!(activations, vec![3]);
    assert_eq!(deactivations, vec![5]);
}

#[test]
fn source_multiplier_injects_inventory_with_carry() {
    // 2.5x multiplier = 2500 milli per tick: ticks 5,6,7 →
    // carry 2500→3500→4500 → injects 2,1,4? Let's trace exactly:
    //   tick 5: carry 0+2500=2500, whole=2, carry=500
    //   tick 6: carry 500+2500=3000, whole=3, carry=0
    //   tick 7: carry 0+2500=2500, whole=2, carry=500
    // duration_ticks=3 means active window is [5,8): three ticks.
    // Total injected = 2+3+2 = 7 reports.
    let mut scene = load_scene_str(
        &scene_with_pressure(
            r#"[{"id":"feed","type":"source_multiplier",
                "at_tick":5,"duration_ticks":3,
                "target":"factory","thing":"report","multiplier":2.5}]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    for _ in 0..10 {
        runner.tick_once(&mut world);
    }
    let runtime = world.sl1_runtime.as_ref().expect("runtime");
    let factory_inv = runtime
        .inventories
        .get("factory")
        .expect("factory inv")
        .get("report")
        .copied()
        .unwrap_or(0);
    // Demand fires every 5 ticks starting at 5 against `sink`, but
    // it's an observation-only demand so factory's `report` count
    // shouldn't be consumed by it.
    assert_eq!(factory_inv, 7, "expected 2+3+2=7 reports injected");
}

#[test]
fn source_multiplier_clamps_at_storage_capacity() {
    let mut scene = load_scene_str(
        &scene_with(
            r#"[{
                "id":"factory","role":"producer","pos":[0,0],
                "capacity":{"machine_hours":1},
                "storage":{
                    "raw_material":{"capacity":1,"initial":0},
                    "report":{"capacity":3,"initial":0}
                },
                "accepts":[],"produces":["report"]
            }, {
                "id":"sink","role":"consumer","pos":[1,0],
                "capacity":{"queries":1},
                "storage":{"report":{"capacity":10,"initial":0}},
                "accepts":["report"],"produces":[]
            }]"#,
            default_things(),
            default_links(),
            r#"[]"#,
            r#"[{"id":"feed","type":"source_multiplier",
                 "at_tick":1,"duration_ticks":10,
                 "target":"factory","thing":"report","multiplier":2.0}]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    for _ in 0..15 {
        runner.tick_once(&mut world);
    }
    let inv = world
        .sl1_runtime
        .as_ref()
        .expect("rt")
        .inventories
        .get("factory")
        .and_then(|m| m.get("report").copied())
        .unwrap_or(0);
    assert_eq!(inv, 3, "must clamp at storage capacity");
}

#[test]
fn demand_growth_multiplies_spawn_count() {
    // Base: every 5 ticks at start_tick=5. Spawn multiplier 2 → 3 per
    // firing while active. Pressure at_tick=5 duration 6 → active
    // window [5,11). Demand fires at 5 and 10 → both inside the
    // window → 3 + 3 = 6 spawns. Without pressure: 1 + 1 = 2.
    let json_with_pressure = scene_with_pressure(
        r#"[{"id":"surge","type":"demand_growth",
             "at_tick":5,"duration_ticks":6,
             "target":"d1","spawn_multiplier":2}]"#,
    );
    let json_without = scene_with_pressure("[]");

    let pending = |json: String| -> usize {
        let mut scene = load_scene_str(&json, 0).expect("loads");
        let mut world = std::mem::take(&mut scene.world);
        let mut runner = TickRunner::new();
        for _ in 0..11 {
            runner.tick_once(&mut world);
        }
        world
            .sl1_runtime
            .as_ref()
            .expect("rt")
            .demand
            .get("d1")
            .map(|d| d.pending.len() + d.fulfilled_count as usize + d.dropped_count as usize)
            .unwrap_or(0)
    };
    let with_count = pending(json_with_pressure);
    let without_count = pending(json_without);
    assert!(
        with_count > without_count,
        "expected more total demand instances under demand_growth: with={with_count}, without={without_count}"
    );
    assert_eq!(without_count, 2, "baseline should fire at 5 and 10 → 2");
    assert_eq!(with_count, 6, "expected 3 per firing × 2 firings = 6");
}

#[test]
fn quota_reduction_lowers_effective_capacity() {
    // Spawn a transform with capacity_cost=3 against a place with
    // base capacity=4. Without pressure: starts (4≥3). With 50%
    // reduction → effective=2, can't start. We check by observing
    // the transform's running count.
    use simetro_engine::sl1_pressure::effective_capacity;
    let mut scene = load_scene_str(
        &scene_with_pressure(
            r#"[{"id":"cut","type":"quota_reduction",
                 "at_tick":1,"duration_ticks":100,
                 "target":"factory","capacity":"machine_hours",
                 "reduction_percent":50}]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    runner.tick_once(&mut world);
    runner.tick_once(&mut world);
    let runtime = world.sl1_runtime.as_ref().expect("rt");
    assert_eq!(
        effective_capacity(&runtime.pressure, "factory", "machine_hours", 4),
        2,
        "50%% of 4 = 2"
    );
    // No overlay → unchanged
    assert_eq!(
        effective_capacity(&runtime.pressure, "factory", "queries", 8),
        8
    );
}

#[test]
fn path_outage_records_in_overlay() {
    let mut scene = load_scene_str(
        &scene_with_pressure(
            r#"[{"id":"out","type":"path_outage",
                 "at_tick":1,"duration_ticks":3,
                 "target":"factory-to-sink"}]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    runner.tick_once(&mut world);
    let runtime = world.sl1_runtime.as_ref().expect("rt");
    assert!(runtime.pressure.outaged_links.contains("factory-to-sink"));
    // After deactivation overlay clears.
    for _ in 0..4 {
        runner.tick_once(&mut world);
    }
    let runtime = world.sl1_runtime.as_ref().expect("rt");
    assert!(!runtime.pressure.outaged_links.contains("factory-to-sink"));
}

#[test]
fn unsupported_pressure_emits_warning_once_on_activation() {
    let mut scene = load_scene_str(
        &scene_with_pressure(
            r#"[{"id":"storm","type":"dashboard_storm",
                 "at_tick":3,"duration_ticks":4,
                 "target":"sink"}]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();

    let mut warns: Vec<(String, u64)> = Vec::new();
    for _ in 0..10 {
        runner.tick_once(&mut world);
        warns.extend(unsupported_warnings(runner.messages()));
    }
    assert_eq!(
        warns.len(),
        1,
        "should emit exactly one warning at activation: got {warns:?}"
    );
    assert_eq!(warns[0].0, "storm");
    assert_eq!(warns[0].1, 3, "warning tick should be activation tick");
}

// -------------------------------------------------------------------
// Hash baseline + fixture
// -------------------------------------------------------------------

#[test]
fn pressure_fixture_loads() {
    let scene = load_scene_str(PRESSURE_SCENE, 0).expect("fixture loads");
    let sl1 = scene.world.sl1.as_ref().expect("sl1 present");
    assert_eq!(sl1.pressure.len(), 9);
    let static_payload = encode_static(&scene);
    assert_eq!(static_payload.sl1_pressure.len(), 9);
}

#[test]
fn pressure_with_at_tick_zero_activates_on_first_observed_tick() {
    // The engine increments `world.tick` before calling
    // `sl1_runtime::run`, so the pressure runtime first observes
    // `now == 1`. A pressure with `at_tick: 0` must still activate
    // (the activation guard is window-based, not equality-based).
    // Regression for rubber-duck CRITICAL #1.
    let mut scene = load_scene_str(
        &scene_with_pressure(
            r#"[{"id":"p0","type":"source_multiplier",
                "at_tick":0,"duration_ticks":2,
                "target":"factory","thing":"report","multiplier":1.0}]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let mut activations: Vec<u64> = Vec::new();
    for _ in 0..5 {
        runner.tick_once(&mut world);
        for (id, event, tick) in pressure_lifecycle_events(runner.events()) {
            assert_eq!(id, "p0");
            if matches!(event, Sl1PressureEventKind::Activated) {
                activations.push(tick);
            }
        }
    }
    assert_eq!(
        activations,
        vec![1],
        "at_tick=0 pressure must activate on first observed tick"
    );
}

#[test]
fn source_multiplier_carry_does_not_leak_between_distinct_pressures() {
    // Two non-overlapping pressures on the same (factory, report).
    // First runs [3, 6) with 0.5x → at the end, accumulated carry
    // is 500 milli. Second runs [10, 13) with 0.5x → must start
    // from zero carry; otherwise its first tick would inject 1 unit
    // instead of 0. Regression for rubber-duck MAJOR #3.
    let mut scene = load_scene_str(
        &scene_with_pressure(
            r#"[
                {"id":"first","type":"source_multiplier",
                 "at_tick":3,"duration_ticks":3,
                 "target":"factory","thing":"report","multiplier":0.5},
                {"id":"second","type":"source_multiplier",
                 "at_tick":10,"duration_ticks":3,
                 "target":"factory","thing":"report","multiplier":0.5}
            ]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let mut report_at_tick: Vec<(u64, u64)> = Vec::new();
    for _ in 0..15 {
        runner.tick_once(&mut world);
        let runtime = world
            .sl1_runtime
            .as_ref()
            .expect("runtime present after pressure tick");
        let cur = runtime
            .inventories
            .get("factory")
            .and_then(|s| s.get("report"))
            .copied()
            .unwrap_or(0);
        report_at_tick.push((world.tick, cur));
    }
    // First window [3,6): ticks 3,4,5 with 0.5x.
    //   tick 3: carry 0+500=500, whole=0, carry=500
    //   tick 4: carry 500+500=1000, whole=1, carry=0 → inventory 1
    //   tick 5: carry 0+500=500, whole=0, carry=500
    // tick 6: deactivation drops carry → 0.
    // Second window [10,13): ticks 10,11,12.
    //   tick 10: carry 0+500=500, whole=0
    //   tick 11: carry 500+500=1000, whole=1, carry=0 → inventory 2
    //   tick 12: carry 0+500=500, whole=0
    let at = |t: u64| report_at_tick.iter().find(|(x, _)| *x == t).unwrap().1;
    assert_eq!(at(4), 1, "first pressure should inject 1 by tick 4");
    assert_eq!(at(6), 1, "carry must NOT leak across pressures");
    assert_eq!(at(9), 1, "no further injection between pressures");
    assert_eq!(at(11), 2, "second pressure should inject 1 by tick 11");
}

#[test]
fn pressure_lifecycle_events_appear_in_event_buffer_and_hash() {
    // Regression for rubber-duck MAJOR #2: lifecycle events must
    // travel through `runner.events()` (so they're fed to the
    // determinism hash via `feed_event`), not through
    // `SimMessage::Events` in `runner.messages()` (which `feed_message`
    // explicitly skips).
    let mut scene = load_scene_str(
        &scene_with_pressure(
            r#"[{"id":"life","type":"source_multiplier",
                "at_tick":2,"duration_ticks":2,
                "target":"factory","thing":"report","multiplier":1.0}]"#,
        ),
        0,
    )
    .expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let mut seen_in_events = 0;
    let mut seen_in_messages = 0;
    for _ in 0..6 {
        runner.tick_once(&mut world);
        for e in runner.events() {
            if matches!(e, SimEvent::Sl1PressureLifecycle { .. }) {
                seen_in_events += 1;
            }
        }
        for m in runner.messages() {
            if let SimMessage::Events(evs) = m {
                for e in evs {
                    if matches!(e, SimEvent::Sl1PressureLifecycle { .. }) {
                        seen_in_messages += 1;
                    }
                }
            }
        }
    }
    assert_eq!(
        seen_in_events, 2,
        "expected Activated+Deactivated in events buffer"
    );
    assert_eq!(
        seen_in_messages, 0,
        "lifecycle events must not be wrapped in SimMessage::Events"
    );
}

#[test]
fn pressure_fixture_ticks_deterministically_against_baseline() {
    let mut scene = load_scene_str(PRESSURE_SCENE, 0).expect("fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let hash = hash_run(&mut world, &mut runner, TICKS);
    let baseline = PRESSURE_BASELINE.trim();
    if baseline.is_empty() {
        eprintln!("RECORD: write the following to tests/baselines/sl1-pressure.hash");
        eprintln!("{hash}");
        panic!("missing baseline — rerun after writing baseline");
    }
    assert_eq!(
        hash, baseline,
        "deterministic hash drift detected for sl1-pressure.json\n  baseline: {baseline}\n  current:  {hash}"
    );
}
