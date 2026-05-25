//! Action application — turn an agent's chosen `Action` into world
//! mutations and `SimEvent`s, with typed validation and non-fatal
//! `WarningPayload::InvalidAction` rejections for malformed requests.

use simetro_protocol::{Action, HighlightReason, SimEvent, WarningPayload};

use crate::components::{
    Consumer, ConsumerId, MoverId, MoverState, Node, NodeId, NodeShape, Path, PathId, Producer,
    ProducerId, Resource, ResourceId,
};
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
const COORD_LIMIT: f32 = 1.0e6;
const MAX_PIECE_KIND_LEN: usize = 64;

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

        Action::PlacePiece { piece_kind, pos } => {
            place_piece(world, agent_id, piece_kind, *pos, events)
        }

        Action::ConnectPieces { from, to } => {
            connect_pieces(world, agent_id, NodeId(*from), NodeId(*to), events)
        }

        Action::RemovePiece { id } => remove_piece(world, agent_id, NodeId(*id)),

        // ---- Author tools  -----------------------------
        Action::DefineResource { name, color } => define_resource(world, agent_id, name, *color),
        Action::AddProducer {
            resource,
            amount,
            interval_ticks,
        } => add_producer(world, agent_id, resource, *amount, *interval_ticks),
        Action::AddConsumer {
            resource,
            amount,
            interval_ticks,
        } => add_consumer(world, agent_id, resource, *amount, *interval_ticks),
        Action::SetGoal { goal } => set_goal(world, agent_id, goal),
    }
}

fn place_piece(
    world: &mut World,
    agent_id: &str,
    piece_kind: &str,
    pos: [f32; 2],
    events: &mut Vec<SimEvent>,
) -> Outcome {
    if piece_kind.trim().is_empty() {
        return invalid(agent_id, "piece_kind is empty");
    }
    if piece_kind.chars().count() > MAX_PIECE_KIND_LEN {
        return invalid(
            agent_id,
            format!("piece_kind longer than {MAX_PIECE_KIND_LEN} chars"),
        );
    }
    let Some(shape) = node_shape_for_piece_kind(piece_kind) else {
        return invalid(
            agent_id,
            format!("unsupported piece_kind `{piece_kind}`; expected node or node shape"),
        );
    };
    if pos
        .iter()
        .any(|coord| !coord.is_finite() || coord.abs() > COORD_LIMIT)
    {
        return invalid(
            agent_id,
            format!(
                "position [{}, {}] is non-finite or outside ±{COORD_LIMIT}",
                pos[0], pos[1]
            ),
        );
    }

    let Some(id) = next_node_id(world) else {
        return invalid(agent_id, "node id space exhausted");
    };
    world.nodes.insert(
        id,
        Node {
            id,
            pos,
            shape,
            color: 0,
        },
    );
    events.push(SimEvent::NodeHighlighted {
        node: id.0,
        reason: HighlightReason::AgentFocus,
    });
    Outcome::Applied
}

fn connect_pieces(
    world: &mut World,
    agent_id: &str,
    from: NodeId,
    to: NodeId,
    events: &mut Vec<SimEvent>,
) -> Outcome {
    if from == to {
        return invalid(
            agent_id,
            format!("cannot connect node {} to itself", from.0),
        );
    }
    if !world.nodes.contains_key(&from) {
        return invalid(agent_id, format!("unknown from node {}", from.0));
    }
    if !world.nodes.contains_key(&to) {
        return invalid(agent_id, format!("unknown to node {}", to.0));
    }
    if world.paths.values().any(|p| p.from == from && p.to == to) {
        return invalid(
            agent_id,
            format!("path from node {} to node {} already exists", from.0, to.0),
        );
    }

    let Some(id) = next_path_id(world) else {
        return invalid(agent_id, "path id space exhausted");
    };
    world.paths.insert(
        id,
        Path {
            id,
            from,
            to,
            color: 0,
        },
    );
    events.push(SimEvent::PathPulsed { path: id.0 });
    Outcome::Applied
}

