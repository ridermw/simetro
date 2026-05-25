// The loader guarantees `runtime.{metric_states,dashboard_states,
// alert_states}` is keyed for every declared id at load time, and that
// every cross-reference in `Sl1Observability` resolves to a declared
// scene element. If those invariants are violated, silent skips would
// hide bugs in the observability pipeline — `expect` makes the
// loader-enforced invariant audible. Same precedent as
// `sl1_objectives.rs:7`.
#![allow(clippy::expect_used)]

//! SL1 observability runtime (PR 9).
//!
//! Tick order inside [`crate::sl1_runtime::run`]: this runs LAST,
//! after pressure → freshness → transforms → demand → objectives. Per
//! the spec ordering "objectives → observability → agents →
//! milestones": each system observes everything that ran before it.
//!
//! Steps each tick (skip entirely if no `observability` block):
//!
//! 1. Recompute each declared dashboard's state into
//!    [`Sl1DashboardState`]. Compute first because
//!    `dashboard_freshness` metrics read it.
//! 2. Recompute each declared metric's value into
//!    [`Sl1MetricState`]. Stable id order.
//! 3. Update each declared alert. Edge-triggered: only emit
//!    [`SimEvent::Sl1AlertFired`] on `Inactive → Firing` and
//!    [`SimEvent::Sl1AlertCleared`] on `Firing → Inactive`. If the
//!    referenced metric is `NoData`, treat the predicate as "cannot
//!    fire" — a currently-firing alert is cleared.
//!
//! All emitted events are pushed in stable id order so determinism
//! baselines stay reproducible.

use simetro_protocol::SimEvent;

use crate::scenario_language_v1::{
    FreshnessState, Sl1AlertState, Sl1Dashboard, Sl1DashboardState, Sl1Metric, Sl1MetricSource,
    Sl1MetricState, Sl1RuntimeState, Sl1Scene,
};

/// Run the per-tick observability pipeline. No-op if the scene has no
/// `observability` block or the block declared zero metrics, zero
/// dashboards, and zero alerts.
pub fn run(scene: &Sl1Scene, runtime: &mut Sl1RuntimeState, now: u64, events: &mut Vec<SimEvent>) {
    let Some(obs) = scene.observability.as_ref() else {
        return;
    };
    if obs.metrics.is_empty() && obs.dashboards.is_empty() && obs.alerts.is_empty() {
        return;
    }

    // 1. Dashboards first — metrics may read dashboard freshness.
    for dashboard in &obs.dashboards {
        let new_state = compute_dashboard_state(dashboard, runtime, now);
        let prev_state = *runtime
            .dashboard_states
            .get(&dashboard.id)
            .expect("loader initialized dashboard_states for every declared dashboard");
        if discriminants_differ(prev_state, new_state) {
            let freshness_ticks = match new_state {
                Sl1DashboardState::Stale { freshness_ticks } => Some(freshness_ticks),
                _ => None,
            };
            events.push(SimEvent::Sl1DashboardStateChanged {
                dashboard_id: dashboard.id.clone(),
                from: prev_state.discriminant_str().to_string(),
                to: new_state.discriminant_str().to_string(),
                tick: now,
                freshness_ticks,
            });
        }
        runtime
            .dashboard_states
            .insert(dashboard.id.clone(), new_state);
    }

    // 2. Metrics. Read post-dashboard state for dashboard_freshness.
    for metric in &obs.metrics {
        let new_state = compute_metric_state(metric, scene, runtime, now);
        runtime.metric_states.insert(metric.id.clone(), new_state);
    }

    // 3. Alerts. Edge-triggered. Read post-metric state.
    for alert in &obs.alerts {
        let metric_state = *runtime
            .metric_states
            .get(&alert.metric)
            .expect("loader validated alert.metric references a declared metric");
        let prev_state = *runtime
            .alert_states
            .get(&alert.id)
            .expect("loader initialized alert_states for every declared alert");

        let (new_state, emitted_value) = match metric_state {
            Sl1MetricState::Ok { value } => {
                if alert.predicate.fires(value) {
                    let fired_at_tick = match prev_state {
                        Sl1AlertState::Firing { fired_at_tick } => fired_at_tick,
                        Sl1AlertState::Inactive => now,
                    };
                    (Sl1AlertState::Firing { fired_at_tick }, Some(value))
                } else {
                    (Sl1AlertState::Inactive, None)
                }
            }
            // NoData: predicate cannot fire. If currently firing, clear.
            Sl1MetricState::NoData => (Sl1AlertState::Inactive, None),
        };

        match (prev_state, new_state) {
            (Sl1AlertState::Inactive, Sl1AlertState::Firing { .. }) => {
                events.push(SimEvent::Sl1AlertFired {
                    alert_id: alert.id.clone(),
                    metric_id: alert.metric.clone(),
                    value: emitted_value
                        .expect("emitted_value is always Some when transitioning Inactive→Firing"),
                    severity: alert.severity.as_str().to_string(),
                    predicate: alert.predicate.summary(),
                    tick: now,
                });
            }
            (Sl1AlertState::Firing { .. }, Sl1AlertState::Inactive) => {
                events.push(SimEvent::Sl1AlertCleared {
                    alert_id: alert.id.clone(),
                    metric_id: alert.metric.clone(),
                    tick: now,
                });
            }
            _ => {}
        }
        runtime.alert_states.insert(alert.id.clone(), new_state);
    }
}

