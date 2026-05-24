//! Fixed-timestep tick loop.
//!
//! ```text
//!     accumulator += real_dt
//!     while accumulator >= world.dt:
//!         tick_once(world) ──▶ TickOutput { events, snapshot_dirty }
//!         accumulator -= world.dt
//! ```
//!
//! Each `tick_once` runs the system pipeline in fixed order:
//!     lifecycle → movement → interaction
//! and finally emits a `SimEvent::Tick` so subscribers can synchronize
//! frame markers (PLAN §16 — deterministic, ordered system execution).

use simetro_protocol::SimEvent;

use crate::systems::{interaction, lifecycle, movement};
use crate::world::{RunState, World};

#[derive(Debug, Default)]
pub struct TickOutput {
    pub events: Vec<SimEvent>,
    /// Set whenever any positional state changed this tick. Renderer uses
    /// this to skip encoding work when nothing moved.
    pub snapshot_dirty: bool,
}

/// Advance the world by exactly one fixed timestep.
pub fn tick_once(world: &mut World) -> TickOutput {
    if matches!(world.state, RunState::Idle | RunState::Loaded) {
        world.state = RunState::Running;
    }
    world.tick = world.tick.saturating_add(1);

    let mut events: Vec<SimEvent> = Vec::with_capacity(8);
    lifecycle::run(world, &mut events);
    let arrivals = movement::run(world, &mut events);
    interaction::run(world, &mut events);

    events.push(SimEvent::Tick { tick: world.tick });

    TickOutput {
        events,
        snapshot_dirty: arrivals > 0 || !world.movers.is_empty(),
    }
}

/// Run a fixed-step accumulator over `real_dt`. Returns one
/// [`TickOutput`] per tick that fired, in order.
///
/// `max_ticks_per_call` caps catch-up to prevent the spiral-of-death
/// problem (frontend stall → huge accumulator → engine stalls trying
/// to catch up). Surplus accumulator is discarded; PLAN §14.1 budgets
/// 2ms per tick so we cap at e.g. 8 ticks per frame at 60Hz.
pub fn tick_accumulator(
    world: &mut World,
    real_dt: f32,
    accumulator: &mut f32,
    max_ticks_per_call: u32,
) -> Vec<TickOutput> {
    *accumulator += real_dt;
    let mut outputs = Vec::new();
    let mut fired = 0;
    while *accumulator >= world.dt && fired < max_ticks_per_call {
        outputs.push(tick_once(world));
        *accumulator -= world.dt;
        fired += 1;
    }
    if *accumulator >= world.dt {
        // Hit the cap; drop remaining surplus so we don't keep falling
        // behind on the next frame. The engine emits a typed Warning::Behind
        // for this in Step 11.
        *accumulator = 0.0;
    }
    outputs
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn tick_once_advances_counter() {
        let mut w = World::new(0);
        assert_eq!(w.tick, 0);
        let out = tick_once(&mut w);
        assert_eq!(w.tick, 1);
        // Empty world still emits the Tick marker.
        assert!(out
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::Tick { .. })));
    }

    #[test]
    fn empty_world_ticks_cleanly_many_times() {
        let mut w = World::new(0);
        for _ in 0..1_000 {
            let _ = tick_once(&mut w);
        }
        assert_eq!(w.tick, 1_000);
        assert_eq!(w.state, RunState::Running);
    }

    #[test]
    fn accumulator_fires_expected_count() {
        let mut w = World::new(0).with_dt(0.01);
        let mut acc = 0.0_f32;
        // 0.035s at dt=0.01 -> 3 ticks; 0.005 remaining.
        let outs = tick_accumulator(&mut w, 0.035, &mut acc, 1000);
        assert_eq!(outs.len(), 3);
        assert!((acc - 0.005).abs() < 1e-6, "remaining acc = {acc}");
        assert_eq!(w.tick, 3);
    }

    #[test]
    fn accumulator_caps_at_max_ticks() {
        let mut w = World::new(0).with_dt(0.01);
        let mut acc = 0.0_f32;
        let outs = tick_accumulator(&mut w, 1.0, &mut acc, 8);
        assert_eq!(outs.len(), 8);
        // Cap engaged: surplus dropped.
        assert!((acc - 0.0).abs() < 1e-6);
    }
}
