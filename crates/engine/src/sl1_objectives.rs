// Objectives, FCs, and VCs must NEVER silently no-op: if the loader
// invariant (`runtime.objectives`/`failure_conditions`/`victory_conditions`
// initialized for every declared id at scene load) is violated, the
// terminal `GameOutcome` would silently fail to fire `Lost`, which is
// worse than a panic. `expect` is the right tool here — let it speak
// the invariant out loud.
#![allow(clippy::expect_used)]

//! SL1 objectives / failure conditions / victory conditions runtime
//! (PR 8).
//!
//! Run order on each tick (post pressure / freshness / transforms /
//! demand, inside [`crate::sl1_runtime::run`]):
//!
//! 1. Short-circuit if `runtime.game_outcome` is already terminal so
//!    `Won` and `Lost` are sticky.
//! 2. Evaluate every objective in stable id order. Update
//!    `Sl1ObjectiveRuntime.status`, emit one
//!    [`SimEvent::Sl1ObjectiveStateChanged`] per transition, increment
//!    `breach_tick_count` while the objective is `Breached`. Emit a
//!    one-shot [`WarningPayload::Sl1Objective`] with
//!    [`Sl1ObjectiveWarningKind::UnsupportedInThisPr`]
//!    for `cost_budget` / `data_quality` / `query_latency`.
//! 3. Evaluate every failure condition in stable id order using the
//!    post-objective state (so `objective_breach_count` fires the same
//!    tick its target objective accumulates the disqualifying count).
//!    Maintain `breach_streak_ticks`; fire when streak > `grace_ticks`.
//!    Emit one [`SimEvent::Sl1FailureConditionFired`] per FC per run.
//! 4. Evaluate every victory condition. `survive_until` is met the
//!    first tick `now >= at_tick`. Emit one
//!    [`SimEvent::Sl1VictoryConditionMet`] per VC per run.
//! 5. Recompute `game_outcome`. Stable rule:
//!    - any FC fired on this or a previous tick → `Lost { reason:
//!      "failure_condition:<id>" }` (lowest id wins).
//!    - else, if `victory_conditions` is non-empty and every VC is
//!      met → `Won`.
//!    - else → `InProgress`.
//!
//!    Emit one [`SimEvent::Sl1GameOutcomeChanged`] on transition.
//! 6. Recompute `game_phase` server-side from the post-step-5 state.

use simetro_protocol::{
    SimEvent, SimMessage, Sl1ObjectiveStatusTag, Sl1ObjectiveWarningKind, WarningPayload,
};

use crate::scenario_language_v1::{
    FreshnessState, GameOutcome, Sl1FailureConditionParams, Sl1GamePhase, Sl1Objective,
    Sl1ObjectiveKind, Sl1ObjectiveParams, Sl1ObjectiveStatus, Sl1OperatingPredicate,
    Sl1RuntimeState, Sl1Scene, Sl1VictoryConditionParams,
};

pub fn run(
    scene: &Sl1Scene,
    runtime: &mut Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
    messages: &mut Vec<SimMessage>,
) {
    if runtime.game_outcome.is_terminal() {
        return;
    }

    evaluate_objectives(scene, runtime, now, events, messages);
    evaluate_failure_conditions(scene, runtime, now, events);
    evaluate_victory_conditions(scene, runtime, now, events);
    recompute_outcome(scene, runtime, now, events);
    runtime.game_phase = compute_phase(scene, runtime);
}

