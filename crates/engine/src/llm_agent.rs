//! # LlmAgent: `Agent`-trait impl backed by an [`AgentRuntime`]
//!
//! Thin wrapper that makes an LLM-backed decision look like any other
//! engine [`Agent`]. When `act()` fires, it:
//!
//! 1. Serializes the [`Observation`] to JSON (opaque to the engine —
//!    the bridge interprets it).
//! 2. Calls [`AgentRuntime::enqueue_decision`] to allocate a
//!    `TimelineId`, push an `AgentRequest` to the outbox, and seed a
//!    `Pending` entry on the `DecisionTimeline`.
//! 3. Returns a "thinking" [`AgentReport`] **immediately** with
//!    `chosen: None`. The real action is applied later by
//!    [`AgentRuntime::process_reply`] when the bridge writes back into
//!    the inbox — typically on a later tick, per lifecycle invariant.
//!
//! ## Why a shared `Arc<Mutex<AgentRuntime>>`?
//!
//! The [`Agent`] trait requires `Send` but not `Sync`. The engine is
//! single-threaded so contention is impossible in practice; `Mutex` is
//! used purely to satisfy the type system. `Rc<RefCell<…>>` would have
//! worked semantically but is `!Send` and therefore incompatible with
//! the trait bound. Lock acquisition cost in the uncontended single-
//! threaded case is ~5 ns — well under the engine tick budget.
//!
//! ## How does `LlmAgent` know `current_tick`?
//!
//! [`Agent::act`] does not receive a `&World`. We read `current_tick`
//! from [`Observation::tick`] which is always populated by
//! [`Observation::from_world`]. This keeps the trait surface stable.

use std::sync::{Arc, Mutex};

use simetro_protocol::AgentReport;

use crate::agent::{Agent, Observation};
use crate::agent_runtime::{AgentRuntime, EnqueueDecisionOutcome};
use crate::error::AgentError;
use crate::world::World;

/// LLM-backed `Agent` whose `act()` enqueues a request and returns a
/// "thinking" placeholder. The real action is delivered later via
/// [`AgentRuntime::process_reply`].
pub struct LlmAgent {
    id: String,
    interval_ticks: u32,
    deadline_ticks: u32,
    runtime: Arc<Mutex<AgentRuntime>>,
}

impl LlmAgent {
    /// Construct a new `LlmAgent`.
    ///
    /// - `id`: stable identifier; surfaces in `AgentLog`, `Inspector`,
    ///   and the `RequestId.agent_id` field on every outbox entry.
    /// - `interval_ticks`: how often the engine ticks the agent. Must
    ///   match the same loader constraint as other agents (1..=10_000).
    /// - `deadline_ticks`: per lifecycle invariant, the bridge has this many
    ///   ticks to reply before the request is expired and (possibly)
    ///   re-issued. Typical values 30–600 depending on backend speed.
    /// - `runtime`: shared handle to the engine's [`AgentRuntime`].
    pub fn new(
        id: impl Into<String>,
        interval_ticks: u32,
        deadline_ticks: u32,
        runtime: Arc<Mutex<AgentRuntime>>,
    ) -> Self {
        Self {
            id: id.into(),
            interval_ticks,
            deadline_ticks,
            runtime,
        }
    }

    /// Convenience: shared-handle constructor that also creates the
    /// `Arc<Mutex<…>>` for callers wiring a single agent in tests.
    pub fn with_fresh_runtime(
        id: impl Into<String>,
        interval_ticks: u32,
        deadline_ticks: u32,
    ) -> (Self, Arc<Mutex<AgentRuntime>>) {
        let runtime = Arc::new(Mutex::new(AgentRuntime::new()));
        let agent = Self::new(id, interval_ticks, deadline_ticks, Arc::clone(&runtime));
        (agent, runtime)
    }
}

impl Agent for LlmAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn interval_ticks(&self) -> u32 {
        self.interval_ticks
    }

    fn observe(&mut self, world: &World) -> Observation {
        Observation::from_world(world)
    }

    fn act(&mut self, obs: &Observation) -> Result<AgentReport, AgentError> {
        let observation_json =
            serde_json::to_string(obs).map_err(|err| AgentError::InvalidAction {
                agent_id: self.id.clone(),
                reason: format!("failed to serialize observation: {err}"),
            })?;

        // Engine is single-threaded; a poisoned Mutex here would
        // indicate a panic on another caller — surface as a typed
        // agent error rather than swallowing or panicking.
        let mut rt = self.runtime.lock().map_err(|_| AgentError::Panicked {
            agent_id: self.id.clone(),
            message: "AgentRuntime mutex poisoned".to_string(),
        })?;

        let outcome =
            rt.enqueue_decision(&self.id, observation_json, self.deadline_ticks, obs.tick);
        Ok(thinking_report(&self.id, obs.tick, &outcome))
    }
}

