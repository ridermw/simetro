//! # AgentRuntime (P2.A task 8 — engine half of "LlmAgent wrapper")
//!
//! Orchestrates the engine-side request/reply machinery for LLM-backed
//! agents. Owns the two state machines that must stay in sync:
//!
//! - [`RequestLifecycle`] — the spec §10.2.1 outbox/inbox state
//!   machine (pending / completed / expired / re-issue / give-up).
//! - [`DecisionTimeline`] — the user-facing, addressable, version-
//!   pinned ledger of every decision (spec §3 task 7 + §10).
//!
//! The runtime is the single place that both writes to the lifecycle
//! AND records the corresponding DecisionTimeline transition, so
//! callers can't drift the two out of sync.
//!
//! ## Layering
//!
//! ```text
//!   ┌──────────┐   enqueue/drain/expire   ┌─────────────────┐
//!   │ TickRunner│ ────────────────────────▶│  AgentRuntime  │
//!   │   (P2.A   │                          │                 │
//!   │   task 10)│ ◀──── outbox / reply ────│  • Lifecycle    │
//!   └──────────┘                           │  • Timeline     │
//!                                          └─────────────────┘
//! ```
//!
//! This PR delivers the runtime + its public API; wiring it into
//! `TickRunner::run_agents` is a follow-up PR (task 10 scene wiring).
//!
//! ## Why a separate orchestrator module
//!
//! The driver.rs taste guard from spec §3 task 8 forbids growing
//! `src-tauri/src/driver.rs`. The orchestration belongs in the engine
//! (it's pure data-flow over deterministic state). New code goes in
//! its own module so `driver.rs` stays a thin shim.

use std::collections::VecDeque;

use simetro_protocol::{
    DecisionResponse, DecisionTimeline, DecisionTimelineError, FaultPayload, SimMessage, TimelineId,
};

use crate::lifecycle::{
    AgentReply, AgentRequest, DrainOutcome, EnqueueOutcome, ExpiryOutcome, RequestId,
    RequestLifecycle,
};

/// Outcome of [`AgentRuntime::enqueue_decision`].
#[derive(Debug, Clone)]
pub enum EnqueueDecisionOutcome {
    /// Request was accepted and placed in the outbox. Carries the
    /// [`TimelineId`] the caller should use to correlate the eventual
    /// reply (e.g. when surfacing the "thinking…" placeholder in the
    /// Inspector).
    Enqueued { id: TimelineId },
    /// Backpressure: an earlier request for the same agent is still
    /// pending. The decision was NOT allocated a `TimelineId` and was
    /// NOT placed in the outbox. The caller receives the warning to
    /// emit to consumers.
    BackpressureDropped { message: SimMessage },
}

/// Outcome of [`AgentRuntime::process_reply`].
#[derive(Debug, Clone)]
pub enum ProcessReplyOutcome {
    /// On-time apply. Caller should run `chosen` through the
    /// deterministic action pipeline.
    Apply {
        id: TimelineId,
        agent_id: String,
        chosen: Option<simetro_protocol::Action>,
        rationale: String,
        confidence: f32,
    },
    /// Duplicate / stale / unknown-id reply. Caller emits `message`
    /// and applies no action. The DecisionTimeline is unchanged
    /// (duplicate / stale targets are already in their terminal state;
    /// unknown-id has no timeline entry).
    Drop { message: SimMessage },
}

/// Outcome of one expired pending request, paired with the runtime's
/// re-issue or give-up decision.
#[derive(Debug, Clone)]
pub enum ExpireOutcome {
    /// Lifecycle re-issued; runtime updated the timeline (status
    /// Expired → Pending; attempts bumped). The caller should ALSO
    /// drain the new [`AgentRequest`] from the outbox via
    /// [`AgentRuntime::drain_outbox`] — the request has already been
    /// pushed there.
    Reissued { id: TimelineId, warning: SimMessage },
    /// Re-issue cap exhausted; timeline marked GaveUp. Caller emits
    /// `message`.
    GaveUp { id: TimelineId, message: SimMessage },
}