fn remove_piece(world: &mut World, agent_id: &str, id: NodeId) -> Outcome {
    if !world.nodes.contains_key(&id) {
        return invalid(agent_id, format!("unknown node {}", id.0));
    }

    let incident_paths: Vec<PathId> = world
        .paths
        .iter()
        .filter_map(|(pid, path)| {
            if path.from == id || path.to == id {
                Some(*pid)
            } else {
                None
            }
        })
        .collect();

    for mover in world.movers.values() {
        match mover.state() {
            MoverState::Waiting { at } if at == id => {
                return invalid(
                    agent_id,
                    format!("node {} has mover {} waiting at it", id.0, mover.id.0),
                );
            }
            MoverState::Traveling { path, .. } if incident_paths.contains(&path) => {
                return invalid(
                    agent_id,
                    format!(
                        "node {} is used by mover {} traveling on path {}",
                        id.0, mover.id.0, path.0
                    ),
                );
            }
            _ => {}
        }
        if incident_paths.contains(&mover.home_path) {
            return invalid(
                agent_id,
                format!(
                    "node {} is used by mover {} home path {}",
                    id.0, mover.id.0, mover.home_path.0
                ),
            );
        }
    }

    for pid in incident_paths {
        world.paths.remove(&pid);
    }
    world.nodes.remove(&id);
    Outcome::Applied
}

// =====================================================================
//  Author tools . All four mutate the world's resource /
//  production graph via the deterministic action-application pipeline.
//  Invalid requests surface as `Warning::InvalidAction`.
// =====================================================================

/// Max chars in a resource name. Matches the loader's id validator
/// (see `validate_id` in `loader.rs`).
const MAX_RESOURCE_NAME_LEN: usize = 64;

/// Bound on author-supplied amounts. Mirrors the loader's
/// `AMOUNT_MAX` so authoring and authoring-via-LLM share the same
/// envelope.
const AMOUNT_MAX: u64 = 1_000_000;

/// Bound on interval ticks. Mirrors `INTERVAL_MAX` in the loader.
const INTERVAL_MAX: u32 = 10_000;

