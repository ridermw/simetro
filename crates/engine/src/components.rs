//! Component definitions for the simetro simulation.
//!
//! ```text
//!     Node ──┐               ┌── Mover (on_path)
//!            │               │
//!            └─▶ Path (from, to) ◀─┘
//! ```
//!
//! Renamed from `ecs.rs` per plan Issue 8A. Numeric IDs are owned by the
//! loader, which interns string IDs from JSON to `u32` at parse time
//! (PLAN §5.2). The engine never carries string IDs in hot paths.

use serde::{Deserialize, Serialize};

/// Stable numeric handle for a [`Node`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u32);

/// Stable numeric handle for a [`Path`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PathId(pub u32);

/// Stable numeric handle for a [`Mover`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MoverId(pub u32);

/// Stable numeric handle for a [`Resource`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u32);

/// Stable numeric handle for a [`Producer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProducerId(pub u32);

/// Stable numeric handle for a [`Consumer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConsumerId(pub u32);

/// Geometric primitive a [`Node`] is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeShape {
    Circle,
    Square,
    Triangle,
    Diamond,
    Hexagon,
}

/// A stationary point in the world. Movers traverse between nodes along paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub pos: [f32; 2],
    pub shape: NodeShape,
    /// Index into the active theme palette.
    pub color: u8,
}

/// A directed connection between two nodes. Movers travel along paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub id: PathId,
    pub from: NodeId,
    pub to: NodeId,
    pub color: u8,
}

/// Lifecycle state of a [`Mover`].
///
/// ```text
///     ┌─────────┐  spawn   ┌─────────┐  enter path  ┌──────────┐
///     │  Empty  │─────────▶│ Waiting │─────────────▶│ Traveling │
///     └─────────┘          └─────────┘              └──────────┘
///                               ▲                         │
///                               │  arrive at node         │
///                               └─────────────────────────┘
///
/// Invalid transitions are prevented by making the only mutators
/// the methods on `Mover` (`begin_travel`, `arrive`, `despawn`).
/// External code cannot construct an invalid transition because the
/// `state` field is private and the methods enforce the FSM.
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MoverState {
    /// Mover slot exists but is unallocated; not rendered.
    Empty,
    /// Mover is parked at a node, waiting to enter a path.
    Waiting { at: NodeId },
    /// Mover is partway along a path; `progress` is `0.0..=1.0`.
    Traveling { path: PathId, progress: f32 },
}

/// An agent (or simulated entity) that traverses paths between nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mover {
    pub id: MoverId,
    /// Path the mover prefers when leaving a node. Loader sets this from
    /// the JSON `on_path` field. Movement system reads this to choose
    /// the next path when `Waiting`.
    pub home_path: PathId,
    /// Speed in units of path-progress per second (1.0 means cross any
    /// path in one second).
    pub speed: f32,
    state: MoverState,
}

/// A globally tracked resource kind used by production chains.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    pub id: ResourceId,
    /// Palette index used by future inspection/rendering surfaces.
    pub color: u8,
}

/// Deterministic source that adds one resource kind to global inventory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Producer {
    pub id: ProducerId,
    pub resource: ResourceId,
    pub amount: u64,
    pub interval_ticks: u32,
}

/// Deterministic sink that removes one resource kind from global inventory
/// when enough stock is available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Consumer {
    pub id: ConsumerId,
    pub resource: ResourceId,
    pub amount: u64,
    pub interval_ticks: u32,
}

impl Mover {
    /// Construct a new mover in the `Empty` state. Use [`Mover::spawn_at`]
    /// to place it.
    #[must_use]
    pub fn new(id: MoverId, home_path: PathId, speed: f32) -> Self {
        Self {
            id,
            home_path,
            speed,
            state: MoverState::Empty,
        }
    }

    /// Read-only view of the current state.
    #[must_use]
    pub fn state(&self) -> MoverState {
        self.state
    }

    /// Transition `Empty → Waiting`. Any other source state is rejected.
    ///
    /// # Errors
    /// Returns the unchanged current state if the mover is not `Empty`.
    pub fn spawn_at(&mut self, node: NodeId) -> Result<(), MoverState> {
        match self.state {
            MoverState::Empty => {
                self.state = MoverState::Waiting { at: node };
                Ok(())
            }
            other => Err(other),
        }
    }

    /// Transition `Waiting → Traveling`. Any other source state is rejected.
    ///
    /// # Errors
    /// Returns the unchanged current state if the mover is not `Waiting`.
    pub fn begin_travel(&mut self, path: PathId) -> Result<(), MoverState> {
        match self.state {
            MoverState::Waiting { .. } => {
                self.state = MoverState::Traveling {
                    path,
                    progress: 0.0,
                };
                Ok(())
            }
            other => Err(other),
        }
    }

