//! Agent subsystem.
//!
//! ```text
//!   ┌──────────┐  observe(World)   ┌──────────────┐
//!   │  World   │──────────────────▶│ Observation  │
//!   └──────────┘                   └──────────────┘
//!                                          │
//!                                          ▼  act()
//!                                  ┌──────────────┐
//!                                  │ AgentReport  │  ── considered, chosen,
//!                                  └──────────────┘     rationale, confidence
//!
//!   The `AgentHost` wraps `agent.act()` in `std::panic::catch_unwind`
//!   so a panicking agent never takes down the engine (agent isolation contract).
//!   Panics surface as `AgentError::Panicked`, which Step 11 will turn
//!   into a typed `SimMessage::Fault::AgentCrashed` event.
//! ```
//!
//! Built-in agents implement [`Agent`] directly. LLM-backed agents (future)
//! will be a thin wrapper that forwards observations to the bridge.

mod observation;
mod speed_tuner;

pub use observation::{MoverObservation, Observation};
pub use speed_tuner::SpeedTuner;

use std::panic::{catch_unwind, AssertUnwindSafe};

use simetro_protocol::AgentReport;

use crate::error::AgentError;
use crate::world::World;

/// Hard cap on how many alternatives an agent may include in
/// `AgentReport.considered` (agent-report cap).
pub const MAX_CONSIDERED: usize = 1000;

/// Hard cap on `AgentReport.rationale` length in characters (agent observation contract).
pub const MAX_RATIONALE_CHARS: usize = 512;

/// Behavior contract for an in-process agent.
///
/// Implementors are observed every `interval_ticks` ticks and asked to
/// produce an [`AgentReport`]. The engine never calls `observe` or
/// `act` directly — it goes through [`AgentHost::step`] so panics and
/// invalid output are caught.
pub trait Agent: Send {
    /// Stable identifier surfaced in the Inspector and AgentLog.
    fn id(&self) -> &str;

    /// Tick interval at which the engine should invoke this agent.
    /// Must be `>=1`; the loader enforces `1..=10_000`.
    fn interval_ticks(&self) -> u32;

    /// Read-only inspection of the world. Must not mutate.
    fn observe(&mut self, world: &World) -> Observation;

    /// Decide what to do. Must return a valid [`AgentReport`]; errors
    /// surface as typed [`AgentError`] (no panics, no `unwrap`).
    fn act(&mut self, obs: &Observation) -> Result<AgentReport, AgentError>;
}

/// Wraps an `Agent` with the engine's safety boundary.
///
/// - Calls `observe` then `act` once per scheduled invocation.
/// - Catches panics and converts them to `AgentError::Panicked`.
/// - Trims oversized `considered` lists and rationale strings to the
///   plan-mandated caps (agent observation contract, §13).
pub struct AgentHost {
    agent: Box<dyn Agent>,
}

impl AgentHost {
    #[must_use]
    pub fn new(agent: Box<dyn Agent>) -> Self {
        Self { agent }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.agent.id()
    }

    #[must_use]
    pub fn interval_ticks(&self) -> u32 {
        self.agent.interval_ticks()
    }

    /// True iff this agent should fire on `world.tick`.
    /// Agents fire on ticks where `tick % interval == 0`, skipping
    /// tick 0 (no work has happened yet).
    #[must_use]
    pub fn should_fire(&self, tick: u64) -> bool {
        let i = u64::from(self.interval_ticks().max(1));
        tick > 0 && tick % i == 0
    }

    /// Observe only — does **not** call `act`. Used by the tick loop
    /// to capture exactly what the agent saw so the same observation
    /// can be hashed into the AgentLog. Wraps observe in
    /// `catch_unwind`; on panic returns `Observation::default()`
    /// (the subsequent `step()` will surface the same panic as a
    /// proper `AgentError::Panicked`).
    pub fn observe_only(&mut self, world: &World) -> Observation {
        catch_unwind(AssertUnwindSafe(|| self.agent.observe(world)))
            .unwrap_or_else(|_| Observation::default())
    }

