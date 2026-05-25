//! `scenario_language_v1` Agents + scoped actions integration tests (PR 10).
//!
//! Exercises:
//!   - Every typed loader error variant via minimal scenes.
//!   - All three agent kinds (`mock`, `builtin`, `llm`).
//!   - Cadence: an agent fires only on multiples of `interval_ticks`.
//!   - ScriptedBackend successfully proposing `ThrottleDemand`:
//!     `Sl1AgentActionApplied` emitted, `agent_demand_pauses` set,
//!     spawn skipped during the pause window.
//!   - Rejection variants surfaced as `Sl1AgentActionRejected` with
//!     the typed reason:
//!       * `ActionNotAllowed`
//!       * `ActionTargetOutOfScope`
//!       * `ActionTargetUnknown`
//!       * `Cooldown`
//!       * `InvalidActionParameter` (`pause_ticks = 0`)
//!   - `llm` agents emit exactly one `Sl1AgentLlmDisabled` per scene
//!     run regardless of how many cadence ticks fire.
//!   - Declaration-order independence (canonical sort by id).
//!   - Protocol round-trip of static `sl1_agents` payload.
//!   - Determinism: hash baseline.
//!
//! Notes:
//!   - `CostExceedsBudget` is unreachable in PR 10 because every
//!     action costs `1` and the loader requires
//!     `max_cost_per_decision >= 1`. The rejection path exists for
//!     future PRs and is left untested here.
//!   - `EffectUnsupportedInThisPr` is unreachable from any backend
//!     shipped in PR 10 (only `ThrottleDemand` is constructible).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::sl1_agents::{run_with_factory, ScriptedBackend};
use simetro_engine::{
    hash_run, load_scene_str, LoadError, Sl1AgentAction, Sl1AgentKind, Sl1AgentRuntimeState,
    Sl1LoadError, TickRunner, World,
};
use simetro_protocol::{SimEvent, SimMessage, StaticPayload};

const AGENTS_SCENE: &str = include_str!("fixtures/sl1-agents.json");
const AGENTS_BASELINE: &str = include_str!("../../../tests/baselines/sl1-agents.hash");
const TICKS: u64 = 60;

// -------------------------------------------------------------------
// Scene helpers — minimal SL1 scenes for loader error coverage
// -------------------------------------------------------------------

fn scene_with(agents_json: &str) -> String {
    scene_with_extras(
        default_places(),
        default_things(),
        default_demand(),
        "{}",
        "[]",
        agents_json,
    )
}

fn scene_with_extras(
    places: &str,
    things: &str,
    demand: &str,
    observability: &str,
    objectives: &str,
    agents: &str,
) -> String {
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-agents-test",
            "theme": {{ "palette": ["#000000"], "background_index": 0, "font": "system-ui" }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places},
                "things": {things},
                "demand": {demand},
                "observability": {observability},
                "objectives": {objectives},
                "agents": {agents}
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
        },
        {
            "id": "sink", "role": "consumer", "pos": [10,0],
            "capacity": { "queries": 8 },
            "storage": { "report": { "capacity": 100, "initial": 0 } },
            "accepts": ["report"], "produces": []
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
            "id": "report_demand",
            "type": "throughput",
            "target": { "type": "place", "id": "sink" },
            "requires": ["report"],
            "spawn_schedule": { "type": "fixed", "every_ticks": 4, "start_tick": 1 },
            "deadline_ticks": 200,
            "priority": "normal",
            "value": 10,
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

fn run_ticks(world: &mut World, ticks: u64) -> (Vec<SimEvent>, Vec<SimMessage>) {
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
fn agent_invalid_id_rejected() {
    let json = scene_with(
        r#"[ { "id":"BAD ID", "kind":"mock", "role":"r", "interval_ticks":1,
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentInvalidId { .. }
    ));
}

#[test]
fn agent_duplicate_id_rejected() {
    let json = scene_with(
        r#"[
            { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
              "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } },
            { "id":"a", "kind":"mock", "role":"r", "interval_ticks":2,
              "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } }
        ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentDuplicateId { .. }
    ));
}

#[test]
fn agent_unknown_kind_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"wizard", "role":"r", "interval_ticks":1,
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentUnknownKind { .. }
    ));
}

