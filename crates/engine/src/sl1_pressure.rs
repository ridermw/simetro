//! SL1 pressure runtime (PR 7).
//!
//! Pressures are scheduled scene events with a typed discriminator.
//! Each pressure has an inclusive activation window
//! `[at_tick, at_tick + duration_ticks)`. The pressure system fires
//! BEFORE freshness, transforms, and demand on every tick so the
//! overlay state is observable to downstream systems on the same tick
//! the pressure activates.
//!
//! Tick order (within `crate::sl1_runtime::run`):
//!
//! ```text
//! deactivate (end_tick == now) → activate (at_tick == now) →
//! rebuild overlay → apply source_multiplier injection →
//! freshness aging → transforms → demand
//! ```
//!
//! The four "supported in PR 7" variants drive real behavior:
//!
//! * `source_multiplier` writes inventory into the target place's
//!   storage of `thing` at `multiplier_milli / 1000` units per tick,
//!   clamped by the storage capacity. Fractional rates carry forward
//!   in `Sl1PressureRuntime::source_inject_carry_milli` so the
//!   per-tick arithmetic stays integer-deterministic.
//! * `demand_growth` adds `spawn_multiplier` to a per-demand overlay
//!   read by `run_demand`'s spawn loop.
//! * `quota_reduction` adds `reduction_percent` to a per-(place,
//!   capacity_bucket) overlay read by `try_start`'s capacity check.
//! * `path_outage` records the target link in
//!   `Sl1PressureRuntime::outaged_links` for snapshot/observability.
//!   Link transport is not implemented in PRs 0–6, so PR 7 only
//!   records the outage; transport gating lands when links transport.
//!
//! The five recognized-but-unsupported variants
//! (`schema_drift`, `dashboard_storm`, `spot_eviction_wave`,
//! `storage_metadata_storm`, `cooling_degradation`) emit
//! `Sl1Warning::PressureUnsupportedInThisPr` exactly once on
//! activation so authors are never misled into thinking a scheduled
//! pressure is silently driving behavior.

use std::collections::{BTreeMap, BTreeSet};

use simetro_protocol::{
    SimEvent, SimMessage, Sl1PressureEventKind, Sl1PressureWarningKind, WarningPayload,
};

use crate::scenario_language_v1::{Sl1Pressure, Sl1PressureParams, Sl1PressureRuntime, Sl1Scene};

