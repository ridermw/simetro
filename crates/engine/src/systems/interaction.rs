//! Interaction system: routes movers that just arrived onto the next
//! outgoing path from their current node.
//!
//! Legacy routing policy is "lowest PathId out of this node,
//! deterministic". That keeps v1/v2 demo scenes predictable. Stakes-v1
//! game-language work will add capacity-aware, agent-directed policy on
//! top of the same deterministic tick discipline.

use simetro_protocol::SimEvent;

use crate::components::{MoverId, MoverState, NodeId, PathId};
use crate::world::World;

/// Scratch entry: `(mover, next_path, from_node)`.
pub type RouteScratch = (MoverId, PathId, NodeId);

/// For every mover that is `Waiting`, pick the lowest-id outgoing path
/// from the current node and begin travel. Emits `MoverDeparted`.
///
/// `scratch` is reused across ticks to preserve the steady-state
/// zero-allocation target.
pub fn run(world: &mut World, events: &mut Vec<SimEvent>, scratch: &mut Vec<RouteScratch>) {
    scratch.clear();

    for (mid, mover) in &world.movers {
        if let MoverState::Waiting { at } = mover.state() {
            if let Some((pid, _path)) = world
                .paths
                .iter()
                .find(|(_, p)| p.from == at)
                .map(|(pid, p)| (*pid, p))
            {
                scratch.push((*mid, pid, at));
            }
        }
    }

    for &(mid, pid, from_node) in scratch.iter() {
        if let Some(mover) = world.movers.get_mut(&mid) {
            if mover.begin_travel(pid).is_ok() {
                events.push(SimEvent::MoverDeparted {
                    mover: mid.0,
                    from_node: from_node.0,
                    path: pid.0,
                });
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::loader::load_scene_str;
    use crate::systems::{lifecycle, movement};

    const SCENE: &str = include_str!("../../../../games/demo-paths.json");

    #[test]
    fn arrived_movers_get_rerouted() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 100.0;
        let mut events = Vec::new();
        let mut spawn = Vec::new();
        let mut arrivals = Vec::new();
        let mut routes = Vec::new();
        lifecycle::run(&mut loaded.world, &mut events, &mut spawn);
        events.clear();
        movement::run(&mut loaded.world, &mut events, &mut arrivals);
        events.clear();
        run(&mut loaded.world, &mut events, &mut routes);
        assert_eq!(events.len(), 3);
        for e in &events {
            assert!(matches!(e, SimEvent::MoverDeparted { .. }));
        }
        for m in loaded.world.movers.values() {
            assert!(matches!(m.state(), MoverState::Traveling { .. }));
        }
    }

    #[test]
    fn cycle_completes_in_expected_steps() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 100.0;
        let mut events = Vec::new();
        let mut spawn = Vec::new();
        let mut arrivals = Vec::new();
        let mut routes = Vec::new();
        lifecycle::run(&mut loaded.world, &mut events, &mut spawn);

        for _ in 0..3 {
            events.clear();
            movement::run(&mut loaded.world, &mut events, &mut arrivals);
            run(&mut loaded.world, &mut events, &mut routes);
        }
        events.clear();
        movement::run(&mut loaded.world, &mut events, &mut arrivals);
        let m1 = loaded.world.movers.get(&MoverId(0)).unwrap();
        assert!(matches!(m1.state(), MoverState::Waiting { .. }));
    }
}