    /// Advance progress while in `Traveling`. Saturates at 1.0; caller
    /// detects arrival by checking `progress >= 1.0` and calling
    /// [`Mover::arrive`].
    ///
    /// # Errors
    /// Returns the unchanged current state if the mover is not `Traveling`.
    pub fn advance(&mut self, dt: f32) -> Result<f32, MoverState> {
        match self.state {
            MoverState::Traveling {
                path,
                ref mut progress,
            } => {
                *progress = (*progress + self.speed * dt).min(1.0);
                let p = *progress;
                // The `path` binding above is only there to make the destructure exhaustive.
                let _ = path;
                Ok(p)
            }
            other => Err(other),
        }
    }

    /// Transition `Traveling → Waiting { at: node }`. Any other source
    /// state is rejected.
    ///
    /// # Errors
    /// Returns the unchanged current state if the mover is not `Traveling`.
    pub fn arrive(&mut self, node: NodeId) -> Result<(), MoverState> {
        match self.state {
            MoverState::Traveling { .. } => {
                self.state = MoverState::Waiting { at: node };
                Ok(())
            }
            other => Err(other),
        }
    }

    /// Explicit termination. The only path to `Empty` after spawn.
    ///
    /// # Errors
    /// Returns the unchanged current state if the mover is already `Empty`.
    pub fn despawn(&mut self) -> Result<(), MoverState> {
        match self.state {
            MoverState::Empty => Err(MoverState::Empty),
            _ => {
                self.state = MoverState::Empty;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn mover() -> Mover {
        Mover::new(MoverId(1), PathId(0), 1.0)
    }

    #[test]
    fn new_mover_starts_empty() {
        let m = mover();
        assert_eq!(m.state(), MoverState::Empty);
    }

    #[test]
    fn spawn_transitions_empty_to_waiting() {
        let mut m = mover();
        m.spawn_at(NodeId(7)).unwrap();
        assert_eq!(m.state(), MoverState::Waiting { at: NodeId(7) });
    }

    #[test]
    fn spawn_rejects_when_not_empty() {
        let mut m = mover();
        m.spawn_at(NodeId(7)).unwrap();
        let err = m.spawn_at(NodeId(8)).unwrap_err();
        assert_eq!(err, MoverState::Waiting { at: NodeId(7) });
    }

    #[test]
    fn begin_travel_requires_waiting() {
        let mut m = mover();
        // Cannot begin from Empty.
        let err = m.begin_travel(PathId(3)).unwrap_err();
        assert_eq!(err, MoverState::Empty);
        // Spawn, then begin works.
        m.spawn_at(NodeId(0)).unwrap();
        m.begin_travel(PathId(3)).unwrap();
        assert_eq!(
            m.state(),
            MoverState::Traveling {
                path: PathId(3),
                progress: 0.0,
            }
        );
    }

    #[test]
    fn advance_progresses_and_saturates() {
        let mut m = Mover::new(MoverId(1), PathId(0), 2.0);
        m.spawn_at(NodeId(0)).unwrap();
        m.begin_travel(PathId(3)).unwrap();
        let p1 = m.advance(0.25).unwrap();
        assert!((p1 - 0.5).abs() < 1e-6, "expected 0.5, got {p1}");
        let p2 = m.advance(10.0).unwrap();
        assert!((p2 - 1.0).abs() < 1e-6, "should saturate at 1.0, got {p2}");
    }

    #[test]
    fn arrive_requires_traveling() {
        let mut m = mover();
        // Cannot arrive from Empty.
        assert_eq!(m.arrive(NodeId(0)).unwrap_err(), MoverState::Empty);
        m.spawn_at(NodeId(0)).unwrap();
        // Cannot arrive from Waiting.
        assert_eq!(
            m.arrive(NodeId(1)).unwrap_err(),
            MoverState::Waiting { at: NodeId(0) }
        );
        m.begin_travel(PathId(3)).unwrap();
        m.arrive(NodeId(1)).unwrap();
        assert_eq!(m.state(), MoverState::Waiting { at: NodeId(1) });
    }

    #[test]
    fn despawn_only_succeeds_from_non_empty() {
        let mut m = mover();
        assert!(m.despawn().is_err(), "despawn from Empty should fail");
        m.spawn_at(NodeId(0)).unwrap();
        m.despawn().unwrap();
        assert_eq!(m.state(), MoverState::Empty);
    }
}