/// Drive the pressure system one tick. Called by `sl1_runtime::run`
/// BEFORE freshness, transforms, and demand so the overlay state and
/// any inventory injections are observable on the same tick the
/// pressure activates.
///
/// Lifecycle events (`Sl1PressureLifecycle`) are pushed to `events`
/// so they appear in `TickRunner::events()` and are included in the
/// deterministic state hash via `feed_event`. Warnings (e.g. the
/// one-shot `UnsupportedInThisPr`) are pushed to `messages` because
/// the message channel is the canonical home for `Warning`/`Fault`.
pub fn run(
    scene: &Sl1Scene,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
    messages: &mut Vec<SimMessage>,
) {
    if scene.pressure.is_empty() {
        return;
    }

    // ---------- Step 1: deactivations FIRST ----------
    // Deactivate any pressure whose end tick (exclusive) is reached
    // at `now`. Iterate in id order for determinism.
    let mut to_deactivate: Vec<String> = Vec::new();
    for id in runtime.pressure.active.keys() {
        // The pressure definition the active id refers to; if the
        // scene rotates pressures (future PRs), the lookup may miss
        // and we conservatively keep the active entry.
        let def = pressure_def(scene, id);
        if let Some(def) = def {
            if now >= def.end_tick() {
                to_deactivate.push(id.clone());
            }
        }
    }
    for id in &to_deactivate {
        runtime.pressure.active.remove(id);
        // Drop any leftover fractional carry that belongs to this
        // pressure so a later distinct pressure on the same
        // (place, thing) starts from zero (rubber-duck #3).
        runtime
            .pressure
            .source_inject_carry_milli
            .retain(|(pid, _, _), _| pid != id);
        // unsupported_warned is edge-triggered per occurrence; we
        // clear on deactivation so a re-scheduled pressure (future
        // PRs) re-arms the warning.
        runtime.pressure.unsupported_warned.remove(id);
        if let Some(def) = pressure_def(scene, id) {
            events.push(SimEvent::Sl1PressureLifecycle {
                pressure_id: id.clone(),
                pressure_kind: def.kind.as_str().to_string(),
                event: Sl1PressureEventKind::Deactivated,
                tick: now,
            });
        }
    }

    // ---------- Step 2: activations ----------
    // Activate any pressure whose window `[at_tick, end_tick)` contains
    // `now` and that is not already active. The window form (instead
    // of `at_tick == now`) handles the case where the engine's tick
    // counter has already advanced past `at_tick` before the pressure
    // runtime first sees the scene — most notably `at_tick: 0`, which
    // would otherwise silently never activate because the runtime
    // first observes `now == 1` (the engine increments `world.tick`
    // before calling `sl1_runtime::run`). Pressures defined with
    // `at_tick: 0` therefore activate on the first observed tick.
    for def in &scene.pressure {
        if def.at_tick <= now
            && now < def.end_tick()
            && !runtime.pressure.active.contains_key(&def.id)
        {
            runtime.pressure.active.insert(def.id.clone(), now);
            events.push(SimEvent::Sl1PressureLifecycle {
                pressure_id: def.id.clone(),
                pressure_kind: def.kind.as_str().to_string(),
                event: Sl1PressureEventKind::Activated,
                tick: now,
            });
            if !def.kind.has_runtime_effect_in_pr7()
                && runtime.pressure.unsupported_warned.insert(def.id.clone())
            {
                messages.push(SimMessage::Warning(WarningPayload::Sl1Pressure {
                    pressure_id: def.id.clone(),
                    event: Sl1PressureWarningKind::UnsupportedInThisPr,
                    pressure_kind: def.kind.as_str().to_string(),
                    tick: now,
                }));
            }
        }
    }

    // ---------- Step 3: rebuild overlay from active set ----------
    rebuild_overlays(scene, runtime);

    // ---------- Step 4: apply source_multiplier injection ----------
    apply_source_injection(scene, runtime);
}

fn pressure_def<'a>(scene: &'a Sl1Scene, id: &str) -> Option<&'a Sl1Pressure> {
    // Linear search is fine: scenes have at most MAX_SL1_ITEMS_PER_SECTION
    // pressures, and the slice is sorted by id (binary search would
    // be marginally faster but adds risk).
    scene.pressure.iter().find(|p| p.id == id)
}

fn rebuild_overlays(scene: &Sl1Scene, runtime: &mut crate::scenario_language_v1::Sl1RuntimeState) {
    let active = &runtime.pressure.active;
    let mut demand_growth: BTreeMap<String, u32> = BTreeMap::new();
    let mut quota: BTreeMap<(String, String), u8> = BTreeMap::new();
    let mut outaged: BTreeSet<String> = BTreeSet::new();

    for def in &scene.pressure {
        if !active.contains_key(&def.id) {
            continue;
        }
        match &def.params {
            Sl1PressureParams::DemandGrowth { spawn_multiplier } => {
                let entry = demand_growth.entry(def.target.clone()).or_insert(0);
                *entry = entry.saturating_add(*spawn_multiplier);
            }
            Sl1PressureParams::QuotaReduction {
                capacity,
                reduction_percent,
            } => {
                let entry = quota
                    .entry((def.target.clone(), capacity.clone()))
                    .or_insert(0);
                *entry = entry.saturating_add(*reduction_percent).min(100);
            }
            Sl1PressureParams::PathOutage => {
                outaged.insert(def.target.clone());
            }
            // source_multiplier injection is applied separately
            // because it mutates inventory rather than producing an
            // overlay that downstream systems read.
            Sl1PressureParams::SourceMultiplier { .. } | Sl1PressureParams::UnsupportedInThisPr => {
            }
        }
    }
    runtime.pressure.demand_spawn_multiplier = demand_growth;
    runtime.pressure.quota_reduction = quota;
    runtime.pressure.outaged_links = outaged;
}

