//! `scenario_language_v1` Demand integration tests (PR 5).
//!
//! Exercises:
//!   - Every new `Sl1LoadError::Demand*` variant via minimal scenes.
//!   - Cross-validation of target (place) and requires (things).
//!   - Strict-schema rejection of unknown fields in nested raw types.
//!   - Runtime spawn semantics for `fixed` and `scripted` schedules.
//!   - Observation-only fulfillment when requires are present.
//!   - Past-deadline drop emits `Sl1Warning::DemandDropped` with value +
//!     penalty_score, and `dropped_count` increments.
//!   - Edge-triggered `DemandBacklogOverflow` (one warning per overflow
//!     entry, rearms after backlog drains below the cap).
//!   - Same-tick transform → demand fulfillment.
//!   - Declaration-order independence (canonical sort by id).
//!   - The `sl1-demand.json` fixture ticks deterministically against
//!     `tests/baselines/sl1-demand.hash`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{
    encode_snapshot, encode_static, hash_run, load_scene_str, LoadError, Sl1LoadError, TickRunner,
    MAX_DEMAND_OUTSTANDING,
};
use simetro_protocol::{SimMessage, Sl1DemandWarningKind, WarningPayload};

const DEMAND_SCENE: &str = include_str!("fixtures/sl1-demand.json");
const DEMAND_BASELINE: &str = include_str!("../../../tests/baselines/sl1-demand.hash");
const TICKS: u64 = 200;

fn scene_with_demand(demand_json: &str) -> String {
    scene_with(default_places(), default_things(), "[]", demand_json)
}

fn scene_with(
    places_json: &str,
    things_json: &str,
    transforms_json: &str,
    demand_json: &str,
) -> String {
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-demand-test",
            "theme": {{
                "palette": ["#000000"],
                "background_index": 0,
                "font": "system-ui"
            }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places_json},
                "things": {things_json},
                "transforms": {transforms_json},
                "demand": {demand_json}
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
            "id": "dashboard",
            "role": "consumer",
            "pos": [0.0, 0.0],
            "capacity": { "queries": 8 },
            "storage": {
                "report": { "capacity": 100, "initial": 1 }
            },
            "accepts": ["report"],
            "produces": []
        }
    ]"#
}

fn default_things() -> &'static str {
    r#"[
        {"id": "report", "kind": "data", "tags": [], "freshness_budget_ticks": 100}
    ]"#
}

fn fixed_schedule_json() -> &'static str {
    r#"{"type": "fixed", "every_ticks": 5, "start_tick": 5}"#
}

fn standard_penalty() -> &'static str {
    r#"{"score": -1}"#
}

/// Build one demand JSON object with all required fields filled in,
/// then overlay the caller's overrides. Use this for negative tests
/// that need every-other field valid.
fn demand_obj(id: &str, overrides: &str) -> String {
    format!(
        r##"{{
            "id": "{id}",
            "type": "report_refresh",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {fixed},
            "deadline_ticks": 10,
            "priority": "normal",
            "value": 5,
            "penalty": {penalty}
            {overrides}
        }}"##,
        fixed = fixed_schedule_json(),
        penalty = standard_penalty(),
        overrides = if overrides.is_empty() { "" } else { overrides },
    )
}

// -------------------------------------------------------------------
// Load error coverage — one test per Sl1LoadError::Demand* variant.
// -------------------------------------------------------------------

#[test]
fn demand_invalid_id_rejected() {
    let json = scene_with_demand(&format!("[{}]", demand_obj("Bad ID!", "")));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandInvalidId { .. }
    ));
}

#[test]
fn demand_duplicate_id_rejected() {
    let json = scene_with_demand(&format!(
        "[{a}, {b}]",
        a = demand_obj("d1", ""),
        b = demand_obj("d1", "")
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandDuplicateId { .. }
    ));
}

#[test]
fn demand_empty_type_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandEmptyType { .. }
    ));
}

