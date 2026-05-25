//! `scenario_language_v1` Objectives / FailureConditions / VictoryConditions
//! integration tests (PR 8).
//!
//! Exercises:
//!   - Every typed loader error variant via minimal scenes.
//!   - All three evaluated objective kinds (`keep_fresh`,
//!     `complete_jobs_before_deadline`, `maintain_utilization`).
//!   - All three FC kinds (`stale_target`, `place_state`,
//!     `objective_breach_count`).
//!   - The single VC kind `survive_until`.
//!   - `Sl1ObjectiveStatus` transitions emit
//!     `SimEvent::Sl1ObjectiveStateChanged` exactly once per change.
//!   - Recognized-but-unsupported objectives emit a one-shot
//!     `WarningPayload::Sl1Objective::UnsupportedInThisPr`.
//!   - `GameOutcome` transitions: `InProgress -> Won` and
//!     `InProgress -> Lost { reason: "failure_condition:<id>" }`,
//!     and that Won/Lost are sticky.
//!   - `Sl1GamePhase` derivation rules.
//!   - The `sl1-objectives.json` fixture ticks deterministically
//!     against `tests/baselines/sl1-objectives.hash`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{
    hash_run, load_scene_str, GameOutcome, LoadError, Sl1LoadError, TickRunner, World,
};
use simetro_protocol::{
    SimEvent, SimMessage, Sl1ObjectiveStatusTag, Sl1ObjectiveWarningKind, WarningPayload,
};

const OBJ_SCENE: &str = include_str!("fixtures/sl1-objectives.json");
const OBJ_BASELINE: &str = include_str!("../../../tests/baselines/sl1-objectives.hash");
const TICKS: u64 = 60;

// -------------------------------------------------------------------
// Scene helpers
// -------------------------------------------------------------------

fn scene_with(objectives_json: &str, failures_json: &str, victories_json: &str) -> String {
    scene_with_full(
        default_places(),
        default_things(),
        default_demand(),
        objectives_json,
        failures_json,
        victories_json,
    )
}

fn scene_with_full(
    places_json: &str,
    things_json: &str,
    demand_json: &str,
    objectives_json: &str,
    failures_json: &str,
    victories_json: &str,
) -> String {
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-obj-test",
            "theme": {{ "palette": ["#000000"], "background_index": 0, "font": "system-ui" }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places_json},
                "things": {things_json},
                "demand": {demand_json},
                "objectives": {objectives_json},
                "failure_conditions": {failures_json},
                "victory_conditions": {victories_json}
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
            "accepts": ["raw_material"], "produces": ["report"],
            "operating_states": {
                "saturated": { "when": "machine_hours.used_percent >= 90" }
            }
        }
    ]"#
}

fn default_things() -> &'static str {
    r#"[
        { "id": "raw_material", "kind": "input", "tags": [] },
        { "id": "report",       "kind": "data",  "tags": [], "freshness_budget_ticks": 50 }
    ]"#
}

fn default_demand() -> &'static str {
    r#"[
        {
            "id": "d1", "type": "report_refresh",
            "target": { "type": "place", "id": "factory" },
            "requires": ["report"],
            "spawn_schedule": { "type": "fixed", "every_ticks": 5, "start_tick": 5 },
            "deadline_ticks": 10, "priority": "normal", "value": 5,
            "penalty": { "score": -1 }
        }
    ]"#
}

fn expect_sl1_err(json: String) -> Sl1LoadError {
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(e) => e,
        other => panic!("expected LoadError::Sl1, got {other:?}"),
    }
}

/// Run `ticks` ticks against `world`, accumulating events and messages
/// across ticks. The `TickRunner` clears its per-tick buffer on every
/// `tick_once`, so tests that need to observe events from earlier ticks
/// must drain after every step.
fn run_ticks(world: &mut simetro_engine::World, ticks: u64) -> (Vec<SimEvent>, Vec<SimMessage>) {
    let mut runner = TickRunner::new();
    let mut events: Vec<SimEvent> = Vec::new();
    let mut messages: Vec<SimMessage> = Vec::new();
    for _ in 0..ticks {
        runner.tick_once(world);
        events.extend_from_slice(runner.events());
        messages.extend_from_slice(runner.messages());
    }
    (events, messages)
}

