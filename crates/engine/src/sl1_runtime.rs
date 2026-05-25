//! SL1 per-tick runtime.
//!
//! Two responsibilities:
//!
//! 1. **Freshness aging** (PR 3): walk the runtime freshness map in
//!    stable order and age each budget-bearing `Ok` entry against
//!    `world.tick`.
//! 2. **Transform tick** (PR 4): drive each transform's deterministic
//!    state machine (`Idle | Running | Starved | Blocked | Late`) in
//!    stable id order, consume/produce typed inventories, reserve and
//!    release typed capacity, and emit one-shot warnings on state
//!    entry through the shared `messages` buffer.

use std::collections::BTreeMap;

use simetro_protocol::{SimMessage, Sl1DemandWarningKind, Sl1TransformWarningKind, WarningPayload};

use crate::scenario_language_v1::{
    FreshnessState, Sl1Demand, Sl1DemandInstance, Sl1DemandSchedule, Sl1DemandTarget,
    Sl1FailurePolicy, Sl1Scene, Sl1Transform, Sl1TransformState, MAX_DEMAND_OUTSTANDING,
};
use crate::world::World;

/// Recompute SL1 runtime freshness and drive transforms for one tick.
///
/// Warnings produced by transform state-machine transitions are
/// appended to `messages` as `SimMessage::Warning(WarningPayload::Sl1Transform { .. })`.
/// Demand-driven warnings (PR 5) — `Dropped`, `BacklogOverflow` —
/// are appended as `WarningPayload::Sl1Demand { .. }`.
pub fn run(world: &mut World, messages: &mut Vec<SimMessage>) {
    let Some(scene) = world.sl1.as_ref() else {
        return;
    };
    let Some(runtime) = world.sl1_runtime.as_mut() else {
        return;
    };
    let now = world.tick;

    age_freshness(scene, runtime, now);

    if !scene.transforms.is_empty() {
        run_transforms(scene, runtime, now, messages);
    }
    // Demand runs AFTER transforms so same-tick produced outputs are
    // visible to fulfillment on the same tick.
    if !scene.demand.is_empty() {
        run_demand(scene, runtime, now, messages);
    }
}

fn age_freshness(
    scene: &Sl1Scene,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    now: u64,
) {
    let budgets = thing_budgets(scene);
    for ((_place_id, thing_id), state) in runtime.freshness.iter_mut() {
        let budget = match budgets.get(thing_id.as_str()) {
            Some(Some(b)) => *b,
            Some(None) | None => continue,
        };
        if let FreshnessState::Ok { last_set_tick } = *state {
            let age = now.saturating_sub(last_set_tick);
            if age > budget {
                *state = FreshnessState::Stale { last_set_tick };
            }
        }
    }
}

fn thing_budgets(scene: &Sl1Scene) -> BTreeMap<&str, Option<u64>> {
    scene
        .things
        .iter()
        .map(|t| (t.id.as_str(), t.freshness_budget_ticks))
        .collect()
}

fn run_transforms(
    scene: &Sl1Scene,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    now: u64,
    messages: &mut Vec<SimMessage>,
) {
    // Build a stable id → definition lookup. `scene.transforms` is
    // canonicalized sorted by id at load time so iteration order is
    // stable; we still build a map because the runtime state is keyed
    // by id and the two layers must agree.
    let defs: BTreeMap<&str, &Sl1Transform> = scene
        .transforms
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();

    // Build place storage capacity lookup: (place_id, thing_id) -> capacity.
    let mut storage_caps: BTreeMap<(&str, &str), u64> = BTreeMap::new();
    for place in &scene.places {
        for (thing_id, slot) in &place.storage {
            storage_caps.insert((place.id.as_str(), thing_id.as_str()), slot.capacity);
        }
    }
    // Place typed-capacity caps.
    let mut place_caps: BTreeMap<&str, &BTreeMap<String, u64>> = BTreeMap::new();
    for place in &scene.places {
        place_caps.insert(place.id.as_str(), &place.capacity);
    }

    // Walk in stable id order (BTreeMap iteration is ordered).
    // Collect ids first so we can mutate runtime fields freely.
    let ids: Vec<String> = runtime.transforms.keys().cloned().collect();

    for id in ids {
        // Look up the transform definition. If missing (shouldn't
        // happen — loader ensures parity), skip.
        let Some(def) = defs.get(id.as_str()).copied() else {
            continue;
        };
        process_one(def, &id, runtime, &storage_caps, &place_caps, now, messages);
    }
}