#[test]
fn agent_role_empty_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"  ", "interval_ticks":1,
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentRoleEmpty { .. }
    ));
}

#[test]
fn agent_interval_ticks_zero_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":0,
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentIntervalTicksZero { .. }
    ));
}

#[test]
fn agent_interval_ticks_out_of_range_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":9999999,
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentIntervalTicksOutOfRange { .. }
    ));
}

#[test]
fn agent_max_cost_per_decision_zero_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "budgets": { "max_cost_per_decision": 0, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentMaxCostPerDecisionZero { .. }
    ));
}

#[test]
fn agent_cooldown_ticks_out_of_range_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 9999999 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentCooldownTicksOutOfRange { .. }
    ));
}

#[test]
fn agent_observation_scope_malformed_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "observation_scope": ["no_colon"],
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentObservationScopeMalformed { .. }
    ));
}

#[test]
fn agent_observation_scope_malformed_empty_id_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "observation_scope": ["place:"],
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentObservationScopeMalformed { .. }
    ));
}

#[test]
fn agent_observation_scope_unknown_id_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "observation_scope": ["place:nope"],
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentObservationScopeUnknownId { .. }
    ));
}

#[test]
fn agent_observation_scope_duplicate_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "observation_scope": ["place:factory", "place:factory"],
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentObservationScopeDuplicate { .. }
    ));
}

#[test]
fn agent_allowed_actions_unknown_kind_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "allowed_actions": ["delete_universe"],
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentAllowedActionsUnknownKind { .. }
    ));
}

#[test]
fn agent_allowed_actions_duplicate_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "allowed_actions": ["throttle_demand", "throttle_demand"],
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentAllowedActionsDuplicate { .. }
    ));
}

#[test]
fn agent_objective_weight_unknown_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "objective_weights": { "no_such_objective": 0.5 },
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentObjectiveWeightUnknown { .. }
    ));
}

#[test]
fn agent_objective_weight_out_of_range_rejected() {
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "objective_weights": { "obj1": 1.5 },
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    assert!(matches!(
        expect_sl1_err(json),
        Sl1LoadError::AgentObjectiveWeightOutOfRange { .. }
    ));
}

#[test]
fn agent_objective_weight_non_finite_rejected() {
    // Serde itself rejects `1e9999` as a parse-time number-out-of-range
    // error before reaching the typed validator. Either outcome counts
    // as load failure: the loader never accepts non-finite weights.
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "objective_weights": { "obj1": 1e9999 },
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(Sl1LoadError::AgentObjectiveWeightNonFinite { .. })
        | LoadError::Sl1(Sl1LoadError::Parse { .. })
        | LoadError::Parse { .. } => {}
        other => panic!("expected non-finite or parse load error, got {other:?}"),
    }
}

#[test]
fn agent_unknown_field_rejected() {
    // Strict-schema at the agent level — typos in field names fail load.
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "wat_is_this": true,
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } } ]"#,
    );
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(_) | LoadError::Parse { .. } => {}
        other => panic!("expected Sl1 or Parse load error, got {other:?}"),
    }
}

#[test]
fn agent_budgets_unknown_field_rejected() {
    // Strict-schema applies to the nested `budgets` struct too: typos
    // inside it must not silently no-op.
    let json = scene_with(
        r#"[ { "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
               "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0, "lol_nope": 7 } } ]"#,
    );
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(_) | LoadError::Parse { .. } => {}
        other => panic!("expected Sl1 or Parse load error, got {other:?}"),
    }
}

// -------------------------------------------------------------------
// Fixture happy path
// -------------------------------------------------------------------

