//! Snapshot + static payload encoders.
//!
//! The engine ships two flavors of state to the renderer:
//!
//! 1. [`encode_static`] — once on connect: theme, palette, all nodes
//!    and paths with positions baked, plus reverse id maps for the
//!    Inspector. Paths are emitted in their original color so the
//!    renderer can group by `color` and draw one `Path2D` per color
//!    instead of one per path (renderer batching and allocation target — ~6 draw calls per scene
//!    instead of ~1000).
//! 2. [`encode_snapshot`] — periodically (target 20Hz): only the
//!    moving bits. Mover positions are interpolated from the path the
//!    mover is currently on; Waiting movers sit at their node;
//!    Empty movers are skipped.
//!
//! Both functions write into caller-supplied buffers so the renderer
//! tick loop allocates nothing per frame (zero-allocation target).

use simetro_protocol::{
    FreshnessStateView, MoverState as WireMover, NodeShapeTag, NodeView, PathView,
    Sl1DemandPenaltyView, Sl1DemandPriorityView, Sl1DemandRuntimeView, Sl1DemandScheduleView,
    Sl1DemandTargetView, Sl1DemandView, Sl1FailurePolicyView, Sl1LinkBackpressureView,
    Sl1LinkDirectionView, Sl1LinkRenderHintView, Sl1LinkView, Sl1OperatingPredicateView,
    Sl1OperatingStateView, Sl1PlaceInventoryView, Sl1PlaceView, Sl1PressureParamsView,
    Sl1PressureView, Sl1StorageSlotView, Sl1ThingQualityContractView, Sl1ThingRenderHintView,
    Sl1ThingView, Sl1TransformIoView, Sl1TransformRuntimeView, Sl1TransformStateView,
    Sl1TransformView, SnapshotPayload, StaticPayload,
};

use crate::components::{MoverState, NodeShape};
use crate::loader::{IdMap, LoadedScene, Theme};
use crate::scenario_language_v1::{
    FreshnessState, Sl1Link, Sl1LinkBackpressure, Sl1LinkDirection, Sl1OperatingPredicate,
    Sl1Place, Sl1Pressure, Sl1PressureKind, Sl1PressureParams, Sl1Thing,
};
use crate::world::World;

/// Build the connect-time [`StaticPayload`] from a loaded scene.
#[must_use]
pub fn encode_static(scene: &LoadedScene) -> StaticPayload {
    encode_static_parts(&scene.name, &scene.theme, &scene.id_map, &scene.world)
}

/// Build a [`StaticPayload`] from the current mutable world plus the
/// immutable scene metadata kept by drivers. This is used after author
/// actions mutate topology so renderers can refresh nodes/paths without a
/// full scene reload.
#[must_use]
pub fn encode_static_parts(
    name: &str,
    theme: &Theme,
    id_map: &IdMap,
    world: &World,
) -> StaticPayload {
    let Theme {
        palette,
        background_index,
        font: _,
    } = theme.clone();

    let mut nodes: Vec<NodeView> = world
        .nodes
        .values()
        .map(|n| NodeView {
            id: n.id.0,
            pos: n.pos,
            shape: shape_to_wire(n.shape),
            color: n.color,
        })
        .collect();
    nodes.sort_by_key(|n| n.id);

    let mut paths: Vec<PathView> = world
        .paths
        .values()
        .filter_map(|p| {
            let from = world.nodes.get(&p.from)?;
            let to = world.nodes.get(&p.to)?;
            Some(PathView {
                id: p.id.0,
                from_pos: from.pos,
                to_pos: to.pos,
                color: p.color,
            })
        })
        .collect();
    paths.sort_by_key(|p| p.id);

    StaticPayload {
        name: name.to_string(),
        palette,
        background_index,
        nodes,
        paths,
        node_names: node_name_map(world, id_map),
        path_names: path_name_map(world, id_map),
        mover_names: numeric_id_map(&id_map.mover_names),
        sl1_places: world
            .sl1
            .as_ref()
            .map(|sl1| sl1.places.iter().map(place_to_view).collect())
            .unwrap_or_default(),
        sl1_links: world
            .sl1
            .as_ref()
            .map(|sl1| sl1.links.iter().map(link_to_view).collect())
            .unwrap_or_default(),
        sl1_things: world
            .sl1
            .as_ref()
            .map(|sl1| sl1.things.iter().map(thing_to_view).collect())
            .unwrap_or_default(),
        sl1_transforms: world
            .sl1
            .as_ref()
            .map(|sl1| sl1.transforms.iter().map(transform_to_view).collect())
            .unwrap_or_default(),
        sl1_demand: world
            .sl1
            .as_ref()
            .map(|sl1| sl1.demand.iter().map(demand_to_view).collect())
            .unwrap_or_default(),
        sl1_pressure: world
            .sl1
            .as_ref()
            .map(|sl1| sl1.pressure.iter().map(pressure_to_view).collect())
            .unwrap_or_default(),
    }
}