// -------------------------------------------------------------------
// Loader error coverage
// -------------------------------------------------------------------

#[test]
fn objective_invalid_id_rejected() {
    let json = scene_with(
        r#"[{"id":"BAD ID","type":"keep_fresh","place":"factory","thing":"report","max_stale_ticks":10}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveInvalidId { .. }
    ));
}

#[test]
fn objective_duplicate_id_rejected() {
    let json = scene_with(
        r#"[
            {"id":"o","type":"keep_fresh","place":"factory","thing":"report","max_stale_ticks":10},
            {"id":"o","type":"keep_fresh","place":"factory","thing":"report","max_stale_ticks":20}
        ]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveDuplicateId { .. }
    ));
}

#[test]
fn objective_unknown_type_rejected() {
    let json = scene_with(r#"[{"id":"o","type":"not_a_kind"}]"#, r#"[]"#, r#"[]"#);
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveUnknownType { .. }
    ));
}

#[test]
fn objective_keep_fresh_missing_field_rejected() {
    let json = scene_with(
        r#"[{"id":"o","type":"keep_fresh","place":"factory"}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveMissingField { field: "thing", .. }
    ));
}

#[test]
fn objective_keep_fresh_unexpected_field_rejected() {
    let json = scene_with(
        r#"[{"id":"o","type":"keep_fresh","place":"factory","thing":"report",
             "max_stale_ticks":10,"max_missed":5}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveUnexpectedField {
            field: "max_missed",
            ..
        }
    ));
}

#[test]
fn objective_keep_fresh_unknown_place_rejected() {
    let json = scene_with(
        r#"[{"id":"o","type":"keep_fresh","place":"ghost","thing":"report","max_stale_ticks":10}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveUnknownTarget { .. }
    ));
}

#[test]
fn objective_keep_fresh_no_storage_slot_rejected() {
    // The thing exists in the registry but the target place lacks a
    // storage slot for it → ObjectiveNoStorageSlot.
    let json = scene_with_full(
        r#"[{"id":"factory","role":"p","pos":[0,0],
             "capacity":{"h":1},"storage":{"raw_material":{"capacity":1,"initial":0}},
             "accepts":[],"produces":[]}]"#,
        r#"[{"id":"raw_material","kind":"x","tags":[]},
            {"id":"report","kind":"x","tags":[],"freshness_budget_ticks":10}]"#,
        r#"[]"#,
        r#"[{"id":"o","type":"keep_fresh","place":"factory","thing":"report","max_stale_ticks":10}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveNoStorageSlot { .. }
    ));
}

#[test]
fn objective_complete_jobs_unknown_demand_rejected() {
    let json = scene_with(
        r#"[{"id":"o","type":"complete_jobs_before_deadline","demand":"ghost","max_missed":1}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveUnknownTarget { .. }
    ));
}

#[test]
fn objective_maintain_utilization_invalid_range_rejected() {
    let json = scene_with(
        r#"[{"id":"o","type":"maintain_utilization","place":"factory",
             "capacity":"machine_hours","min_percent":80,"max_percent":50}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveInvalidPercentRange { .. }
    ));
}

#[test]
fn objective_maintain_utilization_unknown_capacity_rejected() {
    let json = scene_with(
        r#"[{"id":"o","type":"maintain_utilization","place":"factory",
             "capacity":"ghost","min_percent":0,"max_percent":100}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveUnknownCapacityBucket { .. }
    ));
}