#[test]
fn fixture_loads_and_initializes_runtime_state() {
    let scene = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let sl1 = scene.world.sl1.as_ref().expect("sl1 scene present");
    // Canonical sort by id.
    let ids: Vec<&str> = sl1.agents.iter().map(|a| a.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
    assert_eq!(ids, vec!["mock_agent", "throttler"]);

    // Runtime state seeded for every agent.
    let runtime = scene.world.sl1_runtime.as_ref().expect("runtime present");
    assert_eq!(runtime.agents.len(), 2);
    assert!(runtime.agents.contains_key("mock_agent"));
    assert!(runtime.agents.contains_key("throttler"));
    assert!(runtime.agent_demand_pauses.is_empty());
}

#[test]
fn declaration_order_independent() {
    // Renaming an agent and reloading should still produce canonical
    // sort order (by id), regardless of declaration order in JSON.
    let reordered_scene =
        AGENTS_SCENE.replace(r#""id": "mock_agent","#, r#""id": "zzz_temp_mock","#);
    let scene = load_scene_str(&reordered_scene, 0).expect("loads with renamed agent");
    let sl1 = scene.world.sl1.as_ref().expect("sl1 scene present");
    let ids: Vec<&str> = sl1.agents.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["throttler", "zzz_temp_mock"]);
}

// -------------------------------------------------------------------
// Cadence (MockBackend, no actions)
// -------------------------------------------------------------------

#[test]
fn mock_agent_emits_no_action_events() {
    let mut scene = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    let (events, _) = run_ticks(&mut world, TICKS);
    // No applied/rejected events because MockBackend returns None.
    for ev in &events {
        match ev {
            SimEvent::Sl1AgentActionApplied { .. }
            | SimEvent::Sl1AgentActionRejected { .. }
            | SimEvent::Sl1AgentLlmDisabled { .. } => {
                panic!("unexpected agent event: {ev:?}");
            }
            _ => {}
        }
    }
}

#[test]
fn cadence_tracked_in_runtime_state() {
    let mut scene = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    // mock_agent has interval_ticks=3; throttler has interval_ticks=5.
    // After 15 ticks, mock_agent fired on 3,6,9,12,15 and throttler on
    // 5,10,15. Each agent's last_decision_tick reflects the most recent
    // fire.
    let _ = run_ticks(&mut world, 15);
    let runtime = world.sl1_runtime.as_ref().expect("runtime");
    let mock_state = runtime.agents.get("mock_agent").expect("mock state");
    let throttler_state = runtime.agents.get("throttler").expect("throttler state");
    assert_eq!(mock_state.last_decision_tick, Some(15));
    assert_eq!(throttler_state.last_decision_tick, Some(15));
}

// -------------------------------------------------------------------
// ScriptedBackend — happy path (action applied)
// -------------------------------------------------------------------

#[test]
fn scripted_throttle_demand_applies_and_pauses_spawn() {
    let mut scene_loaded = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let world = std::mem::take(&mut scene_loaded.world);
    let scene = world.sl1.expect("sl1 scene present");
    let mut runtime = world.sl1_runtime.expect("runtime present");
    let mut events: Vec<SimEvent> = Vec::new();

    // Tick 5: throttler fires (interval_ticks=5). Script it to throttle
    // the demand for 3 ticks. mock_agent fires on tick 3 but the script
    // only matches `throttler`'s tick (5) — because the script is on
    // tick 5, mock_agent's tick-3 fire produces no action.
    let script = vec![(
        5_u64,
        Sl1AgentAction::ThrottleDemand {
            demand_id: "report_demand".to_string(),
            pause_ticks: 3,
        },
    )];
    let factory = |kind| -> Box<dyn simetro_engine::sl1_agents::AgentBackend> {
        match kind {
            Sl1AgentKind::Builtin => Box::new(ScriptedBackend {
                script: vec![(
                    5,
                    Sl1AgentAction::ThrottleDemand {
                        demand_id: "report_demand".to_string(),
                        pause_ticks: 3,
                    },
                )],
            }),
            _ => Box::new(simetro_engine::sl1_agents::MockBackend),
        }
    };
    let _ = script;

    // Step ticks 1..=6 manually so we can drive only the agent runtime
    // with our factory (we are bypassing the full tick to inject the
    // scripted backend).
    for now in 1..=6_u64 {
        run_with_factory(&scene, &mut runtime, now, &mut events, factory);
    }

    // Exactly one Sl1AgentActionApplied on tick 5.
    let applied: Vec<&SimEvent> = events
        .iter()
        .filter(|e| matches!(e, SimEvent::Sl1AgentActionApplied { .. }))
        .collect();
    assert_eq!(applied.len(), 1, "events: {events:?}");
    let SimEvent::Sl1AgentActionApplied {
        agent_id,
        action_kind,
        target_id,
        cost,
        tick,
    } = applied[0]
    else {
        unreachable!()
    };
    assert_eq!(agent_id, "throttler");
    assert_eq!(action_kind, "throttle_demand");
    assert_eq!(target_id, "report_demand");
    assert_eq!(*cost, 1);
    assert_eq!(*tick, 5);

    // agent_demand_pauses populated with `now + 1 + pause_ticks = 5+1+3 = 9`.
    let pause_until = *runtime
        .agent_demand_pauses
        .get("report_demand")
        .expect("pause set");
    assert_eq!(pause_until, 9);

    // Cooldown set to 5 + 4 = 9.
    let state = runtime.agents.get("throttler").expect("state");
    assert_eq!(state.cooldown_until_tick, Some(9));
}

// -------------------------------------------------------------------
// Rejections
// -------------------------------------------------------------------

fn load_minimal_with_one_agent(agent: &str) -> (Vec<simetro_engine::Sl1Agent>, World) {
    let json = scene_with_extras(
        default_places(),
        default_things(),
        default_demand(),
        "{}",
        "[]",
        &format!("[{}]", agent),
    );
    let scene = load_scene_str(&json, 0).expect("loads");
    (
        scene.world.sl1.as_ref().unwrap().agents.clone(),
        scene.world,
    )
}

fn drive_one_tick_with_scripted(
    world: &mut World,
    script: Vec<(u64, Sl1AgentAction)>,
    now: u64,
) -> Vec<SimEvent> {
    let scene = world.sl1.as_ref().expect("sl1");
    let runtime = world.sl1_runtime.as_mut().expect("runtime");
    let mut events: Vec<SimEvent> = Vec::new();
    let factory = |_kind| -> Box<dyn simetro_engine::sl1_agents::AgentBackend> {
        Box::new(ScriptedBackend {
            script: script.clone(),
        })
    };
    run_with_factory(scene, runtime, now, &mut events, factory);
    events
}

#[test]
fn rejection_action_not_allowed() {
    // Agent has empty allowed_actions but the scripted backend proposes
    // ThrottleDemand on tick 1.
    let (_, mut world) = load_minimal_with_one_agent(
        r#"{ "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
             "observation_scope": ["demand:report_demand"],
             "allowed_actions": [],
             "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } }"#,
    );
    let events = drive_one_tick_with_scripted(
        &mut world,
        vec![(
            1,
            Sl1AgentAction::ThrottleDemand {
                demand_id: "report_demand".to_string(),
                pause_ticks: 2,
            },
        )],
        1,
    );
    let rejected: Vec<&SimEvent> = events
        .iter()
        .filter(|e| matches!(e, SimEvent::Sl1AgentActionRejected { .. }))
        .collect();
    assert_eq!(rejected.len(), 1);
    let SimEvent::Sl1AgentActionRejected { reason, .. } = rejected[0] else {
        unreachable!()
    };
    assert_eq!(reason, "action_not_allowed");
}

#[test]
fn rejection_action_target_out_of_scope() {
    // Agent's observation_scope does not include the demand.
    let (_, mut world) = load_minimal_with_one_agent(
        r#"{ "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
             "observation_scope": ["place:factory"],
             "allowed_actions": ["throttle_demand"],
             "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } }"#,
    );
    let events = drive_one_tick_with_scripted(
        &mut world,
        vec![(
            1,
            Sl1AgentAction::ThrottleDemand {
                demand_id: "report_demand".to_string(),
                pause_ticks: 2,
            },
        )],
        1,
    );
    let SimEvent::Sl1AgentActionRejected { reason, .. } = events
        .iter()
        .find(|e| matches!(e, SimEvent::Sl1AgentActionRejected { .. }))
        .expect("rejection event")
    else {
        unreachable!()
    };
    assert_eq!(reason, "action_target_out_of_scope");
}

#[test]
fn rejection_action_target_unknown() {
    // Demand id does not match any declared demand.
    let (_, mut world) = load_minimal_with_one_agent(
        r#"{ "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
             "observation_scope": ["demand:report_demand"],
             "allowed_actions": ["throttle_demand"],
             "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } }"#,
    );
    let events = drive_one_tick_with_scripted(
        &mut world,
        vec![(
            1,
            Sl1AgentAction::ThrottleDemand {
                demand_id: "ghost_demand".to_string(),
                pause_ticks: 2,
            },
        )],
        1,
    );
    let SimEvent::Sl1AgentActionRejected { reason, .. } = events
        .iter()
        .find(|e| matches!(e, SimEvent::Sl1AgentActionRejected { .. }))
        .expect("rejection event")
    else {
        unreachable!()
    };
    assert_eq!(reason, "action_target_unknown");
}

#[test]
fn rejection_cooldown() {
    // Agent has cooldown_ticks=5. First action applies on tick 1 and
    // sets cooldown_until_tick=6. A second proposed action on tick 2
    // is rejected with Cooldown.
    let (_, mut world) = load_minimal_with_one_agent(
        r#"{ "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
             "observation_scope": ["demand:report_demand"],
             "allowed_actions": ["throttle_demand"],
             "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 5 } }"#,
    );

    // Tick 1: apply.
    let events1 = drive_one_tick_with_scripted(
        &mut world,
        vec![(
            1,
            Sl1AgentAction::ThrottleDemand {
                demand_id: "report_demand".to_string(),
                pause_ticks: 2,
            },
        )],
        1,
    );
    assert!(events1
        .iter()
        .any(|e| matches!(e, SimEvent::Sl1AgentActionApplied { .. })));

    // Tick 2: cooldown_until=6, so propose again → Cooldown.
    let events2 = drive_one_tick_with_scripted(
        &mut world,
        vec![(
            2,
            Sl1AgentAction::ThrottleDemand {
                demand_id: "report_demand".to_string(),
                pause_ticks: 2,
            },
        )],
        2,
    );
    let SimEvent::Sl1AgentActionRejected { reason, .. } = events2
        .iter()
        .find(|e| matches!(e, SimEvent::Sl1AgentActionRejected { .. }))
        .expect("rejection event")
    else {
        unreachable!()
    };
    assert_eq!(reason, "cooldown");
}

#[test]
fn rejection_invalid_action_parameter_pause_ticks_zero() {
    let (_, mut world) = load_minimal_with_one_agent(
        r#"{ "id":"a", "kind":"mock", "role":"r", "interval_ticks":1,
             "observation_scope": ["demand:report_demand"],
             "allowed_actions": ["throttle_demand"],
             "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } }"#,
    );
    let events = drive_one_tick_with_scripted(
        &mut world,
        vec![(
            1,
            Sl1AgentAction::ThrottleDemand {
                demand_id: "report_demand".to_string(),
                pause_ticks: 0,
            },
        )],
        1,
    );
    let SimEvent::Sl1AgentActionRejected { reason, .. } = events
        .iter()
        .find(|e| matches!(e, SimEvent::Sl1AgentActionRejected { .. }))
        .expect("rejection event")
    else {
        unreachable!()
    };
    assert_eq!(reason, "invalid_action_parameter");
}

// -------------------------------------------------------------------
// LLM-disabled one-shot
// -------------------------------------------------------------------

#[test]
fn llm_agent_emits_llm_disabled_once() {
    // Build a fresh scene with an LLM-kind agent and run it through
    // the regular tick loop for several cadence ticks. The runtime
    // should emit exactly one Sl1AgentLlmDisabled event.
    let json = scene_with_extras(
        default_places(),
        default_things(),
        default_demand(),
        "{}",
        "[]",
        r#"[
            { "id":"llmer", "kind":"llm", "role":"r", "interval_ticks":2,
              "budgets": { "max_cost_per_decision": 1, "cooldown_ticks": 0 } }
        ]"#,
    );
    let mut scene = load_scene_str(&json, 0).expect("loads");
    let mut world = std::mem::take(&mut scene.world);
    let (events, _) = run_ticks(&mut world, 10);
    let llm_evs: Vec<&SimEvent> = events
        .iter()
        .filter(|e| matches!(e, SimEvent::Sl1AgentLlmDisabled { .. }))
        .collect();
    assert_eq!(
        llm_evs.len(),
        1,
        "exactly one disabled event; got {llm_evs:?}"
    );
    let SimEvent::Sl1AgentLlmDisabled { agent_id, tick } = llm_evs[0] else {
        unreachable!()
    };
    assert_eq!(agent_id, "llmer");
    // First cadence tick is 2 (interval_ticks=2, runtime skips tick 0).
    assert_eq!(*tick, 2);

    // Runtime flag set so it isn't re-emitted on subsequent ticks.
    let runtime = world.sl1_runtime.as_ref().unwrap();
    let state = runtime.agents.get("llmer").unwrap();
    assert!(state.llm_disabled_emitted);
}