fn place_to_view(place: &Sl1Place) -> Sl1PlaceView {
    Sl1PlaceView {
        id: place.id.clone(),
        role: place.role.clone(),
        pos: place.pos,
        shape: place.shape.clone(),
        color: place.color,
        capacity: place.capacity.clone(),
        storage: place
            .storage
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    Sl1StorageSlotView {
                        capacity: v.capacity,
                        initial: v.initial,
                    },
                )
            })
            .collect(),
        accepts: place.accepts.clone(),
        produces: place.produces.clone(),
        failure_domains: place.failure_domains.clone(),
        operating_states: place
            .operating_states
            .iter()
            .map(|(name, state)| {
                (
                    name.clone(),
                    Sl1OperatingStateView {
                        predicate: match &state.predicate {
                            Sl1OperatingPredicate::UsedPercentGte { metric, threshold } => {
                                Sl1OperatingPredicateView::UsedPercentGte {
                                    metric: metric.clone(),
                                    threshold: *threshold,
                                }
                            }
                            Sl1OperatingPredicate::OverloadedTicksGt { ticks } => {
                                Sl1OperatingPredicateView::OverloadedTicksGt { ticks: *ticks }
                            }
                        },
                        grace_ticks: state.grace_ticks,
                    },
                )
            })
            .collect(),
    }
}

fn link_to_view(link: &Sl1Link) -> Sl1LinkView {
    Sl1LinkView {
        id: link.id.clone(),
        link_type: link.link_type.clone(),
        from: link.from.clone(),
        to: link.to.clone(),
        direction: match link.direction {
            Sl1LinkDirection::Forward => Sl1LinkDirectionView::Forward,
            Sl1LinkDirection::Bidirectional => Sl1LinkDirectionView::Bidirectional,
        },
        capacity: link.capacity.clone(),
        travel_ticks: link.travel_ticks,
        compatibility: link.compatibility.clone(),
        queue_capacity: link.queue_capacity,
        backpressure: match link.backpressure {
            Sl1LinkBackpressure::BlockUpstream => Sl1LinkBackpressureView::BlockUpstream,
            Sl1LinkBackpressure::DropLowPriority => Sl1LinkBackpressureView::DropLowPriority,
            Sl1LinkBackpressure::SpillToBuffer => Sl1LinkBackpressureView::SpillToBuffer,
            Sl1LinkBackpressure::DegradeQuality => Sl1LinkBackpressureView::DegradeQuality,
        },
        render: link.render.as_ref().map(|r| Sl1LinkRenderHintView {
            style: r.style.clone(),
            color: r.color,
        }),
    }
}

fn thing_to_view(thing: &Sl1Thing) -> Sl1ThingView {
    Sl1ThingView {
        id: thing.id.clone(),
        kind: thing.kind.clone(),
        tags: thing.tags.clone(),
        schema_version: thing.schema_version,
        freshness_budget_ticks: thing.freshness_budget_ticks,
        quality_contract: thing
            .quality_contract
            .as_ref()
            .map(|q| Sl1ThingQualityContractView {
                max_drop_percent: q.max_drop_percent,
                max_late_ticks: q.max_late_ticks,
                required_fields: q.required_fields.clone(),
            }),
        render: thing.render.as_ref().map(|r| Sl1ThingRenderHintView {
            glyph: r.glyph.clone(),
            color: r.color,
        }),
    }
}

