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
//! frame markers (determinism contract — deterministic, ordered system execution).
//!
//! `TickRunner` owns all per-tick scratch buffers so steady-state ticks
//! make zero heap allocations (zero-allocation invariant; gated by the dhat test
//! in `tests/zero_alloc.rs`).

use simetro_protocol::{ActionTag, SimEvent, SimMessage};

use crate::actions::{apply_action, Outcome};
use crate::agent::AgentHost;
use crate::agent_log::{AgentLog, AgentLogEntry};
use crate::events::agent_error_to_message;
use crate::systems::{interaction, lifecycle, movement, production};
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
    production_scratch: production::ProductionScratch,
    /// Built-in / in-process agents driven by the engine.
    hosts: Vec<AgentHost>,
    /// Optional append-only log of agent decisions (AgentLog contract). When
    /// present, every successful agent action is written here.
    agent_log: Option<AgentLog>,
    last_arrivals: u32,
    topology_dirty: bool,
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
    ///
    /// Hosts are kept sorted by stable `agent_id` so that multi-agent
    /// scenes execute in a registration-order-independent order. This
    /// is a determinism invariant: two builds of the same scene that
    /// register the same agents in different orders (e.g. due to a
    /// loader refactor or scene-JSON reordering) must produce
    /// bit-identical world hashes (see `crates/engine/src/state_hash.rs`
    /// and the multi-agent ordering test in this file).
    ///
    /// # Panics
    /// Does NOT panic on duplicate `agent_id`; the loader is
    /// responsible for rejecting duplicates with a `LoadError` before
    /// `register_agent` is called. Two hosts with the same `agent_id`
    /// would still iterate in insertion-order-after-sort which is
    /// stable but semantically ambiguous.
    pub fn register_agent(&mut self, host: AgentHost) {
        // Find the insertion point that keeps `hosts` sorted by id.
        // `partition_point` is O(log n) for the position lookup;
        // `insert` is O(n) for the shift. n is tiny in practice
        // (typically 1-5 agents per scene) so total cost is trivial.
        let pos = self.hosts.partition_point(|h| h.id() < host.id());
        self.hosts.insert(pos, host);
    }

    /// Attach an AgentLog. All subsequent successful agent decisions
    /// will be appended to it. A fallback warning (`AgentLogSlow`) is
    /// emitted via `messages()` when the log first degrades to its
    /// ring buffer.
    pub fn attach_agent_log(&mut self, log: AgentLog) {
        self.agent_log = Some(log);
    }

    /// Detach and return the AgentLog, if any. Useful for flushing
    /// before shutdown or for inspecting ring contents in tests.
    pub fn take_agent_log(&mut self) -> Option<AgentLog> {
        self.agent_log.take()
    }

    /// Borrow the attached AgentLog (mostly for tests).
    #[must_use]
    pub fn agent_log(&self) -> Option<&AgentLog> {
        self.agent_log.as_ref()
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

    /// True when the most recent tick changed nodes or paths. Drivers
    /// should re-emit a static payload and fresh snapshot before continuing.
    #[must_use]
    pub fn topology_dirty(&self) -> bool {
        self.topology_dirty
    }

    /// Advance the world by exactly one fixed timestep. Returns the
    /// number of mover arrivals this tick.
    ///
    /// **Early return when world is not runnable** (run-state safety review):
    /// if a prior tick or agent step left `world.state` in
    /// [`RunState::Paused`] or [`RunState::Faulted`], the systems
    /// pipeline is skipped and the tick counter is NOT advanced. This
    /// prevents state from silently mutating after a fatal fault when
    /// outer drivers (e.g. `cmd_run`) loop unconditionally.
    pub fn tick_once(&mut self, world: &mut World) -> u32 {
        if matches!(world.state, RunState::Paused | RunState::Faulted) {
            // Clear any messages/events from a prior call so callers
            // that still drain them see an empty batch — no stale data.
            self.events.clear();
            self.messages.clear();
            self.last_arrivals = 0;
            self.topology_dirty = false;
            return 0;
        }
        if matches!(world.state, RunState::Idle | RunState::Loaded) {
            world.state = RunState::Running;
        }
        world.tick = world.tick.saturating_add(1);

        self.events.clear();
        self.messages.clear();
        self.topology_dirty = false;
        lifecycle::run(world, &mut self.events, &mut self.spawn_scratch);
        let arrivals = movement::run(world, &mut self.events, &mut self.arrival_scratch);

        // Agents observe state after lifecycle+movement so newly Waiting
        // movers (just spawned or just arrived) are visible. Interaction
        // then routes whoever is still Waiting after agent acts.
        self.run_agents(world);

        interaction::run(world, &mut self.events, &mut self.route_scratch);
        production::run(world, &mut self.production_scratch);
        crate::sl1_runtime::run(world, &mut self.messages);

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
            // We need the observation both to drive the agent and to
            // log it. `AgentHost::step` already observes internally;
            // we re-observe here for the log so we capture exactly
            // what the agent saw. This is cheap and side-effect-free.
            let obs_for_log = if self.agent_log.is_some() {
                Some(host.observe_only(world))
            } else {
                None
            };

            match host.step(world) {
                Ok(report) => {
                    if let Some(action) = report.chosen.clone() {
                        let outcome = apply_action(world, &agent_id, &action, &mut self.events);
                        if let Outcome::Rejected(w) = outcome {
                            self.messages.push(SimMessage::Warning(w));
                        } else if matches!(outcome, Outcome::Applied)
                            && action_changes_topology(&action)
                        {
                            self.topology_dirty = true;
                        }
                        self.events.push(SimEvent::AgentDecided {
                            agent_id: agent_id.clone(),
                            action: action.tag(),
                        });
                    } else {
                        self.events.push(SimEvent::AgentDecided {
                            agent_id: agent_id.clone(),
                            action: ActionTag::NoOp,
                        });
                    }
                    if let (Some(log), Some(obs)) = (self.agent_log.as_mut(), obs_for_log) {
                        let entry = AgentLogEntry::new(
                            &obs,
                            &agent_id,
                            report.chosen.clone(),
                            report.considered.len(),
                            report.rationale.clone(),
                            None,
                        );
                        if let Some(warn) = log.append(&entry) {
                            self.messages.push(SimMessage::Warning(warn));
                        }
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

fn action_changes_topology(action: &simetro_protocol::Action) -> bool {
    matches!(
        action,
        simetro_protocol::Action::PlacePiece { .. }
            | simetro_protocol::Action::ConnectPieces { .. }
            | simetro_protocol::Action::RemovePiece { .. }
    )
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
    use simetro_protocol::{Action, AgentReport, FaultPayload, WarningPayload};

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

    // ---- Agent integration ---------------------------------------------

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

    struct InvalidAuthorAgent;
    impl Agent for InvalidAuthorAgent {
        fn id(&self) -> &str {
            "author"
        }
        fn interval_ticks(&self) -> u32 {
            1
        }
        fn observe(&mut self, _w: &World) -> Observation {
            Observation::default()
        }
        fn act(&mut self, _o: &Observation) -> Result<AgentReport, AgentError> {
            Ok(AgentReport {
                tick: 0,
                agent_id: "author".into(),
                considered: vec![],
                chosen: Some(Action::PlacePiece {
                    piece_kind: "mover".into(),
                    pos: [0.0, 0.0],
                }),
                rationale: "invalid author action".into(),
                confidence: 1.0,
            })
        }
    }

    struct ValidAuthorAgent;
    impl Agent for ValidAuthorAgent {
        fn id(&self) -> &str {
            "author"
        }
        fn interval_ticks(&self) -> u32 {
            1
        }
        fn observe(&mut self, _w: &World) -> Observation {
            Observation::default()
        }
        fn act(&mut self, _o: &Observation) -> Result<AgentReport, AgentError> {
            Ok(AgentReport {
                tick: 0,
                agent_id: "author".into(),
                considered: vec![],
                chosen: Some(Action::PlacePiece {
                    piece_kind: "circle".into(),
                    pos: [10.0, 20.0],
                }),
                rationale: "place a visible node".into(),
                confidence: 1.0,
            })
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
    fn invalid_author_action_emits_visible_warning() {
        let mut world = World::new(0);
        let mut runner = TickRunner::new();
        runner.register_agent(AgentHost::new(Box::new(InvalidAuthorAgent)));
        runner.tick_once(&mut world);

        assert!(runner.messages().iter().any(|m| matches!(
            m,
            SimMessage::Warning(WarningPayload::InvalidAction { agent_id, reason })
                if agent_id == "author" && reason.contains("unsupported piece_kind")
        )));
        assert!(!runner.topology_dirty());
    }

    #[test]
    fn valid_author_action_marks_topology_dirty() {
        let mut world = World::new(0);
        let mut runner = TickRunner::new();
        runner.register_agent(AgentHost::new(Box::new(ValidAuthorAgent)));
        runner.tick_once(&mut world);

        assert!(runner.topology_dirty());
        assert_eq!(world.nodes.len(), 1);
    }

    /// Once an agent panics and the world is `Faulted`, subsequent
    /// `tick_once` calls must NOT advance the tick counter or run
    /// systems — otherwise outer drivers that loop unconditionally
    /// (e.g. `cmd_run`) would silently keep mutating state after the
    /// fault (review feedback on PR #1, codex).
    #[test]
    fn faulted_world_does_not_advance() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let mut runner = TickRunner::new();
        runner.register_agent(AgentHost::new(Box::new(PanickingAgent)));
        runner.tick_once(&mut loaded.world);
        assert_eq!(loaded.world.state, RunState::Faulted);
        let tick_at_fault = loaded.world.tick;

        // Drive 5 more ticks; the engine must hold.
        for _ in 0..5 {
            let arrivals = runner.tick_once(&mut loaded.world);
            assert_eq!(arrivals, 0);
            assert!(runner.events().is_empty());
            assert!(runner.messages().is_empty());
        }
        assert_eq!(loaded.world.tick, tick_at_fault);
        assert_eq!(loaded.world.state, RunState::Faulted);
    }

    /// Pausing the world likewise freezes the tick loop; resuming
    /// (Running) lets it advance again.
    #[test]
    fn paused_world_holds_and_resumes() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let mut runner = TickRunner::new();
        runner.tick_once(&mut loaded.world);
        assert_eq!(loaded.world.state, RunState::Running);
        let tick_at_pause = loaded.world.tick;

        loaded.world.state = RunState::Paused;
        for _ in 0..3 {
            runner.tick_once(&mut loaded.world);
        }
        assert_eq!(loaded.world.tick, tick_at_pause);
        assert_eq!(loaded.world.state, RunState::Paused);

        loaded.world.state = RunState::Running;
        runner.tick_once(&mut loaded.world);
        assert_eq!(loaded.world.tick, tick_at_pause + 1);
    }

    /// AgentDecided must carry the real agent_id, not a hardcoded 0
    /// (review feedback on PR #1, codex).
    #[test]
    fn agent_decided_carries_real_agent_id() {
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 10.0;
        let mut runner = TickRunner::new();
        runner.register_agent(AgentHost::new(Box::new(SpeedTuner::new(1))));
        runner.tick_once(&mut loaded.world);
        runner.tick_once(&mut loaded.world);

        let decided = runner
            .events()
            .iter()
            .filter_map(|e| match e {
                SimEvent::AgentDecided { agent_id, .. } => Some(agent_id.clone()),
                _ => None,
            })
            .next()
            .expect("expected at least one AgentDecided event");
        assert_eq!(decided, "speed_tuner_0");
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

    // ---- AgentLog wiring -----------------------------------------------

    #[test]
    fn agent_log_records_decisions_when_attached() {
        use std::io;
        struct CountSink {
            lines: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        }
        impl io::Write for CountSink {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                if b == b"\n" {
                    self.lines
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let log = AgentLog::new(Box::new(CountSink {
            lines: counter.clone(),
        }));

        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 10.0;
        let mut runner = TickRunner::new();
        runner.attach_agent_log(log);
        runner.register_agent(AgentHost::new(Box::new(SpeedTuner::new(1))));
        for _ in 0..3 {
            runner.tick_once(&mut loaded.world);
        }
        assert!(counter.load(std::sync::atomic::Ordering::Relaxed) >= 1);
    }

    #[test]
    fn agent_log_failure_emits_slow_warning_once() {
        use std::io;
        struct AlwaysErr;
        impl io::Write for AlwaysErr {
            fn write(&mut self, _b: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("no disk"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let log = AgentLog::with_capacity(Box::new(AlwaysErr), 32);
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        loaded.world.dt = 10.0;
        let mut runner = TickRunner::new();
        runner.attach_agent_log(log);
        runner.register_agent(AgentHost::new(Box::new(SpeedTuner::new(1))));

        let mut total_slow = 0;
        for _ in 0..5 {
            runner.tick_once(&mut loaded.world);
            total_slow += runner
                .messages()
                .iter()
                .filter(|m| {
                    matches!(
                        m,
                        SimMessage::Warning(simetro_protocol::WarningPayload::AgentLogSlow)
                    )
                })
                .count();
        }
        assert_eq!(total_slow, 1, "AgentLogSlow should warn exactly once");
        let log = runner.take_agent_log().unwrap();
        assert!(log.is_degraded());
        assert!(!log.ring_snapshot().is_empty());
    }

    // ---- Stable agent-host ordering (rubber-duck CRITICAL fix) -----

    /// Tracks the order in which agents are invoked across multiple
    /// ticks. Used by `agent_hosts_run_in_id_sorted_order` to assert
    /// the iteration order does NOT depend on registration order.
    struct RecordingAgent {
        id: &'static str,
        log: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
    }

    impl Agent for RecordingAgent {
        fn id(&self) -> &str {
            self.id
        }
        fn interval_ticks(&self) -> u32 {
            1
        }
        fn observe(&mut self, _w: &World) -> Observation {
            Observation::default()
        }
        fn act(&mut self, _o: &Observation) -> Result<AgentReport, AgentError> {
            self.log.lock().unwrap().push(self.id);
            Ok(AgentReport {
                tick: 0,
                agent_id: self.id.into(),
                considered: vec![],
                chosen: None,
                rationale: String::new(),
                confidence: 1.0,
            })
        }
    }

    fn record_ids_after_one_tick(register_order: &[&'static str]) -> Vec<&'static str> {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut loaded = load_scene_str(SCENE, 0).unwrap();
        let mut runner = TickRunner::new();
        for id in register_order {
            runner.register_agent(AgentHost::new(Box::new(RecordingAgent {
                id,
                log: std::sync::Arc::clone(&log),
            })));
        }
        runner.tick_once(&mut loaded.world);
        let result = log.lock().unwrap().clone();
        result
    }

    /// Multi-agent invocation order MUST be by stable `agent_id`,
    /// not by registration order. Without this, multi-agent scenes
    /// would produce different world hashes depending on JSON
    /// declaration order or loader-side iteration order — a
    /// determinism gap rubber-duck CRITICAL #10 identified.
    #[test]
    fn agent_hosts_run_in_id_sorted_order_regardless_of_registration_order() {
        // Same three agents, two different registration orders.
        let abc = record_ids_after_one_tick(&["alpha", "bravo", "charlie"]);
        let cba = record_ids_after_one_tick(&["charlie", "bravo", "alpha"]);
        let bca = record_ids_after_one_tick(&["bravo", "charlie", "alpha"]);

        // All three must run in the same alphabetical order.
        assert_eq!(abc, vec!["alpha", "bravo", "charlie"]);
        assert_eq!(cba, vec!["alpha", "bravo", "charlie"]);
        assert_eq!(bca, vec!["alpha", "bravo", "charlie"]);
    }
}
