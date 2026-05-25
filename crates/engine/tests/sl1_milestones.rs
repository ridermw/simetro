//! `scenario_language_v1` Milestones integration tests (PR 11).
//!
//! Exercises:
//!   - Every typed loader error variant via minimal scenes.
//!   - All four trigger variants (`pressure_activated`,
//!     `pressure_deactivated`, `metric_threshold`, `dashboard_state`).
//!   - All four metric predicates (`gte`, `lte`, `gt`, `lt`).
//!   - All three dashboard target states (`ok`, `stale`, `no_data`).
//!   - One-shot semantics: a milestone fires at most once per run.
//!   - `pressure_deactivated` requires prior activation (does not fire
//!     on tick 0 if the pressure was never active).
//!   - `metric_threshold` with `NoData` never satisfies.
//!   - Declaration-order independence (canonical sort by id).
//!   - Protocol round-trip of static + event payloads.
//!   - Determinism: hash baseline.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{hash_run, load_scene_str, LoadError, Sl1LoadError, TickRunner, World};
use simetro_protocol::SimEvent;

const MILESTONES_SCENE: &str = include_str!("fixtures/sl1-milestones.json");
const MILESTONES_BASELINE: &str = include_str!("../../../tests/baselines/sl1-milestones.hash");
const TICKS: u64 = 60;

// -------------------------------------------------------------------
// Scene helpers
// -------------------------------------------------------------------

fn scene_with(milestones_json: &str) -> String {
    scene_with_extras(
        default_places(),
        default_things(),
        "[]",
        "null",
        milestones_json,
    )
}

fn scene_with_pressure(pressure_json: &str, milestones_json: &str) -> String {
    scene_with_extras(
        default_places(),
        default_things(),
        pressure_json,
        "null",
        milestones_json,
    )
}

fn scene_with_obs(obs_json: &str, milestones_json: &str) -> String {
    scene_with_extras(
        default_places(),
        default_things(),
        "[]",
        obs_json,
        milestones_json,
    )
}

fn scene_with_extras(
    places_json: &str,
    things_json: &str,
    pressure_json: &str,
    obs_json: &str,
    milestones_json: &str,
) -> String {
    let obs_block = if obs_json == "null" {
        String::new()
    } else {
        format!(r#", "observability": {obs_json}"#)
    };
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-milestones-test",
            "theme": {{ "palette": ["#000000"], "background_index": 0, "font": "system-ui" }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places_json},
                "things": {things_json},
                "pressure": {pressure_json}{obs_block},
                "milestones": {milestones_json}
            }}
        }}"##
    )
}

fn default_places() -> &'static str {
    r#"[
        {
            "id": "factory", "role": "producer", "pos": [0,0],
            "capacity": { "machine_hours": 4 },
            "storage": {
                "raw_material": { "capacity": 100, "initial": 100 },
                "report":       { "capacity": 100, "initial": 0 }
            },
            "accepts": ["raw_material"], "produces": ["report"]
        }
    ]"#
}

fn default_things() -> &'static str {
    r#"[
        { "id": "raw_material", "kind": "input", "tags": [] },
        { "id": "report",       "kind": "data",  "tags": [], "freshness_budget_ticks": 50 }
    ]"#
}

fn expect_sl1_err(json: String) -> Sl1LoadError {
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(e) => e,
        other => panic!("expected LoadError::Sl1, got {other:?}"),
    }
}

fn run_ticks(world: &mut World, ticks: u64) -> Vec<SimEvent> {
    let mut runner = TickRunner::new();
    let mut events: Vec<SimEvent> = Vec::new();
    for _ in 0..ticks {
        runner.tick_once(world);
        events.extend_from_slice(runner.events());
    }
    events
}

fn milestone_fires<'a>(events: &'a [SimEvent], id: &str) -> Vec<&'a SimEvent> {
    events
        .iter()
        .filter(
            |e| matches!(e, SimEvent::Sl1MilestoneFired { milestone_id, .. } if milestone_id == id),
        )
        .collect()
}

// -------------------------------------------------------------------
// Loader error coverage
// -------------------------------------------------------------------