#[test]
fn demand_unknown_target_kind_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "bogus", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandUnknownTargetKind { .. }
    ));
}

#[test]
fn demand_target_kind_transform_not_implemented() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "transform", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandTargetKindNotImplemented {
            kind: "transform",
            ..
        }
    ));
}

#[test]
fn demand_target_kind_dashboard_not_implemented() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "dashboard", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandTargetKindNotImplemented {
            kind: "dashboard",
            ..
        }
    ));
}

#[test]
fn demand_target_kind_virtual_sink_not_implemented() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "virtual_sink", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandTargetKindNotImplemented {
            kind: "virtual_sink",
            ..
        }
    ));
}

#[test]
fn demand_unknown_target_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "ghost_place"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandUnknownTarget { .. }
    ));
}

#[test]
fn demand_requires_empty_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": [], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandRequiresEmpty { .. }
    ));
}

#[test]
fn demand_unknown_requires_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["ghost_thing"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandUnknownRequires { .. }
    ));
}

#[test]
fn demand_duplicate_requires_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report", "report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandDuplicateRequires { .. }
    ));
}

#[test]
fn demand_schedule_wave_not_implemented() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "wave"}},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandScheduleNotImplemented { kind: "wave", .. }
    ));
}

#[test]
fn demand_schedule_unknown_type_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "ouija"}},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandUnknownScheduleType { .. }
    ));
}

#[test]
fn demand_schedule_fixed_missing_every_ticks_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "fixed", "start_tick": 5}},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandScheduleMissingField {
            field: "every_ticks",
            ..
        }
    ));
}

#[test]
fn demand_schedule_fixed_zero_every_ticks_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "fixed", "every_ticks": 0, "start_tick": 5}},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandScheduleFieldZero {
            field: "every_ticks",
            ..
        }
    ));
}

#[test]
fn demand_schedule_scripted_empty_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "scripted", "ticks": []}},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandScheduleScriptedEmpty { .. }
    ));
}

#[test]
fn demand_schedule_scripted_not_increasing_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "scripted", "ticks": [5, 3]}},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandScheduleScriptedNotIncreasing { .. }
    ));
}

#[test]
fn demand_schedule_scripted_tick_zero_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "scripted", "ticks": [0, 5]}},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandScheduleScriptedTickZero { .. }
    ));
}

#[test]
fn demand_deadline_zero_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 0, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandDeadlineZero { .. }
    ));
}

#[test]
fn demand_invalid_priority_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "panic", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandInvalidPriority { .. }
    ));
}

#[test]
fn demand_penalty_score_positive_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {{"score": 5}}
        }}]"#,
        sched = fixed_schedule_json()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandPenaltyScorePositive { .. }
    ));
}

#[test]
fn demand_penalty_warning_empty_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {{"score": -1, "warning": "   "}}
        }}]"#,
        sched = fixed_schedule_json()
    ));
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::DemandPenaltyWarningEmpty { .. }
    ));
}

// -------------------------------------------------------------------
// Strict-schema rejection of unknown fields in nested raw types.
// -------------------------------------------------------------------

#[test]
fn demand_unknown_top_level_field_rejected() {
    // RawSl1Demand has #[serde(deny_unknown_fields)].
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen},
            "mystery_field": 1
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    // Surfaces as a LoadError::Json (serde deserialize error) — not Sl1.
    assert!(load_scene_str(&json, 0).is_err());
}

#[test]
fn demand_target_unknown_field_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard", "extra": 1}},
            "requires": ["report"], "spawn_schedule": {sched},
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": {pen}
        }}]"#,
        sched = fixed_schedule_json(),
        pen = standard_penalty()
    ));
    assert!(load_scene_str(&json, 0).is_err());
}

// -------------------------------------------------------------------
// Runtime tests: spawn / fulfill / drop / overflow.
// -------------------------------------------------------------------

