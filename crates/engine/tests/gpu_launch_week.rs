//! GPU Launch Week scene integration test (PRs 6–12a).
//!
//! The dedicated scene under `games/gpu-launch-week.json` exercises
//! every `scenario_language_v1` primitive: places, links, things,
//! transforms, demand, pressure (PR 7), objectives, failure conditions,
//! victory conditions (PR 8), observability (PR 9), agents (PR 10),
//! and milestones (PR 11). PR 12a wires all of these together and
//! proves the scene reaches `GameOutcome::Won` deterministically.
//!
//! Scene shape (as of PR 12a):
//! - 4 places, 3 links, 4 things, 3 transforms, 1 demand
//! - 2 pressures: `gpu-fault-storm` (ticks 1500–2100),
//!   `dashboard-storm` (ticks 2400–2700)
//! - 3 objectives, 2 failure conditions, 1 victory condition
//!   (`survive_until` at tick 2800)
//! - observability: 3 metrics, 2 dashboards, 2 alerts
//! - 2 agents (mock observer + builtin demand-throttler)
//! - 6 milestones (4 pressure-lifecycle, 1 dashboard-state, 1
//!   metric-threshold)
//!
//! The deterministic hash baseline is captured at `BASELINE_TICKS=2800`
//! (the victory tick) so the hash is not coupled to post-terminal
//! behavior.
//!
//! This test suite asserts:
//! - the file loads cleanly and exposes all SL1 static-payload counts,
//! - the pipeline runs for 600 ticks with zero warnings,
//! - the demand `exec-dashboard-refresh` fulfills on cadence,
//! - `GameOutcome::Won` is reached exactly at tick 2800,
//! - the four pressure-lifecycle milestones fire and the two
//!   health-signal milestones do NOT fire on the winning path,
//! - tightening the `stale_target` FC drives the run to
//!   `GameOutcome::Lost`,
//! - the state hash is stable across two identical runs and matches
//!   the committed baseline.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{encode_static, hash_run, load_scene_str, GameOutcome, TickRunner};
use simetro_protocol::SimMessage;

const SCENE: &str = include_str!("../../../games/gpu-launch-week.json");
const BASELINE: &str = include_str!("../../../tests/baselines/gpu-launch-week.hash");
const SEED: u64 = 42;
const TICK_BUDGET: u64 = 600;
const WIN_TICK_BUDGET: u64 = 3000;
/// Tick budget used for the deterministic hash baseline. Captures the
/// full winning trajectory up to and including the `survive_until:
/// 2800` victory tick. Avoid running past terminal so the baseline
/// is not coupled to "what runs after Won" behavior.
const BASELINE_TICKS: u64 = 2800;

#[test]
fn scene_loads_and_exposes_sl1_static_metadata() {
    let loaded = load_scene_str(SCENE, SEED).expect("gpu-launch-week scene should load");

    let sl1 = loaded
        .world
        .sl1
        .as_ref()
        .expect("gpu-launch-week is an SL1 scene");

    assert_eq!(sl1.places.len(), 4, "gpu-launch-week declares four places");
    assert_eq!(sl1.links.len(), 3, "gpu-launch-week declares three links");
    assert_eq!(sl1.things.len(), 4, "gpu-launch-week declares four things");
    assert_eq!(
        sl1.transforms.len(),
        3,
        "gpu-launch-week declares three transforms"
    );
    assert_eq!(sl1.demand.len(), 1, "gpu-launch-week declares one demand");
    assert_eq!(
        sl1.pressure.len(),
        2,
        "gpu-launch-week declares two pressures (PR 7)"
    );
    assert_eq!(
        sl1.objectives.len(),
        3,
        "gpu-launch-week declares three objectives (PR 12a)"
    );
    assert_eq!(
        sl1.failure_conditions.len(),
        2,
        "gpu-launch-week declares two failure conditions (PR 12a)"
    );
    assert_eq!(
        sl1.victory_conditions.len(),
        1,
        "gpu-launch-week declares one victory condition (PR 12a)"
    );
    let observability = sl1
        .observability
        .as_ref()
        .expect("gpu-launch-week declares an observability block (PR 12a)");
    assert_eq!(observability.metrics.len(), 3);
    assert_eq!(observability.dashboards.len(), 2);
    assert_eq!(observability.alerts.len(), 2);
    assert_eq!(
        sl1.agents.len(),
        2,
        "gpu-launch-week declares two agents (PR 12a)"
    );
    assert_eq!(
        sl1.milestones.len(),
        6,
        "gpu-launch-week declares six milestones (PR 12a)"
    );

    // The protocol static payload mirrors the SL1 metadata so the
    // frontend (and replay) can render topology without reaching into
    // engine internals.
    let static_payload = encode_static(&loaded);
    assert_eq!(static_payload.sl1_places.len(), 4);
    assert_eq!(static_payload.sl1_links.len(), 3);
    assert_eq!(static_payload.sl1_things.len(), 4);
    assert_eq!(static_payload.sl1_transforms.len(), 3);
    assert_eq!(static_payload.sl1_demand.len(), 1);
    assert_eq!(static_payload.sl1_pressure.len(), 2);
    assert_eq!(static_payload.sl1_objectives.len(), 3);
    assert_eq!(static_payload.sl1_failure_conditions.len(), 2);
    assert_eq!(static_payload.sl1_victory_conditions.len(), 1);
    assert_eq!(static_payload.sl1_observability_metrics.len(), 3);
    assert_eq!(static_payload.sl1_observability_dashboards.len(), 2);
    assert_eq!(static_payload.sl1_observability_alerts.len(), 2);
    assert_eq!(static_payload.sl1_agents.len(), 2);
    assert_eq!(static_payload.sl1_milestones.len(), 6);
}