fn apply_source_injection(
    scene: &Sl1Scene,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
) {
    // Build a snapshot of (pressure_id, place, thing, multiplier_milli)
    // triples for active source_multiplier pressures, then mutate the
    // runtime. Carry milli-units across ticks so fractional rates
    // (e.g. 2.5x → 2500 milli) deterministically release integer units.
    // Carry is keyed by pressure_id (in addition to place/thing) so a
    // deactivated pressure cannot leak leftover fractional state into
    // a later, distinct pressure on the same (place, thing).
    let mut active_injections: Vec<(String, String, String, u64)> = Vec::new();
    for def in &scene.pressure {
        if !runtime.pressure.active.contains_key(&def.id) {
            continue;
        }
        if let Sl1PressureParams::SourceMultiplier {
            thing,
            multiplier_milli,
        } = &def.params
        {
            active_injections.push((
                def.id.clone(),
                def.target.clone(),
                thing.clone(),
                *multiplier_milli,
            ));
        }
    }

    // Precompute storage capacities so we can clamp injections.
    let mut storage_caps: BTreeMap<(String, String), u64> = BTreeMap::new();
    for place in &scene.places {
        for (thing_id, slot) in &place.storage {
            storage_caps.insert((place.id.clone(), thing_id.clone()), slot.capacity);
        }
    }

    for (pressure_id, place_id, thing_id, milli_per_tick) in active_injections {
        let carry_key = (pressure_id.clone(), place_id.clone(), thing_id.clone());
        let cap_key = (place_id.clone(), thing_id.clone());
        let carry = runtime
            .pressure
            .source_inject_carry_milli
            .entry(carry_key)
            .or_insert(0);
        *carry = carry.saturating_add(milli_per_tick);
        let whole = *carry / 1000;
        *carry %= 1000;
        if whole == 0 {
            continue;
        }
        let cap = storage_caps.get(&cap_key).copied().unwrap_or(0);
        let inv_slots = runtime.inventories.entry(place_id.clone()).or_default();
        let cur = inv_slots.entry(thing_id.clone()).or_insert(0);
        let headroom = cap.saturating_sub(*cur);
        let inject = whole.min(headroom);
        *cur = cur.saturating_add(inject);
        // Discarded units (clamped by capacity) are intentionally not
        // added back to the carry: storing them would mean an
        // unbounded multi-tick burst once headroom opens up, which is
        // not the intent of `source_multiplier`. Authors who want
        // burstable inflow should use a higher multiplier or longer
        // duration instead.
    }
}

/// Returns the effective spawn count for a demand at this tick given
/// the overlay state. Base spawn count is 1; each active
/// `demand_growth` pressure adds its `spawn_multiplier`.
#[must_use]
pub fn effective_spawn_count(runtime: &Sl1PressureRuntime, demand_id: &str) -> u32 {
    1u32.saturating_add(
        runtime
            .demand_spawn_multiplier
            .get(demand_id)
            .copied()
            .unwrap_or(0),
    )
}

/// Returns the effective capacity for a (place, bucket) given the
/// overlay state. Returns `base` unchanged when no overlay applies.
#[must_use]
pub fn effective_capacity(
    runtime: &Sl1PressureRuntime,
    place_id: &str,
    bucket: &str,
    base: u64,
) -> u64 {
    let key = (place_id.to_string(), bucket.to_string());
    let reduction = runtime.quota_reduction.get(&key).copied().unwrap_or(0);
    if reduction == 0 {
        return base;
    }
    let remaining = u64::from(100u8.saturating_sub(reduction));
    // floor(base * remaining / 100). Saturating ops keep arithmetic
    // safe for the entire u64 range.
    base.saturating_mul(remaining) / 100
}