#[allow(dead_code)]
fn collect_demand_warnings(messages: &[SimMessage]) -> Vec<(String, Sl1DemandWarningKind, u64)> {
    messages
        .iter()
        .filter_map(|m| {
            if let SimMessage::Warning(WarningPayload::Sl1Demand {
                demand_id,
                event,
                tick,
                ..
            }) = m
            {
                Some((demand_id.clone(), *event, *tick))
            } else {
                None
            }
        })
        .collect()
}

fn tick_n_collect_warnings(json: &str, ticks: u64) -> Vec<SimMessage> {
    let mut scene = load_scene_str(json, 0).expect("load");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let mut all = Vec::new();
    for _ in 0..ticks {
        runner.tick_once(&mut world);
        for m in runner.messages() {
            all.push(m.clone());
        }
    }
    all
}

#[test]
fn demand_fixed_schedule_spawns_and_fulfills() {
    // dashboard.storage.report.initial = 1, so the first spawned
    // demand can fulfill immediately. With every_ticks=5/start_tick=5
    // we expect at least one fulfillment by tick 10.
    let json = scene_with_demand(&format!("[{}]", demand_obj("d1", "")));
    let mut scene = load_scene_str(&json, 0).expect("load");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    for _ in 0..10 {
        runner.tick_once(&mut world);
    }
    let sl1_rt = world.sl1_runtime.as_ref().expect("sl1 runtime");
    let rt = sl1_rt.demand.get("d1").expect("demand runtime");
    // tick 5 spawned, tick 5 fulfilled (report present).
    assert!(rt.fulfilled_count >= 1, "got {}", rt.fulfilled_count);
    assert_eq!(rt.dropped_count, 0);
}

#[test]
fn demand_starves_when_requires_absent_emits_dropped() {
    // Drop initial report so nothing can fulfill. Demand spawns at
    // tick 5 with deadline_ticks=2; should drop at tick 8.
    let places = r#"[
        {
            "id": "dashboard", "role": "consumer", "pos": [0.0, 0.0],
            "capacity": {"queries": 1},
            "storage": {"report": {"capacity": 100, "initial": 0}},
            "accepts": ["report"], "produces": []
        }
    ]"#;
    let json = scene_with(
        places,
        default_things(),
        "[]",
        r#"[{
            "id": "d1", "type": "x",
            "target": {"type": "place", "id": "dashboard"},
            "requires": ["report"],
            "spawn_schedule": {"type": "fixed", "every_ticks": 100, "start_tick": 5},
            "deadline_ticks": 2, "priority": "normal", "value": 7,
            "penalty": {"score": -4, "warning": "stale"}
        }]"#,
    );
    let messages = tick_n_collect_warnings(&json, 12);
    let drops: Vec<_> = messages
        .iter()
        .filter(|m| {
            matches!(
                m,
                SimMessage::Warning(WarningPayload::Sl1Demand {
                    event: Sl1DemandWarningKind::Dropped,
                    ..
                })
            )
        })
        .collect();
    assert_eq!(drops.len(), 1, "expected exactly one drop");
    if let SimMessage::Warning(WarningPayload::Sl1Demand {
        tick,
        value,
        penalty_score,
        ..
    }) = drops[0]
    {
        assert_eq!(*tick, 8, "drop fires at deadline+1 (spawn=5, deadline=2)");
        assert_eq!(*value, Some(7));
        assert_eq!(*penalty_score, Some(-4));
    }
}