    /// Run one observe→act cycle. Panics inside the agent become
    /// `AgentError::Panicked`. Invalid output (oversized rationale or
    /// `considered`) is trimmed and a warning attached implicitly via
    /// the `chosen` / `rationale` fields.
    ///
    /// # Errors
    /// Returns `AgentError::Panicked` if the agent panicked. Other
    /// agent errors are passed through.
    pub fn step(&mut self, world: &World) -> Result<AgentReport, AgentError> {
        let agent_id = self.agent.id().to_string();

        let observe_result = catch_unwind(AssertUnwindSafe(|| self.agent.observe(world)));
        let obs = match observe_result {
            Ok(obs) => obs,
            Err(payload) => {
                return Err(AgentError::Panicked {
                    agent_id,
                    message: panic_message(payload),
                });
            }
        };

        let act_result = catch_unwind(AssertUnwindSafe(|| self.agent.act(&obs)));
        match act_result {
            Ok(Ok(mut report)) => {
                trim_report(&mut report);
                Ok(report)
            }
            Ok(Err(err)) => Err(err),
            Err(payload) => Err(AgentError::Panicked {
                agent_id,
                message: panic_message(payload),
            }),
        }
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn trim_report(report: &mut AgentReport) {
    if report.considered.len() > MAX_CONSIDERED {
        report.considered.truncate(MAX_CONSIDERED);
    }
    if report.rationale.chars().count() > MAX_RATIONALE_CHARS {
        let trimmed: String = report.rationale.chars().take(MAX_RATIONALE_CHARS).collect();
        report.rationale = trimmed;
    }
    report.confidence = report.confidence.clamp(0.0, 1.0);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use simetro_protocol::{Action, ConsideredAction};

    struct PanickingAgent;
    impl Agent for PanickingAgent {
        fn id(&self) -> &str {
            "boom"
        }
        fn interval_ticks(&self) -> u32 {
            1
        }
        fn observe(&mut self, _world: &World) -> Observation {
            Observation::default()
        }
        fn act(&mut self, _obs: &Observation) -> Result<AgentReport, AgentError> {
            panic!("kaboom!")
        }
    }

    struct PanickingObserver;
    impl Agent for PanickingObserver {
        fn id(&self) -> &str {
            "obs-boom"
        }
        fn interval_ticks(&self) -> u32 {
            1
        }
        fn observe(&mut self, _world: &World) -> Observation {
            panic!("observe blew up")
        }
        fn act(&mut self, _obs: &Observation) -> Result<AgentReport, AgentError> {
            Ok(AgentReport::default())
        }
    }

    struct OversizedAgent;
    impl Agent for OversizedAgent {
        fn id(&self) -> &str {
            "big"
        }
        fn interval_ticks(&self) -> u32 {
            1
        }
        fn observe(&mut self, _world: &World) -> Observation {
            Observation::default()
        }
        fn act(&mut self, _obs: &Observation) -> Result<AgentReport, AgentError> {
            let mut considered = Vec::with_capacity(2000);
            for _ in 0..2000 {
                considered.push(ConsideredAction {
                    action: Action::NoOp,
                    confidence: 1.5,
                });
            }
            Ok(AgentReport {
                tick: 0,
                agent_id: "big".into(),
                considered,
                chosen: Some(Action::NoOp),
                rationale: "x".repeat(2000),
                confidence: 5.0,
            })
        }
    }

    #[test]
    fn act_panic_becomes_agent_error() {
        let mut host = AgentHost::new(Box::new(PanickingAgent));
        let world = World::new(0);
        let err = host.step(&world).expect_err("should panic");
        match err {
            AgentError::Panicked { agent_id, message } => {
                assert_eq!(agent_id, "boom");
                assert!(message.contains("kaboom"), "got: {message}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn observe_panic_also_becomes_agent_error() {
        let mut host = AgentHost::new(Box::new(PanickingObserver));
        let world = World::new(0);
        let err = host.step(&world).expect_err("should panic in observe");
        assert!(matches!(err, AgentError::Panicked { .. }));
    }

    #[test]
    fn oversized_report_is_trimmed() {
        let mut host = AgentHost::new(Box::new(OversizedAgent));
        let world = World::new(0);
        let rep = host.step(&world).expect("ok");
        assert_eq!(rep.considered.len(), MAX_CONSIDERED);
        assert!(rep.rationale.chars().count() <= MAX_RATIONALE_CHARS);
        assert!(rep.confidence <= 1.0 && rep.confidence >= 0.0);
    }

    #[test]
    fn should_fire_skips_tick_zero() {
        let host = AgentHost::new(Box::new(PanickingAgent));
        assert!(!host.should_fire(0));
        assert!(host.should_fire(1));
    }

    #[test]
    fn should_fire_respects_interval() {
        struct A;
        impl Agent for A {
            fn id(&self) -> &str {
                "a"
            }
            fn interval_ticks(&self) -> u32 {
                30
            }
            fn observe(&mut self, _w: &World) -> Observation {
                Observation::default()
            }
            fn act(&mut self, _o: &Observation) -> Result<AgentReport, AgentError> {
                Ok(AgentReport::default())
            }
        }
        let host = AgentHost::new(Box::new(A));
        assert!(!host.should_fire(29));
        assert!(host.should_fire(30));
        assert!(host.should_fire(60));
        assert!(!host.should_fire(31));
    }
}