// -------------------------------------------------------------------
// Demand pause integration (full tick path)
// -------------------------------------------------------------------

#[test]
fn pause_is_monotonic_longer_pause_wins_over_shorter() {
    // Safety: if a longer pause is already installed, a later
    // ThrottleDemand with a shorter window must NOT shrink it.
    // (Otherwise a second agent could accidentally early-unthrottle
    // a demand that another valid agent had paused for longer.)
    let mut scene_loaded = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let mut world = std::mem::take(&mut scene_loaded.world);
    {
        let runtime = world.sl1_runtime.as_mut().unwrap();
        runtime
            .agent_demand_pauses
            .insert("report_demand".to_string(), 100);
    }

    // Tick 5 is the throttler's first cadence (interval_ticks=5).
    // Apply a short pause via scripted backend: 5+1+1 = 7.
    let _ = drive_one_tick_with_scripted(
        &mut world,
        vec![(
            5_u64,
            Sl1AgentAction::ThrottleDemand {
                demand_id: "report_demand".to_string(),
                pause_ticks: 1,
            },
        )],
        5,
    );

    let pause = world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .agent_demand_pauses
        .get("report_demand")
        .copied()
        .expect("pause still present");
    assert_eq!(
        pause, 100,
        "longer existing pause must not be shortened by a later shorter pause"
    );
}

