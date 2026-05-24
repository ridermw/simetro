//! Fixed-timestep tick loop.
//!
//! ```text
//!     accumulator += real_dt
//!     while accumulator >= world.dt:
//!         tick(world) ──▶ TickOutput { events, snapshot_dirty }
//!         accumulator -= world.dt
//! ```
//!
//! The engine itself does not own the wall clock — callers feed it
//! `real_dt` (or simply call `tick_once` from headless mode where time
//! is logical, not wall-clock). PLAN §16 requires fixed-timestep ticks
//! for deterministic baselines.

use simetro_protocol::SimEvent;

use crate::world::{RunState, World};

#[derive(Debug, Default)]
pub struct TickOutput {
    pub events: Vec<SimEvent>,
    /// Set whenever any positional state changed this tick. Renderer uses
    /// this to skip encoding work when nothing moved.
    pub snapshot_dirty: bool,
}

/// Advance the world by exactly one fixed timestep.
///
/// In Step 5 this just bumps the tick counter and emits a single
/// `SimEvent::Tick`. Movement and interaction systems land in Step 7.
pub fn tick_once(world: &mut World) -> TickOutput {
    if world.state == RunState::Idle {
        // A world with no scene loaded simply increments tick — useful
        // for smoke-testing that the tick loop is wired up.
        world.state = RunState::Running;
    }
    world.tick = world.tick.saturating_add(1);
    TickOutput {
        events: vec![SimEvent::Tick { tick: world.tick }],
        snapshot_dirty: false,
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
        // We hit the cap. Drop remaining surplus so we don't keep falling
        // behind on the next frame. The frontend should surface a
        // `Warning::Behind` when it observes this; engine emits the
        // warning in Step 11 when the typed-warning surface lands.
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
        assert_eq!(out.events.len(), 1);
        match out.events[0] {
            SimEvent::Tick { tick } => assert_eq!(tick, 1),
            ref other => panic!("expected Tick, got {other:?}"),
        }
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