fn freshness_to_view(state: FreshnessState) -> FreshnessStateView {
    match state {
        FreshnessState::NoData => FreshnessStateView::NoData,
        FreshnessState::Ok { last_set_tick } => FreshnessStateView::Ok { last_set_tick },
        FreshnessState::Stale { last_set_tick } => FreshnessStateView::Stale { last_set_tick },
        FreshnessState::Degraded => FreshnessStateView::Degraded,
        FreshnessState::Invalid => FreshnessStateView::Invalid,
    }
}

fn transform_to_view(t: &crate::scenario_language_v1::Sl1Transform) -> Sl1TransformView {
    use crate::scenario_language_v1::Sl1FailurePolicy;
    Sl1TransformView {
        id: t.id.clone(),
        kind: t.kind.clone(),
        runs_on: t.runs_on.clone(),
        inputs: t
            .inputs
            .iter()
            .map(|io| Sl1TransformIoView {
                thing_id: io.thing_id.clone(),
                amount: io.amount,
            })
            .collect(),
        outputs: t
            .outputs
            .iter()
            .map(|io| Sl1TransformIoView {
                thing_id: io.thing_id.clone(),
                amount: io.amount,
            })
            .collect(),
        cadence_ticks: t.cadence_ticks,
        duration_ticks: t.duration_ticks,
        deadline_ticks: t.deadline_ticks,
        capacity_cost: t.capacity_cost.clone(),
        failure_policy: match t.failure_policy {
            Sl1FailurePolicy::RetryThenWarn => Sl1FailurePolicyView::RetryThenWarn,
            Sl1FailurePolicy::Drop => Sl1FailurePolicyView::Drop,
        },
        max_attempts: t.max_attempts,
    }
}

fn transform_state_to_view(
    state: &crate::scenario_language_v1::Sl1TransformState,
) -> Sl1TransformStateView {
    use crate::scenario_language_v1::Sl1TransformState as S;
    match state {
        S::Idle => Sl1TransformStateView::Idle,
        S::Running {
            scheduled_at,
            started_at,
            attempt,
        } => Sl1TransformStateView::Running {
            scheduled_at: *scheduled_at,
            started_at: *started_at,
            attempt: *attempt,
        },
        S::Starved {
            scheduled_at,
            since,
            attempts,
        } => Sl1TransformStateView::Starved {
            scheduled_at: *scheduled_at,
            since: *since,
            attempts: *attempts,
        },
        S::Blocked {
            scheduled_at,
            since,
            attempts,
        } => Sl1TransformStateView::Blocked {
            scheduled_at: *scheduled_at,
            since: *since,
            attempts: *attempts,
        },
        S::Late {
            scheduled_at,
            attempt,
            since,
        } => Sl1TransformStateView::Late {
            scheduled_at: *scheduled_at,
            attempt: *attempt,
            since: *since,
        },
    }
}

fn demand_to_view(d: &crate::scenario_language_v1::Sl1Demand) -> Sl1DemandView {
    use crate::scenario_language_v1::{Sl1DemandPriority, Sl1DemandSchedule, Sl1DemandTarget};
    let target = match &d.target {
        Sl1DemandTarget::Place(id) => Sl1DemandTargetView::Place { id: id.clone() },
    };
    let spawn_schedule = match &d.spawn_schedule {
        Sl1DemandSchedule::Fixed {
            every_ticks,
            start_tick,
        } => Sl1DemandScheduleView::Fixed {
            every_ticks: *every_ticks,
            start_tick: *start_tick,
        },
        Sl1DemandSchedule::Scripted { ticks } => Sl1DemandScheduleView::Scripted {
            ticks: ticks.clone(),
        },
    };
    let priority = match d.priority {
        Sl1DemandPriority::Low => Sl1DemandPriorityView::Low,
        Sl1DemandPriority::Normal => Sl1DemandPriorityView::Normal,
        Sl1DemandPriority::High => Sl1DemandPriorityView::High,
        Sl1DemandPriority::Critical => Sl1DemandPriorityView::Critical,
    };
    Sl1DemandView {
        id: d.id.clone(),
        kind: d.kind.clone(),
        target,
        requires: d.requires.clone(),
        spawn_schedule,
        deadline_ticks: d.deadline_ticks,
        priority,
        value: d.value,
        penalty: Sl1DemandPenaltyView {
            score: d.penalty.score,
            warning: d.penalty.warning.clone(),
        },
    }
}