/// Internal counter for assigning RequestId fields whose values aren't
/// the timeline_id. We track the `next attempt to enqueue` distinctly
/// from `current_tick` because tests can fire requests at arbitrary
/// ticks without advancing the runtime tick.
#[derive(Debug, Default)]
pub struct AgentRuntime {
    timeline: DecisionTimeline,
    lifecycle: RequestLifecycle,
    outbox: VecDeque<AgentRequest>,
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timeline_capacity(cap: usize) -> Self {
        Self {
            timeline: DecisionTimeline::with_capacity(cap),
            lifecycle: RequestLifecycle::new(),
            outbox: VecDeque::new(),
        }
    }

    /// Read-only view of the DecisionTimeline (e.g. for the
    /// Inspector).
    pub fn timeline(&self) -> &DecisionTimeline {
        &self.timeline
    }

    /// Read-only view of the lifecycle (mostly for tests / Inspector).
    pub fn lifecycle(&self) -> &RequestLifecycle {
        &self.lifecycle
    }

    /// How many requests are currently sitting in the outbox waiting
    /// for the bridge to consume them.
    pub fn outbox_len(&self) -> usize {
        self.outbox.len()
    }

    /// Allocate a [`TimelineId`], build an [`AgentRequest`], and push
    /// it through both the lifecycle and the outbox. If the lifecycle
    /// rejects with backpressure, the timeline allocation is **rolled
    /// back** so timeline IDs stay 1:1 with actually-issued requests.
    pub fn enqueue_decision(
        &mut self,
        agent_id: &str,
        observation_json: String,
        deadline_ticks: u32,
        current_tick: u64,
    ) -> EnqueueDecisionOutcome {
        // We allocate the TimelineId optimistically because the
        // lifecycle's backpressure check needs the request to be built.
        // If it rejects, we DO NOT mutate the timeline — and to keep
        // ids monotonic we instead use a probe pattern.
        if self.lifecycle.has_pending_for_agent(agent_id) {
            // Replicate the lifecycle's exact warning shape (it
            // computes the same lag_frames). Easier than constructing
            // a throw-away RequestId just to call try_enqueue.
            let message =
                crate::lifecycle::backpressure_warning_for(agent_id, current_tick, current_tick);
            return EnqueueDecisionOutcome::BackpressureDropped { message };
        }

        let id = self.timeline.allocate(current_tick, agent_id);
        let request = AgentRequest {
            id: RequestId {
                timeline_id: id.0,
                agent_id: agent_id.to_string(),
                source_tick: current_tick,
                attempt: 0,
            },
            deadline_ticks,
            observation_json,
        };

        match self.lifecycle.try_enqueue(request.clone(), current_tick) {
            EnqueueOutcome::Enqueued => {
                self.outbox.push_back(request);
                EnqueueDecisionOutcome::Enqueued { id }
            }
            EnqueueOutcome::BackpressureDropped { message } => {
                // Defensive: the pre-check above should have caught
                // this. If it didn't (race or future bug), at least
                // record a fault on the just-allocated entry so the
                // timeline reflects reality.
                let _ = self.timeline.record_fault(
                    id,
                    current_tick,
                    "lifecycle backpressure after timeline allocation (unexpected)",
                );
                EnqueueDecisionOutcome::BackpressureDropped { message }
            }
        }
    }

    /// Drain the outbox of all queued [`AgentRequest`]s. The caller
    /// (bridge transport) forwards them and eventually returns
    /// [`AgentReply`]s via [`process_reply`](Self::process_reply).
    pub fn drain_outbox(&mut self) -> Vec<AgentRequest> {
        self.outbox.drain(..).collect()
    }

    /// Process one reply from the inbox. Synchronizes the lifecycle
    /// drain outcome with the corresponding DecisionTimeline update.
    pub fn process_reply(&mut self, reply: AgentReply, current_tick: u64) -> ProcessReplyOutcome {
        let timeline_id = TimelineId(reply.id.timeline_id);
        let agent_id = reply.id.agent_id.clone();
        let outcome = self.lifecycle.drain_reply(reply, current_tick);
        match outcome {
            DrainOutcome::Apply {
                agent_id,
                chosen,
                rationale,
                confidence,
            } => {
                let response = DecisionResponse {
                    chosen: chosen.clone(),
                    rationale: rationale.clone(),
                    confidence,
                    latency_ms: None,
                    raw_response_ref: None,
                };
                // Best-effort: if the timeline entry was evicted
                // (sliding window) we can't record. Still apply.
                let _ = self
                    .timeline
                    .record_reply(timeline_id, current_tick, response);
                ProcessReplyOutcome::Apply {
                    id: timeline_id,
                    agent_id,
                    chosen,
                    rationale,
                    confidence,
                }
            }
            DrainOutcome::Duplicate { message }
            | DrainOutcome::Stale { message }
            | DrainOutcome::UnknownId { message } => {
                // Don't touch the timeline:
                // - Duplicate: the entry is already Applied.
                // - Stale: the entry is already GaveUp / Faulted /
                //   Applied (per the spec's expire-before-drain order).
                // - UnknownId: bridge bug; no timeline entry exists.
                let _ = agent_id; // currently unused; kept for future use
                ProcessReplyOutcome::Drop { message }
            }
        }
    }

