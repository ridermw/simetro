//! Lifecycle system: ensures every mover that hasn't started yet is
//! spawned at the source node of its home path.
//!
//! Runs once on the first tick after `Loaded → Running` and is idempotent
//! on subsequent ticks (a Mover that's already past Empty is left alone).

use simetro_protocol::SimEvent;

use crate::components::MoverState;
use crate::world::World;

/// Spawn every `Empty` mover at the `from` node of its `home_path`,
/// then begin travel along that path. Emits one `MoverDeparted` per
/// spawned mover.
pub fn run(world: &mut World, events: &mut Vec<SimEvent>) {
    // Two-phase to avoid holding the mover borrow across path lookup.
    let mut to_spawn: Vec<(crate::components::MoverId, u32, u32, u32)> = Vec::new();
    for (mid, mover) in &world.movers {
        if matches!(mover.state(), MoverState::Empty) {
            if let Some(path) = world.paths.get(&mover.home_path) {
                to_spawn.push((*mid, path.id.0, path.from.0, path.to.0));
            }
        }
    }
    for (mid, path_id, from_id, _to_id) in to_spawn {
        if let Some(mover) = world.movers.get_mut(&mid) {
            let from_node = crate::components::NodeId(from_id);
            let path = crate::components::PathId(path_id);
            // Empty → Waiting → Traveling. Errors here are unreachable
            // because we just verified the state is Empty; if the FSM
            // disagrees, drop the spawn and let movement notice next tick.
            if mover.spawn_at(from_node).is_ok() && mover.begin_travel(path).is_ok() {
                events.push(SimEvent::MoverDeparted {
                    mover: mid.0,
                    from_node: from_id,
                    path: path_id,
                });
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::components::{MoverId, MoverState, NodeId, PathId};
    use crate::loader::load_scene_str;

    const SCENE: &str = include_str!("../../../../games/demo-paths.json");

    #[test]
    fn spawns_all_empty_movers() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let mut events = Vec::new();
        run(&mut loaded.world, &mut events);

        assert_eq!(events.len(), 3, "one MoverDeparted per mover");
        for m in loaded.world.movers.values() {
            assert!(
                matches!(m.state(), MoverState::Traveling { .. }),
                "mover {:?} not Traveling: {:?}",
                m.id,
                m.state()
            );
        }
    }

    #[test]
    fn idempotent_when_movers_already_moving() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let mut events = Vec::new();
        run(&mut loaded.world, &mut events);
        events.clear();
        run(&mut loaded.world, &mut events);
        assert!(events.is_empty(), "second run should spawn nothing");
    }

    #[test]
    fn handles_missing_home_path_gracefully() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        // Corrupt one mover's home_path to a non-existent id. The system
        // must not panic; it just skips the mover.
        if let Some(m) = loaded.world.movers.get_mut(&MoverId(0)) {
            m.home_path = PathId(9999);
        }
        let mut events = Vec::new();
        run(&mut loaded.world, &mut events);
        // The other two movers should still spawn.
        assert_eq!(events.len(), 2);
        // The corrupted one stays Empty.
        let stuck = loaded.world.movers.get(&MoverId(0)).unwrap();
        assert!(matches!(stuck.state(), MoverState::Empty));
        // Silence unused-warnings on imports for non-feature builds.
        let _ = NodeId(0);
    }
}
