//! `World` is the owning container for simulation state.
//!
//! Collections are `BTreeMap` rather than `HashMap` so iteration order
//! is deterministic across runs and platforms (determinism contract).

use std::collections::BTreeMap;

use crate::components::{
    Consumer, ConsumerId, Mover, MoverId, Node, NodeId, Path, PathId, Producer, ProducerId,
    Resource, ResourceId,
};
use crate::rng::SimRng;

/// Top-level run state per run-state model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// World exists but has nothing loaded.
    Idle,
    /// JSON scene loaded; tick has not started.
    Loaded,
    /// Tick loop is active.
    Running,
    /// Tick loop is paused by user.
    Paused,
    /// An engine fault has paused the loop; export-session is recommended.
    Faulted,
}

#[derive(Debug)]
pub struct World {
    /// Monotonically increasing tick counter.
    pub tick: u64,
    /// Fixed simulation timestep in seconds (typically 1/60).
    pub dt: f32,
    /// Sim-owned RNG.
    pub rng: SimRng,
    /// Run state machine.
    pub state: RunState,
    /// Nodes by stable id.
    pub nodes: BTreeMap<NodeId, Node>,
    /// Paths by stable id.
    pub paths: BTreeMap<PathId, Path>,
    /// Movers by stable id.
    pub movers: BTreeMap<MoverId, Mover>,
    /// Resource kinds by stable id.
    pub resources: BTreeMap<ResourceId, Resource>,
    /// Global inventory by resource id.
    pub inventory: BTreeMap<ResourceId, u64>,
    /// Producers by stable id.
    pub producers: BTreeMap<ProducerId, Producer>,
    /// Consumers by stable id.
    pub consumers: BTreeMap<ConsumerId, Consumer>,
}

impl World {
    /// Create an empty world. `dt` defaults to 1/60s; override with
    /// [`World::with_dt`] for tests that want larger steps.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            tick: 0,
            dt: 1.0 / 60.0,
            rng: SimRng::from_seed(seed),
            state: RunState::Idle,
            nodes: BTreeMap::new(),
            paths: BTreeMap::new(),
            movers: BTreeMap::new(),
            resources: BTreeMap::new(),
            inventory: BTreeMap::new(),
            producers: BTreeMap::new(),
            consumers: BTreeMap::new(),
        }
    }

    /// Override the default fixed timestep.
    #[must_use]
    pub fn with_dt(mut self, dt: f32) -> Self {
        self.dt = dt;
        self
    }

    /// True if the world has at least one node loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.paths.is_empty()
            && self.movers.is_empty()
            && self.resources.is_empty()
            && self.inventory.is_empty()
            && self.producers.is_empty()
            && self.consumers.is_empty()
    }

    /// Find a resource by its stable string name. Linear in number of
    /// resources — fine for the typical few-dozen-resource scenes. Used
    /// by author-tool actions  so the LLM can reference
    /// resources by the same name the scene JSON uses.
    #[must_use]
    pub fn resource_id_by_name(&self, name: &str) -> Option<crate::components::ResourceId> {
        self.resources
            .values()
            .find(|r| r.name == name)
            .map(|r| r.id)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn new_world_is_empty_and_idle() {
        let w = World::new(42);
        assert_eq!(w.tick, 0);
        assert!(w.is_empty());
        assert_eq!(w.state, RunState::Idle);
        assert!((w.dt - 1.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn with_dt_overrides_default() {
        let w = World::new(0).with_dt(0.1);
        assert!((w.dt - 0.1).abs() < 1e-9);
    }
}
