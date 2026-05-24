//! Fixed-timestep tick loop.
//!
//! ```text
//!     accumulator += real_dt
//!     while accumulator >= world.dt:
//!         runner.tick_once(world) ──▶ events drained from runner
//!         accumulator -= world.dt
//! ```
//!
//! Each `tick_once` runs the system pipeline in fixed order:
//!     lifecycle → movement → interaction
//! and finally emits a `SimEvent::Tick` so subscribers can synchronize
//! frame markers (PLAN §16 — deterministic, ordered system execution).
//!
//! `TickRunner` owns all per-tick scratch buffers so steady-state ticks
//! make zero heap allocations (PLAN §14 target; gated by the dhat test
//! in `tests/zero_alloc.rs`).

use simetro_protocol::{ActionTag, SimEvent, SimMessage};

use crate::actions::{apply_action, Outcome};
use crate::agent::AgentHost;
use crate::events::agent_error_to_message;
use crate::systems::{interaction, lifecycle, movement};
use crate::world::{RunState, World};

#[derive(Debug, Default)]
pub struct TickOutput {
    pub events: Vec<SimEvent>,
    /// Set whenever any positional state changed this tick. Renderer uses
    /// this to skip encoding work when nothing moved.
    pub snapshot_dirty: bool,
}

/// Owns reusable scratch buffers so the steady-state tick loop allocates
/// nothing after warmup. Construct one per simulation; call
/// [`TickRunner::tick_once`] every fixed step.
#[derive(Default)]
pub struct TickRunner {
    events: Vec<SimEvent>,
    /// Non-event wire messages produced this tick (AgentReport, Fault,
    /// Warning, …). The transport composes these alongside
    /// `SimMessage::Events(events.clone())`.
    messages: Vec<SimMessage>,
    spawn_scratch: Vec<lifecycle::SpawnScratch>,
    arrival_scratch: Vec<movement::ArrivalScratch>,
    route_scratch: Vec<interaction::RouteScratch>,
    /// Built-in / in-process agents driven by the engine.
    hosts: Vec<AgentHost>,
    last_arrivals: u32,
}

impl std::fmt::Debug for TickRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TickRunner")
            .field("events", &self.events.len())
            .field("messages", &self.messages.len())
            .field("hosts", &self.hosts.len())
            .field("last_arrivals", &self.last_arrivals)
            .finish()
    }
}

impl TickRunner {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-size internal buffers for a known mover count. Reduces
    /// allocations during warmup; not required for correctness.
    pub fn reserve_for(&mut self, mover_capacity: usize) {
        self.events.reserve(mover_capacity.saturating_mul(2));
        self.messages.reserve(8);
        self.spawn_scratch.reserve(mover_capacity);
        self.arrival_scratch.reserve(mover_capacity);
        self.route_scratch.reserve(mover_capacity);
    }

    /// Register a built-in agent. Agents fire per their own
    /// [`AgentHost::should_fire`] schedule.
    pub fn register_agent(&mut self, host: AgentHost) {
        self.hosts.push(host);
    }

    /// Borrow the per-tick event buffer. Valid until the next `tick_once`.
    #[must_use]
    pub fn events(&self) -> &[SimEvent] {
        &self.events
    }

    /// Borrow the non-event wire messages produced this tick
    /// (AgentReport / Fault / Warning).
    #[must_use]
    pub fn messages(&self) -> &[SimMessage] {
        &self.messages
    }

    /// Number of mover arrivals in the most recent tick.
    #[must_use]
    pub fn arrivals(&self) -> u32 {
        self.last_arrivals
    }

    /// Advance the world by exactly one fixed timestep. Returns the
    /// number of mover arrivals this tick.
    pub fn tick_once(&mut self, world: &mut World) -> u32 {
        if matches!(world.state, RunState::Idle | RunState::Loaded) {
            world.state = RunState::Running;
        }
        world.tick = world.tick.saturating_add(1);

        self.events.clear();
        self.messages.clear();
        lifecycle::run(world, &mut self.events, &mut self.spawn_scratch);
        let arrivals = movement::run(world, &mut self.events, &mut self.arrival_scratch);

        // Agents observe state after lifecycle+movement so newly Waiting
        // movers (just spawned or just arrived) are visible. Interaction
        // then routes whoever is still Waiting after agent acts.
        self.run_agents(world);

        interaction::run(world, &mut self.events, &mut self.route_scratch);

        self.events.push(SimEvent::Tick { tick: world.tick });
        self.last_arrivals = arrivals;
        arrivals
    }

    fn run_agents(&mut self, world: &mut World) {
        let tick = world.tick;
        for host in &mut self.hosts {
            if !host.should_fire(tick) {
                continue;
            }
            let agent_id = host.id().to_string();
            match host.step(world) {
                Ok(report) => {
                    if let Some(action) = report.chosen.clone() {
                        let outcome = apply_action(world, &agent_id, &action, &mut self.events);
                        if let Outcome::Rejected(w) = outcome {
                            self.messages.push(SimMessage::Warning(w));
                        }
                        self.events.push(SimEvent::AgentDecided {
                            agent_id: 0,
                            action: action.tag(),
                        });
                    } else {
                        self.events.push(SimEvent::AgentDecided {
                            agent_id: 0,
                            action: ActionTag::NoOp,
                        });
                    }
                    self.messages.push(SimMessage::AgentReport(report));
                }
                Err(err) => {
                    // Panic / invalid / timeout → typed Fault or Warning.
                    let msg = agent_error_to_message(&err);
                    // If it's a hard Fault, pause the engine.
                    if matches!(msg, SimMessage::Fault(_)) {
                        world.state = RunState::Faulted;
                    }
                    self.messages.push(msg);
                }
            }
        }
    }
}