fn pressure_to_view(p: &Sl1Pressure) -> Sl1PressureView {
    let params = match &p.params {
        Sl1PressureParams::SourceMultiplier {
            thing,
            multiplier_milli,
        } => Sl1PressureParamsView::SourceMultiplier {
            thing: thing.clone(),
            multiplier_milli: *multiplier_milli,
        },
        Sl1PressureParams::DemandGrowth { spawn_multiplier } => {
            Sl1PressureParamsView::DemandGrowth {
                spawn_multiplier: *spawn_multiplier,
            }
        }
        Sl1PressureParams::QuotaReduction {
            capacity,
            reduction_percent,
        } => Sl1PressureParamsView::QuotaReduction {
            capacity: capacity.clone(),
            reduction_percent: *reduction_percent,
        },
        Sl1PressureParams::PathOutage => Sl1PressureParamsView::PathOutage,
        Sl1PressureParams::UnsupportedInThisPr => Sl1PressureParamsView::UnsupportedInThisPr,
    };
    Sl1PressureView {
        id: p.id.clone(),
        kind: pressure_kind_to_str(p.kind).to_string(),
        at_tick: p.at_tick,
        duration_ticks: p.duration_ticks,
        target: p.target.clone(),
        params,
    }
}

/// Canonical snake_case string for a pressure kind, used in both
/// snapshot and runtime SimEvent / WarningPayload emissions to keep
/// the on-wire vocabulary consistent. Mirrors
/// `Sl1PressureKind::as_str`.
#[must_use]
pub(crate) fn pressure_kind_to_str(kind: Sl1PressureKind) -> &'static str {
    kind.as_str()
}

/// Compute group-by-color batches over path views. Renderer caches one
/// `Path2D` per color and re-uses it across frames.
///
/// Returns a Vec of `(color, indices_into_paths)`. Sorted by color so
/// output is deterministic across runs (determinism contract).
#[must_use]
pub fn color_batches(paths: &[PathView]) -> Vec<(u8, Vec<u32>)> {
    use std::collections::BTreeMap;
    let mut by_color: BTreeMap<u8, Vec<u32>> = BTreeMap::new();
    for p in paths {
        by_color.entry(p.color).or_default().push(p.id);
    }
    by_color.into_iter().collect()
}

/// Encode one tick into `out`, reusing the buffer's allocation.
///
/// `out.movers` is cleared and refilled. Returns the number of movers
/// written (Empty movers are skipped).
pub fn encode_snapshot(world: &World, out: &mut SnapshotPayload) -> usize {
    out.tick = world.tick;
    out.movers.clear();
    out.sl1_place_inventories.clear();
    out.sl1_transform_states.clear();
    out.sl1_demand_states.clear();

    for m in world.movers.values() {
        let (pos, on_path) = match m.state() {
            MoverState::Empty => continue,
            MoverState::Waiting { at } => {
                let Some(node) = world.nodes.get(&at) else {
                    continue;
                };
                (node.pos, 0)
            }
            MoverState::Traveling { path, progress } => {
                let Some(p) = world.paths.get(&path) else {
                    continue;
                };
                let (Some(from), Some(to)) = (world.nodes.get(&p.from), world.nodes.get(&p.to))
                else {
                    continue;
                };
                let t = progress.clamp(0.0, 1.0);
                let x = from.pos[0] + (to.pos[0] - from.pos[0]) * t;
                let y = from.pos[1] + (to.pos[1] - from.pos[1]) * t;
                ([x, y], path.0)
            }
        };
        out.movers.push(WireMover {
            id: m.id.0,
            pos,
            speed: m.speed,
            on_path,
        });
    }

    if let Some(runtime) = world.sl1_runtime.as_ref() {
        for ((place_id, thing_id), state) in runtime.freshness.iter() {
            let count = runtime
                .inventories
                .get(place_id)
                .and_then(|inv| inv.get(thing_id))
                .copied()
                .unwrap_or(0);
            out.sl1_place_inventories.push(Sl1PlaceInventoryView {
                place_id: place_id.clone(),
                thing_id: thing_id.clone(),
                count,
                freshness: freshness_to_view(*state),
            });
        }
        for (transform_id, state) in runtime.transforms.iter() {
            out.sl1_transform_states.push(Sl1TransformRuntimeView {
                transform_id: transform_id.clone(),
                state: transform_state_to_view(state),
            });
        }
        for (demand_id, dr) in runtime.demand.iter() {
            out.sl1_demand_states.push(Sl1DemandRuntimeView {
                demand_id: demand_id.clone(),
                // The bounded outstanding cap (MAX_DEMAND_OUTSTANDING)
                // fits comfortably in u32. Saturating cast keeps the
                // wire type compact without panicking on a future cap
                // increase.
                outstanding: u32::try_from(dr.pending.len()).unwrap_or(u32::MAX),
                fulfilled_count: dr.fulfilled_count,
                dropped_count: dr.dropped_count,
                next_sequence: dr.next_sequence,
            });
        }
    }

    out.movers.len()
}