#[test]
fn objective_weight_out_of_range_rejected() {
    let json = scene_with(
        r#"[{"id":"o","type":"keep_fresh","weight":999999,
             "place":"factory","thing":"report","max_stale_ticks":10}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::ObjectiveWeightOutOfRange { .. }
    ));
}

// ---- failure conditions ----

#[test]
fn fc_invalid_id_rejected() {
    let json = scene_with(
        r#"[]"#,
        r#"[{"id":"BAD ID","type":"stale_target","place":"factory","thing":"report","threshold_ticks":10}]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::FailureConditionInvalidId { .. }
    ));
}

#[test]
fn fc_unknown_type_rejected() {
    let json = scene_with(r#"[]"#, r#"[{"id":"f","type":"unknown_kind"}]"#, r#"[]"#);
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::FailureConditionUnknownType { .. }
    ));
}

#[test]
fn fc_place_state_unsupported_predicate_rejected() {
    // operating_state predicate `OverloadedTicksGt` is rejected at
    // load time for `place_state` FCs (PR 8 supports only UsedPercentGte).
    let json = scene_with_full(
        r#"[{
            "id":"factory","role":"p","pos":[0,0],
            "capacity":{"machine_hours":4},
            "storage":{"report":{"capacity":10,"initial":0}},
            "accepts":[],"produces":[],
            "operating_states":{
                "long":{"when":"overloaded_ticks > 5"}
            }
        }]"#,
        default_things(),
        r#"[]"#,
        r#"[]"#,
        r#"[{"id":"f","type":"place_state","place":"factory","state":"long"}]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::FailureConditionPlaceStatePredicateUnsupported { .. }
    ));
}

#[test]
fn fc_objective_breach_count_unknown_objective_rejected() {
    let json = scene_with(
        r#"[]"#,
        r#"[{"id":"f","type":"objective_breach_count","objective_id":"ghost","max_count":5}]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::FailureConditionUnknownObjective { .. }
    ));
}

#[test]
fn fc_unexpected_field_rejected() {
    let json = scene_with(
        r#"[]"#,
        r#"[{"id":"f","type":"stale_target","place":"factory","thing":"report",
             "threshold_ticks":10,"max_count":7}]"#,
        r#"[]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::FailureConditionUnexpectedField { .. }
    ));
}

// ---- victory conditions ----

#[test]
fn vc_invalid_id_rejected() {
    let json = scene_with(
        r#"[]"#,
        r#"[]"#,
        r#"[{"id":"BAD ID","type":"survive_until","at_tick":100}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::VictoryConditionInvalidId { .. }
    ));
}

#[test]
fn vc_unknown_type_rejected() {
    let json = scene_with(r#"[]"#, r#"[]"#, r#"[{"id":"v","type":"win_the_game"}]"#);
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::VictoryConditionUnknownType { .. }
    ));
}

#[test]
fn vc_missing_at_tick_rejected() {
    let json = scene_with(r#"[]"#, r#"[]"#, r#"[{"id":"v","type":"survive_until"}]"#);
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::VictoryConditionMissingField { .. }
    ));
}

#[test]
fn vc_at_tick_zero_rejected() {
    let json = scene_with(
        r#"[]"#,
        r#"[]"#,
        r#"[{"id":"v","type":"survive_until","at_tick":0}]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::VictoryConditionAtTickOutOfRange { .. }
    ));
}

// -------------------------------------------------------------------
// Runtime: GameOutcome + GamePhase + status transitions
// -------------------------------------------------------------------

fn objective_state_changes(events: &[SimEvent]) -> Vec<(String, Sl1ObjectiveStatusTag, u64)> {
    events
        .iter()
        .filter_map(|e| match e {
            SimEvent::Sl1ObjectiveStateChanged {
                objective_id,
                to,
                tick,
                ..
            } => Some((objective_id.clone(), *to, *tick)),
            _ => None,
        })
        .collect()
}

