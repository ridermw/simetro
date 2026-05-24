//! Interaction system: routes movers that just arrived onto the next
//! outgoing path from their current node.
//!
//! Routing policy in P1 is "lowest PathId out of this node, deterministic".
//! That makes the demo scene's a→b→c→a cycle behave predictably. Real
//! routing (capacity-aware, agent-directed) lands in P2.

use simetro_protocol::SimEvent;

use crate::components::{MoverState, PathId};
use crate::world::World;

/// For every mover that is `Waiting`, pick the lowest-id outgoing path
/// from the current node and begin travel. Emits `MoverDeparted`.
pub fn run(world: &mut World, events: &mut Vec<SimEvent>) {
    let mut transitions: Vec<(crate::components::MoverId, PathId, u32, u32)> = Vec::new();

    for (mid, mover) in &world.movers {
        if let MoverState::Waiting { at } = mover.state() {
            // Find lowest-id path whose `from` is `at`.
            if let Some((pid, path)) = world
                .paths
                .iter()
                .find(|(_, p)| p.from == at)
                .map(|(pid, p)| (*pid, p.clone()))
            {
                transitions.push((*mid, pid, at.0, path.to.0));
            }
        }
    }

    for (mid, pid, from_node, _to_node) in transitions {
        if let Some(mover) = world.movers.get_mut(&mid) {
            if mover.begin_travel(pid).is_ok() {
                events.push(SimEvent::MoverDeparted {
                    mover: mid.0,
                    from_node,
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
    use crate::components::MoverId;
    use crate::loader::load_scene_str;
    use crate::systems::{lifecycle, movement};

    const SCENE: &str = include_str!("../../../../games/demo-paths.json");

    #[test]
    fn arrived_movers_get_rerouted() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 100.0; // saturate immediately
        let mut events = Vec::new();
        lifecycle::run(&mut loaded.world, &mut events);
        events.clear();
        movement::run(&mut loaded.world, &mut events); // arrive
        events.clear();
        run(&mut loaded.world, &mut events); // re-depart
        assert_eq!(events.len(), 3, "all three movers re-depart");
        for e in &events {
            assert!(matches!(e, SimEvent::MoverDeparted { .. }));
        }
        // All movers should be Traveling again.
        for m in loaded.world.movers.values() {
            assert!(matches!(
                m.state(),
                crate::components::MoverState::Traveling { .. }
            ));
        }
    }

    #[test]
    fn cycle_completes_in_expected_steps() {
        // Demo scene is a→b→c→a; each mover should return to its start
        // node after exactly 3 arrivals.
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 100.0; // each tick = full traversal
        let mut events = Vec::new();
        lifecycle::run(&mut loaded.world, &mut events);

        // After 3 arrivals each mover should be back at its starting node.
        // m1 starts at a (path ab from a). After ab→arrived@b,
        //   bc→arrived@c, ca→arrived@a. So mover 0 should be Waiting at a.
        for _ in 0..3 {
            events.clear();
            movement::run(&mut loaded.world, &mut events); // arrive
            run(&mut loaded.world, &mut events); // re-depart
        }
        // We've done 3 arrivals + 3 redepartures, so movers are now
        // Traveling on their 4th leg. To check position we step one more
        // arrival without redeparture.
        events.clear();
        movement::run(&mut loaded.world, &mut events);
        let m1 = loaded.world.movers.get(&MoverId(0)).unwrap();
        // After 4 arrivals, m1 has traversed ab, bc, ca, ab (returned to b).
        assert!(matches!(
            m1.state(),
            crate::components::MoverState::Waiting { .. }
        ));
    }
}