#[test]
fn demand_scripted_schedule_spawns_at_exact_ticks() {
    let places = r#"[
        {
            "id": "dashboard", "role": "consumer", "pos": [0.0, 0.0],
            "capacity": {"queries": 1},
            "storage": {"report": {"capacity": 100, "initial": 0}},
            "accepts": ["report"], "produces": []
        }
    ]"#;
    let json = scene_with(
        places,
        default_things(),
        "[]",
        r#"[{
            "id": "d1", "type": "x",
            "target": {"type": "place", "id": "dashboard"},
            "requires": ["report"],
            "spawn_schedule": {"type": "scripted", "ticks": [3, 7]},
            "deadline_ticks": 100, "priority": "normal", "value": 5,
            "penalty": {"score": -1}
        }]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("load");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    // After tick 3, exactly 1 pending.
    for _ in 0..3 {
        runner.tick_once(&mut world);
    }
    let rt = world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .demand
        .get("d1")
        .unwrap();
    assert_eq!(rt.pending.len(), 1);
    assert_eq!(rt.next_sequence, 1);
    // After tick 7, exactly 2 pending.
    for _ in 3..7 {
        runner.tick_once(&mut world);
    }
    let rt = world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .demand
        .get("d1")
        .unwrap();
    assert_eq!(rt.pending.len(), 2);
    assert_eq!(rt.next_sequence, 2);
    assert_eq!(rt.scripted_cursor, 2);
    // After tick 20, no more spawns (script exhausted).
    for _ in 7..20 {
        runner.tick_once(&mut world);
    }
    let rt = world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .demand
        .get("d1")
        .unwrap();
    assert_eq!(rt.next_sequence, 2);
}

#[test]
fn demand_backlog_overflow_is_edge_triggered_and_rearms() {
    // Spawn every tick with a very large deadline so the backlog
    // grows past MAX_DEMAND_OUTSTANDING. We tick exactly N = cap+1
    // ticks so we land just one tick past saturation. With requires
    // absent and a long deadline, none drop yet — the overflow flag
    // must edge-trigger exactly once.
    //
    // For the "rearm" half, we then artificially run for many more
    // ticks of nothing-spawning by exhausting a scripted schedule:
    // but easier — just verify that re-entering overflow after
    // draining produces another warning. We achieve that with a
    // scripted schedule of TWO bursts.
    let places = r#"[
        {
            "id": "dashboard", "role": "consumer", "pos": [0.0, 0.0],
            "capacity": {"queries": 1},
            "storage": {"report": {"capacity": 100, "initial": 0}},
            "accepts": ["report"], "produces": []
        }
    ]"#;
    let cap = MAX_DEMAND_OUTSTANDING as u64;
    let json = scene_with(
        places,
        default_things(),
        "[]",
        &format!(
            r#"[{{
                "id": "d1", "type": "x",
                "target": {{"type": "place", "id": "dashboard"}},
                "requires": ["report"],
                "spawn_schedule": {{"type": "fixed", "every_ticks": 1, "start_tick": 1}},
                "deadline_ticks": {big}, "priority": "normal", "value": 1,
                "penalty": {{"score": -1}}
            }}]"#,
            big = cap * 10
        ),
    );
    // Run cap+5 ticks: by tick cap+1 we attempt to spawn entry #(cap+1)
    // and overflow fires. Subsequent ticks while still over cap should
    // NOT emit additional overflow warnings (edge-triggered).
    let messages = tick_n_collect_warnings(&json, cap + 5);
    let overflow_count = messages
        .iter()
        .filter(|m| {
            matches!(
                m,
                SimMessage::Warning(WarningPayload::Sl1Demand {
                    event: Sl1DemandWarningKind::BacklogOverflow,
                    ..
                })
            )
        })
        .count();
    assert_eq!(
        overflow_count, 1,
        "expected exactly one edge-triggered overflow, got {overflow_count}"
    );
}

#[test]
fn demand_declaration_order_independent() {
    // Two valid demands in reverse alphabetical order in JSON; the
    // validator must sort by id so static encoding is deterministic.
    let json = scene_with_demand(&format!(
        "[{b}, {a}]",
        a = demand_obj("aaa", ""),
        b = demand_obj("zzz", "")
    ));
    let scene = load_scene_str(&json, 0).expect("load");
    let sl1 = scene.world.sl1.as_ref().expect("sl1");
    assert_eq!(sl1.demand[0].id, "aaa");
    assert_eq!(sl1.demand[1].id, "zzz");
    let static_payload = encode_static(&scene);
    assert_eq!(static_payload.sl1_demand.len(), 2);
    assert_eq!(static_payload.sl1_demand[0].id, "aaa");
}