/// Build the "I have queued a request; await reply" placeholder
/// [`AgentReport`] returned by [`LlmAgent::act`] when the request is
/// accepted (or the backpressure-dropped variant when it isn't).
fn thinking_report(agent_id: &str, tick: u64, outcome: &EnqueueDecisionOutcome) -> AgentReport {
    let rationale = match outcome {
        EnqueueDecisionOutcome::Enqueued { id } => {
            format!("awaiting reply (decision #{id})")
        }
        EnqueueDecisionOutcome::BackpressureDropped { .. } => {
            "backpressure-dropped: prior request still pending".to_string()
        }
    };
    AgentReport {
        tick,
        agent_id: agent_id.to_string(),
        considered: vec![],
        chosen: None,
        rationale,
        confidence: 0.0,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::agent::MoverObservation;
    use crate::components::{MoverId, MoverState, PathId};
    use simetro_protocol::DecisionStatus;

    fn make_obs(tick: u64) -> Observation {
        Observation {
            tick,
            movers: vec![MoverObservation {
                id: MoverId(1),
                state: MoverState::Empty,
                speed: 1.0,
                home_path: PathId(0),
            }],
        }
    }

    #[test]
    fn act_enqueues_and_returns_thinking_report() {
        let (mut agent, rt) = LlmAgent::with_fresh_runtime("trafficker", 30, 60);
        let obs = make_obs(100);
        let report = agent.act(&obs).unwrap();
        assert_eq!(report.agent_id, "trafficker");
        assert_eq!(report.tick, 100);
        assert!(
            report.chosen.is_none(),
            "chosen must be None until reply drains"
        );
        assert!(report.rationale.contains("awaiting reply"));
        // Runtime now has a pending entry and an outbox request.
        let guard = rt.lock().unwrap();
        assert_eq!(guard.outbox_len(), 1);
        assert_eq!(guard.timeline().len(), 1);
        let entry = guard.timeline().iter().next().unwrap();
        assert_eq!(entry.agent_id, "trafficker");
        assert_eq!(entry.source_tick, 100);
        assert!(matches!(entry.status, DecisionStatus::Pending));
    }

    #[test]
    fn act_emits_backpressure_report_on_second_request_without_reply() {
        let (mut agent, rt) = LlmAgent::with_fresh_runtime("trafficker", 30, 60);
        let obs1 = make_obs(100);
        let _ = agent.act(&obs1).unwrap();
        // Second act before any reply drained → backpressure
        let obs2 = make_obs(130);
        let report = agent.act(&obs2).unwrap();
        assert!(report.rationale.contains("backpressure-dropped"));
        assert!(report.chosen.is_none());
        let guard = rt.lock().unwrap();
        // Only the first request was enqueued; outbox still has 1
        assert_eq!(guard.outbox_len(), 1);
        assert_eq!(
            guard.timeline().len(),
            1,
            "no second timeline entry on backpressure"
        );
    }

    #[test]
    fn id_and_interval_match_constructor() {
        let (agent, _) = LlmAgent::with_fresh_runtime("scout", 45, 90);
        assert_eq!(agent.id(), "scout");
        assert_eq!(agent.interval_ticks(), 45);
    }

    #[test]
    fn observation_json_round_trips_through_outbox() {
        let (mut agent, rt) = LlmAgent::with_fresh_runtime("trafficker", 30, 60);
        let obs = make_obs(42);
        let _ = agent.act(&obs).unwrap();
        let mut guard = rt.lock().unwrap();
        let req = guard.drain_outbox().pop().unwrap();
        // The serialized observation in the request must be parseable
        // back into Observation (it's the bridge's contract).
        let parsed: Observation = serde_json::from_str(&req.observation_json).unwrap();
        assert_eq!(parsed.tick, 42);
        assert_eq!(parsed.movers.len(), 1);
        assert_eq!(parsed.movers[0].id, MoverId(1));
    }

    #[test]
    fn two_llm_agents_share_runtime_and_get_distinct_timeline_ids() {
        let runtime = Arc::new(Mutex::new(AgentRuntime::new()));
        let mut agent_a = LlmAgent::new("a", 30, 60, Arc::clone(&runtime));
        let mut agent_b = LlmAgent::new("b", 30, 60, Arc::clone(&runtime));
        let obs = make_obs(100);
        let _ = agent_a.act(&obs).unwrap();
        let _ = agent_b.act(&obs).unwrap();
        let guard = runtime.lock().unwrap();
        assert_eq!(guard.timeline().len(), 2);
        let ids: Vec<_> = guard
            .timeline()
            .iter()
            .map(|e| (&e.agent_id, e.id))
            .collect();
        // Distinct timeline IDs, monotonic.
        assert_ne!(ids[0].1, ids[1].1);
        assert_eq!(ids[0].0, "a");
        assert_eq!(ids[1].0, "b");
    }

    #[test]
    fn report_after_apply_cycle_reflects_world() {
        // Drive a full enqueue → process_reply → apply cycle and
        // verify the timeline is updated and the runtime is ready for
        // the next decision.
        use crate::agent_runtime::ProcessReplyOutcome;
        use crate::lifecycle::AgentReply;
        use simetro_protocol::Action;

        let (mut agent, rt) = LlmAgent::with_fresh_runtime("trafficker", 30, 60);
        let obs = make_obs(100);
        let _ = agent.act(&obs).unwrap();

        let (req, timeline_id) = {
            let mut guard = rt.lock().unwrap();
            let req = guard.drain_outbox().pop().unwrap();
            let timeline_id = guard.timeline().iter().next().unwrap().id;
            (req, timeline_id)
        };

        let reply = AgentReply {
            id: req.id.clone(),
            chosen: Some(Action::NoOp),
            rationale: "did the thing".to_string(),
            confidence: 0.9,
        };
        let outcome = {
            let mut guard = rt.lock().unwrap();
            guard.process_reply(reply, 105)
        };
        match outcome {
            ProcessReplyOutcome::Apply {
                id,
                agent_id,
                chosen,
                ..
            } => {
                assert_eq!(id, timeline_id);
                assert_eq!(agent_id, "trafficker");
                assert_eq!(chosen, Some(Action::NoOp));
            }
            other => panic!("expected Apply, got {other:?}"),
        }

        // Agent can fire again on a later tick (lifecycle no longer
        // blocked).
        let report2 = agent.act(&make_obs(200)).unwrap();
        assert!(report2.rationale.contains("awaiting reply"));
        let guard = rt.lock().unwrap();
        assert_eq!(guard.timeline().len(), 2);
    }
}
