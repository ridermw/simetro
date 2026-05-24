//! Integration test: the canonical demo scene loads cleanly.
//!
//! Guards the contract between `games/demo-paths.json` and the engine
//! loader. If either drifts, this fails first.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{load_scene_str, Goal, RunState};

#[test]
fn demo_paths_scene_loads() {
    let json = include_str!("../../../games/demo-paths.json");
    let loaded = load_scene_str(json, 42).expect("demo-paths.json should load");

    assert_eq!(loaded.name, "demo-paths");
    assert_eq!(loaded.theme.palette.len(), 5);
    assert_eq!(loaded.world.nodes.len(), 3);
    assert_eq!(loaded.world.paths.len(), 3);
    assert_eq!(loaded.world.movers.len(), 3);
    assert_eq!(loaded.goals, vec![Goal::LoopForever]);
    assert_eq!(loaded.agents.len(), 1);
    assert_eq!(loaded.agents[0].kind, "speed_tuner");
    assert_eq!(loaded.world.state, RunState::Loaded);
}