#[test]
fn paused_demand_does_not_spawn_during_pause_window() {
    // Compare two parallel scene loads of the same fixture: one with a
    // ThrottleDemand pause installed, one without. After running the
    // same number of ticks, the paused scene must spawn strictly fewer
    // demand instances. This locks in that `agent_demand_pauses` is
    // actually honored by `run_demand` — not just present in state.

    // Paused branch: install a pause until tick 12.
    let mut paused_scene = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let mut paused_world = std::mem::take(&mut paused_scene.world);
    {
        let runtime = paused_world.sl1_runtime.as_mut().unwrap();
        runtime
            .agent_demand_pauses
            .insert("report_demand".to_string(), 12);
    }

    // Control branch: no pause.
    let mut control_scene = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let mut control_world = std::mem::take(&mut control_scene.world);

    // The fixed schedule is `every_ticks=4, start_tick=1` → spawns at
    // ticks 1, 5, 9, 13. The control should spawn at all four; the
    // paused branch should skip 5 and 9 (still inside `now < 12`).
    let _ = run_ticks(&mut paused_world, 16);
    let _ = run_ticks(&mut control_world, 16);

    let paused_seq = paused_world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .demand
        .get("report_demand")
        .map(|d| d.next_sequence)
        .expect("paused demand runtime present");
    let control_seq = control_world
        .sl1_runtime
        .as_ref()
        .unwrap()
        .demand
        .get("report_demand")
        .map(|d| d.next_sequence)
        .expect("control demand runtime present");

    assert!(
        paused_seq < control_seq,
        "paused branch must spawn fewer demands than control: paused={paused_seq} control={control_seq}",
    );
}

