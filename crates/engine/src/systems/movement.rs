//! Movement system: advances every `Traveling` mover by `world.dt` and
//! emits `MoverArrived` when `progress >= 1.0`.
//!
//! Arrived movers transition `Traveling → Waiting { at: path.to }`.
//! Routing them onto the next path is the interaction system's job,
//! so two-phase tick discipline keeps arrival distinct from departure.

use simetro_protocol::SimEvent;

use crate::components::{MoverId, MoverState, NodeId, PathId};
use crate::world::World;

/// Scratch entry: `(mover, path)` ids of arrivals this tick.
pub type ArrivalScratch = (MoverId, PathId);

/// Advance every `Traveling` mover. Returns the number of arrivals
/// this tick (handy for benches/asserts).
///
/// `scratch` is reused across ticks (zero-allocation target).
pub fn run(
    world: &mut World,
    events: &mut Vec<SimEvent>,
    scratch: &mut Vec<ArrivalScratch>,
) -> u32 {
    let dt = world.dt;
    scratch.clear();

    for (mid, mover) in world.movers.iter_mut() {
        if let MoverState::Traveling { path, .. } = mover.state() {
            if let Ok(progress) = mover.advance(dt) {
                if progress >= 1.0 {
                    scratch.push((*mid, path));
                }
            }
        }
    }

    let mut n = 0_u32;
    for &(mid, path_id) in scratch.iter() {
        let to_node: Option<NodeId> = world.paths.get(&path_id).map(|p| p.to);
        if let (Some(mover), Some(to)) = (world.movers.get_mut(&mid), to_node) {
            if mover.arrive(to).is_ok() {
                events.push(SimEvent::MoverArrived {
                    mover: mid.0,
                    at_node: to.0,
                    path: path_id.0,
                });
                n += 1;
            }
        }
    }
    n
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::components::MoverState;
    use crate::loader::load_scene_str;
    use crate::systems::lifecycle;

    const SCENE: &str = include_str!("../../../../games/demo-paths.json");

    #[test]
    fn advances_progress_each_tick() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 0.1;
        let mut events = Vec::new();
        let mut spawn_scratch = Vec::new();
        let mut arrival_scratch = Vec::new();
        lifecycle::run(&mut loaded.world, &mut events, &mut spawn_scratch);
        events.clear();

        run(&mut loaded.world, &mut events, &mut arrival_scratch);
        for m in loaded.world.movers.values() {
            match m.state() {
                MoverState::Traveling { progress, .. } => {
                    assert!(progress > 0.0 && progress < 1.0, "progress: {progress}");
                }
                other => panic!("expected Traveling, got {other:?}"),
            }
        }
        assert!(events.is_empty(), "no arrivals on partial advance");
    }

    #[test]
    fn emits_arrived_when_progress_saturates() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 100.0;
        let mut events = Vec::new();
        let mut spawn_scratch = Vec::new();
        let mut arrival_scratch = Vec::new();
        lifecycle::run(&mut loaded.world, &mut events, &mut spawn_scratch);
        events.clear();

        let arrivals = run(&mut loaded.world, &mut events, &mut arrival_scratch);
        assert_eq!(arrivals, 3);
        assert_eq!(events.len(), 3);
        for e in &events {
            assert!(matches!(e, SimEvent::MoverArrived { .. }));
        }
    }
}
