//! Observation type. What an agent sees on a single tick.
//!
//! PLAN §8 requires that `observe()` is read-only — it produces a
//! flattened, allocation-light snapshot of just the bits an agent
//! needs to decide. We intentionally keep this minimal in P1; LLM
//! backends will need richer context (P2).

use crate::components::{MoverId, MoverState, PathId};
use crate::world::World;

/// Per-mover slice of state exposed to agents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoverObservation {
    pub id: MoverId,
    pub state: MoverState,
    pub speed: f32,
    pub home_path: PathId,
}

/// Read-only view of the world an agent decides against.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Observation {
    pub tick: u64,
    pub movers: Vec<MoverObservation>,
}

impl Observation {
    /// Snapshot the world. Iteration order is deterministic (BTreeMap).
    #[must_use]
    pub fn from_world(world: &World) -> Self {
        let movers = world
            .movers
            .values()
            .map(|m| MoverObservation {
                id: m.id,
                state: m.state(),
                speed: m.speed,
                home_path: m.home_path,
            })
            .collect();
        Self {
            tick: world.tick,
            movers,
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
    fn observation_captures_all_movers() {
        let loaded = load_scene_str(SCENE, 0).unwrap();
        let obs = Observation::from_world(&loaded.world);
        assert_eq!(obs.tick, 0);
        assert_eq!(obs.movers.len(), loaded.world.movers.len());
        assert!(obs.movers.iter().all(|m| m.speed > 0.0));
    }

    #[test]
    fn observation_iteration_is_deterministic() {
        let loaded_a = load_scene_str(SCENE, 0).unwrap();
        let loaded_b = load_scene_str(SCENE, 0).unwrap();
        let a = Observation::from_world(&loaded_a.world);
        let b = Observation::from_world(&loaded_b.world);
        assert_eq!(a, b);
    }
}