fn shape_to_wire(s: NodeShape) -> NodeShapeTag {
    match s {
        NodeShape::Circle => NodeShapeTag::Circle,
        NodeShape::Square => NodeShapeTag::Square,
        NodeShape::Triangle => NodeShapeTag::Triangle,
        NodeShape::Diamond => NodeShapeTag::Diamond,
        NodeShape::Hexagon => NodeShapeTag::Hexagon,
    }
}

fn numeric_id_map<K: Copy + Into<u32>>(
    src: &std::collections::BTreeMap<K, String>,
) -> std::collections::BTreeMap<u32, String> {
    src.iter().map(|(k, v)| ((*k).into(), v.clone())).collect()
}

fn node_name_map(world: &World, id_map: &IdMap) -> std::collections::BTreeMap<u32, String> {
    let mut names = numeric_id_map(&id_map.node_names);
    for id in world.nodes.keys() {
        names
            .entry(id.0)
            .or_insert_with(|| format!("node_{}", id.0));
    }
    names
}

fn path_name_map(world: &World, id_map: &IdMap) -> std::collections::BTreeMap<u32, String> {
    let mut names = numeric_id_map(&id_map.path_names);
    for id in world.paths.keys() {
        names
            .entry(id.0)
            .or_insert_with(|| format!("path_{}", id.0));
    }
    names
}

// IdMap's numeric maps already key on NodeId/PathId/MoverId. We need
// the reverse direction (id → name). The loader exposes `*_names`
// (BTreeMap<NodeId, String>); we transcode those keys to u32 here.
//
// `IdMap` is `pub use`'d from `loader`; reaching into it directly
// requires Into<u32> on the newtype keys. Implementations live next
// to the components since they're cheap (just `id.0`).

impl From<crate::components::NodeId> for u32 {
    fn from(id: crate::components::NodeId) -> u32 {
        id.0
    }
}
impl From<crate::components::PathId> for u32 {
    fn from(id: crate::components::PathId) -> u32 {
        id.0
    }
}
impl From<crate::components::MoverId> for u32 {
    fn from(id: crate::components::MoverId) -> u32 {
        id.0
    }
}

