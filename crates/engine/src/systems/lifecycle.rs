//! Lifecycle system: ensures every mover that hasn't started yet is
//! spawned at the source node of its home path.
//!
//! Runs once on the first tick after `Loaded → Running` and is idempotent
//! on subsequent ticks (a Mover that's already past Empty is left alone).

use simetro_protocol::SimEvent;

use crate::components::{MoverId, MoverState, NodeId, PathId};
use crate::world::World;

/// Scratch entry: `(mover, path, from_node)` ids.
pub type SpawnScratch = (MoverId, PathId, NodeId);

/// Spawn every `Empty` mover at the `from` node of its `home_path`,
/// then begin travel along that path. Emits one `MoverDeparted` per
/// spawned mover.
///
/// `scratch` is reused across ticks to avoid per-tick allocations
/// (zero-allocation target). Callers should own one buffer and
/// pass it back every tick.
pub fn run(world: &mut World, events: &mut Vec<SimEvent>, scratch: &mut Vec<SpawnScratch>) {
    scratch.clear();
    for (mid, mover) in &world.movers {
        if matches!(mover.state(), MoverState::Empty) {
            if let Some(path) = world.paths.get(&mover.home_path) {
                scratch.push((*mid, path.id, path.from));
            }
        }
    }
    for &(mid, path_id, from_node) in scratch.iter() {
        if let Some(mover) = world.movers.get_mut(&mid) {
            if mover.spawn_at(from_node).is_ok() && mover.begin_travel(path_id).is_ok() {
                events.push(SimEvent::MoverDeparted {
                    mover: mid.0,
                    from_node: from_node.0,
                    path: path_id.0,
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

    const SCENE: &str = include_str!("../../../../games/demo-paths.json");

    #[test]
    fn spawns_all_empty_movers() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let mut events = Vec::new();
        let mut scratch = Vec::new();
        run(&mut loaded.world, &mut events, &mut scratch);

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
        let mut scratch = Vec::new();
        run(&mut loaded.world, &mut events, &mut scratch);
        events.clear();
        run(&mut loaded.world, &mut events, &mut scratch);
        assert!(events.is_empty(), "second run should spawn nothing");
    }

    #[test]
    fn handles_missing_home_path_gracefully() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        if let Some(m) = loaded.world.movers.get_mut(&MoverId(0)) {
            m.home_path = PathId(9999);
        }
        let mut events = Vec::new();
        let mut scratch = Vec::new();
        run(&mut loaded.world, &mut events, &mut scratch);
        // The other two movers should still spawn.
        assert_eq!(events.len(), 2);
        // The corrupted one stays Empty.
        let stuck = loaded.world.movers.get(&MoverId(0)).unwrap();
        assert!(matches!(stuck.state(), MoverState::Empty));
    }
}