#[allow(clippy::too_many_lines)]
fn process_one(
    def: &Sl1Transform,
    id: &str,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    storage_caps: &BTreeMap<(&str, &str), u64>,
    place_caps: &BTreeMap<&str, &BTreeMap<String, u64>>,
    now: u64,
    messages: &mut Vec<SimMessage>,
) {
    // Snapshot current state.
    let state = runtime
        .transforms
        .get(id)
        .cloned()
        .unwrap_or(Sl1TransformState::Idle);

    // (1) Advance the existing instance, if any.
    let state = match state {
        Sl1TransformState::Idle => Sl1TransformState::Idle,
        Sl1TransformState::Running {
            scheduled_at,
            started_at,
            attempt,
        } => advance_running(
            def,
            id,
            runtime,
            scheduled_at,
            started_at,
            attempt,
            now,
            messages,
        ),
        Sl1TransformState::Starved {
            scheduled_at,
            since,
            attempts,
        }
        | Sl1TransformState::Blocked {
            scheduled_at,
            since,
            attempts,
        } => advance_waiting(
            def,
            id,
            runtime,
            scheduled_at,
            since,
            attempts,
            now,
            storage_caps,
            place_caps,
            messages,
        ),
        Sl1TransformState::Late {
            scheduled_at,
            attempt,
            since,
        } => advance_late(
            def,
            id,
            runtime,
            scheduled_at,
            attempt,
            since,
            now,
            storage_caps,
            place_caps,
            messages,
        ),
    };

    // (2) Cadence trigger. Fires when `now % cadence_ticks == 0` AND
    // `now > 0` (skip the tick-0 slot so initial state isn't both
    // "freshly loaded" and "first cadence fired"). The deadline-from-
    // scheduled-at semantics mean tick 0 acts as a setup tick.
    let cadence_fires = now > 0 && def.cadence_ticks > 0 && now % def.cadence_ticks == 0;

    let new_state = match (state, cadence_fires) {
        (Sl1TransformState::Idle, true) => start_attempt(
            def,
            id,
            runtime,
            now,
            now,
            1,
            storage_caps,
            place_caps,
            messages,
        ),
        (Sl1TransformState::Idle, false) => Sl1TransformState::Idle,
        (other, true) => {
            // New cadence slot fired while another instance is still
            // in flight; emit SlotMissed and skip.
            emit_warning(messages, id, Sl1TransformWarningKind::SlotMissed, now, None);
            other
        }
        (other, false) => other,
    };

    runtime.transforms.insert(id.to_string(), new_state);
}

#[allow(clippy::too_many_arguments)]
fn advance_running(
    def: &Sl1Transform,
    id: &str,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    scheduled_at: u64,
    started_at: u64,
    attempt: u32,
    now: u64,
    messages: &mut Vec<SimMessage>,
) -> Sl1TransformState {
    let completion_tick = started_at.saturating_add(def.duration_ticks);
    let deadline_tick = scheduled_at.saturating_add(def.deadline_ticks);

    // Check deadline FIRST. Otherwise a late-started transform whose
    // completion_tick happens to equal a tick where `now > deadline_tick`
    // would still produce outputs (e.g. scheduled=10, deadline_ticks=10,
    // duration=5, but started=16 due to recovery from Starved → at tick
    // 21 the completion check would fire before the deadline check).
    if now > deadline_tick {
        release_capacity(def, runtime);
        emit_warning(
            messages,
            id,
            Sl1TransformWarningKind::Late,
            now,
            Some(attempt),
        );
        return handle_failure_policy(def, id, scheduled_at, attempt, now, messages);
    }

    if now >= completion_tick {
        // Produce outputs, update freshness, release capacity, → Idle.
        let place_id = def.runs_on.clone();
        let inv = runtime.inventories.entry(place_id.clone()).or_default();
        for out in &def.outputs {
            let entry = inv.entry(out.thing_id.clone()).or_default();
            *entry = entry.saturating_add(out.amount);
            runtime.freshness.insert(
                (place_id.clone(), out.thing_id.clone()),
                FreshnessState::Ok { last_set_tick: now },
            );
        }
        release_capacity(def, runtime);
        return Sl1TransformState::Idle;
    }

    Sl1TransformState::Running {
        scheduled_at,
        started_at,
        attempt,
    }
}