#[test]
fn demand_snapshot_reflects_runtime_state() {
    let json = scene_with_demand(&format!("[{}]", demand_obj("d1", "")));
    let mut scene = load_scene_str(&json, 0).expect("load");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    for _ in 0..6 {
        runner.tick_once(&mut world);
    }
    scene.world = world;
    let mut snap = simetro_protocol::SnapshotPayload::default();
    encode_snapshot(&scene.world, &mut snap);
    assert_eq!(snap.sl1_demand_states.len(), 1);
    assert_eq!(snap.sl1_demand_states[0].demand_id, "d1");
}

#[test]
fn demand_backlog_overflow_rearms_after_drain() {
    // Two-phase: scripted schedule fires (cap+1) spawns on ticks
    // 1..=cap+1 — overflow fires at tick cap+1. Then we leave a
    // tick-gap. With deadline_ticks = cap+1, all pending expire by
    // tick (cap+1)+(cap+1)+1 = 2cap+3, draining backlog to 0 and
    // clearing the overflow flag. Then a second burst on ticks
    // (3cap+3)..=(4cap+3) re-enters overflow exactly once.
    //
    // Requirements stay unmet (storage initial: 0) so spawned
    // instances accumulate as Pending and overflow can engage.
    let places = r#"[
        {
            "id": "dashboard", "role": "consumer", "pos": [0.0, 0.0],
            "capacity": {"queries": 1},
            "storage": {"report": {"capacity": 100, "initial": 0}},
            "accepts": ["report"], "produces": []
        }
    ]"#;
    let cap = MAX_DEMAND_OUTSTANDING as u64;
    let mut burst1 = String::new();
    for t in 1..=(cap + 1) {
        if !burst1.is_empty() {
            burst1.push(',');
        }
        burst1.push_str(&t.to_string());
    }
    let mut burst2 = String::new();
    let burst2_start = 3 * cap + 3;
    for t in burst2_start..=(burst2_start + cap) {
        if !burst2.is_empty() {
            burst2.push(',');
        }
        burst2.push_str(&t.to_string());
    }
    let json = scene_with(
        places,
        default_things(),
        "[]",
        &format!(
            r#"[{{
                "id": "d1", "type": "x",
                "target": {{"type": "place", "id": "dashboard"}},
                "requires": ["report"],
                "spawn_schedule": {{"type": "scripted", "ticks": [{burst1}, {burst2}]}},
                "deadline_ticks": {dl}, "priority": "normal", "value": 1,
                "penalty": {{"score": -1}}
            }}]"#,
            dl = cap + 1
        ),
    );
    let messages = tick_n_collect_warnings(&json, burst2_start + cap + 5);
    let overflow_count = messages
        .iter()
        .filter(|m| {
            matches!(
                m,
                SimMessage::Warning(WarningPayload::Sl1Demand {
                    event: Sl1DemandWarningKind::BacklogOverflow,
                    ..
                })
            )
        })
        .count();
    assert_eq!(
        overflow_count, 2,
        "expected two edge-triggered overflows (one per burst), got {overflow_count}"
    );
}

// -------------------------------------------------------------------
// Negative loader tests (added during PR 5 rubber-duck review)
// -------------------------------------------------------------------