#[test]
fn milestone_invalid_id_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[{"id":"BAD ID","label":"x","trigger":{"type":"pressure_activated","pressure":"p"}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneInvalidId { .. }
    ));
}

#[test]
fn milestone_duplicate_id_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[
            {"id":"m","label":"a","trigger":{"type":"pressure_activated","pressure":"p"}},
            {"id":"m","label":"b","trigger":{"type":"pressure_activated","pressure":"p"}}
        ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneDuplicateId { .. }
    ));
}

#[test]
fn milestone_empty_label_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[{"id":"m","label":"","trigger":{"type":"pressure_activated","pressure":"p"}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneEmptyLabel { .. }
    ));
}

#[test]
fn milestone_unknown_pressure_rejected() {
    let json = scene_with(
        r#"[{"id":"m","label":"x","trigger":{"type":"pressure_activated","pressure":"nope"}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneUnknownPressure { .. }
    ));
}

#[test]
fn milestone_unknown_metric_rejected() {
    let json = scene_with(
        r#"[{"id":"m","label":"x","trigger":{"type":"metric_threshold","metric":"nope","predicate":"gte","value":1}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneUnknownMetric { .. }
    ));
}

#[test]
fn milestone_unknown_predicate_rejected() {
    // Bad predicate string fails the typed parse before validator sees
    // it. The closed enum lives in the loader so it surfaces as Parse.
    let json = scene_with_obs(
        r#"{ "metrics":[{"id":"m1","source":"place_inventory_count","place":"factory","thing":"report"}], "dashboards":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"x","trigger":{"type":"metric_threshold","metric":"m1","predicate":"weird","value":1}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::Parse { .. } | Sl1LoadError::MilestoneUnknownPredicate { .. }
    ));
}

#[test]
fn milestone_unknown_dashboard_rejected() {
    let json = scene_with(
        r#"[{"id":"m","label":"x","trigger":{"type":"dashboard_state","dashboard":"nope","state":"ok"}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneUnknownDashboard { .. }
    ));
}

#[test]
fn milestone_unknown_dashboard_state_rejected() {
    let json = scene_with_obs(
        r#"{ "dashboards":[{"id":"d","type":"ad_hoc","depends_on":[],"freshness_slo_ticks":5}], "metrics":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"x","trigger":{"type":"dashboard_state","dashboard":"d","state":"weird"}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::Parse { .. } | Sl1LoadError::MilestoneUnknownDashboardState { .. }
    ));
}

#[test]
fn milestone_empty_camera_focus_entry_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[{"id":"m","label":"x","camera_focus":[""],"trigger":{"type":"pressure_activated","pressure":"p"}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneEmptyCameraFocusEntry { .. }
    ));
}

#[test]
fn milestone_duplicate_camera_focus_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[{"id":"m","label":"x","camera_focus":["factory","factory"],"trigger":{"type":"pressure_activated","pressure":"p"}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneDuplicateCameraFocus { .. }
    ));
}

#[test]
fn milestone_empty_highlight_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[{"id":"m","label":"x","highlight":"","trigger":{"type":"pressure_activated","pressure":"p"}}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::MilestoneEmptyHighlight { .. }
    ));
}

#[test]
fn milestone_unknown_top_level_field_rejected() {
    let json = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[{"id":"m","label":"x","trigger":{"type":"pressure_activated","pressure":"p"},"extra":1}]"#,
    );
    // deny_unknown_fields on RawSl1Milestone causes serde to fail.
    assert!(matches!(expect_sl1_err(json), Sl1LoadError::Parse { .. }));
}

// -------------------------------------------------------------------
// Runtime semantics
// -------------------------------------------------------------------