    /// Expire all pending requests whose deadlines have passed at
    /// `current_tick`. Updates lifecycle AND timeline; re-issues
    /// (which the lifecycle returns as new `AgentRequest`s) are
    /// pushed onto the outbox automatically.
    pub fn expire_overdue(&mut self, current_tick: u64) -> Vec<ExpireOutcome> {
        let lifecycle_outcomes = self.lifecycle.expire_overdue(current_tick);
        let mut out = Vec::with_capacity(lifecycle_outcomes.len());

        for lc in lifecycle_outcomes {
            match lc {
                ExpiryOutcome::Reissue {
                    next_request,
                    warning,
                } => {
                    let timeline_id = TimelineId(next_request.id.timeline_id);
                    // Sync timeline: pending → expired → reissue (so
                    // attempts bumps and status returns to Pending).
                    let _ = self.timeline.record_expiry(
                        timeline_id,
                        current_tick,
                        warning_payload(&warning).cloned(),
                    );
                    let _ = self.timeline.reissue(timeline_id);
                    // Re-enqueue at the lifecycle with the new
                    // current_tick so deadline_abs rebases (per PR #12
                    // R2 Codex P1 fix). Push the request to the outbox
                    // for the bridge to pick up.
                    match self
                        .lifecycle
                        .try_enqueue(next_request.clone(), current_tick)
                    {
                        EnqueueOutcome::Enqueued => {
                            self.outbox.push_back(next_request);
                        }
                        EnqueueOutcome::BackpressureDropped { .. } => {
                            // Defensive: an expire-then-reenqueue
                            // shouldn't backpressure (the original was
                            // just removed from pending). Record a
                            // fault on the timeline entry rather than
                            // silently swallowing.
                            let _ = self.timeline.record_fault(
                                timeline_id,
                                current_tick,
                                "lifecycle backpressure after re-issue (unexpected)",
                            );
                        }
                    }
                    out.push(ExpireOutcome::Reissued {
                        id: timeline_id,
                        warning,
                    });
                }
                ExpiryOutcome::GiveUp { message } => {
                    let timeline_id = give_up_target(&message)
                        .and_then(|tid| self.find_pending_id_for_agent(&tid));
                    let id = if let Some(id) = timeline_id {
                        let _ = self.timeline.record_expiry(
                            id,
                            current_tick,
                            warning_payload(&message).cloned(),
                        );
                        let _ = self.timeline.give_up(
                            id,
                            current_tick,
                            warning_payload(&message).cloned(),
                        );
                        id
                    } else {
                        TimelineId(0)
                    };
                    out.push(ExpireOutcome::GaveUp { id, message });
                }
            }
        }
        out
    }

    /// Record an external fault on a known timeline entry. Used when
    /// the bridge reports a non-retriable error (e.g. parse failure
    /// after MAX_ATTEMPTS exhausted, or an engine-side panic in the
    /// drain pipeline).
    pub fn record_fault(
        &mut self,
        id: TimelineId,
        fault_tick: u64,
        reason: impl Into<String>,
    ) -> Result<(), DecisionTimelineError> {
        self.timeline.record_fault(id, fault_tick, reason)
    }

    fn find_pending_id_for_agent(&self, agent_id: &str) -> Option<TimelineId> {
        // The lifecycle's GiveUp message carries only agent_id, not
        // the original RequestId. Find the timeline entry currently
        // Pending|Expired for that agent and use it. The one-
        // outstanding-per-agent backpressure invariant guarantees at
        // most one such entry exists.
        self.timeline
            .iter()
            .find(|entry| {
                entry.agent_id == agent_id
                    && matches!(
                        entry.status,
                        simetro_protocol::DecisionStatus::Pending
                            | simetro_protocol::DecisionStatus::Expired { .. }
                    )
            })
            .map(|e| e.id)
    }
}