// -------------------------------------------------------------------
// Protocol round-trip
// -------------------------------------------------------------------

#[test]
fn static_payload_serializes_agents() {
    let scene = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let payload: StaticPayload = simetro_engine::encode_static(&scene);
    assert_eq!(payload.sl1_agents.len(), 2);
    let ids: Vec<&str> = payload.sl1_agents.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["mock_agent", "throttler"]);

    // Round-trip via JSON.
    let json = serde_json::to_string(&payload).expect("serialize");
    let de: StaticPayload = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de.sl1_agents.len(), 2);
    assert_eq!(de.sl1_agents[0].id, "mock_agent");
    assert_eq!(de.sl1_agents[0].kind, "mock");
    assert_eq!(de.sl1_agents[1].id, "throttler");
    assert_eq!(de.sl1_agents[1].kind, "builtin");
}

#[test]
fn static_payload_empty_when_no_agents() {
    // Scene without an agents block: sl1_agents should be empty AND
    // skip-serialized.
    let json = format!(
        r##"{{
            "schema_version": 1,
            "name": "no-agents",
            "theme": {{ "palette": ["#000000"], "background_index": 0, "font": "system-ui" }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places},
                "things": {things}
            }}
        }}"##,
        places = default_places(),
        things = default_things()
    );
    let scene = load_scene_str(&json, 0).expect("loads");
    let payload: StaticPayload = simetro_engine::encode_static(&scene);
    assert!(payload.sl1_agents.is_empty());

    let serialized = serde_json::to_string(&payload).expect("serialize");
    assert!(
        !serialized.contains("\"sl1_agents\""),
        "empty sl1_agents should be skipped, got: {serialized}"
    );
}