fn evaluate_objectives(
    scene: &Sl1Scene,
    runtime: &mut Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
    messages: &mut Vec<SimMessage>,
) {
    for obj in &scene.objectives {
        let (status, unsupported) = match &obj.params {
            Sl1ObjectiveParams::KeepFresh {
                place,
                thing,
                max_stale_ticks,
            } => (
                evaluate_keep_fresh(scene, runtime, place, thing, *max_stale_ticks, now),
                false,
            ),
            Sl1ObjectiveParams::CompleteJobsBeforeDeadline { demand, max_missed } => {
                (evaluate_complete_jobs(runtime, demand, *max_missed), false)
            }
            Sl1ObjectiveParams::MaintainUtilization {
                place,
                capacity,
                min_percent,
                max_percent,
            } => (
                evaluate_utilization(scene, runtime, place, capacity, *min_percent, *max_percent),
                false,
            ),
            Sl1ObjectiveParams::UnsupportedInThisPr => (Sl1ObjectiveStatus::Unsupported, true),
        };
        let rt = runtime
            .objectives
            .get_mut(&obj.id)
            .expect("objective runtime initialized at scene load");
        if unsupported && runtime_first_warning(&obj.id, &mut runtime.unsupported_objectives_warned)
        {
            messages.push(SimMessage::Warning(WarningPayload::Sl1Objective {
                objective_id: obj.id.clone(),
                event: Sl1ObjectiveWarningKind::UnsupportedInThisPr,
                objective_kind: obj.kind.as_str().to_string(),
                tick: now,
            }));
        }
        let prev = rt.status;
        if prev != status {
            rt.status = status;
            rt.last_change_tick = now;
            events.push(SimEvent::Sl1ObjectiveStateChanged {
                objective_id: obj.id.clone(),
                from: status_tag(prev),
                to: status_tag(status),
                tick: now,
            });
        }
        if rt.status == Sl1ObjectiveStatus::Breached {
            rt.breach_tick_count = rt.breach_tick_count.saturating_add(1);
        }
    }
}

fn runtime_first_warning(id: &str, set: &mut std::collections::BTreeSet<String>) -> bool {
    set.insert(id.to_string())
}

fn status_tag(s: Sl1ObjectiveStatus) -> Sl1ObjectiveStatusTag {
    match s {
        Sl1ObjectiveStatus::Unknown => Sl1ObjectiveStatusTag::Unknown,
        Sl1ObjectiveStatus::Met => Sl1ObjectiveStatusTag::Met,
        Sl1ObjectiveStatus::Breached => Sl1ObjectiveStatusTag::Breached,
        Sl1ObjectiveStatus::Unsupported => Sl1ObjectiveStatusTag::Unsupported,
    }
}

fn evaluate_keep_fresh(
    _scene: &Sl1Scene,
    runtime: &Sl1RuntimeState,
    place: &str,
    thing: &str,
    max_stale_ticks: u64,
    now: u64,
) -> Sl1ObjectiveStatus {
    let state = runtime
        .freshness
        .get(&(place.to_string(), thing.to_string()))
        .copied()
        .unwrap_or(FreshnessState::NoData);
    // Semantic: `max_stale_ticks` is the maximum allowed age, measured
    // from the most recent inventory write (`last_set_tick`), regardless
    // of whether the freshness state has flipped from `Ok` to `Stale`.
    // Both branches apply the same age comparison so the objective
    // breaches the moment `now - last_set_tick > max_stale_ticks`,
    // independent of the thing's `freshness_budget_ticks` choice.
    match state {
        FreshnessState::Ok { last_set_tick } | FreshnessState::Stale { last_set_tick } => {
            let age = now.saturating_sub(last_set_tick);
            if age <= max_stale_ticks {
                Sl1ObjectiveStatus::Met
            } else {
                Sl1ObjectiveStatus::Breached
            }
        }
        FreshnessState::NoData | FreshnessState::Degraded | FreshnessState::Invalid => {
            Sl1ObjectiveStatus::Breached
        }
    }
}

fn evaluate_complete_jobs(
    runtime: &Sl1RuntimeState,
    demand_id: &str,
    max_missed: u64,
) -> Sl1ObjectiveStatus {
    let dropped = runtime
        .demand
        .get(demand_id)
        .map(|d| d.dropped_count)
        .unwrap_or(0);
    if dropped <= max_missed {
        Sl1ObjectiveStatus::Met
    } else {
        Sl1ObjectiveStatus::Breached
    }
}

fn evaluate_utilization(
    scene: &Sl1Scene,
    runtime: &Sl1RuntimeState,
    place_id: &str,
    capacity_bucket: &str,
    min_percent: u8,
    max_percent: u8,
) -> Sl1ObjectiveStatus {
    let cap = scene
        .places
        .iter()
        .find(|p| p.id == place_id)
        .and_then(|p| p.capacity.get(capacity_bucket).copied())
        .unwrap_or(0);
    if cap == 0 {
        // Zero declared capacity → "0% used" → only Met if 0 is in range.
        return if min_percent == 0 {
            Sl1ObjectiveStatus::Met
        } else {
            Sl1ObjectiveStatus::Breached
        };
    }
    let used = runtime
        .place_capacity_used
        .get(place_id)
        .and_then(|m| m.get(capacity_bucket).copied())
        .unwrap_or(0);
    // Integer percent (truncating). Used 0..=cap.
    let pct = used_percent(used, cap);
    if pct >= min_percent && pct <= max_percent {
        Sl1ObjectiveStatus::Met
    } else {
        Sl1ObjectiveStatus::Breached
    }
}