#[test]
fn pressure_activated_fires_at_pressure_start() {
    let json = scene_with_pressure(
        r#"[{"id":"p1","type":"schema_drift","at_tick":3,"duration_ticks":2,"target":"report"}]"#,
        r#"[{"id":"m","label":"started","trigger":{"type":"pressure_activated","pressure":"p1"}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 10);

    let fires = milestone_fires(&events, "m");
    assert_eq!(fires.len(), 1, "expected one fire, got {:?}", fires);
    // Should fire at tick 3 (the activation tick).
    if let SimEvent::Sl1MilestoneFired { tick, .. } = fires[0] {
        assert_eq!(*tick, 3, "expected fire at tick 3, got {tick}");
    }
}

#[test]
fn pressure_deactivated_fires_after_pressure_ends() {
    // Pressure active ticks 3..=4 (at_tick=3, duration=2). Should fire
    // the first tick the pressure is NOT active (tick 5).
    let json = scene_with_pressure(
        r#"[{"id":"p1","type":"schema_drift","at_tick":3,"duration_ticks":2,"target":"report"}]"#,
        r#"[{"id":"m","label":"ended","trigger":{"type":"pressure_deactivated","pressure":"p1"}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 10);

    let fires = milestone_fires(&events, "m");
    assert_eq!(fires.len(), 1, "expected one fire, got {:?}", fires);
    if let SimEvent::Sl1MilestoneFired { tick, .. } = fires[0] {
        assert_eq!(
            *tick, 5,
            "pressure_deactivated should fire at exactly tick 5, got {tick}"
        );
    }
}

#[test]
fn pressure_deactivated_does_not_fire_if_never_activated() {
    // Pressure scheduled for tick 100 — never activates during the
    // 10-tick run. Milestone must NOT fire.
    let json = scene_with_pressure(
        r#"[{"id":"p1","type":"schema_drift","at_tick":100,"duration_ticks":2,"target":"report"}]"#,
        r#"[{"id":"m","label":"x","trigger":{"type":"pressure_deactivated","pressure":"p1"}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 10);

    let fires = milestone_fires(&events, "m");
    assert!(
        fires.is_empty(),
        "pressure_deactivated should not fire without prior activation, got {:?}",
        fires
    );
}

#[test]
fn metric_threshold_fires_when_predicate_satisfied() {
    let json = scene_with_obs(
        r#"{ "metrics":[{"id":"count","source":"place_inventory_count","place":"factory","thing":"raw_material"}], "dashboards":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"got","trigger":{"type":"metric_threshold","metric":"count","predicate":"gte","value":50}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 5);

    // raw_material starts at 100 ≥ 50 → fires tick 0.
    let fires = milestone_fires(&events, "m");
    assert_eq!(fires.len(), 1);
}

#[test]
fn metric_threshold_lt_predicate_works() {
    let json = scene_with_obs(
        r#"{ "metrics":[{"id":"count","source":"place_inventory_count","place":"factory","thing":"report"}], "dashboards":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"low","trigger":{"type":"metric_threshold","metric":"count","predicate":"lt","value":1}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 3);

    // report starts at 0, 0 < 1 → fires tick 0.
    assert_eq!(milestone_fires(&events, "m").len(), 1);
}

#[test]
fn metric_threshold_lte_predicate_works() {
    let json = scene_with_obs(
        r#"{ "metrics":[{"id":"count","source":"place_inventory_count","place":"factory","thing":"report"}], "dashboards":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"le","trigger":{"type":"metric_threshold","metric":"count","predicate":"lte","value":0}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 3);

    // report starts at 0, 0 <= 0 → fires.
    assert_eq!(milestone_fires(&events, "m").len(), 1);
}

#[test]
fn metric_threshold_gt_predicate_works() {
    let json = scene_with_obs(
        r#"{ "metrics":[{"id":"count","source":"place_inventory_count","place":"factory","thing":"raw_material"}], "dashboards":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"hi","trigger":{"type":"metric_threshold","metric":"count","predicate":"gt","value":50}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 3);

    // raw_material starts at 100, 100 > 50 → fires.
    assert_eq!(milestone_fires(&events, "m").len(), 1);
}

#[test]
fn metric_threshold_no_data_never_satisfies() {
    // Metric source references a place that has no `report` storage
    // populated AND no transform ever produces it → metric remains at
    // value 0 in Ok state (place_inventory_count emits Ok{0}).
    // For true NoData we must use dashboard_freshness on a depends_on
    // that never gets produced; that returns NoData. Even with a
    // permissive predicate (`gte 0`), the NoData state must not fire.
    let json = scene_with_obs(
        r#"{
            "dashboards":[{"id":"d","type":"report","depends_on":["report"],"freshness_slo_ticks":5}],
            "metrics":[{"id":"age","source":"dashboard_freshness","dashboard":"d"}],
            "alerts":[]
        }"#,
        r#"[{"id":"m","label":"nd","trigger":{"type":"metric_threshold","metric":"age","predicate":"gte","value":0}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 5);

    assert!(
        milestone_fires(&events, "m").is_empty(),
        "milestone must not fire when metric is NoData even with permissive predicate"
    );
}

#[test]
fn metric_threshold_does_not_fire_when_predicate_unmet() {
    let json = scene_with_obs(
        r#"{ "metrics":[{"id":"count","source":"place_inventory_count","place":"factory","thing":"report"}], "dashboards":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"x","trigger":{"type":"metric_threshold","metric":"count","predicate":"gte","value":999}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 5);

    assert!(milestone_fires(&events, "m").is_empty());
}

#[test]
fn dashboard_state_fires_on_target_state_match() {
    // Empty depends_on → dashboard always Ok → fires immediately.
    let json = scene_with_obs(
        r#"{ "dashboards":[{"id":"d","type":"ad_hoc","depends_on":[],"freshness_slo_ticks":5}], "metrics":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"ok","trigger":{"type":"dashboard_state","dashboard":"d","state":"ok"}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 3);

    assert_eq!(milestone_fires(&events, "m").len(), 1);
}

#[test]
fn dashboard_no_data_state_fires() {
    // Dashboard depends on `report` which never gets produced → NoData.
    let json = scene_with_obs(
        r#"{ "dashboards":[{"id":"d","type":"report","depends_on":["report"],"freshness_slo_ticks":5}], "metrics":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"nd","trigger":{"type":"dashboard_state","dashboard":"d","state":"no_data"}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 3);

    assert_eq!(milestone_fires(&events, "m").len(), 1);
}

#[test]
fn dashboard_stale_state_fires() {
    // factory starts with report=initial=10, but freshness_slo=1; after
    // a couple of ticks aging puts dashboard into Stale. Build a place
    // that has `report` initial > 0 so the dashboard transitions Ok →
    // Stale rather than NoData.
    let places = r#"[
        {
            "id":"factory","role":"producer","pos":[0,0],
            "capacity":{ "machine_hours": 4 },
            "storage": {
                "raw_material": { "capacity": 100, "initial": 100 },
                "report":       { "capacity": 100, "initial": 1 }
            },
            "accepts":["raw_material"],"produces":["report"]
        }
    ]"#;
    let things = default_things();
    let scene_json = scene_with_extras(
        places,
        things,
        "[]",
        r#"{ "dashboards":[{"id":"d","type":"report","depends_on":["report"],"freshness_slo_ticks":1}], "metrics":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"st","trigger":{"type":"dashboard_state","dashboard":"d","state":"stale"}}]"#,
    );
    let mut scene = load_scene_str(&scene_json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 10);

    assert_eq!(
        milestone_fires(&events, "m").len(),
        1,
        "milestone for dashboard `stale` should fire exactly once"
    );
}

#[test]
fn milestone_fires_at_most_once_over_many_ticks() {
    // Persistent condition: raw_material always >= 1.
    let json = scene_with_obs(
        r#"{ "metrics":[{"id":"count","source":"place_inventory_count","place":"factory","thing":"raw_material"}], "dashboards":[], "alerts":[] }"#,
        r#"[{"id":"m","label":"once","trigger":{"type":"metric_threshold","metric":"count","predicate":"gte","value":1}}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, 50);

    assert_eq!(
        milestone_fires(&events, "m").len(),
        1,
        "milestone must be one-shot even across many ticks"
    );
}

// -------------------------------------------------------------------
// Declaration-order independence
// -------------------------------------------------------------------

#[test]
fn declaration_order_independence() {
    // Same scene, two declaration orders → same canonical id sort.
    let order_a = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[
            {"id":"a","label":"A","trigger":{"type":"pressure_activated","pressure":"p"}},
            {"id":"b","label":"B","trigger":{"type":"pressure_deactivated","pressure":"p"}}
        ]"#,
    );
    let order_b = scene_with_pressure(
        r#"[{"id":"p","type":"schema_drift","at_tick":1,"duration_ticks":1,"target":"report"}]"#,
        r#"[
            {"id":"b","label":"B","trigger":{"type":"pressure_deactivated","pressure":"p"}},
            {"id":"a","label":"A","trigger":{"type":"pressure_activated","pressure":"p"}}
        ]"#,
    );

    let sa = load_scene_str(&order_a, 0).expect("loads a");
    let sb = load_scene_str(&order_b, 0).expect("loads b");
    let ids_a: Vec<&str> = sa
        .world
        .sl1
        .as_ref()
        .unwrap()
        .milestones
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    let ids_b: Vec<&str> = sb
        .world
        .sl1
        .as_ref()
        .unwrap()
        .milestones
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(ids_a, vec!["a", "b"]);
    assert_eq!(ids_a, ids_b);
}

// -------------------------------------------------------------------
// Protocol round-trip
// -------------------------------------------------------------------

#[test]
fn fixture_static_protocol_round_trips() {
    let scene = load_scene_str(MILESTONES_SCENE, 0).expect("fixture loads");
    let sl1 = scene.world.sl1.as_ref().expect("sl1");
    assert_eq!(sl1.milestones.len(), 4);

    use simetro_engine::snapshot::encode_static;
    let sp = encode_static(&scene);
    assert_eq!(sp.sl1_milestones.len(), 4);

    let json = serde_json::to_string(&sp).expect("ser");
    let back: simetro_protocol::StaticPayload = serde_json::from_str(&json).expect("de");
    assert_eq!(back.sl1_milestones.len(), 4);
}

#[test]
fn fixture_emits_milestone_events_with_safe_text_fields() {
    let mut scene = load_scene_str(MILESTONES_SCENE, 0).expect("fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    let events = run_ticks(&mut world, TICKS);

    // Fixture has 4 milestones — most should fire within 60 ticks given
    // the scene shape. Specifically:
    //   first-report          → fires once raw_material → report happens
    //   drift-activated       → fires at tick 8
    //   drift-cleared         → fires at tick 12 (after drift ends)
    let fired_ids: std::collections::BTreeSet<String> = events
        .iter()
        .filter_map(|e| {
            if let SimEvent::Sl1MilestoneFired { milestone_id, .. } = e {
                Some(milestone_id.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        fired_ids.contains("drift-activated"),
        "drift-activated should fire; fired_ids={fired_ids:?}"
    );
    assert!(
        fired_ids.contains("drift-cleared"),
        "drift-cleared should fire; fired_ids={fired_ids:?}"
    );

    // Round-trip events through JSON.
    for ev in events
        .iter()
        .filter(|e| matches!(e, SimEvent::Sl1MilestoneFired { .. }))
    {
        let json = serde_json::to_string(ev).expect("ser event");
        let back: SimEvent = serde_json::from_str(&json).expect("de event");
        assert_eq!(*ev, back);
    }
}

// -------------------------------------------------------------------
// Determinism baseline
// -------------------------------------------------------------------

#[test]
fn fixture_hash_matches_baseline() {
    let mut scene = load_scene_str(MILESTONES_SCENE, 0).expect("fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let hash = hash_run(&mut world, &mut runner, TICKS);
    let expected = MILESTONES_BASELINE.trim();
    if expected == "0000000000000000000000000000000000000000000000000000000000000000" {
        panic!(
            "sl1-milestones hash baseline not yet captured; write this to \
             tests/baselines/sl1-milestones.hash:\n{hash}"
        );
    }
    assert_eq!(
        hash, expected,
        "sl1-milestones hash drifted; if intentional, update baseline"
    );
}

#[test]
fn fixture_hash_is_stable_across_two_runs() {
    let mut scene1 = load_scene_str(MILESTONES_SCENE, 0).expect("fixture loads");
    let mut world1 = std::mem::take(&mut scene1.world);
    let mut runner1 = TickRunner::new();
    let h1 = hash_run(&mut world1, &mut runner1, TICKS);

    let mut scene2 = load_scene_str(MILESTONES_SCENE, 0).expect("fixture loads");
    let mut world2 = std::mem::take(&mut scene2.world);
    let mut runner2 = TickRunner::new();
    let h2 = hash_run(&mut world2, &mut runner2, TICKS);

    assert_eq!(h1, h2);
}