fn fc_fires(events: &[SimEvent]) -> Vec<(String, u64)> {
    events
        .iter()
        .filter_map(|e| match e {
            SimEvent::Sl1FailureConditionFired {
                failure_condition_id,
                tick,
            } => Some((failure_condition_id.clone(), *tick)),
            _ => None,
        })
        .collect()
}

fn vc_mets(events: &[SimEvent]) -> Vec<(String, u64)> {
    events
        .iter()
        .filter_map(|e| match e {
            SimEvent::Sl1VictoryConditionMet {
                victory_condition_id,
                tick,
            } => Some((victory_condition_id.clone(), *tick)),
            _ => None,
        })
        .collect()
}

fn outcome_changes(events: &[SimEvent]) -> Vec<(String, String, u64, Option<String>)> {
    events
        .iter()
        .filter_map(|e| match e {
            SimEvent::Sl1GameOutcomeChanged {
                from,
                to,
                tick,
                reason,
            } => Some((from.clone(), to.clone(), *tick, reason.clone())),
            _ => None,
        })
        .collect()
}

fn unsupported_warnings(messages: &[SimMessage]) -> Vec<(String, u64)> {
    messages
        .iter()
        .filter_map(|m| match m {
            SimMessage::Warning(WarningPayload::Sl1Objective {
                objective_id,
                event: Sl1ObjectiveWarningKind::UnsupportedInThisPr,
                tick,
                ..
            }) => Some((objective_id.clone(), *tick)),
            _ => None,
        })
        .collect()
}

#[test]
fn survive_until_emits_won_at_target_tick() {
    let json = scene_with(
        r#"[]"#,
        r#"[]"#,
        r#"[{"id":"v","type":"survive_until","at_tick":3}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 5);
    let mets = vc_mets(&events);
    assert_eq!(mets, vec![("v".to_string(), 3)]);
    let outcomes = outcome_changes(&events);
    assert_eq!(outcomes.len(), 1, "exactly one outcome change");
    assert_eq!(outcomes[0].1, "won");
    assert_eq!(outcomes[0].2, 3);
    assert_eq!(world.sl1_outcome(), GameOutcome::Won);
}

#[test]
fn stale_target_fires_after_grace_and_emits_lost() {
    // No producer of `report` -> NoData -> stale_target sees it as
    // "stale beyond threshold" every tick. grace=2 means it fires on
    // tick 3 (streak goes 1,2,3 → fires when streak>grace).
    let json = scene_with_full(
        default_places(),
        default_things(),
        r#"[]"#,
        r#"[]"#,
        r#"[{"id":"f","type":"stale_target","place":"factory","thing":"report",
             "threshold_ticks":10,"grace_ticks":2}]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 10);
    let fires = fc_fires(&events);
    assert_eq!(fires.len(), 1, "fc fires exactly once");
    assert_eq!(fires[0].0, "f");
    assert_eq!(fires[0].1, 3, "streak 1,2,3 → fires on tick 3");

    let outcomes = outcome_changes(&events);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].1, "lost");
    assert_eq!(outcomes[0].3, Some("failure_condition:f".to_string()));
    assert!(matches!(world.sl1_outcome(), GameOutcome::Lost { .. }));
}

#[test]
fn lost_is_sticky_no_further_events_after_terminal() {
    let json = scene_with_full(
        default_places(),
        default_things(),
        r#"[]"#,
        r#"[]"#,
        r#"[{"id":"f","type":"stale_target","place":"factory","thing":"report",
             "threshold_ticks":10,"grace_ticks":1}]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 20);
    let outcomes = outcome_changes(&events);
    assert_eq!(
        outcomes.len(),
        1,
        "no additional outcome changes after Lost"
    );
    let fires = fc_fires(&events);
    assert_eq!(fires.len(), 1, "fc fires exactly once even after Lost");
}

