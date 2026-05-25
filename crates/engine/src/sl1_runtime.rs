//! SL1 per-tick runtime (PR 3).
//!
//! The deterministic system that recomputes per-(place, thing)
//! freshness state every tick. PR 3 only handles the budget-driven
//! `Ok → Stale` transition; quality-contract evaluation that can
//! reach [`FreshnessState::Degraded`] / [`FreshnessState::Invalid`]
//! lands with PR 8 (objectives + failure conditions).
//!
//! Inventory mutation lands in PR 4 (transforms) and PR 5 (demand).
//! For now the runtime treats inventory counts as immutable after
//! load — only the freshness machinery advances on the tick clock.

use crate::scenario_language_v1::{FreshnessState, Sl1Scene};
use crate::world::World;

/// Recompute SL1 runtime freshness for one tick.
///
/// Walks the runtime freshness map in stable order and ages each
/// budget-bearing `Ok` entry against `world.tick`. `saturating_sub`
/// keeps the math safe even if `world.tick < last_set_tick` (which
/// should never happen in practice, but we refuse to panic on a
/// clock anomaly).
///
/// `Stale` entries do not currently recover — that requires an
/// inventory write (PRs 4/5). `NoData`, `Degraded`, and `Invalid`
/// are left untouched.
pub fn run(world: &mut World) {
    let Some(scene) = world.sl1.as_ref() else {
        return;
    };
    let Some(runtime) = world.sl1_runtime.as_mut() else {
        return;
    };
    let now = world.tick;
    // Build a thing-id → budget map for O(1) lookup. Cheap because
    // `things` is sorted + small.
    let budgets = thing_budgets(scene);

    for ((_place_id, thing_id), state) in runtime.freshness.iter_mut() {
        let budget = match budgets.get(thing_id.as_str()) {
            Some(Some(b)) => *b,
            // Thing exists but is not time-budgeted: never ages
            // out purely on elapsed ticks.
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

fn thing_budgets(scene: &Sl1Scene) -> std::collections::BTreeMap<&str, Option<u64>> {
    scene
        .things
        .iter()
        .map(|t| (t.id.as_str(), t.freshness_budget_ticks))
        .collect()
}