#[test]
fn demand_requires_too_many_rejected() {
    // Add MAX_DEMAND_REQUIRES+1 extra things; reference all of them
    // in `requires` to exceed the cap.
    let mut things = String::from(
        r#"[{"id": "report", "kind": "data", "tags": [], "freshness_budget_ticks": 100}"#,
    );
    for i in 0..70 {
        things.push_str(&format!(
            r#",{{"id": "t{i}", "kind": "data", "tags": [], "freshness_budget_ticks": 100}}"#
        ));
    }
    things.push(']');
    let mut requires = String::from(r#"["report""#);
    for i in 0..70 {
        requires.push_str(&format!(r#","t{i}""#));
    }
    requires.push(']');
    let json = scene_with(
        default_places(),
        &things,
        "[]",
        &format!(
            r#"[{{
                "id": "d1", "type": "x",
                "target": {{"type": "place", "id": "dashboard"}},
                "requires": {requires},
                "spawn_schedule": {fixed},
                "deadline_ticks": 10, "priority": "normal", "value": 1,
                "penalty": {{"score": -1}}
            }}]"#,
            fixed = fixed_schedule_json()
        ),
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::DemandRequiresTooMany { .. }),
        "expected DemandRequiresTooMany, got {err:?}"
    );
}

#[test]
fn demand_schedule_field_out_of_range_rejected() {
    // every_ticks > MAX_DEMAND_TICKS.
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "fixed", "every_ticks": {huge}, "start_tick": 1}},
            "deadline_ticks": 10, "priority": "normal", "value": 1,
            "penalty": {{"score": -1}}
        }}]"#,
        huge = u64::MAX
    ));
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::DemandScheduleFieldOutOfRange { .. }),
        "expected DemandScheduleFieldOutOfRange, got {err:?}"
    );
}

#[test]
fn demand_schedule_scripted_too_many_rejected() {
    // Build a strictly-increasing tick list longer than the cap.
    // The constant lives in the engine; we approximate by exceeding
    // any practical cap with 100_001 entries.
    let mut ticks_vec = String::from("[");
    for i in 1..=100_001u64 {
        if i > 1 {
            ticks_vec.push(',');
        }
        ticks_vec.push_str(&i.to_string());
    }
    ticks_vec.push(']');
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {{"type": "scripted", "ticks": {ticks_vec}}},
            "deadline_ticks": 10, "priority": "normal", "value": 1,
            "penalty": {{"score": -1}}
        }}]"#
    ));
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::DemandScheduleScriptedTooMany { .. }),
        "expected DemandScheduleScriptedTooMany, got {err:?}"
    );
}

#[test]
fn demand_deadline_out_of_range_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {fixed},
            "deadline_ticks": {huge}, "priority": "normal", "value": 1,
            "penalty": {{"score": -1}}
        }}]"#,
        fixed = fixed_schedule_json(),
        huge = u64::MAX
    ));
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::DemandDeadlineOutOfRange { .. }),
        "expected DemandDeadlineOutOfRange, got {err:?}"
    );
}

#[test]
fn demand_value_out_of_range_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {fixed},
            "deadline_ticks": 10, "priority": "normal", "value": {huge},
            "penalty": {{"score": -1}}
        }}]"#,
        fixed = fixed_schedule_json(),
        huge = u64::MAX
    ));
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::DemandValueOutOfRange { .. }),
        "expected DemandValueOutOfRange, got {err:?}"
    );
}

#[test]
fn demand_penalty_score_out_of_range_rejected() {
    let json = scene_with_demand(&format!(
        r#"[{{
            "id": "d1", "type": "x",
            "target": {{"type": "place", "id": "dashboard"}},
            "requires": ["report"],
            "spawn_schedule": {fixed},
            "deadline_ticks": 10, "priority": "normal", "value": 1,
            "penalty": {{"score": {worse}}}
        }}]"#,
        fixed = fixed_schedule_json(),
        worse = i64::MIN
    ));
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::DemandPenaltyScoreOutOfRange { .. }),
        "expected DemandPenaltyScoreOutOfRange, got {err:?}"
    );
}