// -------------------------------------------------------------------
// Determinism baseline
// -------------------------------------------------------------------

#[test]
fn fixture_hash_matches_baseline() {
    let mut scene = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let hash = hash_run(&mut world, &mut runner, TICKS);
    let expected = AGENTS_BASELINE.trim();
    if expected == "0000000000000000000000000000000000000000000000000000000000000000" {
        panic!(
            "sl1-agents hash baseline not yet captured; write this to \
             tests/baselines/sl1-agents.hash:\n{hash}"
        );
    }
    assert_eq!(
        hash, expected,
        "sl1-agents hash drifted; if intentional, update baseline"
    );
}

#[test]
fn fixture_hash_is_stable_across_two_runs() {
    let mut scene1 = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let mut world1 = std::mem::take(&mut scene1.world);
    let mut runner1 = TickRunner::new();
    let h1 = hash_run(&mut world1, &mut runner1, TICKS);

    let mut scene2 = load_scene_str(AGENTS_SCENE, 0).expect("fixture loads");
    let mut world2 = std::mem::take(&mut scene2.world);
    let mut runner2 = TickRunner::new();
    let h2 = hash_run(&mut world2, &mut runner2, TICKS);

    assert_eq!(h1, h2);
}

// Silence unused-import warnings if the type is not referenced from
// any active test arm but is part of the public surface we exercise.
#[allow(dead_code)]
fn _proof_types_exported() {
    let _ = std::mem::size_of::<Sl1AgentRuntimeState>();
}
