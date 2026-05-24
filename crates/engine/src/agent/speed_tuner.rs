//! `SpeedTuner` — the canonical P1 built-in agent.
//!
//! Every `interval_ticks` ticks it looks at the world, picks the
//! lowest-id Waiting mover, and proposes three options: speed up, hold,
//! or no-op. Always chooses speed-up with confidence 0.7 when a Waiting
//! mover exists, else NoOp. Deliberately simple — it exists to validate
//! the trait, Inspector wiring, and AgentLog, not to play the game.

use simetro_protocol::{Action, AgentReport, ConsideredAction};

use super::{Agent, Observation};
use crate::components::MoverState;
use crate::error::AgentError;
use crate::world::World;

/// Speed multiplier applied when SpeedTuner picks "speed up".
pub const SPEED_UP_FACTOR: f32 = 1.5;

/// Hard upper bound on commanded speed; mirrors the loader's range so
/// SpeedTuner can never push a mover past it.
pub const SPEED_CAP: f32 = 100.0;

#[derive(Debug, Clone)]
pub struct SpeedTuner {
    id: String,
    interval: u32,
}

impl SpeedTuner {
    #[must_use]
    pub fn new(interval_ticks: u32) -> Self {
        Self {
            id: "speed_tuner_0".to_string(),
            interval: interval_ticks.max(1),
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
}

impl Agent for SpeedTuner {
    fn id(&self) -> &str {
        &self.id
    }

    fn interval_ticks(&self) -> u32 {
        self.interval
    }

    fn observe(&mut self, world: &World) -> Observation {
        Observation::from_world(world)
    }

    fn act(&mut self, obs: &Observation) -> Result<AgentReport, AgentError> {
        let waiting = obs
            .movers
            .iter()
            .find(|m| matches!(m.state, MoverState::Waiting { .. }));

        let mut considered = Vec::with_capacity(3);
        let (chosen, rationale, confidence) = match waiting {
            Some(target) => {
                let current = target.speed;
                let proposed = (current * SPEED_UP_FACTOR).min(SPEED_CAP);
                let speed_up = Action::SetSpeed {
                    mover: target.id.0,
                    speed: proposed,
                };
                let hold = Action::SetSpeed {
                    mover: target.id.0,
                    speed: current,
                };
                considered.push(ConsideredAction {
                    action: speed_up.clone(),
                    confidence: 0.7,
                });
                considered.push(ConsideredAction {
                    action: hold,
                    confidence: 0.2,
                });
                considered.push(ConsideredAction {
                    action: Action::NoOp,
                    confidence: 0.1,
                });
                let rationale = format!(
                    "mover {} is waiting; nudging speed {:.2} → {:.2} to clear the queue",
                    target.id.0, current, proposed
                );
                (Some(speed_up), rationale, 0.7)
            }
            None => {
                considered.push(ConsideredAction {
                    action: Action::NoOp,
                    confidence: 0.5,
                });
                let rationale = "no mover is waiting; nothing to tune".to_string();
                (Some(Action::NoOp), rationale, 0.5)
            }
        };

        Ok(AgentReport {
            tick: obs.tick,
            agent_id: self.id.clone(),
            considered,
            chosen,
            rationale,
            confidence,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::agent::AgentHost;
    use crate::components::{Mover, MoverId, NodeId, PathId};
    use crate::loader::load_scene_str;

    const SCENE: &str = include_str!("../../../../games/demo-paths.json");

    #[test]
    fn no_waiting_means_no_op() {
        let mut tuner = SpeedTuner::new(30);
        let obs = Observation::default();
        let rep = tuner.act(&obs).unwrap();
        assert_eq!(rep.chosen, Some(Action::NoOp));
        assert!(rep.rationale.contains("nothing to tune"));
        assert_eq!(rep.considered.len(), 1);
    }

    #[test]
    fn waiting_mover_gets_speed_up() {
        let loaded = load_scene_str(SCENE, 0).unwrap();
        let mut world = loaded.world;
        if let Some(m) = world.movers.values_mut().next() {
            m.spawn_at(NodeId(0)).ok();
        }
        let obs = Observation::from_world(&world);
        let mut tuner = SpeedTuner::new(30);
        let rep = tuner.act(&obs).unwrap();
        match rep.chosen {
            Some(Action::SetSpeed { speed, .. }) => {
                assert!(speed > 0.0 && speed <= SPEED_CAP);
            }
            other => panic!("expected SetSpeed, got {other:?}"),
        }
        assert!(rep.confidence > 0.0 && rep.confidence <= 1.0);
        assert_eq!(rep.considered.len(), 3);
    }

    #[test]
    fn speed_up_respects_cap() {
        let mut tuner = SpeedTuner::new(1);
        let mut world = World::new(0);
        let mut m = Mover::new(MoverId(0), PathId(0), 80.0);
        m.spawn_at(NodeId(0)).ok();
        world.movers.insert(m.id, m);
        let obs = Observation::from_world(&world);
        let rep = tuner.act(&obs).unwrap();
        match rep.chosen.unwrap() {
            Action::SetSpeed { speed, .. } => {
                assert!(speed <= SPEED_CAP, "{speed} should be capped");
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn host_runs_speed_tuner_safely() {
        let loaded = load_scene_str(SCENE, 0).unwrap();
        let mut host = AgentHost::new(Box::new(SpeedTuner::new(30)));
        let report = host.step(&loaded.world).expect("agent ok");
        assert_eq!(report.agent_id, "speed_tuner_0");
    }

    #[test]
    fn interval_is_clamped_to_at_least_one() {
        let tuner = SpeedTuner::new(0);
        assert_eq!(tuner.interval_ticks(), 1);
    }
}