/// Compute the post-aging freshness age for one dashboard, in ticks.
///
/// Returns `None` when at least one `depends_on` thing has no `Ok`
/// freshness entry anywhere in the world (no place has ever produced
/// or received the thing). Otherwise returns the maximum age across
/// every `(place, thing)` pair for the dashboard's `depends_on` set —
/// this is "how stale is the freshest copy of the latest depended-on
/// thing?"
///
/// Public so [`crate::snapshot`] can re-derive the freshness chip for
/// `Sl1DashboardState::Ok` (the state carries no age, but the HUD
/// wants a numeric chip).
#[must_use]
pub fn dashboard_freshness(
    dashboard: &Sl1Dashboard,
    runtime: &Sl1RuntimeState,
    now: u64,
) -> Option<u64> {
    if dashboard.depends_on.is_empty() {
        return Some(0);
    }
    let mut max_age: u64 = 0;
    for thing_id in &dashboard.depends_on {
        // For this thing: find the freshest (= minimum age) Ok entry
        // across all places. If no place has Ok for this thing → NoData.
        let mut min_age_for_thing: Option<u64> = None;
        for ((_place_id, t), state) in runtime.freshness.iter() {
            if t != thing_id {
                continue;
            }
            if let FreshnessState::Ok { last_set_tick } = *state {
                let age = now.saturating_sub(last_set_tick);
                min_age_for_thing = Some(match min_age_for_thing {
                    Some(prev) => prev.min(age),
                    None => age,
                });
            }
        }
        match min_age_for_thing {
            None => return None,
            Some(a) => max_age = max_age.max(a),
        }
    }
    Some(max_age)
}

fn compute_dashboard_state(
    dashboard: &Sl1Dashboard,
    runtime: &Sl1RuntimeState,
    now: u64,
) -> Sl1DashboardState {
    match dashboard_freshness(dashboard, runtime, now) {
        None => Sl1DashboardState::NoData,
        Some(age) if age > dashboard.freshness_slo_ticks => Sl1DashboardState::Stale {
            freshness_ticks: age,
        },
        Some(_) => Sl1DashboardState::Ok,
    }
}

fn compute_metric_state(
    metric: &Sl1Metric,
    scene: &Sl1Scene,
    runtime: &Sl1RuntimeState,
    now: u64,
) -> Sl1MetricState {
    match &metric.source {
        Sl1MetricSource::PlaceCapacityUsedPercent { place, capacity } => {
            let place_def = scene
                .places
                .iter()
                .find(|p| p.id == *place)
                .expect("loader validated metric place reference");
            let cap = place_def.capacity.get(capacity).copied().unwrap_or(0);
            let used = runtime
                .place_capacity_used
                .get(place)
                .and_then(|m| m.get(capacity))
                .copied()
                .unwrap_or(0);
            // Same zero-cap policy as `sl1_objectives.rs:used_percent` —
            // return 0% rather than divide-by-zero / NoData.
            let pct = if cap == 0 {
                0u64
            } else {
                #[allow(clippy::cast_possible_truncation)]
                let v = (u128::from(used.min(cap)) * 100 / u128::from(cap)) as u64;
                v
            };
            Sl1MetricState::Ok { value: pct }
        }
        Sl1MetricSource::PlaceInventoryCount { place, thing } => {
            let count = runtime
                .inventories
                .get(place)
                .and_then(|inv| inv.get(thing))
                .copied()
                .unwrap_or(0);
            Sl1MetricState::Ok { value: count }
        }
        Sl1MetricSource::DashboardFreshness { dashboard } => {
            let dash = scene
                .observability
                .as_ref()
                .expect("metric only declarable inside observability block")
                .dashboards
                .iter()
                .find(|d| d.id == *dashboard)
                .expect("loader validated metric.dashboard references a declared dashboard");
            match dashboard_freshness(dash, runtime, now) {
                Some(age) => Sl1MetricState::Ok { value: age },
                None => Sl1MetricState::NoData,
            }
        }
    }
}

// Intentionally coarse: events fire only on variant transitions
// (Ok ↔ Stale ↔ NoData), not on `freshness_ticks` value changes
// within Stale.  Tick-level freshness updates are surfaced via the
// snapshot's `sl1_dashboard_states`, not via events.
fn discriminants_differ(a: Sl1DashboardState, b: Sl1DashboardState) -> bool {
    a.discriminant_str() != b.discriminant_str()
}