#[test]
fn demand_schedule_unexpected_field_fixed_with_ticks_rejected() {
    let json = scene_with_demand(
        r#"[{
            "id": "d1", "type": "x",
            "target": {"type": "place", "id": "dashboard"},
            "requires": ["report"],
            "spawn_schedule": {"type": "fixed", "every_ticks": 5, "start_tick": 5, "ticks": [1, 2]},
            "deadline_ticks": 10, "priority": "normal", "value": 1,
            "penalty": {"score": -1}
        }]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(
            err,
            Sl1LoadError::DemandScheduleUnexpectedField {
                kind: "fixed",
                field: "ticks",
                ..
            }
        ),
        "expected DemandScheduleUnexpectedField(fixed,ticks), got {err:?}"
    );
}

#[test]
fn demand_schedule_unexpected_field_scripted_with_every_ticks_rejected() {
    let json = scene_with_demand(
        r#"[{
            "id": "d1", "type": "x",
            "target": {"type": "place", "id": "dashboard"},
            "requires": ["report"],
            "spawn_schedule": {"type": "scripted", "ticks": [3], "every_ticks": 5},
            "deadline_ticks": 10, "priority": "normal", "value": 1,
            "penalty": {"score": -1}
        }]"#,
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(
            err,
            Sl1LoadError::DemandScheduleUnexpectedField {
                kind: "scripted",
                field: "every_ticks",
                ..
            }
        ),
        "expected DemandScheduleUnexpectedField(scripted,every_ticks), got {err:?}"
    );
}

#[test]
fn demand_expired_pending_is_dropped_not_fulfilled_when_requires_arrive_late() {
    // Regression for the spec rule "Dropped when deadline passes".
    // A demand spawned at tick 1 with deadline_ticks=2 has
    // deadline_tick=3. We make `report` arrive only at tick 5 by
    // running a slow transform. The expired instance MUST be dropped
    // and NOT fulfilled when the requirement finally shows up.
    let places = r#"[
        {
            "id": "dashboard", "role": "consumer", "pos": [1.0, 0.0],
            "capacity": {"queries": 1, "slots": 1},
            "storage": {"report": {"capacity": 4, "initial": 0}},
            "accepts": ["report"], "produces": ["report"]
        }
    ]"#;
    let transforms = r#"[
        {
            "id": "slow_report",
            "type": "report_refresh",
            "runs_on": "dashboard",
            "inputs": [],
            "outputs": [{"thing": "report", "amount": 1}],
            "cadence_ticks": 100,
            "duration_ticks": 5,
            "deadline_ticks": 100,
            "capacity_cost": {"slots": 1},
            "failure_policy": "retry_then_warn",
            "max_attempts": 1
        }
    ]"#;
    let demand = r#"[{
        "id": "d1", "type": "x",
        "target": {"type": "place", "id": "dashboard"},
        "requires": ["report"],
        "spawn_schedule": {"type": "fixed", "every_ticks": 100, "start_tick": 1},
        "deadline_ticks": 2,
        "priority": "normal", "value": 7,
        "penalty": {"score": -3}
    }]"#;
    let json = scene_with(places, default_things(), transforms, demand);
    let mut scene = load_scene_str(&json, 0).expect("load");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    // Tick to 6 — transform completes at tick 6 (cadence start +
    // duration 5); demand spawned at tick 1, deadline tick = 3, so
    // by tick 4 it must drop before any fulfill attempt.
    for _ in 0..6 {
        runner.tick_once(&mut world);
    }
    let rt = world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .demand
        .get("d1")
        .unwrap();
    assert_eq!(
        rt.dropped_count, 1,
        "expired demand must drop, not fulfill, when requires arrive late"
    );
    assert_eq!(rt.fulfilled_count, 0, "no fulfillment expected");
    assert_eq!(rt.pending.len(), 0, "no pending left");
}

