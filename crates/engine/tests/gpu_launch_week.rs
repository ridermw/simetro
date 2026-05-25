//! PR 6 — GPU Launch Week scene v0.
//!
//! The dedicated scene under `games/gpu-launch-week.json` is the first
//! `scenario_language_v1` world to ship in the polished `games/`
//! catalog. PR 6 v0 only uses primitives shipped in PRs 0-5:
//! places, links, things, transforms, demand.
//!
//! Per the rubber-duck design review on this PR, this test does NOT
//! commit a state-hash baseline yet. The scene is explicitly expected
//! to grow pressure (PR 7), objectives (PR 8), observability (PR 9),
//! agents (PR 10), and milestones (PR 11) — baselining the hash now
//! would create maintenance noise rather than a useful determinism
//! gate. The hash baseline is locked in at PR 12 (scene polish).
//!
//! Instead, this test asserts the v0 contract:
//!
//! - the file loads cleanly via the SL1 loader,
//! - the deterministic single-place pipeline runs for 600 ticks with
//!   zero warnings (no starvation, no blocked transforms, no demand
//!   drops, no backlog overflow),
//! - the protocol static payload exposes the expected SL1 metadata
//!   counts (places, links, things, transforms, demand),
//! - the demand `exec-dashboard-refresh` actually fulfills on its
//!   scheduled cadence — proving the inventory wiring is real, not
//!   just declarative.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{encode_static, load_scene_str, TickRunner};
use simetro_protocol::SimMessage;

const SCENE: &str = include_str!("../../../games/gpu-launch-week.json");
const SEED: u64 = 42;
const TICK_BUDGET: u64 = 600;

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

    // The protocol static payload mirrors the SL1 metadata so the
    // frontend (and replay) can render topology without reaching into
    // engine internals.
    let static_payload = encode_static(&loaded);
    assert_eq!(static_payload.sl1_places.len(), 4);
    assert_eq!(static_payload.sl1_links.len(), 3);
    assert_eq!(static_payload.sl1_things.len(), 4);
    assert_eq!(static_payload.sl1_transforms.len(), 3);
    assert_eq!(static_payload.sl1_demand.len(), 1);
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