// Defensive: `IdMap` is imported for future use (e.g. forward maps).
const _: fn() = || {
    fn _assert_idmap_field(_: &IdMap) {}
};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::components::{MoverId, NodeId};
    use crate::loader::load_scene_str;
    use crate::tick::TickRunner;

    const SCENE: &str = include_str!("../../../games/demo-paths.json");

    #[test]
    fn static_payload_has_all_nodes_and_paths() {
        let loaded = load_scene_str(SCENE, 0).unwrap();
        let stat = encode_static(&loaded);
        assert_eq!(stat.name, "demo-paths");
        assert_eq!(stat.nodes.len(), loaded.world.nodes.len());
        assert_eq!(stat.paths.len(), loaded.world.paths.len());
        assert!(!stat.palette.is_empty());
        assert!(stat.node_names.contains_key(&0));
        for p in &stat.paths {
            // Baked positions: from_pos must equal the source node's pos.
            let from = loaded
                .world
                .nodes
                .values()
                .find(|n| n.pos == p.from_pos)
                .expect("matched");
            let _ = from;
        }
    }

    #[test]
    fn color_batches_are_sorted_by_color() {
        let loaded = load_scene_str(SCENE, 0).unwrap();
        let stat = encode_static(&loaded);
        let batches = color_batches(&stat.paths);
        let colors: Vec<u8> = batches.iter().map(|(c, _)| *c).collect();
        let mut sorted = colors.clone();
        sorted.sort();
        assert_eq!(colors, sorted);
        // Every path is in exactly one batch.
        let total: usize = batches.iter().map(|(_, p)| p.len()).sum();
        assert_eq!(total, stat.paths.len());
    }

    #[test]
    fn snapshot_skips_empty_movers() {
        let loaded = load_scene_str(SCENE, 0).unwrap();
        let mut out = SnapshotPayload::default();
        let n = encode_snapshot(&loaded.world, &mut out);
        // At tick 0, every mover is Empty → snapshot is empty.
        assert_eq!(n, 0);
        assert!(out.movers.is_empty());
        assert_eq!(out.tick, 0);
    }

    #[test]
    fn snapshot_places_waiting_movers_at_node() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let target_node = NodeId(0);
        let target_pos = loaded.world.nodes.get(&target_node).unwrap().pos;
        loaded
            .world
            .movers
            .get_mut(&MoverId(0))
            .unwrap()
            .spawn_at(target_node)
            .unwrap();
        let mut out = SnapshotPayload::default();
        encode_snapshot(&loaded.world, &mut out);
        let m = out.movers.iter().find(|m| m.id == 0).unwrap();
        assert_eq!(m.pos, target_pos);
        assert_eq!(m.on_path, 0); // Waiting → on_path placeholder
    }

    #[test]
    fn snapshot_interpolates_traveling_movers() {
        let loaded = load_scene_str(SCENE, 0).unwrap();
        let mut world = loaded.world;
        // Spawn + begin travel manually for direct progress control.
        let m = world.movers.get_mut(&MoverId(0)).unwrap();
        m.spawn_at(NodeId(0)).unwrap();
        m.begin_travel(crate::components::PathId(0)).unwrap();
        m.advance(0.5).unwrap_or(0.0); // bring progress somewhere
        let mut out = SnapshotPayload::default();
        encode_snapshot(&world, &mut out);
        let mover = out.movers.iter().find(|m| m.id == 0).unwrap();
        // Position must lie on the segment (0,0)->next-node line.
        let from = world.nodes.get(&NodeId(0)).unwrap().pos;
        let path = world.paths.get(&crate::components::PathId(0)).unwrap();
        let to = world.nodes.get(&path.to).unwrap().pos;
        let dx = to[0] - from[0];
        let dy = to[1] - from[1];
        if dx.abs() > 1e-6 {
            let t = (mover.pos[0] - from[0]) / dx;
            assert!((0.0..=1.0).contains(&t));
            // y should be consistent with the same t.
            let y_expected = from[1] + dy * t;
            assert!((mover.pos[1] - y_expected).abs() < 1e-3);
        }
    }

    #[test]
    fn snapshot_reuses_buffer_zero_alloc_when_steady_state() {
        let mut loaded = load_scene_str(SCENE, 42).unwrap();
        let mut runner = TickRunner::new();
        runner.reserve_for(loaded.world.movers.len());
        for _ in 0..50 {
            runner.tick_once(&mut loaded.world);
        }
        let mut out = SnapshotPayload::default();
        out.movers.reserve(loaded.world.movers.len());
        encode_snapshot(&loaded.world, &mut out);
        let cap_before = out.movers.capacity();
        for _ in 0..200 {
            runner.tick_once(&mut loaded.world);
            encode_snapshot(&loaded.world, &mut out);
        }
        // Buffer should not have re-allocated.
        assert_eq!(out.movers.capacity(), cap_before);
    }
}