#[test]
fn unsupported_objective_emits_one_shot_warning() {
    let json = scene_with(
        r#"[{"id":"budget","type":"cost_budget","max_cost":100}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (_events, msgs) = run_ticks(&mut world, 10);
    let warns = unsupported_warnings(&msgs);
    assert_eq!(warns.len(), 1, "exactly one unsupported warning per run");
    assert_eq!(warns[0].0, "budget");
}

#[test]
fn simultaneous_fc_and_vc_lost_wins() {
    // Both fire on tick 1: stale_target with grace=0 fires immediately
    // on NoData, and survive_until at_tick=1 is met on tick 1. The
    // outcome rule is "any FC wins over Won" → expect Lost, not Won.
    let json = scene_with_full(
        default_places(),
        default_things(),
        r#"[]"#,
        r#"[]"#,
        r#"[{"id":"f","type":"stale_target","place":"factory","thing":"report",
             "threshold_ticks":1,"grace_ticks":0}]"#,
        r#"[{"id":"v","type":"survive_until","at_tick":1}]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 3);
    let outcomes = outcome_changes(&events);
    assert_eq!(outcomes.len(), 1, "exactly one terminal transition");
    assert_eq!(outcomes[0].1, "lost", "Lost wins over Won on same tick");
    assert_eq!(outcomes[0].3, Some("failure_condition:f".to_string()));
}

#[test]
fn multiple_fcs_firing_same_tick_lowest_id_wins() {
    // Two stale_target FCs with the same trigger conditions both fire
    // on tick 1. Lost reason should reference the lowest id ("a"),
    // not the highest ("z"), and exactly one outcome change is emitted.
    let json = scene_with_full(
        default_places(),
        default_things(),
        r#"[]"#,
        r#"[]"#,
        r#"[
            {"id":"z","type":"stale_target","place":"factory","thing":"report",
             "threshold_ticks":1,"grace_ticks":0},
            {"id":"a","type":"stale_target","place":"factory","thing":"report",
             "threshold_ticks":1,"grace_ticks":0}
        ]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 3);
    let outcomes = outcome_changes(&events);
    assert_eq!(outcomes.len(), 1, "exactly one terminal transition");
    assert_eq!(outcomes[0].1, "lost");
    assert_eq!(
        outcomes[0].3,
        Some("failure_condition:a".to_string()),
        "lowest FC id wins the Lost reason"
    );
}

#[test]
fn keep_fresh_breached_when_no_data() {
    let json = scene_with_full(
        default_places(),
        default_things(),
        r#"[]"#,
        r#"[{"id":"o","type":"keep_fresh","place":"factory","thing":"report","max_stale_ticks":5}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 3);
    let changes = objective_state_changes(&events);
    assert!(
        changes
            .iter()
            .any(|(id, to, _)| id == "o" && *to == Sl1ObjectiveStatusTag::Breached),
        "expected Breached transition, got {changes:?}"
    );
}

#[test]
fn keep_fresh_with_initial_data_stays_met_until_max_stale_age() {
    // Place starts with report.initial=10, so freshness starts `Ok{last_set_tick:0}`.
    // `max_stale_ticks=5` => Met while `now <= 5`, Breached on tick 6.
    // `freshness_budget_ticks=50` ensures the Ok→Stale transition does
    // not occur before the objective breaches, so this test exercises
    // the Ok branch's age check, not the Stale branch.
    let places = r#"[
        {
            "id": "factory", "role": "producer", "pos": [0,0],
            "capacity": { "machine_hours": 4 },
            "storage": {
                "report": { "capacity": 100, "initial": 10 }
            },
            "accepts": [], "produces": []
        }
    ]"#;
    let json = scene_with_full(
        places,
        default_things(),
        r#"[]"#,
        r#"[{"id":"o","type":"keep_fresh","place":"factory","thing":"report","max_stale_ticks":5}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 7);
    let changes = objective_state_changes(&events);
    // First state change is initial Met (tick 1) — Breached fires once age > 5.
    let breach = changes
        .iter()
        .find(|(id, to, _)| id == "o" && *to == Sl1ObjectiveStatusTag::Breached)
        .expect("expected Breached transition");
    assert_eq!(
        breach.2, 6,
        "Breached should fire on tick 6 (age=6 > max_stale_ticks=5), got {breach:?}"
    );
}

#[test]
fn maintain_utilization_met_when_in_range() {
    let json = scene_with_full(
        default_places(),
        default_things(),
        r#"[]"#,
        r#"[{"id":"u","type":"maintain_utilization","place":"factory",
             "capacity":"machine_hours","min_percent":0,"max_percent":100}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 3);
    let changes = objective_state_changes(&events);
    assert!(
        changes
            .iter()
            .any(|(id, to, _)| id == "u" && *to == Sl1ObjectiveStatusTag::Met),
        "expected Met transition, got {changes:?}"
    );
}

