//! Action application — turn an agent's chosen `Action` into world
//! mutations and `SimEvent`s, with typed rejection for unsupported
//! variants (PLAN §11.2 / §23 deferred items).
//!
//! P1 supports `Action::NoOp` and `Action::SetSpeed`. Author actions
//! (`PlacePiece`, `ConnectPieces`, `RemovePiece`) are protocol-only —
//! they fail with `Outcome::Rejected` carrying a `WarningPayload`.

use simetro_protocol::{Action, SimEvent, WarningPayload};

use crate::components::MoverId;
use crate::world::World;

#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Action applied; any side-effect events have been pushed.
    Applied,
    /// Action accepted but had no effect (e.g., NoOp or speed unchanged).
    NoChange,
    /// Action rejected; emit the carried `WarningPayload`.
    Rejected(WarningPayload),
}

/// Bound on commanded speed; mirrors loader and SpeedTuner.
pub const SPEED_MIN: f32 = 0.0;
pub const SPEED_MAX: f32 = 100.0;

/// Apply `action` from `agent_id` to `world`. Returns the outcome and
/// pushes any resulting [`SimEvent`]s onto `events`.
pub fn apply_action(
    world: &mut World,
    agent_id: &str,
    action: &Action,
    events: &mut Vec<SimEvent>,
) -> Outcome {
    match action {
        Action::NoOp => Outcome::NoChange,

        Action::SetSpeed { mover, speed } => {
            if !speed.is_finite() || *speed < SPEED_MIN || *speed > SPEED_MAX {
                return Outcome::Rejected(WarningPayload::InvalidAction {
                    agent_id: agent_id.to_string(),
                    reason: format!("speed {speed} out of range [{SPEED_MIN},{SPEED_MAX}]"),
                });
            }
            let Some(m) = world.movers.get_mut(&MoverId(*mover)) else {
                return Outcome::Rejected(WarningPayload::InvalidAction {
                    agent_id: agent_id.to_string(),
                    reason: format!("unknown mover {mover}"),
                });
            };
            let old = m.speed;
            if (old - *speed).abs() < f32::EPSILON {
                return Outcome::NoChange;
            }
            m.speed = *speed;
            events.push(SimEvent::MoverSpeedChange {
                mover: *mover,
                old,
                new: *speed,
            });
            Outcome::Applied
        }

        Action::PlacePiece { .. } | Action::ConnectPieces { .. } | Action::RemovePiece { .. } => {
            Outcome::Rejected(WarningPayload::InvalidAction {
                agent_id: agent_id.to_string(),
                reason: format!("{} not supported in P1", action.tag().as_str()),
            })
        }
    }
}

trait ActionTagStr {
    fn as_str(&self) -> &'static str;
}

impl ActionTagStr for simetro_protocol::ActionTag {
    fn as_str(&self) -> &'static str {
        use simetro_protocol::ActionTag::*;
        match self {
            NoOp => "no_op",
            SetSpeed => "set_speed",
            PlacePiece => "place_piece",
            ConnectPieces => "connect_pieces",
            RemovePiece => "remove_piece",
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::components::{Mover, PathId};
    use crate::world::World;

    fn world_with_one_mover() -> World {
        let mut w = World::new(0);
        w.movers
            .insert(MoverId(0), Mover::new(MoverId(0), PathId(0), 1.0));
        w
    }

    #[test]
    fn noop_makes_no_event() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        let out = apply_action(&mut w, "a", &Action::NoOp, &mut ev);
        assert_eq!(out, Outcome::NoChange);
        assert!(ev.is_empty());
    }

    #[test]
    fn set_speed_applies_and_emits_event() {
        let mut w = world_with_one_mover();
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::SetSpeed {
                mover: 0,
                speed: 2.5,
            },
            &mut ev,
        );
        assert_eq!(out, Outcome::Applied);
        assert_eq!(w.movers.get(&MoverId(0)).unwrap().speed, 2.5);
        assert_eq!(ev.len(), 1);
        match ev[0] {
            SimEvent::MoverSpeedChange { mover, old, new } => {
                assert_eq!(mover, 0);
                assert!((old - 1.0).abs() < 1e-6);
                assert!((new - 2.5).abs() < 1e-6);
            }
            ref other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn set_speed_same_value_is_no_change() {
        let mut w = world_with_one_mover();
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::SetSpeed {
                mover: 0,
                speed: 1.0,
            },
            &mut ev,
        );
        assert_eq!(out, Outcome::NoChange);
        assert!(ev.is_empty());
    }

    #[test]
    fn set_speed_unknown_mover_warns() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::SetSpeed {
                mover: 99,
                speed: 1.0,
            },
            &mut ev,
        );
        match out {
            Outcome::Rejected(WarningPayload::InvalidAction { reason, .. }) => {
                assert!(reason.contains("unknown mover"));
            }
            other => panic!("expected rejection, got {other:?}"),
        }
        assert!(ev.is_empty());
    }

    #[test]
    fn set_speed_out_of_range_warns() {
        let mut w = world_with_one_mover();
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::SetSpeed {
                mover: 0,
                speed: f32::NAN,
            },
            &mut ev,
        );
        assert!(matches!(out, Outcome::Rejected(_)));

        let out = apply_action(
            &mut w,
            "a",
            &Action::SetSpeed {
                mover: 0,
                speed: 1000.0,
            },
            &mut ev,
        );
        assert!(matches!(out, Outcome::Rejected(_)));
    }

    #[test]
    fn p1_rejects_author_actions() {
        let mut w = world_with_one_mover();
        let mut ev = Vec::new();
        for a in [
            Action::PlacePiece {
                piece_kind: "node".into(),
                pos: [0.0, 0.0],
            },
            Action::ConnectPieces { from: 0, to: 1 },
            Action::RemovePiece { id: 0 },
        ] {
            let out = apply_action(&mut w, "a", &a, &mut ev);
            match out {
                Outcome::Rejected(WarningPayload::InvalidAction { reason, .. }) => {
                    assert!(reason.contains("not supported in P1"));
                }
                other => panic!("expected rejection, got {other:?}"),
            }
        }
        assert!(ev.is_empty());
    }
}