#[test]
fn demand_fulfillment_does_not_decrement_inventory() {
    // Observation-only contract: present a report once, fulfill many.
    let places = r#"[
        {
            "id": "dashboard", "role": "consumer", "pos": [0.0, 0.0],
            "capacity": {"queries": 1},
            "storage": {"report": {"capacity": 10, "initial": 3}},
            "accepts": ["report"], "produces": []
        }
    ]"#;
    let demand = r#"[{
        "id": "d1", "type": "x",
        "target": {"type": "place", "id": "dashboard"},
        "requires": ["report"],
        "spawn_schedule": {"type": "fixed", "every_ticks": 1, "start_tick": 1},
        "deadline_ticks": 100,
        "priority": "normal", "value": 1,
        "penalty": {"score": -1}
    }]"#;
    let json = scene_with(places, default_things(), "[]", demand);
    let mut scene = load_scene_str(&json, 0).expect("load");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    for _ in 0..5 {
        runner.tick_once(&mut world);
    }
    // After 5 ticks: 5 demands spawned (one per tick), all fulfilled
    // observation-only. Inventory must still read 3.
    let rt = world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .demand
        .get("d1")
        .unwrap();
    assert_eq!(rt.fulfilled_count, 5);
    let inv = world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .inventories
        .get("dashboard")
        .unwrap();
    assert_eq!(inv.get("report").copied().unwrap_or(0), 3);
}

#[test]
fn demand_penalty_warning_tag_propagates_to_runtime_warning() {
    // The author-supplied `penalty.warning` tag must surface on the
    // runtime Dropped warning. PR 5 carries it opaquely; PR 8/9 will
    // route severity from it.
    let places = r#"[
        {
            "id": "dashboard", "role": "consumer", "pos": [0.0, 0.0],
            "capacity": {"queries": 1},
            "storage": {"report": {"capacity": 4, "initial": 0}},
            "accepts": ["report"], "produces": []
        }
    ]"#;
    let demand = r#"[{
        "id": "d1", "type": "x",
        "target": {"type": "place", "id": "dashboard"},
        "requires": ["report"],
        "spawn_schedule": {"type": "fixed", "every_ticks": 100, "start_tick": 1},
        "deadline_ticks": 2,
        "priority": "normal", "value": 5,
        "penalty": {"score": -3, "warning": "freshness_slo_violated"}
    }]"#;
    let json = scene_with(places, default_things(), "[]", demand);
    let messages = tick_n_collect_warnings(&json, 6);
    let dropped: Vec<&SimMessage> = messages
        .iter()
        .filter(|m| {
            matches!(
                m,
                SimMessage::Warning(WarningPayload::Sl1Demand {
                    event: Sl1DemandWarningKind::Dropped,
                    ..
                })
            )
        })
        .collect();
    assert_eq!(dropped.len(), 1, "expected exactly one Dropped warning");
    if let SimMessage::Warning(WarningPayload::Sl1Demand {
        penalty_warning, ..
    }) = dropped[0]
    {
        assert_eq!(
            penalty_warning.as_deref(),
            Some("freshness_slo_violated"),
            "penalty.warning author tag must be carried through to runtime warning"
        );
    }
}

// -------------------------------------------------------------------
// Fixture + deterministic hash baseline
// -------------------------------------------------------------------

#[test]
fn demand_fixture_loads() {
    let scene = load_scene_str(DEMAND_SCENE, 0).expect("demand fixture loads");
    let sl1 = scene.world.sl1.as_ref().expect("sl1 present");
    assert_eq!(sl1.demand.len(), 2);
    let static_payload = encode_static(&scene);
    assert_eq!(static_payload.sl1_demand.len(), 2);
}

#[test]
fn demand_fixture_ticks_deterministically_against_baseline() {
    let mut scene = load_scene_str(DEMAND_SCENE, 0).expect("demand fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let hash = hash_run(&mut world, &mut runner, TICKS);
    let baseline = DEMAND_BASELINE.trim();
    if baseline.is_empty() {
        eprintln!("RECORD: write the following to tests/baselines/sl1-demand.hash");
        eprintln!("{hash}");
        panic!("missing baseline — rerun after writing baseline");
    }
    assert_eq!(
        hash, baseline,
        "deterministic hash drift detected for sl1-demand.json\n  baseline: {baseline}\n  current:  {hash}"
    );
}