#[test]
fn scene_ticks_for_full_window_without_warnings() {
    let mut scene = load_scene_str(SCENE, SEED).expect("gpu-launch-week scene should load");
    let mut world = std::mem::take(&mut scene.world);

    let mut runner = TickRunner::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut faults: Vec<String> = Vec::new();

    for _ in 0..TICK_BUDGET {
        runner.tick_once(&mut world);
        for msg in runner.messages() {
            match msg {
                SimMessage::Warning(payload) => warnings.push(format!("{payload:?}")),
                SimMessage::Fault(payload) => faults.push(format!("{payload:?}")),
                _ => {}
            }
        }
    }

    assert!(
        warnings.is_empty(),
        "gpu-launch-week v0 should tick {TICK_BUDGET} ticks with zero warnings, got: {warnings:#?}",
    );
    assert!(
        faults.is_empty(),
        "gpu-launch-week v0 should tick {TICK_BUDGET} ticks with zero faults, got: {faults:#?}",
    );
}

#[test]
fn dashboard_demand_actually_fulfills() {
    let mut scene = load_scene_str(SCENE, SEED).expect("gpu-launch-week scene should load");
    let mut world = std::mem::take(&mut scene.world);

    // Snapshot the initial dashboard_result count so the assertion that
    // refresh-dashboard is actually producing inventory does not depend
    // on the literal pre-seed value living in the JSON.
    let initial_dashboard_result = world
        .sl1_runtime
        .as_ref()
        .expect("gpu-launch-week has an SL1 runtime")
        .inventories
        .get("gpu-platform")
        .and_then(|inv| inv.get("dashboard_result"))
        .copied()
        .unwrap_or(0);

    let mut runner = TickRunner::new();
    for _ in 0..TICK_BUDGET {
        runner.tick_once(&mut world);
    }

    let runtime = world
        .sl1_runtime
        .as_ref()
        .expect("gpu-launch-week has an SL1 runtime");
    let demand_state = runtime
        .demand
        .get("exec-dashboard-refresh")
        .expect("the executive dashboard demand should be present");

    // exec-dashboard-refresh fires on the schedule
    // `start_tick + n * every_ticks`. With start_tick=120 and
    // every_ticks=60 against the inclusive 1..=TICK_BUDGET window, the
    // expected spawn count is derived rather than hard-coded so a
    // change in TICK_BUDGET does not silently break the assertion.
    const START_TICK: u64 = 120;
    const EVERY_TICKS: u64 = 60;
    let expected_spawns = if TICK_BUDGET < START_TICK {
        0
    } else {
        (TICK_BUDGET - START_TICK) / EVERY_TICKS + 1
    };
    assert_eq!(
        demand_state.fulfilled_count, expected_spawns,
        "all {expected_spawns} scheduled executive dashboard demands should fulfill in {TICK_BUDGET} ticks",
    );
    assert_eq!(
        demand_state.dropped_count, 0,
        "no executive dashboard demand should be dropped",
    );

    // Per the design review, demand fulfillment only OBSERVES inventory
    // — it does not consume it. So the only way to prove the
    // refresh-dashboard transform is actually producing dashboard_result
    // (and not just coasting on pre-seeded initials) is to confirm the
    // dashboard_result inventory at gpu-platform has grown beyond its
    // pre-seed value.
    let final_dashboard_result = runtime
        .inventories
        .get("gpu-platform")
        .and_then(|inv| inv.get("dashboard_result"))
        .copied()
        .unwrap_or(0);
    assert!(
        final_dashboard_result > initial_dashboard_result,
        "refresh-dashboard should produce new dashboard_result inventory \
         (initial={initial_dashboard_result}, final={final_dashboard_result})",
    );
}