/// Convenience wrapper for callers that don't need a long-lived
/// `TickRunner` (tests, one-off scripts). Allocates per call — prefer
/// `TickRunner` in the hot loop.
pub fn tick_once(world: &mut World) -> TickOutput {
    let mut runner = TickRunner::new();
    let arrivals = runner.tick_once(world);
    TickOutput {
        events: std::mem::take(&mut runner.events),
        snapshot_dirty: arrivals > 0 || !world.movers.is_empty(),
    }
}

/// Run a fixed-step accumulator over `real_dt`. Returns one
/// [`TickOutput`] per tick that fired, in order.
///
/// Allocates one TickOutput per fired tick. For zero-alloc usage, drive
/// a [`TickRunner`] directly in your own loop.
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
        *accumulator = 0.0;
    }
    outputs
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::agent::{Agent, Observation, SpeedTuner};
    use crate::error::AgentError;
    use crate::loader::load_scene_str;
    use simetro_protocol::{AgentReport, FaultPayload};

    const SCENE: &str = include_str!("../../../games/demo-paths.json");

    #[test]
    fn tick_once_advances_counter() {
        let mut w = World::new(0);
        assert_eq!(w.tick, 0);
        let out = tick_once(&mut w);
        assert_eq!(w.tick, 1);
        assert!(out
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::Tick { .. })));
    }

    #[test]
    fn empty_world_ticks_cleanly_many_times() {
        let mut w = World::new(0);
        let mut runner = TickRunner::new();
        for _ in 0..1_000 {
            let _ = runner.tick_once(&mut w);
        }
        assert_eq!(w.tick, 1_000);
        assert_eq!(w.state, RunState::Running);
    }

    #[test]
    fn accumulator_fires_expected_count() {
        let mut w = World::new(0).with_dt(0.01);
        let mut acc = 0.0_f32;
        let outs = tick_accumulator(&mut w, 0.035, &mut acc, 1000);
        assert_eq!(outs.len(), 3);
        assert!((acc - 0.005).abs() < 1e-6);
        assert_eq!(w.tick, 3);
    }

    #[test]
    fn accumulator_caps_at_max_ticks() {
        let mut w = World::new(0).with_dt(0.01);
        let mut acc = 0.0_f32;
        let outs = tick_accumulator(&mut w, 1.0, &mut acc, 8);
        assert_eq!(outs.len(), 8);
        assert!((acc - 0.0).abs() < 1e-6);
    }

    // ---- Step 11: agent integration ------------------------------------

    struct PanickingAgent;
    impl Agent for PanickingAgent {
        fn id(&self) -> &str {
            "panic_one"
        }
        fn interval_ticks(&self) -> u32 {
            1
        }
        fn observe(&mut self, _w: &World) -> Observation {
            Observation::default()
        }
        fn act(&mut self, _o: &Observation) -> Result<AgentReport, AgentError> {
            panic!("kaboom")
        }
    }

    #[test]
    fn agent_panic_emits_fault_and_pauses_engine() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let mut runner = TickRunner::new();
        runner.register_agent(AgentHost::new(Box::new(PanickingAgent)));
        runner.tick_once(&mut loaded.world);
        assert_eq!(loaded.world.state, RunState::Faulted);
        assert!(runner
            .messages()
            .iter()
            .any(|m| matches!(m, SimMessage::Fault(FaultPayload::AgentCrashed { .. }))));
    }

    #[test]
    fn speed_tuner_decision_emits_agent_report_and_event() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        // Big dt so a single tick moves a mover the whole path.
        loaded.world.dt = 10.0;
        let mut runner = TickRunner::new();
        runner.register_agent(AgentHost::new(Box::new(SpeedTuner::new(1))));
        // Tick 1: lifecycle spawns + departs, no arrivals yet → agent NoOp.
        runner.tick_once(&mut loaded.world);
        // Tick 2: movement saturates progress → arrivals → movers Waiting
        // by the time the agent observes. Agent now picks SetSpeed.
        runner.tick_once(&mut loaded.world);

        assert!(runner
            .messages()
            .iter()
            .any(|m| matches!(m, SimMessage::AgentReport(_))));
        assert!(runner
            .events()
            .iter()
            .any(|e| matches!(e, SimEvent::AgentDecided { .. })));
        assert!(
            runner
                .events()
                .iter()
                .any(|e| matches!(e, SimEvent::MoverSpeedChange { .. })),
            "expected a SetSpeed event but got {:?}",
            runner.events()
        );
    }

    #[test]
    fn idle_engine_remains_running_when_no_agents() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let mut runner = TickRunner::new();
        runner.tick_once(&mut loaded.world);
        assert_eq!(loaded.world.state, RunState::Running);
        assert!(runner.messages().is_empty());
    }
}