fn evaluate_failure_conditions(
    scene: &Sl1Scene,
    runtime: &mut Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
) {
    for fc in &scene.failure_conditions {
        let breached = match &fc.params {
            Sl1FailureConditionParams::StaleTarget {
                place,
                thing,
                threshold_ticks,
                grace_ticks: _,
            } => stale_target_breached(runtime, place, thing, *threshold_ticks, now),
            Sl1FailureConditionParams::PlaceState {
                place,
                state,
                grace_ticks: _,
            } => place_state_breached(scene, runtime, place, state),
            Sl1FailureConditionParams::ObjectiveBreachCount {
                objective_id,
                max_count,
            } => runtime
                .objectives
                .get(objective_id)
                .map(|o| o.breach_tick_count > *max_count)
                .unwrap_or(false),
        };
        let grace = fc_grace(&fc.params);
        let rt = runtime
            .failure_conditions
            .get_mut(&fc.id)
            .expect("failure-condition runtime initialized at scene load");
        if breached {
            rt.breach_streak_ticks = rt.breach_streak_ticks.saturating_add(1);
        } else {
            rt.breach_streak_ticks = 0;
        }
        if rt.fired_at_tick.is_none() && rt.breach_streak_ticks > grace {
            rt.fired_at_tick = Some(now);
            events.push(SimEvent::Sl1FailureConditionFired {
                failure_condition_id: fc.id.clone(),
                tick: now,
            });
        }
    }
}

fn fc_grace(params: &Sl1FailureConditionParams) -> u64 {
    match params {
        Sl1FailureConditionParams::StaleTarget { grace_ticks, .. } => *grace_ticks,
        Sl1FailureConditionParams::PlaceState { grace_ticks, .. } => *grace_ticks,
        Sl1FailureConditionParams::ObjectiveBreachCount { .. } => 0,
    }
}

fn stale_target_breached(
    runtime: &Sl1RuntimeState,
    place: &str,
    thing: &str,
    threshold_ticks: u64,
    now: u64,
) -> bool {
    let state = runtime
        .freshness
        .get(&(place.to_string(), thing.to_string()))
        .copied()
        .unwrap_or(FreshnessState::NoData);
    match state {
        FreshnessState::Ok { last_set_tick } | FreshnessState::Stale { last_set_tick } => {
            now.saturating_sub(last_set_tick) > threshold_ticks
        }
        // NoData / Degraded / Invalid are always considered "stale
        // beyond threshold" so the FC fires after grace_ticks.
        FreshnessState::NoData | FreshnessState::Degraded | FreshnessState::Invalid => true,
    }
}

fn used_percent(used: u64, cap: u64) -> u8 {
    debug_assert!(cap > 0, "used_percent requires non-zero capacity");
    ((u128::from(used.min(cap)) * 100) / u128::from(cap)) as u8
}

fn place_state_breached(
    scene: &Sl1Scene,
    runtime: &Sl1RuntimeState,
    place_id: &str,
    state: &str,
) -> bool {
    let Some(place) = scene.places.iter().find(|p| p.id == place_id) else {
        return false;
    };
    let Some(op_state) = place.operating_states.get(state) else {
        return false;
    };
    match &op_state.predicate {
        Sl1OperatingPredicate::UsedPercentGte { metric, threshold } => {
            let cap = place.capacity.get(metric).copied().unwrap_or(0);
            // Zero declared capacity → treat as 0% used (consistent
            // with maintain_utilization's objective evaluator). A
            // predicate `used_percent >= 0` then still fires.
            let pct = if cap == 0 {
                0u8
            } else {
                let used = runtime
                    .place_capacity_used
                    .get(place_id)
                    .and_then(|m| m.get(metric).copied())
                    .unwrap_or(0);
                used_percent(used, cap)
            };
            pct >= *threshold
        }
        // Schema rejects this combination at load time; defensive
        // false here keeps the runtime total even if a future bug
        // bypasses the check.
        Sl1OperatingPredicate::OverloadedTicksGt { .. } => false,
    }
}