#[test]
fn maintain_utilization_zero_cap_with_nonzero_min_is_breached() {
    // Place has machine_hours capacity but no demand/transform → 0% used.
    // min_percent=50 means 0% is out of range → Breached.
    let json = scene_with_full(
        r#"[{
            "id":"factory","role":"p","pos":[0,0],
            "capacity":{"machine_hours":1},
            "storage":{"report":{"capacity":10,"initial":0}},
            "accepts":[],"produces":[]
        }]"#,
        default_things(),
        r#"[]"#,
        r#"[{"id":"u","type":"maintain_utilization","place":"factory",
             "capacity":"machine_hours","min_percent":50,"max_percent":100}]"#,
        r#"[]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 2);
    let changes = objective_state_changes(&events);
    assert!(
        changes
            .iter()
            .any(|(id, to, _)| id == "u" && *to == Sl1ObjectiveStatusTag::Breached),
        "expected Breached for low-utilization, got {changes:?}"
    );
}

#[test]
fn objective_breach_count_fc_fires_after_n_breached_ticks() {
    // KeepFresh on a thing with no data → Breached every tick.
    // ObjectiveBreachCount max_count=3 → fires when breach_tick_count > 3
    // and the FC's streak goes 1 the same tick the underlying condition
    // holds (grace=0 means streak>0 fires).
    let json = scene_with_full(
        default_places(),
        default_things(),
        r#"[]"#,
        r#"[{"id":"o","type":"keep_fresh","place":"factory","thing":"report","max_stale_ticks":1}]"#,
        r#"[{"id":"f","type":"objective_breach_count","objective_id":"o","max_count":3}]"#,
        r#"[]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let (events, _msgs) = run_ticks(&mut world, 15);
    let fires = fc_fires(&events);
    assert_eq!(fires.len(), 1);
    assert_eq!(fires[0].0, "f");
    assert!(
        fires[0].1 >= 4,
        "fc fires after objective breaches >3 ticks, got tick={}",
        fires[0].1
    );
}

// -------------------------------------------------------------------
// Fixture: deterministic ticks against baseline hash
// -------------------------------------------------------------------

#[test]
fn fixture_loads_and_round_trips() {
    let scene = load_scene_str(OBJ_SCENE, 0).expect("fixture loads");
    let sl1 = scene.world.sl1.as_ref().expect("sl1 present");
    assert_eq!(sl1.objectives.len(), 4);
    assert_eq!(sl1.failure_conditions.len(), 2);
    assert_eq!(sl1.victory_conditions.len(), 1);
    // sorted by id at validation time.
    let oids: Vec<&str> = sl1.objectives.iter().map(|o| o.id.as_str()).collect();
    let mut sorted = oids.clone();
    sorted.sort_unstable();
    assert_eq!(oids, sorted);
}

#[test]
fn fixture_hash_matches_baseline() {
    let mut scene = load_scene_str(OBJ_SCENE, 0).expect("fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let hash = hash_run(&mut world, &mut runner, TICKS);
    let expected = OBJ_BASELINE.trim();
    assert_eq!(
        hash, expected,
        "sl1-objectives hash drifted; if intentional, update baseline"
    );
}