/// Extract `agent_id` from a give-up [`SimMessage`] (a
/// `Warning::InvalidAction { agent_id, reason: "max attempts ..." }`).
fn give_up_target(msg: &SimMessage) -> Option<String> {
    if let SimMessage::Warning(simetro_protocol::WarningPayload::InvalidAction {
        agent_id, ..
    }) = msg
    {
        Some(agent_id.clone())
    } else {
        None
    }
}

/// Extract the `WarningPayload` from a `SimMessage::Warning` (for
/// recording on a timeline entry).
fn warning_payload(msg: &SimMessage) -> Option<&simetro_protocol::WarningPayload> {
    match msg {
        SimMessage::Warning(w) => Some(w),
        _ => None,
    }
}

// Used by record_fault docs / convention; not currently emitted by
// AgentRuntime itself but kept here so callers needing to convert an
// error back into a SimMessage stay consistent with §10.2.1.
#[doc(hidden)]
pub fn fault_message_for(reason: impl Into<String>) -> SimMessage {
    SimMessage::Fault(FaultPayload::EngineFault {
        message: reason.into(),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use simetro_protocol::{Action, DecisionStatus, WarningPayload};

    fn reply_for(req: &AgentRequest, chosen: Option<Action>) -> AgentReply {
        AgentReply {
            id: req.id.clone(),
            chosen,
            rationale: "did the thing".to_string(),
            confidence: 0.75,
        }
    }

    #[test]
    fn enqueue_decision_allocates_timeline_id_and_pushes_outbox() {
        let mut rt = AgentRuntime::new();
        let out = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        let id = match out {
            EnqueueDecisionOutcome::Enqueued { id } => id,
            other => panic!("expected Enqueued, got {other:?}"),
        };
        assert_eq!(id, TimelineId(1));
        assert_eq!(rt.outbox_len(), 1);
        assert_eq!(rt.timeline().len(), 1);
        let entry = rt.timeline().get(id).unwrap();
        assert!(matches!(entry.status, DecisionStatus::Pending));
        assert_eq!(entry.attempts, 1);
        assert_eq!(entry.source_tick, 100);
    }

    #[test]
    fn enqueue_decision_for_same_agent_returns_backpressure_no_alloc() {
        let mut rt = AgentRuntime::new();
        let _ = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        let out = rt.enqueue_decision("agent-a", "{}".into(), 5, 101);
        assert!(matches!(
            out,
            EnqueueDecisionOutcome::BackpressureDropped { .. }
        ));
        // No second timeline allocation
        assert_eq!(rt.timeline().len(), 1);
        assert_eq!(rt.outbox_len(), 1);
    }

    #[test]
    fn drain_outbox_yields_all_requests_in_fifo_order() {
        let mut rt = AgentRuntime::new();
        let _ = rt.enqueue_decision("a", "{\"a\":1}".into(), 5, 10);
        let _ = rt.enqueue_decision("b", "{\"b\":2}".into(), 5, 10);
        let _ = rt.enqueue_decision("c", "{\"c\":3}".into(), 5, 10);
        let reqs = rt.drain_outbox();
        assert_eq!(reqs.len(), 3);
        assert_eq!(reqs[0].id.agent_id, "a");
        assert_eq!(reqs[1].id.agent_id, "b");
        assert_eq!(reqs[2].id.agent_id, "c");
        // Outbox is empty after drain
        assert_eq!(rt.outbox_len(), 0);
    }

    #[test]
    fn process_reply_on_time_applies_and_records_timeline() {
        let mut rt = AgentRuntime::new();
        let _ = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        let req = rt.drain_outbox().pop().unwrap();
        let reply = reply_for(&req, Some(Action::NoOp));
        let outcome = rt.process_reply(reply, 102);
        let (id, agent_id, chosen) = match outcome {
            ProcessReplyOutcome::Apply {
                id,
                agent_id,
                chosen,
                ..
            } => (id, agent_id, chosen),
            other => panic!("expected Apply, got {other:?}"),
        };
        assert_eq!(id, TimelineId(1));
        assert_eq!(agent_id, "agent-a");
        assert_eq!(chosen, Some(Action::NoOp));
        let entry = rt.timeline().get(id).unwrap();
        assert!(matches!(
            entry.status,
            DecisionStatus::Applied { applied_tick: 102 }
        ));
        let resp = entry.response.as_ref().unwrap();
        assert_eq!(resp.chosen, Some(Action::NoOp));
        assert_eq!(resp.confidence, 0.75);
        assert_eq!(resp.rationale, "did the thing");
    }

    #[test]
    fn process_reply_stale_drops_no_timeline_mutation() {
        let mut rt = AgentRuntime::new();
        let _ = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        let req = rt.drain_outbox().pop().unwrap();
        // Expire first (deadline=105, expire at 200)
        let _ = rt.expire_overdue(200);
        // After expire+reissue, timeline status went Expired -> Pending again with attempts=2.
        // The stale reply has attempt=0 still; it'll hit Stale.
        let reply = reply_for(&req, Some(Action::NoOp));
        let outcome = rt.process_reply(reply, 210);
        assert!(matches!(outcome, ProcessReplyOutcome::Drop { .. }));
        // The reissue should have left the timeline entry in Pending
        // (attempts=2), NOT clobbered by the stale reply.
        let entry = rt.timeline().get(TimelineId(1)).unwrap();
        assert_eq!(entry.attempts, 2);
        assert!(matches!(entry.status, DecisionStatus::Pending));
    }

    #[test]
    fn expire_overdue_reissues_pushes_to_outbox_and_bumps_attempts() {
        let mut rt = AgentRuntime::new();
        let _ = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        // drain the original from outbox so we can see the reissue
        let _ = rt.drain_outbox();
        let outcomes = rt.expire_overdue(200);
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            ExpireOutcome::Reissued { id, warning } => {
                assert_eq!(*id, TimelineId(1));
                assert!(matches!(
                    warning,
                    SimMessage::Warning(WarningPayload::Behind { .. })
                ));
            }
            other => panic!("expected Reissued, got {other:?}"),
        }
        // Outbox now contains the re-issued request
        assert_eq!(rt.outbox_len(), 1);
        let reissued = rt.drain_outbox().pop().unwrap();
        assert_eq!(reissued.id.attempt, 1);
        assert_eq!(reissued.id.timeline_id, 1);
        // Timeline: attempts bumped to 2 (1 original + 1 reissue), status Pending
        let entry = rt.timeline().get(TimelineId(1)).unwrap();
        assert_eq!(entry.attempts, 2);
        assert!(matches!(entry.status, DecisionStatus::Pending));
        assert_eq!(entry.last_expired_tick, Some(200));
    }

    #[test]
    fn expire_overdue_gives_up_after_max_attempts() {
        let mut rt = AgentRuntime::new();
        let _ = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        let _ = rt.drain_outbox();
        // attempt 0 expires → reissue to attempt 1
        let _ = rt.expire_overdue(200);
        let _ = rt.drain_outbox();
        // attempt 1 expires → reissue to attempt 2
        let _ = rt.expire_overdue(300);
        let _ = rt.drain_outbox();
        // attempt 2 expires → give up (MAX_ATTEMPTS=2; spec literal `attempt < MAX_ATTEMPTS` fails)
        let outcomes = rt.expire_overdue(400);
        assert_eq!(outcomes.len(), 1);
        let (id, _msg) = match &outcomes[0] {
            ExpireOutcome::GaveUp { id, message } => (*id, message.clone()),
            other => panic!("expected GaveUp, got {other:?}"),
        };
        assert_eq!(id, TimelineId(1));
        let entry = rt.timeline().get(id).unwrap();
        assert!(matches!(
            entry.status,
            DecisionStatus::GaveUp { last_tick: 400 }
        ));
        assert_eq!(entry.attempts, 3); // 1 + 2 reissues
    }

    #[test]
    fn full_request_reply_cycle_end_to_end() {
        let mut rt = AgentRuntime::new();
        // Two agents fire on the same tick
        let out_a = rt.enqueue_decision("agent-a", "{\"obs\":\"a\"}".into(), 5, 100);
        let out_b = rt.enqueue_decision("agent-b", "{\"obs\":\"b\"}".into(), 5, 100);
        let id_a = match out_a {
            EnqueueDecisionOutcome::Enqueued { id } => id,
            _ => panic!(),
        };
        let id_b = match out_b {
            EnqueueDecisionOutcome::Enqueued { id } => id,
            _ => panic!(),
        };
        let reqs = rt.drain_outbox();
        assert_eq!(reqs.len(), 2);
        // Bridge replies for B first, then A
        let reply_b = reply_for(&reqs[1], Some(Action::NoOp));
        let reply_a = reply_for(&reqs[0], Some(Action::NoOp));
        let _ = rt.process_reply(reply_b, 102);
        let _ = rt.process_reply(reply_a, 103);
        // Both Applied
        assert!(matches!(
            rt.timeline().get(id_a).unwrap().status,
            DecisionStatus::Applied { applied_tick: 103 }
        ));
        assert!(matches!(
            rt.timeline().get(id_b).unwrap().status,
            DecisionStatus::Applied { applied_tick: 102 }
        ));
    }

    #[test]
    fn process_reply_unknown_id_drops_without_timeline_corruption() {
        let mut rt = AgentRuntime::new();
        // No requests enqueued — reply for a phantom id.
        let phantom_reply = AgentReply {
            id: RequestId {
                timeline_id: 99,
                agent_id: "ghost".into(),
                source_tick: 50,
                attempt: 0,
            },
            chosen: Some(Action::NoOp),
            rationale: "ghost".into(),
            confidence: 1.0,
        };
        let outcome = rt.process_reply(phantom_reply, 100);
        assert!(matches!(outcome, ProcessReplyOutcome::Drop { .. }));
        // Timeline still empty
        assert_eq!(rt.timeline().len(), 0);
    }

    #[test]
    fn timeline_records_last_warning_on_expiry() {
        let mut rt = AgentRuntime::new();
        let _ = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        let _ = rt.drain_outbox();
        let _ = rt.expire_overdue(200);
        let entry = rt.timeline().get(TimelineId(1)).unwrap();
        // Reissued back to Pending, but last_warning preserved from
        // the expiry step.
        assert!(matches!(entry.status, DecisionStatus::Pending));
        assert!(entry.last_warning.is_some());
    }

    #[test]
    fn after_give_up_subsequent_enqueue_for_same_agent_succeeds() {
        let mut rt = AgentRuntime::new();
        let _ = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        // Drive through 3 expirations to give-up
        let _ = rt.drain_outbox();
        let _ = rt.expire_overdue(200);
        let _ = rt.drain_outbox();
        let _ = rt.expire_overdue(300);
        let _ = rt.drain_outbox();
        let _ = rt.expire_overdue(400);
        // Now agent should be able to enqueue again (no pending entry
        // for agent-a in lifecycle).
        let out = rt.enqueue_decision("agent-a", "{}".into(), 5, 500);
        match out {
            EnqueueDecisionOutcome::Enqueued { id } => {
                assert_eq!(id, TimelineId(2)); // fresh id
            }
            other => panic!("expected Enqueued, got {other:?}"),
        }
    }

    #[test]
    fn record_fault_marks_timeline_entry_faulted() {
        let mut rt = AgentRuntime::new();
        let out = rt.enqueue_decision("agent-a", "{}".into(), 5, 100);
        let id = match out {
            EnqueueDecisionOutcome::Enqueued { id } => id,
            _ => panic!(),
        };
        rt.record_fault(id, 110, "bridge parse failure").unwrap();
        let entry = rt.timeline().get(id).unwrap();
        assert!(matches!(
            entry.status,
            DecisionStatus::Faulted {
                fault_tick: 110,
                ..
            }
        ));
    }

    #[test]
    fn with_timeline_capacity_caps_entries_but_keeps_monotonic_ids() {
        let mut rt = AgentRuntime::with_timeline_capacity(2);
        // Enqueue 3 decisions for 3 different agents (different agents
        // avoid backpressure)
        let id1 = match rt.enqueue_decision("a", "{}".into(), 5, 10) {
            EnqueueDecisionOutcome::Enqueued { id } => id,
            _ => panic!(),
        };
        let _ = rt.enqueue_decision("b", "{}".into(), 5, 10);
        let id3 = match rt.enqueue_decision("c", "{}".into(), 5, 10) {
            EnqueueDecisionOutcome::Enqueued { id } => id,
            _ => panic!(),
        };
        // id1 evicted from timeline (cap=2). Still readable via was_evicted.
        assert!(rt.timeline().was_evicted(id1));
        assert_eq!(rt.timeline().len(), 2);
        // id3 retained
        assert!(rt.timeline().get(id3).is_some());
    }
}