fn evaluate_victory_conditions(
    scene: &Sl1Scene,
    runtime: &mut Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
) {
    for vc in &scene.victory_conditions {
        let met = match &vc.params {
            Sl1VictoryConditionParams::SurviveUntil { at_tick } => now >= *at_tick,
        };
        let rt = runtime
            .victory_conditions
            .get_mut(&vc.id)
            .expect("victory-condition runtime initialized at scene load");
        if met && rt.met_at_tick.is_none() {
            rt.met_at_tick = Some(now);
            events.push(SimEvent::Sl1VictoryConditionMet {
                victory_condition_id: vc.id.clone(),
                tick: now,
            });
        }
    }
}

fn recompute_outcome(
    scene: &Sl1Scene,
    runtime: &mut Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
) {
    // FC scan in stable id order. First fired (lowest id) wins.
    let mut new_outcome: Option<GameOutcome> = None;
    for fc in &scene.failure_conditions {
        if let Some(rt) = runtime.failure_conditions.get(&fc.id) {
            if rt.fired_at_tick.is_some() {
                new_outcome = Some(GameOutcome::Lost {
                    reason: format!("failure_condition:{}", fc.id),
                });
                break;
            }
        }
    }
    if new_outcome.is_none()
        && !scene.victory_conditions.is_empty()
        && scene.victory_conditions.iter().all(|vc| {
            runtime
                .victory_conditions
                .get(&vc.id)
                .and_then(|r| r.met_at_tick)
                .is_some()
        })
    {
        new_outcome = Some(GameOutcome::Won);
    }

    let new_outcome = new_outcome.unwrap_or(GameOutcome::InProgress);
    if new_outcome != runtime.game_outcome {
        let from = runtime.game_outcome.variant_str().to_string();
        let to = new_outcome.variant_str().to_string();
        let reason = if let GameOutcome::Lost { reason } = &new_outcome {
            Some(reason.clone())
        } else {
            None
        };
        events.push(SimEvent::Sl1GameOutcomeChanged {
            from,
            to,
            tick: now,
            reason,
        });
        runtime.game_outcome = new_outcome;
    }
}

fn compute_phase(scene: &Sl1Scene, runtime: &Sl1RuntimeState) -> Sl1GamePhase {
    match &runtime.game_outcome {
        GameOutcome::Won => Sl1GamePhase::Won,
        GameOutcome::Lost { .. } => Sl1GamePhase::Lost,
        GameOutcome::InProgress => {
            let any_spiraling = runtime
                .failure_conditions
                .values()
                .any(|r| r.breach_streak_ticks > 0);
            if any_spiraling {
                return Sl1GamePhase::Spiraling;
            }
            let any_breached = runtime
                .objectives
                .values()
                .any(|r| r.status == Sl1ObjectiveStatus::Breached);
            if any_breached {
                return Sl1GamePhase::Losing;
            }
            let has_supported_objs = scene.objectives.iter().any(|o: &Sl1Objective| {
                matches!(
                    o.kind,
                    Sl1ObjectiveKind::KeepFresh
                        | Sl1ObjectiveKind::CompleteJobsBeforeDeadline
                        | Sl1ObjectiveKind::MaintainUtilization
                )
            });
            if has_supported_objs
                && scene.objectives.iter().all(|o| {
                    if !matches!(
                        o.kind,
                        Sl1ObjectiveKind::KeepFresh
                            | Sl1ObjectiveKind::CompleteJobsBeforeDeadline
                            | Sl1ObjectiveKind::MaintainUtilization
                    ) {
                        return true; // unsupported objectives don't gate winning phase
                    }
                    runtime
                        .objectives
                        .get(&o.id)
                        .map(|r| r.status == Sl1ObjectiveStatus::Met)
                        .unwrap_or(false)
                })
            {
                return Sl1GamePhase::Winning;
            }
            Sl1GamePhase::Stabilizing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::used_percent;

    #[test]
    fn used_percent_handles_u64_max_capacity_without_overflow() {
        assert_eq!(used_percent(u64::MAX, u64::MAX), 100);
    }
}