fn validate_resource_name(agent_id: &str, name: &str) -> Result<(), Outcome> {
    if name.is_empty() {
        return Err(invalid(agent_id, "resource name is empty"));
    }
    if name.chars().count() > MAX_RESOURCE_NAME_LEN {
        return Err(invalid(
            agent_id,
            format!("resource name longer than {MAX_RESOURCE_NAME_LEN} chars"),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(invalid(
            agent_id,
            format!("resource name `{name}` must match [A-Za-z0-9_-]+"),
        ));
    }
    Ok(())
}

fn define_resource(world: &mut World, agent_id: &str, name: &str, color: u8) -> Outcome {
    if let Err(o) = validate_resource_name(agent_id, name) {
        return o;
    }
    if world.resources.values().any(|r| r.name == name) {
        return invalid(agent_id, format!("resource `{name}` already exists"));
    }
    let Some(id) = next_resource_id(world) else {
        return invalid(agent_id, "resource id space exhausted");
    };
    world.resources.insert(
        id,
        Resource {
            id,
            name: name.to_string(),
            color,
        },
    );
    world.inventory.insert(id, 0);
    Outcome::Applied
}

fn add_producer(
    world: &mut World,
    agent_id: &str,
    resource: &str,
    amount: u64,
    interval_ticks: u32,
) -> Outcome {
    let Some(rid) = world.resource_id_by_name(resource) else {
        return invalid(agent_id, format!("unknown resource `{resource}`"));
    };
    if let Err(o) = validate_amount_and_interval(agent_id, amount, interval_ticks) {
        return o;
    }
    let Some(id) = next_producer_id(world) else {
        return invalid(agent_id, "producer id space exhausted");
    };
    world.producers.insert(
        id,
        Producer {
            id,
            resource: rid,
            amount,
            interval_ticks,
        },
    );
    Outcome::Applied
}

fn add_consumer(
    world: &mut World,
    agent_id: &str,
    resource: &str,
    amount: u64,
    interval_ticks: u32,
) -> Outcome {
    let Some(rid) = world.resource_id_by_name(resource) else {
        return invalid(agent_id, format!("unknown resource `{resource}`"));
    };
    if let Err(o) = validate_amount_and_interval(agent_id, amount, interval_ticks) {
        return o;
    }
    let Some(id) = next_consumer_id(world) else {
        return invalid(agent_id, "consumer id space exhausted");
    };
    world.consumers.insert(
        id,
        Consumer {
            id,
            resource: rid,
            amount,
            interval_ticks,
        },
    );
    Outcome::Applied
}

/// Set the scene's win/end condition. Today only `"loop_forever"` is
/// recognized — any other string is rejected so a future spec
/// extension that adds new goal kinds can keep the wire format stable.
fn set_goal(world: &mut World, agent_id: &str, goal: &str) -> Outcome {
    let normalized: String = goal
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c == '-' { '_' } else { c })
        .collect();
    match normalized.as_str() {
        "loop_forever" => {
            // World doesn't carry a goal field today (LoadedScene
            // does, but it's loader-output not runtime-mutable).
            // Accepting this as NoChange keeps the contract stable so
            // a future PR that adds a runtime goal field works without
            // changing the wire shape. Suppress the unused-var warning.
            let _ = world;
            Outcome::NoChange
        }
        other => invalid(agent_id, format!("unknown goal `{other}`")),
    }
}

fn validate_amount_and_interval(
    agent_id: &str,
    amount: u64,
    interval_ticks: u32,
) -> Result<(), Outcome> {
    if amount == 0 || amount > AMOUNT_MAX {
        return Err(invalid(
            agent_id,
            format!("amount {amount} out of range (1..={AMOUNT_MAX})"),
        ));
    }
    if interval_ticks == 0 || interval_ticks > INTERVAL_MAX {
        return Err(invalid(
            agent_id,
            format!("interval_ticks {interval_ticks} out of range (1..={INTERVAL_MAX})"),
        ));
    }
    Ok(())
}

fn next_resource_id(world: &World) -> Option<ResourceId> {
    next_dense_u32(world.resources.keys().map(|r| r.0)).map(ResourceId)
}

fn next_producer_id(world: &World) -> Option<ProducerId> {
    next_dense_u32(world.producers.keys().map(|p| p.0)).map(ProducerId)
}

fn next_consumer_id(world: &World) -> Option<ConsumerId> {
    next_dense_u32(world.consumers.keys().map(|c| c.0)).map(ConsumerId)
}

fn next_dense_u32<I: Iterator<Item = u32>>(ids: I) -> Option<u32> {
    let mut candidate = 0_u32;
    for id in ids {
        if id == candidate {
            candidate = candidate.checked_add(1)?;
        } else if id > candidate {
            break;
        }
    }
    Some(candidate)
}

fn node_shape_for_piece_kind(piece_kind: &str) -> Option<NodeShape> {
    let normalized: String = piece_kind
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c == '-' || c == ':' { '_' } else { c })
        .collect();
    match normalized.as_str() {
        "node" | "circle" | "node_circle" => Some(NodeShape::Circle),
        "square" | "node_square" => Some(NodeShape::Square),
        "triangle" | "node_triangle" => Some(NodeShape::Triangle),
        "diamond" | "node_diamond" => Some(NodeShape::Diamond),
        "hexagon" | "node_hexagon" => Some(NodeShape::Hexagon),
        _ => None,
    }
}

fn next_node_id(world: &World) -> Option<NodeId> {
    let mut candidate = 0_u32;
    for id in world.nodes.keys() {
        if id.0 == candidate {
            candidate = candidate.checked_add(1)?;
        } else if id.0 > candidate {
            break;
        }
    }
    Some(NodeId(candidate))
}