/// PR 12a: with the SL1 grammar fully exercised (objectives, failure
/// conditions, victory conditions, observability, agents, milestones),
/// the deterministic GPU Launch Week run reaches `GameOutcome::Won`
/// exactly at the declared `survive_until` tick of 2800. The agents
/// in this scene only have mock/observer or budget-gated builtin
/// behavior, and the [`BuiltinBackend`] currently returns `None` for
/// every decision (PR 10 stub) — so the win is produced by the
/// tuned baseline pipeline, not by any agent action. PR 13 will
/// re-tune this test once real builtin behavior ships.
#[test]
fn scene_reaches_won_outcome_at_victory_tick() {
    let mut scene = load_scene_str(SCENE, SEED).expect("gpu-launch-week scene should load");
    let mut world = std::mem::take(&mut scene.world);

    let mut runner = TickRunner::new();
    let mut faults: Vec<String> = Vec::new();
    let mut terminal_tick: Option<u64> = None;

    for tick in 1..=WIN_TICK_BUDGET {
        runner.tick_once(&mut world);
        for msg in runner.messages() {
            if let SimMessage::Fault(payload) = msg {
                faults.push(format!("{payload:?}"));
            }
        }
        if world.sl1_outcome().is_terminal() {
            terminal_tick = Some(tick);
            break;
        }
    }

    assert!(
        faults.is_empty(),
        "gpu-launch-week should not emit faults during a winning run, got: {faults:#?}",
    );
    assert_eq!(
        world.sl1_outcome(),
        GameOutcome::Won,
        "gpu-launch-week should reach GameOutcome::Won within {WIN_TICK_BUDGET} ticks",
    );
    assert_eq!(
        terminal_tick,
        Some(2800),
        "GameOutcome::Won should be reached exactly at the declared survive_until tick",
    );

    let runtime = world
        .sl1_runtime
        .as_ref()
        .expect("gpu-launch-week has an SL1 runtime");
    for (id, fc) in runtime.failure_conditions.iter() {
        assert!(
            fc.fired_at_tick.is_none(),
            "no failure condition should fire during the winning run, {id} fired at {:?}",
            fc.fired_at_tick,
        );
    }
    for (id, obj) in runtime.objectives.iter() {
        assert_ne!(
            obj.status,
            simetro_engine::Sl1ObjectiveStatus::Breached,
            "objective {id} should not be breached at end of winning run",
        );
    }
}

/// PR 12a: `GameOutcome::Lost` must be reachable for this scene's
/// failure machinery. The built-in agent backend is a no-op stub
/// today (it returns `None` for every decision), so removing the
/// agents does not on its own flip the run to Lost — the scene
/// is tuned to win without any agent assistance. To prove the
/// failure path still works on this scene shape, this test
/// programmatically tightens the `stale_target` failure condition
/// to a value the deterministic run cannot satisfy, then asserts
/// the run transitions to `GameOutcome::Lost`. Once a real
/// builtin agent ships (PR 13), this test should be replaced with
/// a "remove the agent → Lost" comparison.
#[test]
fn scene_can_transition_to_lost_when_failure_condition_tightened() {
    let scene_value: serde_json::Value = serde_json::from_str(SCENE).expect("scene parses as JSON");
    let mut scene_value = scene_value;
    let conds = scene_value
        .get_mut("scenario_language_v1")
        .and_then(|sl1| sl1.get_mut("failure_conditions"))
        .and_then(|c| c.as_array_mut())
        .expect("failure_conditions array");
    let mut mutated = false;
    for cond in conds.iter_mut() {
        if cond.get("id").and_then(|v| v.as_str()) == Some("executive-dashboard-stale") {
            cond["threshold_ticks"] = serde_json::json!(1u64);
            cond["grace_ticks"] = serde_json::json!(0u64);
            mutated = true;
        }
    }
    assert!(
        mutated,
        "test depends on the 'executive-dashboard-stale' failure condition; \
         rename the FC or update this test if you removed it"
    );
    let tightened = serde_json::to_string(&scene_value).expect("scene re-serializes");

    let mut scene = load_scene_str(&tightened, SEED).expect("tightened scene loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    for _ in 0..WIN_TICK_BUDGET {
        runner.tick_once(&mut world);
        if world.sl1_outcome().is_terminal() {
            break;
        }
    }
    match world.sl1_outcome() {
        GameOutcome::Lost { reason } => assert_eq!(
            reason, "failure_condition:executive-dashboard-stale",
            "tightened stale_target should be the firing failure condition",
        ),
        other => panic!(
            "tightened failure condition should drive gpu-launch-week to Lost, got {other:?}",
        ),
    }
}