#[allow(clippy::too_many_arguments)]
fn advance_waiting(
    def: &Sl1Transform,
    id: &str,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    scheduled_at: u64,
    since: u64,
    attempts: u32,
    now: u64,
    storage_caps: &BTreeMap<(&str, &str), u64>,
    place_caps: &BTreeMap<&str, &BTreeMap<String, u64>>,
    messages: &mut Vec<SimMessage>,
) -> Sl1TransformState {
    let deadline_tick = scheduled_at.saturating_add(def.deadline_ticks);
    if now > deadline_tick {
        emit_warning(
            messages,
            id,
            Sl1TransformWarningKind::Late,
            now,
            Some(attempts),
        );
        return handle_failure_policy(def, id, scheduled_at, attempts, now, messages);
    }

    // Try to start again silently — re-entering Starved/Blocked does
    // NOT re-emit warnings while we're still in the same waiting
    // state. New warning emissions only happen on state-class change.
    start_attempt_quiet(
        def,
        id,
        runtime,
        scheduled_at,
        since,
        attempts,
        now,
        storage_caps,
        place_caps,
        messages,
    )
}

#[allow(clippy::too_many_arguments)]
fn advance_late(
    def: &Sl1Transform,
    id: &str,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    scheduled_at: u64,
    attempt: u32,
    since: u64,
    now: u64,
    storage_caps: &BTreeMap<(&str, &str), u64>,
    place_caps: &BTreeMap<&str, &BTreeMap<String, u64>>,
    messages: &mut Vec<SimMessage>,
) -> Sl1TransformState {
    // Drop policy always fails immediately at deadline breach, so any
    // Late state we see here is RetryThenWarn. We treat each tick in
    // Late as a retry slot: try to start, and if we exhaust max_attempts
    // without succeeding, emit Failed and reset to Idle.
    match def.failure_policy {
        Sl1FailurePolicy::Drop => {
            // Unexpected — Drop should never produce a Late state, but
            // be defensive: emit Failed and reset.
            emit_warning(
                messages,
                id,
                Sl1TransformWarningKind::Failed,
                now,
                Some(attempt),
            );
            Sl1TransformState::Idle
        }
        Sl1FailurePolicy::RetryThenWarn => {
            // Try to recover first: re-attempt start. If it succeeds we
            // go back to Running with a FRESH scheduled_at/started_at so
            // the retry gets a full deadline budget (otherwise a retry
            // for any `duration_ticks > 1` would immediately breach the
            // already-passed original deadline). If start fails, count
            // this tick as a consumed retry: increment attempt; if we
            // now exceed `max_attempts`, emit Failed and reset to Idle.
            match try_start(def, runtime, storage_caps, place_caps) {
                StartResult::Started => Sl1TransformState::Running {
                    scheduled_at: now,
                    started_at: now,
                    attempt,
                },
                _ => {
                    let next_attempt = attempt.saturating_add(1);
                    if next_attempt > def.max_attempts {
                        emit_warning(
                            messages,
                            id,
                            Sl1TransformWarningKind::Failed,
                            now,
                            Some(attempt),
                        );
                        Sl1TransformState::Idle
                    } else {
                        Sl1TransformState::Late {
                            scheduled_at,
                            attempt: next_attempt,
                            since,
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn start_attempt(
    def: &Sl1Transform,
    id: &str,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    scheduled_at: u64,
    now: u64,
    attempts: u32,
    storage_caps: &BTreeMap<(&str, &str), u64>,
    place_caps: &BTreeMap<&str, &BTreeMap<String, u64>>,
    messages: &mut Vec<SimMessage>,
) -> Sl1TransformState {
    match try_start(def, runtime, storage_caps, place_caps) {
        StartResult::Started => Sl1TransformState::Running {
            scheduled_at,
            started_at: now,
            attempt: attempts,
        },
        StartResult::Starved => {
            emit_warning(messages, id, Sl1TransformWarningKind::Starved, now, None);
            Sl1TransformState::Starved {
                scheduled_at,
                since: now,
                attempts,
            }
        }
        StartResult::Blocked => {
            emit_warning(messages, id, Sl1TransformWarningKind::Blocked, now, None);
            Sl1TransformState::Blocked {
                scheduled_at,
                since: now,
                attempts,
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn start_attempt_quiet(
    def: &Sl1Transform,
    _id: &str,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    scheduled_at: u64,
    since: u64,
    attempts: u32,
    now: u64,
    storage_caps: &BTreeMap<(&str, &str), u64>,
    place_caps: &BTreeMap<&str, &BTreeMap<String, u64>>,
    _messages: &mut Vec<SimMessage>,
) -> Sl1TransformState {
    match try_start(def, runtime, storage_caps, place_caps) {
        StartResult::Started => Sl1TransformState::Running {
            scheduled_at,
            started_at: now,
            attempt: attempts,
        },
        // Re-classify between Starved and Blocked silently — the
        // original state-entry warning already fired, and constant
        // re-emit would be noisy.
        StartResult::Starved => Sl1TransformState::Starved {
            scheduled_at,
            since,
            attempts,
        },
        StartResult::Blocked => Sl1TransformState::Blocked {
            scheduled_at,
            since,
            attempts,
        },
    }
}

enum StartResult {
    Started,
    Starved,
    Blocked,
}

fn try_start(
    def: &Sl1Transform,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    storage_caps: &BTreeMap<(&str, &str), u64>,
    place_caps: &BTreeMap<&str, &BTreeMap<String, u64>>,
) -> StartResult {
    let place_id = def.runs_on.as_str();

    // Inputs available?
    let empty_inv: BTreeMap<String, u64> = BTreeMap::new();
    let inv_ref = runtime.inventories.get(place_id).unwrap_or(&empty_inv);
    for input in &def.inputs {
        let have = inv_ref.get(&input.thing_id).copied().unwrap_or(0);
        if have < input.amount {
            return StartResult::Starved;
        }
    }

    // Capacity headroom?
    let caps = place_caps.get(place_id).copied();
    let used_empty: BTreeMap<String, u64> = BTreeMap::new();
    let used = runtime
        .place_capacity_used
        .get(place_id)
        .unwrap_or(&used_empty);
    for (k, v) in &def.capacity_cost {
        let cap = caps.and_then(|c| c.get(k)).copied().unwrap_or(0);
        let cur = used.get(k).copied().unwrap_or(0);
        if cur.saturating_add(*v) > cap {
            return StartResult::Blocked;
        }
    }

    // Output storage room? Each output thing's inventory + amount must
    // fit under that place's storage[thing].capacity.
    for out in &def.outputs {
        let cap = storage_caps
            .get(&(place_id, out.thing_id.as_str()))
            .copied()
            .unwrap_or(0);
        let cur = inv_ref.get(&out.thing_id).copied().unwrap_or(0);
        if cur.saturating_add(out.amount) > cap {
            return StartResult::Blocked;
        }
    }

    // Commit: consume inputs, reserve capacity.
    let inv = runtime.inventories.entry(place_id.to_string()).or_default();
    for input in &def.inputs {
        let entry = inv.entry(input.thing_id.clone()).or_default();
        *entry = entry.saturating_sub(input.amount);
    }
    let used = runtime
        .place_capacity_used
        .entry(place_id.to_string())
        .or_default();
    for (k, v) in &def.capacity_cost {
        let entry = used.entry(k.clone()).or_default();
        *entry = entry.saturating_add(*v);
    }
    StartResult::Started
}

fn release_capacity(
    def: &Sl1Transform,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
) {
    if let Some(used) = runtime.place_capacity_used.get_mut(&def.runs_on) {
        for (k, v) in &def.capacity_cost {
            if let Some(entry) = used.get_mut(k) {
                *entry = entry.saturating_sub(*v);
            }
        }
    }
}

fn handle_failure_policy(
    def: &Sl1Transform,
    id: &str,
    scheduled_at: u64,
    attempt: u32,
    now: u64,
    messages: &mut Vec<SimMessage>,
) -> Sl1TransformState {
    match def.failure_policy {
        Sl1FailurePolicy::Drop => {
            emit_warning(
                messages,
                id,
                Sl1TransformWarningKind::Failed,
                now,
                Some(attempt),
            );
            Sl1TransformState::Idle
        }
        Sl1FailurePolicy::RetryThenWarn => {
            if attempt >= def.max_attempts {
                emit_warning(
                    messages,
                    id,
                    Sl1TransformWarningKind::Failed,
                    now,
                    Some(attempt),
                );
                Sl1TransformState::Idle
            } else {
                Sl1TransformState::Late {
                    scheduled_at,
                    attempt: attempt.saturating_add(1),
                    since: now,
                }
            }
        }
    }
}

fn emit_warning(
    messages: &mut Vec<SimMessage>,
    transform_id: &str,
    event: Sl1TransformWarningKind,
    tick: u64,
    attempt: Option<u32>,
) {
    messages.push(SimMessage::Warning(WarningPayload::Sl1Transform {
        transform_id: transform_id.to_string(),
        event,
        tick,
        attempt,
    }));
}

// =====================================================================
// Demand (PR 5).
// =====================================================================
//
// Per-tick demand pipeline, runs AFTER transforms so same-tick produced
// outputs are visible to fulfillment:
//
//   for each declared demand (stable id order):
//     1. spawn:    schedule fires → enqueue one Pending instance
//                  (or DemandBacklogOverflow if outstanding >= cap)
//     2. fulfill:  oldest Pending whose `requires` are all present at
//                  the target place transitions to Fulfilled and is
//                  removed; fulfilled_count++
//     3. drop:     any Pending past its deadline (now > spawned_at +
//                  deadline_ticks) transitions to Dropped, emits
//                  DemandDropped warning, and is removed; dropped_count++
//
// Fulfillment is "observation-only" in PR 5 — presence checked, no
// inventory decremented. PR 8 may add a consuming variant.
//
// Backlog overflow is edge-triggered: a single warning fires on the
// transition into `pending.len() >= cap`; the flag clears once pending
// drops below cap so re-entry rearms the warning.

fn run_demand(
    scene: &Sl1Scene,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    now: u64,
    messages: &mut Vec<SimMessage>,
) {
    for def in &scene.demand {
        // ---------------- spawn ----------------
        let spawn_decision = {
            let Some(rt) = runtime.demand.get_mut(&def.id) else {
                continue;
            };
            if should_spawn(def, rt, now) {
                if rt.pending.len() >= MAX_DEMAND_OUTSTANDING {
                    if !rt.overflow {
                        rt.overflow = true;
                        Some(SpawnOutcome::Overflowed)
                    } else {
                        Some(SpawnOutcome::OverflowSuppressed)
                    }
                } else {
                    let seq = rt.next_sequence;
                    rt.next_sequence = rt.next_sequence.saturating_add(1);
                    rt.pending.push_back(Sl1DemandInstance {
                        sequence: seq,
                        spawned_at: now,
                        deadline_tick: now.saturating_add(def.deadline_ticks),
                    });
                    Some(SpawnOutcome::Spawned)
                }
            } else {
                None
            }
        };
        if matches!(spawn_decision, Some(SpawnOutcome::Overflowed)) {
            messages.push(SimMessage::Warning(WarningPayload::Sl1Demand {
                demand_id: def.id.clone(),
                event: Sl1DemandWarningKind::BacklogOverflow,
                tick: now,
                sequence: None,
                value: None,
                penalty_score: None,
            }));
        }

        // ---------------- drop past-deadline ----------------
        // Drains expired Pending instances FIRST so a tardy requirement
        // arriving in the same tick cannot silently fulfill an expired
        // instance. "Dropped when deadline passes" is the contract.
        let mut dropped_events: Vec<(u64, u64, i64)> = Vec::new();
        if let Some(rt) = runtime.demand.get_mut(&def.id) {
            while let Some(front) = rt.pending.front() {
                if now > front.deadline_tick {
                    if let Some(d) = rt.pending.pop_front() {
                        rt.dropped_count = rt.dropped_count.saturating_add(1);
                        dropped_events.push((d.sequence, def.value, def.penalty.score));
                    }
                } else {
                    break;
                }
            }
        }
        for (sequence, value, penalty_score) in dropped_events {
            messages.push(SimMessage::Warning(WarningPayload::Sl1Demand {
                demand_id: def.id.clone(),
                event: Sl1DemandWarningKind::Dropped,
                tick: now,
                sequence: Some(sequence),
                value: Some(value),
                penalty_score: Some(penalty_score),
            }));
        }

        // ---------------- fulfill ----------------
        // Observation-only: presence of every required thing at the
        // target place. No inventory decrement. Only non-expired
        // instances reach this point (drops above already evicted them).
        let met = demand_requirements_met(def, runtime);
        if met {
            if let Some(rt) = runtime.demand.get_mut(&def.id) {
                if rt.pending.pop_front().is_some() {
                    rt.fulfilled_count = rt.fulfilled_count.saturating_add(1);
                }
            }
        }

        // ---------------- reset overflow rearm ----------------
        if let Some(rt) = runtime.demand.get_mut(&def.id) {
            if rt.overflow && rt.pending.len() < MAX_DEMAND_OUTSTANDING {
                rt.overflow = false;
            }
        }
    }
}

enum SpawnOutcome {
    Spawned,
    Overflowed,
    OverflowSuppressed,
}

fn should_spawn(
    def: &Sl1Demand,
    rt: &mut crate::scenario_language_v1::Sl1DemandRuntime,
    now: u64,
) -> bool {
    if now == 0 {
        // Tick 0 is the pre-tick world; schedules begin at tick 1.
        return false;
    }
    match &def.spawn_schedule {
        Sl1DemandSchedule::Fixed {
            every_ticks,
            start_tick,
        } => now >= *start_tick && (now - *start_tick) % *every_ticks == 0,
        Sl1DemandSchedule::Scripted { ticks } => {
            // The cursor is the index of the next tick we still expect.
            // Advance past any ticks <= now so the cursor never lags.
            // Returns true if `now` was an expected spawn tick.
            let mut fired = false;
            while rt.scripted_cursor < ticks.len() {
                let candidate = ticks[rt.scripted_cursor];
                if candidate == now {
                    fired = true;
                    rt.scripted_cursor += 1;
                    break;
                } else if candidate < now {
                    // Missed a tick — should not happen since we
                    // call run_demand every tick, but keep the
                    // cursor monotonic.
                    rt.scripted_cursor += 1;
                } else {
                    break;
                }
            }
            fired
        }
    }
}

fn demand_requirements_met(
    def: &Sl1Demand,
    runtime: &crate::scenario_language_v1::Sl1RuntimeState,
) -> bool {
    let Sl1DemandTarget::Place(place_id) = &def.target;
    let Some(inv) = runtime.inventories.get(place_id) else {
        return false;
    };
    // Must have at least one Pending instance to fulfill.
    let Some(rt) = runtime.demand.get(&def.id) else {
        return false;
    };
    if rt.pending.is_empty() {
        return false;
    }
    for thing_id in &def.requires {
        let count = inv.get(thing_id).copied().unwrap_or(0);
        if count == 0 {
            return false;
        }
    }
    true
}