fn next_path_id(world: &World) -> Option<PathId> {
    let mut candidate = 0_u32;
    for id in world.paths.keys() {
        if id.0 == candidate {
            candidate = candidate.checked_add(1)?;
        } else if id.0 > candidate {
            break;
        }
    }
    Some(PathId(candidate))
}

fn invalid(agent_id: &str, reason: impl Into<String>) -> Outcome {
    Outcome::Rejected(WarningPayload::InvalidAction {
        agent_id: agent_id.to_string(),
        reason: reason.into(),
    })
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

    fn insert_node(w: &mut World, id: u32, pos: [f32; 2]) {
        let id = NodeId(id);
        w.nodes.insert(
            id,
            Node {
                id,
                pos,
                shape: NodeShape::Circle,
                color: 0,
            },
        );
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
    fn place_piece_adds_node_with_deterministic_id() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::PlacePiece {
                piece_kind: "node_square".into(),
                pos: [10.0, 20.0],
            },
            &mut ev,
        );
        assert_eq!(out, Outcome::Applied);
        let node = w.nodes.get(&NodeId(0)).unwrap();
        assert_eq!(node.pos, [10.0, 20.0]);
        assert_eq!(node.shape, NodeShape::Square);
        assert_eq!(node.color, 0);
        assert_eq!(
            ev,
            vec![SimEvent::NodeHighlighted {
                node: 0,
                reason: HighlightReason::AgentFocus
            }]
        );
    }

    #[test]
    fn place_piece_rejects_bad_kind_and_position() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::PlacePiece {
                piece_kind: "mover".into(),
                pos: [0.0, 0.0],
            },
            &mut ev,
        );
        assert!(matches!(
            out,
            Outcome::Rejected(WarningPayload::InvalidAction { .. })
        ));

        let out = apply_action(
            &mut w,
            "a",
            &Action::PlacePiece {
                piece_kind: "node".into(),
                pos: [f32::NAN, 0.0],
            },
            &mut ev,
        );
        assert!(matches!(
            out,
            Outcome::Rejected(WarningPayload::InvalidAction { .. })
        ));
        assert!(w.nodes.is_empty());
        assert!(ev.is_empty());
    }

    #[test]
    fn connect_pieces_adds_path_with_deterministic_id() {
        let mut w = World::new(0);
        insert_node(&mut w, 0, [0.0, 0.0]);
        insert_node(&mut w, 1, [1.0, 0.0]);
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::ConnectPieces { from: 0, to: 1 },
            &mut ev,
        );
        assert_eq!(out, Outcome::Applied);
        let path = w.paths.get(&PathId(0)).unwrap();
        assert_eq!(path.from, NodeId(0));
        assert_eq!(path.to, NodeId(1));
        assert_eq!(path.color, 0);
        assert_eq!(ev, vec![SimEvent::PathPulsed { path: 0 }]);
    }

    #[test]
    fn connect_pieces_rejects_invalid_edges() {
        let mut w = World::new(0);
        insert_node(&mut w, 0, [0.0, 0.0]);
        insert_node(&mut w, 1, [1.0, 0.0]);
        let mut ev = Vec::new();

        assert!(matches!(
            apply_action(
                &mut w,
                "a",
                &Action::ConnectPieces { from: 0, to: 0 },
                &mut ev
            ),
            Outcome::Rejected(WarningPayload::InvalidAction { .. })
        ));
        assert!(matches!(
            apply_action(
                &mut w,
                "a",
                &Action::ConnectPieces { from: 0, to: 99 },
                &mut ev
            ),
            Outcome::Rejected(WarningPayload::InvalidAction { .. })
        ));

        assert_eq!(
            apply_action(
                &mut w,
                "a",
                &Action::ConnectPieces { from: 0, to: 1 },
                &mut ev
            ),
            Outcome::Applied
        );
        assert!(matches!(
            apply_action(
                &mut w,
                "a",
                &Action::ConnectPieces { from: 0, to: 1 },
                &mut ev
            ),
            Outcome::Rejected(WarningPayload::InvalidAction { .. })
        ));
    }

    #[test]
    fn remove_piece_removes_node_and_safe_incident_paths() {
        let mut w = World::new(0);
        insert_node(&mut w, 0, [0.0, 0.0]);
        insert_node(&mut w, 1, [1.0, 0.0]);
        insert_node(&mut w, 2, [2.0, 0.0]);
        w.paths.insert(
            PathId(0),
            Path {
                id: PathId(0),
                from: NodeId(0),
                to: NodeId(1),
                color: 0,
            },
        );
        w.paths.insert(
            PathId(1),
            Path {
                id: PathId(1),
                from: NodeId(2),
                to: NodeId(0),
                color: 0,
            },
        );
        let mut ev = Vec::new();
        let out = apply_action(&mut w, "a", &Action::RemovePiece { id: 0 }, &mut ev);
        assert_eq!(out, Outcome::Applied);
        assert!(!w.nodes.contains_key(&NodeId(0)));
        assert!(w.nodes.contains_key(&NodeId(1)));
        assert!(w.nodes.contains_key(&NodeId(2)));
        assert!(w.paths.is_empty());
        assert!(ev.is_empty());
    }

    #[test]
    fn remove_piece_rejects_nodes_used_by_movers() {
        let mut w = World::new(0);
        insert_node(&mut w, 0, [0.0, 0.0]);
        insert_node(&mut w, 1, [1.0, 0.0]);
        w.paths.insert(
            PathId(0),
            Path {
                id: PathId(0),
                from: NodeId(0),
                to: NodeId(1),
                color: 0,
            },
        );
        w.movers
            .insert(MoverId(0), Mover::new(MoverId(0), PathId(0), 1.0));
        let mut ev = Vec::new();
        let out = apply_action(&mut w, "a", &Action::RemovePiece { id: 0 }, &mut ev);
        match out {
            Outcome::Rejected(WarningPayload::InvalidAction { reason, .. }) => {
                assert!(reason.contains("home path"));
            }
            other => panic!("expected rejection, got {other:?}"),
        }
        assert!(w.nodes.contains_key(&NodeId(0)));
        assert!(w.paths.contains_key(&PathId(0)));
        assert!(ev.is_empty());
    }

    // ---- Author tools  ---------------------------------

    #[test]
    fn define_resource_adds_named_resource_and_zero_inventory() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::DefineResource {
                name: "ore".into(),
                color: 1,
            },
            &mut ev,
        );
        assert_eq!(out, Outcome::Applied);
        assert_eq!(w.resources.len(), 1);
        let rid = w.resource_id_by_name("ore").unwrap();
        assert_eq!(w.resources[&rid].name, "ore");
        assert_eq!(w.resources[&rid].color, 1);
        assert_eq!(w.inventory[&rid], 0);
    }

    #[test]
    fn define_resource_rejects_duplicate_name() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        apply_action(
            &mut w,
            "a",
            &Action::DefineResource {
                name: "ore".into(),
                color: 1,
            },
            &mut ev,
        );
        let out = apply_action(
            &mut w,
            "a",
            &Action::DefineResource {
                name: "ore".into(),
                color: 2,
            },
            &mut ev,
        );
        match out {
            Outcome::Rejected(WarningPayload::InvalidAction { reason, .. }) => {
                assert!(reason.contains("already exists"));
            }
            other => panic!("expected rejection, got {other:?}"),
        }
        assert_eq!(w.resources.len(), 1);
    }

    #[test]
    fn define_resource_rejects_empty_and_invalid_name() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        for bad in ["", "has space", "back\\slash"] {
            let out = apply_action(
                &mut w,
                "a",
                &Action::DefineResource {
                    name: bad.into(),
                    color: 1,
                },
                &mut ev,
            );
            assert!(matches!(
                out,
                Outcome::Rejected(WarningPayload::InvalidAction { .. })
            ));
        }
        assert!(w.resources.is_empty());
    }

    #[test]
    fn add_producer_attaches_to_existing_resource() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        apply_action(
            &mut w,
            "a",
            &Action::DefineResource {
                name: "ore".into(),
                color: 1,
            },
            &mut ev,
        );
        let out = apply_action(
            &mut w,
            "a",
            &Action::AddProducer {
                resource: "ore".into(),
                amount: 5,
                interval_ticks: 10,
            },
            &mut ev,
        );
        assert_eq!(out, Outcome::Applied);
        assert_eq!(w.producers.len(), 1);
        let p = w.producers.values().next().unwrap();
        assert_eq!(p.amount, 5);
        assert_eq!(p.interval_ticks, 10);
        assert_eq!(p.resource, w.resource_id_by_name("ore").unwrap());
    }

    #[test]
    fn add_producer_rejects_unknown_resource() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::AddProducer {
                resource: "ghost".into(),
                amount: 5,
                interval_ticks: 10,
            },
            &mut ev,
        );
        match out {
            Outcome::Rejected(WarningPayload::InvalidAction { reason, .. }) => {
                assert!(reason.contains("unknown resource"));
            }
            other => panic!("expected rejection, got {other:?}"),
        }
        assert!(w.producers.is_empty());
    }

    #[test]
    fn add_producer_rejects_oob_amount_and_interval() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        apply_action(
            &mut w,
            "a",
            &Action::DefineResource {
                name: "ore".into(),
                color: 1,
            },
            &mut ev,
        );
        for (amount, interval) in [(0_u64, 10_u32), (5, 0), (5, 20_000), (2_000_000, 10)] {
            let out = apply_action(
                &mut w,
                "a",
                &Action::AddProducer {
                    resource: "ore".into(),
                    amount,
                    interval_ticks: interval,
                },
                &mut ev,
            );
            assert!(
                matches!(out, Outcome::Rejected(_)),
                "expected rejection for amount={amount} interval={interval}"
            );
        }
        assert!(w.producers.is_empty());
    }

    #[test]
    fn add_consumer_attaches_to_existing_resource() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        apply_action(
            &mut w,
            "a",
            &Action::DefineResource {
                name: "widget".into(),
                color: 2,
            },
            &mut ev,
        );
        let out = apply_action(
            &mut w,
            "a",
            &Action::AddConsumer {
                resource: "widget".into(),
                amount: 3,
                interval_ticks: 60,
            },
            &mut ev,
        );
        assert_eq!(out, Outcome::Applied);
        assert_eq!(w.consumers.len(), 1);
        let c = w.consumers.values().next().unwrap();
        assert_eq!(c.amount, 3);
        assert_eq!(c.interval_ticks, 60);
        assert_eq!(c.resource, w.resource_id_by_name("widget").unwrap());
    }

    #[test]
    fn set_goal_loop_forever_is_nochange_other_rejected() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        let out = apply_action(
            &mut w,
            "a",
            &Action::SetGoal {
                goal: "loop_forever".into(),
            },
            &mut ev,
        );
        assert_eq!(out, Outcome::NoChange);
        // Hyphenated form normalizes too.
        let out = apply_action(
            &mut w,
            "a",
            &Action::SetGoal {
                goal: "loop-forever".into(),
            },
            &mut ev,
        );
        assert_eq!(out, Outcome::NoChange);
        // Unknown goals rejected.
        let out = apply_action(
            &mut w,
            "a",
            &Action::SetGoal {
                goal: "achieve_world_peace".into(),
            },
            &mut ev,
        );
        assert!(matches!(out, Outcome::Rejected(_)));
    }

    #[test]
    fn next_resource_id_is_dense() {
        let mut w = World::new(0);
        let mut ev = Vec::new();
        for name in ["a", "b", "c"] {
            apply_action(
                &mut w,
                "a",
                &Action::DefineResource {
                    name: name.into(),
                    color: 0,
                },
                &mut ev,
            );
        }
        let ids: Vec<u32> = w.resources.keys().map(|r| r.0).collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }
}