/// PR 12a: the pressure-lifecycle milestones in this scene must fire
/// deterministically during the standard winning run.
/// Metric-threshold
/// and dashboard-state milestones (e.g.
/// `exec-dashboard-went-stale`, `platform-compute-saturated-detected`)
/// are declared so the scene can highlight degenerate runs, but they
/// are NOT expected to fire on the smooth winning path — that is the
/// signal that the tuned baseline pipeline stayed healthy (no real
/// builtin agent decisions exist yet; see PR 13).
#[test]
fn scene_fires_pressure_milestones_during_winning_run() {
    let mut scene = load_scene_str(SCENE, SEED).expect("gpu-launch-week scene should load");
    let mut world = std::mem::take(&mut scene.world);

    let mut runner = TickRunner::new();
    for _ in 0..WIN_TICK_BUDGET {
        runner.tick_once(&mut world);
        if world.sl1_outcome().is_terminal() {
            break;
        }
    }

    let runtime = world
        .sl1_runtime
        .as_ref()
        .expect("gpu-launch-week has an SL1 runtime");
    for id in [
        "gpu-storm-begins",
        "gpu-storm-clears",
        "dashboard-storm-begins",
        "dashboard-storm-clears",
    ] {
        let entry = runtime
            .milestones
            .get(id)
            .unwrap_or_else(|| panic!("milestone {id} should be present in runtime"));
        assert!(
            entry.fired_at_tick.is_some(),
            "pressure-lifecycle milestone {id} should fire during the winning run",
        );
    }
    // Health-signal milestones are declared for degenerate runs only; they
    // must NOT fire on the smooth winning path. If either fires it means the
    // tuned baseline pipeline is no longer healthy.
    for id in [
        "exec-dashboard-went-stale",
        "platform-compute-saturated-detected",
    ] {
        let entry = runtime
            .milestones
            .get(id)
            .unwrap_or_else(|| panic!("milestone {id} should be present in runtime"));
        assert!(
            entry.fired_at_tick.is_none(),
            "health-signal milestone {id} should NOT fire during the winning run; \
             the tuned baseline pipeline should stay healthy",
        );
    }
}

// -------------------------------------------------------------------
// Determinism baseline (PR 12a)
// -------------------------------------------------------------------

#[test]
fn scene_hash_matches_baseline() {
    let mut scene = load_scene_str(SCENE, SEED).expect("gpu-launch-week scene loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let hash = hash_run(&mut world, &mut runner, BASELINE_TICKS);
    let expected = BASELINE.trim();
    if expected == "0000000000000000000000000000000000000000000000000000000000000000" {
        panic!(
            "gpu-launch-week hash baseline not yet captured; write this to \
             tests/baselines/gpu-launch-week.hash:\n{hash}"
        );
    }
    assert_eq!(
        hash, expected,
        "gpu-launch-week hash drifted; if intentional, update baseline"
    );
}

#[test]
fn scene_hash_is_stable_across_two_runs() {
    let mut scene1 = load_scene_str(SCENE, SEED).expect("scene loads");
    let mut world1 = std::mem::take(&mut scene1.world);
    let mut runner1 = TickRunner::new();
    let hash1 = hash_run(&mut world1, &mut runner1, BASELINE_TICKS);

    let mut scene2 = load_scene_str(SCENE, SEED).expect("scene loads");
    let mut world2 = std::mem::take(&mut scene2.world);
    let mut runner2 = TickRunner::new();
    let hash2 = hash_run(&mut world2, &mut runner2, BASELINE_TICKS);

    assert_eq!(
        hash1, hash2,
        "deterministic hash should be stable across two identical runs"
    );
}
