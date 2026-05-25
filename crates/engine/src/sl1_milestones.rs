// The loader guarantees `runtime.milestones` is keyed for every
// declared milestone at load time, and that every cross-reference in
// `Sl1MilestoneTrigger` (pressure / metric / dashboard) resolves to a
// declared scene element. If those invariants are violated, silent
// skips would hide bugs — `expect` makes the loader-enforced invariant
// audible. Same precedent as `sl1_observability.rs:8`.
#![allow(clippy::expect_used)]

//! SL1 milestones runtime (PR 11).
//!
//! Runs LAST in the per-tick pipeline (after pressure → freshness →
//! transforms → demand → objectives → observability → agents) per the
//! spec ordering `"objectives → observability → agents → milestones"`.
//! Each milestone is evaluated against the post-everything state for
//! the current tick so author-supplied triggers observe a stable
//! snapshot.
//!
//! Semantics:
//!
//! - Each milestone fires AT MOST ONCE per scene run. Once
//!   `Sl1MilestoneRuntime::fired_at_tick` is `Some(tick)`, the
//!   milestone is terminal and re-evaluation is skipped.
//! - Triggers are edge-evaluated against the current tick's state.
//!   Because milestones are one-shot, "edge" here collapses to "first
//!   tick the condition is satisfied". No explicit `prev_state` is
//!   tracked.
//! - Pressure triggers read `runtime.pressure.active` directly. An
//!   `activated` trigger fires when the named pressure id is present
//!   in `active`; a `deactivated` trigger fires the first tick after
//!   the pressure id leaves `active` AND was previously active during
//!   the run. We track this by remembering whether the pressure has
//!   ever been observed active on a prior tick.
//! - Metric triggers read `runtime.metric_states`. `Ok { value }`
//!   evaluates the predicate; `NoData` is never satisfying.
//! - Dashboard triggers read `runtime.dashboard_states` and match
//!   discriminator only (no payload comparison for `Stale {
//!   freshness_ticks }`).
//!
//! All emitted [`SimEvent::Sl1MilestoneFired`] events are pushed in
//! stable milestone id order so determinism baselines stay
//! reproducible.

use simetro_protocol::SimEvent;

use crate::scenario_language_v1::{Sl1Milestone, Sl1MilestoneTrigger, Sl1RuntimeState, Sl1Scene};

/// Run the per-tick milestone evaluator. The caller has already
/// gated on `!scene.milestones.is_empty()`.
pub fn run(scene: &Sl1Scene, runtime: &mut Sl1RuntimeState, now: u64, events: &mut Vec<SimEvent>) {
    // Iterate in stable id order. Because `scene.milestones` is sorted
    // by id at load time and `runtime.milestones` is a `BTreeMap`,
    // ordering of emitted events is deterministic.
    for milestone in &scene.milestones {
        let entry = runtime
            .milestones
            .get(&milestone.id)
            .copied()
            .expect("loader initialized runtime.milestones for every declared milestone");
        if entry.fired() {
            continue;
        }

        // For `pressure_deactivated`, arm once the named pressure is
        // observed active. The milestone cannot fire on the same tick
        // it arms — it fires the first tick AFTER arming when the
        // pressure is no longer active.
        if let Sl1MilestoneTrigger::PressureDeactivated { pressure } = &milestone.trigger {
            if !entry.armed && runtime.pressure.active.contains_key(pressure) {
                let entry_mut = runtime
                    .milestones
                    .get_mut(&milestone.id)
                    .expect("entry must exist");
                entry_mut.armed = true;
                continue;
            }
            if !entry.armed {
                continue;
            }
        }

        if !trigger_satisfied(milestone, runtime) {
            continue;
        }
        let entry_mut = runtime
            .milestones
            .get_mut(&milestone.id)
            .expect("entry must exist");
        entry_mut.fired_at_tick = Some(now);
        events.push(SimEvent::Sl1MilestoneFired {
            milestone_id: milestone.id.clone(),
            label: milestone.label.clone(),
            trigger_kind: trigger_kind_str(&milestone.trigger).to_string(),
            camera_focus: milestone.camera_focus.clone(),
            highlight: milestone.highlight.clone(),
            tick: now,
        });
    }
}

fn trigger_kind_str(t: &Sl1MilestoneTrigger) -> &'static str {
    match t {
        Sl1MilestoneTrigger::PressureActivated { .. } => "pressure_activated",
        Sl1MilestoneTrigger::PressureDeactivated { .. } => "pressure_deactivated",
        Sl1MilestoneTrigger::MetricThreshold { .. } => "metric_threshold",
        Sl1MilestoneTrigger::DashboardState { .. } => "dashboard_state",
    }
}

fn trigger_satisfied(milestone: &Sl1Milestone, runtime: &Sl1RuntimeState) -> bool {
    match &milestone.trigger {
        Sl1MilestoneTrigger::PressureActivated { pressure } => {
            runtime.pressure.active.contains_key(pressure)
        }
        Sl1MilestoneTrigger::PressureDeactivated { pressure } => {
            // Armed-state was verified by the caller. Fire when the
            // pressure is currently NOT active.
            !runtime.pressure.active.contains_key(pressure)
        }
        Sl1MilestoneTrigger::MetricThreshold {
            metric,
            predicate,
            value,
        } => match runtime.metric_states.get(metric) {
            Some(crate::scenario_language_v1::Sl1MetricState::Ok { value: observed }) => {
                // `value` (threshold) is `i64` from JSON; observed
                // metric value is `u64`. If observed > i64::MAX we
                // can't represent it as i64, so resolve the predicate
                // by sign: only `gte`/`gt` can be satisfied by a value
                // strictly greater than any representable i64.
                if *observed > i64::MAX as u64 {
                    return matches!(
                        predicate,
                        crate::scenario_language_v1::Sl1MilestonePredicate::Gte
                            | crate::scenario_language_v1::Sl1MilestonePredicate::Gt
                    );
                }
                predicate.evaluate(*observed as i64, *value)
            }
            // `NoData` or missing entry: never satisfying.
            _ => false,
        },
        Sl1MilestoneTrigger::DashboardState {
            dashboard,
            target_state,
        } => match runtime.dashboard_states.get(dashboard) {
            Some(state) => target_state.matches(*state),
            None => false,
        },
    }
}
