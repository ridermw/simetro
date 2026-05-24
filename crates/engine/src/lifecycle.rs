//! P2.A task 5: outbox/inbox boundary with formal request lifecycle.
//!
//! Spec source of truth: `docs/superpowers/specs/2026-05-24-post-pr3-roadmap-design.md`
//! §10.2.1 "Request lifecycle (formal model)".
//!
//! ## What this module does
//!
//! Provides the **engine-side** plumbing for the async LLM boundary:
//!
//! - [`RequestId`] uniquely identifies one decision attempt, keyed by
//!   `{timeline_id, agent_id, source_tick, attempt}` so a re-issued
//!   request (attempt+=1) is distinct from the original.
//! - [`AgentRequest`] is what the engine enqueues to the outbox; the
//!   bridge process drains it asynchronously.
//! - [`AgentReply`] is what the bridge writes to the inbox; the
//!   engine drains it at tick boundaries (deterministic order).
//! - [`RequestLifecycle`] is the per-engine-run state machine: it
//!   tracks `pending` / `completed` / `expired` request IDs and
//!   exposes drain-time rules so the engine can apply on-time
//!   replies, reject duplicates + stale replies, and re-issue
//!   timeouts.
//!
//! ## What this module does NOT do
//!
//! - **No wire protocol.** [`AgentRequest`]/[`AgentReply`] are
//!   in-process types. P2.A task 6 (bridge process split) serializes
//!   them over framed JSON.
//! - **No `LlmAgent`.** P2.A task 8 wires the `LlmAgent` engine wrapper
//!   that PRODUCES `AgentRequest` via this lifecycle and CONSUMES
//!   replies. This module's tests use a `MockReplyChannel` instead.
//! - **No `Action` apply.** The lifecycle reports what should happen
//!   (apply / drop with warning / fault) via the [`DrainOutcome`]
//!   return type; the caller is responsible for actually applying
//!   the `Action` via the engine's deterministic action pipeline.
//!
//! ## Determinism invariants (spec §10.2.1)
//!
//! - All state transitions are keyed to ENGINE TICKS and stable
//!   AGENT IDs, never to wall-clock arrival order. Two runs of the
//!   same scene + seed produce identical `pending`/`completed`/
//!   `expired` transition sequences.
//! - One-outstanding-per-agent backpressure: emitting a second
//!   request for an agent while one is in `pending` produces a
//!   deterministic `Warning::Behind` without enqueuing.
//! - Re-issues bump `attempt` so a late reply for `attempt=0` is
//!   structurally distinct from a fresh `attempt=1`.

use std::collections::{HashMap, VecDeque};

use simetro_protocol::{Action, FaultPayload, SimMessage, WarningPayload};

/// Unique identifier for one decision attempt. Per spec §10.2.1.
///
/// The four fields together pin a request to a specific
/// (decision, agent, tick, attempt) tuple. A re-issued request bumps
/// `attempt` so it's distinct from the original at the type level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestId {
    /// Monotonic-per-engine-run identifier from the DecisionTimeline
    /// (P2.A task 7 will produce these). Distinguishes one logical
    /// decision from another. Never reused.
    pub timeline_id: u64,
    /// Stable agent identifier (matches `AgentHost::id()`).
    pub agent_id: String,
    /// Engine tick at which the request was first emitted (attempt=0).
    /// Re-issues KEEP the same `source_tick` so the lifecycle can
    /// compute total lag from the original emission.
    pub source_tick: u64,
    /// 0-indexed attempt number. 0 = original; 1+ = re-issued after
    /// the previous attempt expired without reply.
    pub attempt: u32,
}

impl RequestId {
    /// Build the next-attempt version of `self`. Preserves
    /// `timeline_id`, `agent_id`, and `source_tick`; increments
    /// `attempt`.
    #[must_use]
    pub fn next_attempt(&self) -> Self {
        Self {
            timeline_id: self.timeline_id,
            agent_id: self.agent_id.clone(),
            source_tick: self.source_tick,
            attempt: self.attempt.saturating_add(1),
        }
    }
}

/// A request the engine emits to the outbox for the bridge to
/// fulfill. The bridge reads this, calls the LLM, and writes back
/// an [`AgentReply`] containing the chosen [`Action`].
///
/// The `observation_json` is the engine's serialized `Observation`
/// — opaque to the lifecycle module; the bridge interprets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRequest {
    pub id: RequestId,
    /// Engine-relative deadline (in ticks) by which a reply must be
    /// drained or the request expires. Absolute tick is computed at
    /// [`RequestLifecycle::try_enqueue`] time as
    /// `current_tick + deadline_ticks` — NOT
    /// `source_tick + deadline_ticks` — so re-issued requests get a
    /// fresh deadline window rather than inheriting an already-overdue
    /// one. Per `(world.dt * deadline_ticks)` real-time.
    pub deadline_ticks: u32,
    /// Serialized observation. Opaque to the engine; bridge converts
    /// to whatever shape the backend wants. Stored as bytes so the
    /// lifecycle module has no dependency on the observation type.
    pub observation_json: String,
}

/// A reply the bridge writes back to the inbox. Carries the chosen
/// action (or `None` for an error reply — equivalent to NoOp at the
/// apply layer).
#[derive(Debug, Clone, PartialEq)]
pub struct AgentReply {
    pub id: RequestId,
    /// `None` ⇒ apply [`Action::NoOp`]; `Some(a)` ⇒ apply `a` via
    /// the deterministic action pipeline.
    pub chosen: Option<Action>,
    /// Free-form rationale, ≤ engine cap (see
    /// `simetro_engine::agent::MAX_RATIONALE_CHARS`). The lifecycle
    /// passes this through; it's the caller's job to log it (e.g.
    /// to AgentLog v2).
    pub rationale: String,
    /// Bridge-reported confidence ∈ [0, 1]. Defaults to 1.0 in error
    /// paths.
    pub confidence: f32,
}

/// Maximum re-issues per decision. Total attempts allowed =
/// `MAX_ATTEMPTS + 1` (the original `attempt=0` plus `MAX_ATTEMPTS`
/// re-issues). With `MAX_ATTEMPTS = 2` the lifecycle accepts
/// attempts `0, 1, 2` before giving up. This matches spec §10.2.1's
/// formula `if attempt < MAX_ATTEMPTS, enqueue a new outbox entry
/// with attempt += 1`.
///
/// Hard-coded for now; can be made configurable per scene in a later
/// PR.
pub const MAX_ATTEMPTS: u32 = 2;

/// Size of the `completed` ring buffer. Larger means we remember
/// more historical request IDs for dedup; smaller means a late
/// duplicate reply MAY squeeze through as "unknown id" instead of
/// being deduped. 1024 should be plenty for typical multi-agent
/// scenes (decisions land every few hundred ticks).
pub const COMPLETED_RING_CAP: usize = 1024;

/// What the lifecycle determined should happen with a drained reply
/// or expired request. The caller (engine `TickRunner`) inspects
/// this and acts: apply the action, emit the warning, re-issue the
/// request, etc.
///
/// All outcomes are DETERMINISTIC — they depend only on the request
/// state and the current tick, never on wall-clock timing.
#[derive(Debug, Clone)]
pub enum DrainOutcome {
    /// On-time reply. Caller should apply `chosen` (or NoOp if None)
    /// via the deterministic action pipeline.
    Apply {
        agent_id: String,
        chosen: Option<Action>,
        rationale: String,
        confidence: f32,
    },
    /// Duplicate reply (request ID was already in `completed`).
    /// Caller emits the [`SimMessage`] and drops the reply.
    Duplicate { message: SimMessage },
    /// Stale post-expiry reply. Caller emits the [`SimMessage`] and
    /// drops the reply (does not mutate world).
    Stale { message: SimMessage },
    /// Reply for a request ID the lifecycle has never seen. Spec
    /// §10.2.1 calls this an engine fault — should never happen under
    /// a correct bridge.
    UnknownId { message: SimMessage },
}

/// What happened when the lifecycle expired a pending request that
/// hit its deadline. Caller acts on this to either re-issue or give
/// up.
#[derive(Debug, Clone)]
pub enum ExpiryOutcome {
    /// Re-issue the request with `attempt += 1`. Caller enqueues the
    /// new `AgentRequest` to the outbox AND emits the warning that
    /// notifies the operator of the lag.
    Reissue {
        next_request: AgentRequest,
        warning: SimMessage,
    },
    /// Max attempts exceeded; give up on this decision. Caller emits
    /// the warning; no re-issue.
    GiveUp { message: SimMessage },
}

/// What happened when the lifecycle tried to enqueue a new request.
#[derive(Debug, Clone)]
pub enum EnqueueOutcome {
    /// Request accepted into the outbox; lifecycle is now tracking
    /// it in `pending`.
    Enqueued,
    /// Backpressure: an earlier request for the same agent is still
    /// in `pending`. Caller emits the warning; the second request is
    /// dropped (NOT enqueued; the engine will try again next agent
    /// interval).
    BackpressureDropped { message: SimMessage },
}

/// Per-engine-run state machine implementing the spec §10.2.1
/// drain-time rules. Holds three sets of request IDs:
///
/// - `pending` — emitted to outbox; not yet drained from inbox; not
///   yet expired. Keyed by full `RequestId` so re-issues coexist
///   with originals (the original is moved to `expired` first).
/// - `completed` — replies that were drained on-time. Bounded ring
///   so memory stays linear in scene runtime.
/// - `expired` — requests that hit their deadline and were either
///   re-issued or given up on. Bounded ring.
///
/// The lifecycle is intentionally INDEPENDENT of `TickRunner` and
/// `AgentHost` so it can be tested with a mock reply channel. P2.A
/// task 8 will wire it into `TickRunner`.
#[derive(Debug, Default)]
pub struct RequestLifecycle {
    pending: HashMap<RequestId, PendingEntry>,
    completed: VecDeque<RequestId>,
    expired: VecDeque<RequestId>,
    completed_cap: usize,
    expired_cap: usize,
}

#[derive(Debug, Clone)]
struct PendingEntry {
    /// Absolute tick by which a reply must arrive
    /// (`enqueue_tick + deadline_ticks`).
    deadline_abs: u64,
    /// The full request so we can reconstruct it on re-issue.
    request: AgentRequest,
}

impl RequestLifecycle {
    /// Build a lifecycle with the default ring sizes
    /// ([`COMPLETED_RING_CAP`]).
    #[must_use]
    pub fn new() -> Self {
        Self::with_caps(COMPLETED_RING_CAP, COMPLETED_RING_CAP)
    }

    /// Build a lifecycle with custom ring sizes (used by tests to
    /// exercise wrap-around behavior).
    #[must_use]
    pub fn with_caps(completed_cap: usize, expired_cap: usize) -> Self {
        Self {
            pending: HashMap::new(),
            completed: VecDeque::with_capacity(completed_cap.min(64)),
            expired: VecDeque::with_capacity(expired_cap.min(64)),
            completed_cap,
            expired_cap,
        }
    }

    /// Returns true iff the agent currently has an outstanding
    /// request in `pending`. Used by the engine to decide whether
    /// to enqueue a new request or apply backpressure.
    #[must_use]
    pub fn has_pending_for_agent(&self, agent_id: &str) -> bool {
        self.pending.keys().any(|id| id.agent_id == agent_id)
    }

    /// Number of requests currently in `pending`.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Try to enqueue a new request. If another request for the same
    /// agent is already `pending`, returns [`EnqueueOutcome::BackpressureDropped`]
    /// per spec §10.2.1 ("one-outstanding-per-agent backpressure").
    ///
    /// **Deadline semantics.** `deadline_abs` is computed from
    /// `current_tick + deadline_ticks`, NOT `source_tick + deadline_ticks`.
    /// This matters for re-issued requests (see
    /// [`expire_overdue`](Self::expire_overdue)): a re-issue preserves
    /// `source_tick` for stable identity, but the deadline rebases to
    /// the re-issue tick so the bridge gets a real chance to respond.
    /// Spec §10.2.1 wording is "tick N + k where k is the configured
    /// deadline" — N is the issue tick (which equals `current_tick`),
    /// not the original `source_tick`.
    pub fn try_enqueue(&mut self, request: AgentRequest, current_tick: u64) -> EnqueueOutcome {
        if self.has_pending_for_agent(&request.id.agent_id) {
            // Backpressure-dropped lag: how far behind real time the
            // dropped request would have been. Clamped to ≥1 for the
            // same reason `llm_error_to_message` clamps Behind variants.
            let lag_frames = current_tick.saturating_sub(request.id.source_tick).max(1) as u32;
            return EnqueueOutcome::BackpressureDropped {
                message: SimMessage::Warning(WarningPayload::Behind {
                    lag_frames,
                    agent_id: Some(request.id.agent_id.clone()),
                }),
            };
        }
        let deadline_abs = current_tick.saturating_add(u64::from(request.deadline_ticks));
        let id = request.id.clone();
        self.pending.insert(
            id,
            PendingEntry {
                deadline_abs,
                request,
            },
        );
        EnqueueOutcome::Enqueued
    }

    /// Drain ONE reply through the lifecycle's drain-time rules.
    /// Returns the [`DrainOutcome`] describing what the caller
    /// should do with the reply.
    ///
    /// **Deadline enforcement.** Per spec §10.2.1, the on-time apply
    /// rule requires BOTH `reply.id ∈ pending` AND
    /// `current_tick ≤ deadline`. If a reply arrives for a request
    /// whose deadline has passed but `expire_overdue` has not yet
    /// been called this tick, this function still rejects the apply
    /// via the `Stale` outcome AND moves the entry to `expired` so
    /// a subsequent `expire_overdue` doesn't double-process it.
    pub fn drain_reply(&mut self, reply: AgentReply, current_tick: u64) -> DrainOutcome {
        let agent_id = reply.id.agent_id.clone();

        // Order matters per spec §10.2.1:
        // 1. Duplicate?
        if self.completed.contains(&reply.id) {
            return DrainOutcome::Duplicate {
                message: SimMessage::Warning(WarningPayload::InvalidAction {
                    agent_id,
                    reason: "duplicate reply".to_string(),
                }),
            };
        }
        // 2. Stale (post-expiry)?
        if self.expired.contains(&reply.id) {
            let lag_frames = current_tick.saturating_sub(reply.id.source_tick) as u32;
            return DrainOutcome::Stale {
                message: SimMessage::Warning(WarningPayload::Behind {
                    lag_frames: lag_frames.max(1),
                    agent_id: Some(agent_id),
                }),
            };
        }
        // 3. On-time apply (must satisfy both pending-membership AND
        //    deadline-not-yet-passed)?
        if let Some(entry) = self.pending.remove(&reply.id) {
            if current_tick <= entry.deadline_abs {
                // On-time: move to completed.
                push_ring(&mut self.completed, reply.id.clone(), self.completed_cap);
                return DrainOutcome::Apply {
                    agent_id,
                    chosen: reply.chosen,
                    rationale: reply.rationale,
                    confidence: reply.confidence,
                };
            }
            // Pending but past deadline — same disposition as the
            // stale-after-expiry branch above, but we also move the
            // entry to `expired` ourselves so a later
            // `expire_overdue(current_tick)` won't re-process it.
            push_ring(&mut self.expired, reply.id.clone(), self.expired_cap);
            let lag_frames = current_tick.saturating_sub(reply.id.source_tick) as u32;
            return DrainOutcome::Stale {
                message: SimMessage::Warning(WarningPayload::Behind {
                    lag_frames: lag_frames.max(1),
                    agent_id: Some(agent_id),
                }),
            };
        }
        // 4. Unknown ID — bridge bug.
        DrainOutcome::UnknownId {
            message: SimMessage::Fault(FaultPayload::EngineFault {
                message: format!(
                    "unknown request id: timeline_id={} agent_id={} source_tick={} attempt={}",
                    reply.id.timeline_id, reply.id.agent_id, reply.id.source_tick, reply.id.attempt
                ),
            }),
        }
    }

    /// Walk `pending` and move any entries past their deadline to
    /// `expired`. Returns one [`ExpiryOutcome`] per expired entry,
    /// in stable agent-ID + timeline-ID order so the sequence is
    /// deterministic.
    pub fn expire_overdue(&mut self, current_tick: u64) -> Vec<ExpiryOutcome> {
        // Collect-then-process so we don't mutate while iterating.
        // Sort by (agent_id, timeline_id, attempt) for stable order.
        let mut overdue: Vec<RequestId> = self
            .pending
            .iter()
            .filter(|(_, e)| e.deadline_abs < current_tick)
            .map(|(id, _)| id.clone())
            .collect();
        overdue.sort_by(|a, b| {
            a.agent_id
                .cmp(&b.agent_id)
                .then(a.timeline_id.cmp(&b.timeline_id))
                .then(a.attempt.cmp(&b.attempt))
        });

        let mut outcomes = Vec::with_capacity(overdue.len());
        for id in overdue {
            let Some(entry) = self.pending.remove(&id) else {
                continue;
            };
            // Move the expired ID to the expired ring.
            push_ring(&mut self.expired, id.clone(), self.expired_cap);
            let lag_frames = current_tick.saturating_sub(id.source_tick) as u32;
            let lag_frames = lag_frames.max(1);

            if id.attempt < MAX_ATTEMPTS {
                // Re-issue with attempt+=1.
                let mut next = entry.request.clone();
                next.id = id.next_attempt();
                outcomes.push(ExpiryOutcome::Reissue {
                    next_request: next,
                    warning: SimMessage::Warning(WarningPayload::Behind {
                        lag_frames,
                        agent_id: Some(id.agent_id.clone()),
                    }),
                });
            } else {
                // Max attempts reached.
                outcomes.push(ExpiryOutcome::GiveUp {
                    message: SimMessage::Warning(WarningPayload::InvalidAction {
                        agent_id: id.agent_id.clone(),
                        reason: format!(
                            "max attempts ({MAX_ATTEMPTS}) exceeded; giving up on decision \
                             (timeline_id={}, last attempt={})",
                            id.timeline_id, id.attempt
                        ),
                    }),
                });
            }
        }
        outcomes
    }
}

fn push_ring(ring: &mut VecDeque<RequestId>, id: RequestId, cap: usize) {
    if ring.len() >= cap {
        ring.pop_front();
    }
    ring.push_back(id);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use simetro_protocol::Action;

    fn req(agent: &str, timeline: u64, source: u64, attempt: u32, deadline: u32) -> AgentRequest {
        AgentRequest {
            id: RequestId {
                timeline_id: timeline,
                agent_id: agent.to_string(),
                source_tick: source,
                attempt,
            },
            deadline_ticks: deadline,
            observation_json: "{}".into(),
        }
    }

    fn reply_for(req: &AgentRequest, chosen: Option<Action>) -> AgentReply {
        AgentReply {
            id: req.id.clone(),
            chosen,
            rationale: String::new(),
            confidence: 1.0,
        }
    }

    // ---- enqueue + backpressure ----

    #[test]
    fn enqueue_accepts_first_request_per_agent() {
        let mut life = RequestLifecycle::new();
        let outcome = life.try_enqueue(req("agent-a", 1, 100, 0, 10), 100);
        assert!(matches!(outcome, EnqueueOutcome::Enqueued));
        assert!(life.has_pending_for_agent("agent-a"));
        assert_eq!(life.pending_count(), 1);
    }

    #[test]
    fn enqueue_drops_second_request_for_same_agent_with_backpressure_warning() {
        let mut life = RequestLifecycle::new();
        life.try_enqueue(req("agent-a", 1, 100, 0, 10), 100);
        let outcome = life.try_enqueue(req("agent-a", 2, 101, 0, 10), 101);
        match outcome {
            EnqueueOutcome::BackpressureDropped { message } => {
                assert!(matches!(
                    message,
                    SimMessage::Warning(WarningPayload::Behind {
                        agent_id: Some(_),
                        ..
                    })
                ));
            }
            other => panic!("expected BackpressureDropped, got {other:?}"),
        }
        assert_eq!(
            life.pending_count(),
            1,
            "second request should NOT be enqueued"
        );
    }

    #[test]
    fn enqueue_allows_independent_agents_in_parallel() {
        let mut life = RequestLifecycle::new();
        assert!(matches!(
            life.try_enqueue(req("agent-a", 1, 100, 0, 10), 100),
            EnqueueOutcome::Enqueued
        ));
        assert!(matches!(
            life.try_enqueue(req("agent-b", 2, 100, 0, 10), 100),
            EnqueueOutcome::Enqueued
        ));
        assert_eq!(life.pending_count(), 2);
    }

    // ---- drain: on-time apply ----

    #[test]
    fn drain_on_time_reply_returns_apply_and_moves_to_completed() {
        let mut life = RequestLifecycle::new();
        let req_a = req("agent-a", 1, 100, 0, 10);
        life.try_enqueue(req_a.clone(), 100);

        let outcome = life.drain_reply(reply_for(&req_a, Some(Action::NoOp)), 105);
        match outcome {
            DrainOutcome::Apply {
                agent_id, chosen, ..
            } => {
                assert_eq!(agent_id, "agent-a");
                assert_eq!(chosen, Some(Action::NoOp));
            }
            other => panic!("expected Apply, got {other:?}"),
        }
        assert_eq!(
            life.pending_count(),
            0,
            "on-time apply must remove from pending"
        );
    }

    // ---- drain: duplicate ----

    #[test]
    fn drain_duplicate_reply_returns_duplicate_warning() {
        let mut life = RequestLifecycle::new();
        let req_a = req("agent-a", 1, 100, 0, 10);
        life.try_enqueue(req_a.clone(), 100);
        let _ = life.drain_reply(reply_for(&req_a, Some(Action::NoOp)), 105);

        // Same reply arrives again.
        let outcome = life.drain_reply(reply_for(&req_a, Some(Action::NoOp)), 106);
        match outcome {
            DrainOutcome::Duplicate { message } => {
                if let SimMessage::Warning(WarningPayload::InvalidAction { agent_id, reason }) =
                    message
                {
                    assert_eq!(agent_id, "agent-a");
                    assert!(reason.contains("duplicate"));
                } else {
                    panic!("expected InvalidAction warning, got {message:?}");
                }
            }
            other => panic!("expected Duplicate, got {other:?}"),
        }
    }

    // ---- drain: stale post-expiry ----

    #[test]
    fn drain_stale_reply_after_expiry_returns_stale_warning() {
        let mut life = RequestLifecycle::new();
        let req_a = req("agent-a", 1, 100, 0, 5); // deadline at tick 105
        life.try_enqueue(req_a.clone(), 100);

        // Expire at tick 200 (well past deadline).
        let outcomes = life.expire_overdue(200);
        assert_eq!(outcomes.len(), 1);

        // Stale reply for the expired attempt-0 arrives at tick 250.
        let outcome = life.drain_reply(reply_for(&req_a, Some(Action::NoOp)), 250);
        match outcome {
            DrainOutcome::Stale { message } => {
                if let SimMessage::Warning(WarningPayload::Behind {
                    lag_frames,
                    agent_id,
                }) = message
                {
                    assert_eq!(agent_id, Some("agent-a".to_string()));
                    assert!(lag_frames > 0, "stale lag_frames must be positive");
                } else {
                    panic!("expected Behind warning, got {message:?}");
                }
            }
            other => panic!("expected Stale, got {other:?}"),
        }
    }

    // ---- drain: unknown id ----

    #[test]
    fn drain_unknown_id_returns_engine_fault() {
        let mut life = RequestLifecycle::new();
        let req_a = req("ghost-agent", 999, 100, 0, 10);

        // Never enqueued.
        let outcome = life.drain_reply(reply_for(&req_a, Some(Action::NoOp)), 105);
        match outcome {
            DrainOutcome::UnknownId { message } => {
                if let SimMessage::Fault(FaultPayload::EngineFault { message }) = message {
                    assert!(message.contains("unknown request id"));
                    assert!(message.contains("ghost-agent"));
                } else {
                    panic!("expected EngineFault, got {message:?}");
                }
            }
            other => panic!("expected UnknownId, got {other:?}"),
        }
    }

    // ---- expiry: re-issue + give-up ----

    #[test]
    fn expire_overdue_reissues_with_attempt_increment() {
        let mut life = RequestLifecycle::new();
        let req_a = req("agent-a", 1, 100, 0, 5); // deadline tick 105
        life.try_enqueue(req_a.clone(), 100);

        let outcomes = life.expire_overdue(110);
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            ExpiryOutcome::Reissue {
                next_request,
                warning,
            } => {
                assert_eq!(next_request.id.attempt, 1, "attempt must increment");
                assert_eq!(
                    next_request.id.source_tick, req_a.id.source_tick,
                    "source_tick is preserved on re-issue"
                );
                assert_eq!(next_request.id.timeline_id, req_a.id.timeline_id);
                assert!(matches!(
                    warning,
                    SimMessage::Warning(WarningPayload::Behind { .. })
                ));
            }
            other => panic!("expected Reissue, got {other:?}"),
        }
    }

    #[test]
    fn expire_overdue_gives_up_at_max_attempts() {
        let mut life = RequestLifecycle::new();
        // Insert at attempt = MAX_ATTEMPTS; the spec formula
        // `attempt < MAX_ATTEMPTS` is FALSE here, so we give up.
        let req_a = req("agent-a", 1, 100, MAX_ATTEMPTS, 5);
        life.try_enqueue(req_a, 100);

        let outcomes = life.expire_overdue(200);
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            ExpiryOutcome::GiveUp { message } => {
                if let SimMessage::Warning(WarningPayload::InvalidAction { reason, .. }) = message {
                    assert!(reason.contains("max attempts"));
                } else {
                    panic!("expected InvalidAction giving up; got {message:?}");
                }
            }
            other => panic!("expected GiveUp, got {other:?}"),
        }
    }

    /// Boundary: at `attempt = MAX_ATTEMPTS - 1`, the spec formula
    /// `attempt < MAX_ATTEMPTS` is still TRUE so we re-issue to
    /// `attempt = MAX_ATTEMPTS`. This + the giveup test above
    /// document the exact attempts allowed: 0, 1, ..., MAX_ATTEMPTS.
    #[test]
    fn expire_overdue_reissues_at_max_attempts_minus_one() {
        let mut life = RequestLifecycle::new();
        let req_a = req("agent-a", 1, 100, MAX_ATTEMPTS - 1, 5);
        life.try_enqueue(req_a.clone(), 100);

        let outcomes = life.expire_overdue(200);
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            ExpiryOutcome::Reissue { next_request, .. } => {
                assert_eq!(
                    next_request.id.attempt, MAX_ATTEMPTS,
                    "attempt = MAX_ATTEMPTS - 1 must re-issue to MAX_ATTEMPTS"
                );
            }
            other => panic!("expected Reissue at the boundary, got {other:?}"),
        }
    }

    #[test]
    fn expire_does_nothing_when_no_request_is_overdue() {
        let mut life = RequestLifecycle::new();
        life.try_enqueue(req("agent-a", 1, 100, 0, 10), 100); // deadline 110
        let outcomes = life.expire_overdue(105); // not yet
        assert!(outcomes.is_empty());
        assert!(life.has_pending_for_agent("agent-a"));
    }

    // ---- determinism: stable ordering ----

    #[test]
    fn expire_returns_outcomes_in_stable_agent_id_order() {
        let mut life = RequestLifecycle::new();
        // Register in REVERSE alphabetical order; expire must return
        // outcomes in alphabetical order.
        life.try_enqueue(req("zulu", 1, 100, 0, 5), 100);
        life.try_enqueue(req("charlie", 2, 100, 0, 5), 100);
        life.try_enqueue(req("alpha", 3, 100, 0, 5), 100);
        let outcomes = life.expire_overdue(200);
        assert_eq!(outcomes.len(), 3);

        fn agent_of(o: &ExpiryOutcome) -> &str {
            match o {
                ExpiryOutcome::Reissue { next_request, .. } => &next_request.id.agent_id,
                ExpiryOutcome::GiveUp { message } => match message {
                    SimMessage::Warning(WarningPayload::InvalidAction { agent_id, .. }) => agent_id,
                    _ => "",
                },
            }
        }
        assert_eq!(agent_of(&outcomes[0]), "alpha");
        assert_eq!(agent_of(&outcomes[1]), "charlie");
        assert_eq!(agent_of(&outcomes[2]), "zulu");
    }

    // ---- ring eviction ----

    #[test]
    fn completed_ring_evicts_oldest_when_full() {
        let mut life = RequestLifecycle::with_caps(3, 100);
        for i in 0..5_u64 {
            let r = req("agent-a", i, 100 + i * 10, 0, 10);
            life.try_enqueue(r.clone(), 100 + i * 10);
            let _ = life.drain_reply(reply_for(&r, Some(Action::NoOp)), 110 + i * 10);
        }
        // Only the last 3 IDs should be in the completed ring.
        assert_eq!(life.completed.len(), 3);
        let ids: Vec<u64> = life.completed.iter().map(|id| id.timeline_id).collect();
        assert_eq!(ids, vec![2, 3, 4]);
    }

    // ---- attempt distinguishes re-issued from original ----

    #[test]
    fn attempt_0_and_attempt_1_are_distinct_request_ids() {
        let r0 = req("agent-a", 1, 100, 0, 5);
        let r1 = req("agent-a", 1, 100, 1, 5);
        assert_ne!(r0.id, r1.id);

        let mut life = RequestLifecycle::new();
        life.try_enqueue(r0.clone(), 100);
        // Expire attempt-0 → re-issue attempt-1.
        let outcomes = life.expire_overdue(200);
        match &outcomes[0] {
            ExpiryOutcome::Reissue { next_request, .. } => {
                assert_eq!(next_request.id, r1.id);
            }
            other => panic!("expected Reissue, got {other:?}"),
        }
    }

    /// Critical: drain_reply must enforce the deadline check (spec
    /// §10.2.1). A reply that arrives for a request whose deadline
    /// has passed but `expire_overdue` has not yet run this tick
    /// must be rejected as Stale, NOT applied. Also: the lifecycle
    /// must move the entry to `expired` so a subsequent
    /// `expire_overdue` doesn't double-process it.
    #[test]
    fn drain_reply_rejects_pending_reply_after_deadline_as_stale() {
        let mut life = RequestLifecycle::new();
        let req_a = req("agent-a", 1, 100, 0, 5); // deadline tick 105
        life.try_enqueue(req_a.clone(), 100);

        // Reply arrives at tick 200, past deadline, but
        // expire_overdue hasn't run yet for any tick > 105.
        let outcome = life.drain_reply(reply_for(&req_a, Some(Action::NoOp)), 200);
        match outcome {
            DrainOutcome::Stale { message } => {
                if let SimMessage::Warning(WarningPayload::Behind {
                    lag_frames,
                    agent_id,
                }) = message
                {
                    assert_eq!(agent_id, Some("agent-a".to_string()));
                    assert!(lag_frames > 0);
                } else {
                    panic!("expected Behind warning, got {message:?}");
                }
            }
            other => panic!("expected Stale, got {other:?}"),
        }
        // The entry was moved to expired (not just left in pending).
        assert!(!life.has_pending_for_agent("agent-a"));
        // And a subsequent expire_overdue produces 0 outcomes (no
        // double-processing).
        let later = life.expire_overdue(250);
        assert!(later.is_empty(), "drained-after-deadline must not re-fire");
    }

    /// Boundary: drain_reply at exactly deadline is on-time per spec
    /// §10.2.1 (`current_tick ≤ deadline`).
    #[test]
    fn drain_reply_at_exact_deadline_is_on_time_apply() {
        let mut life = RequestLifecycle::new();
        let req_a = req("agent-a", 1, 100, 0, 5); // deadline tick 105
        life.try_enqueue(req_a.clone(), 100);

        let outcome = life.drain_reply(reply_for(&req_a, Some(Action::NoOp)), 105);
        assert!(
            matches!(outcome, DrainOutcome::Apply { .. }),
            "drain at exact deadline must be on-time apply; got {outcome:?}"
        );
    }

    /// Regression: Codex bot PR #12 R2 P1 — re-issued requests must
    /// get a deadline rebased to the re-issue tick. If a re-issue
    /// kept the original deadline (`source_tick + deadline_ticks`),
    /// the next `expire_overdue` pass would immediately re-expire it
    /// without giving the bridge a real chance to respond.
    #[test]
    fn reissued_request_gets_fresh_deadline_window_not_immediate_re_expire() {
        let mut life = RequestLifecycle::new();
        let req_a = req("agent-a", 1, 100, 0, 5); // original: deadline_abs = 105
        life.try_enqueue(req_a.clone(), 100);

        // Tick 200: original expires; expire_overdue produces a Reissue
        // (attempt 1). Caller must re-enqueue it via try_enqueue with
        // the current tick, NOT the original source_tick.
        let outcomes = life.expire_overdue(200);
        let next_request = match outcomes.into_iter().next().expect("one outcome") {
            ExpiryOutcome::Reissue { next_request, .. } => next_request,
            other => panic!("expected Reissue, got {other:?}"),
        };
        assert_eq!(next_request.id.attempt, 1);
        assert_eq!(
            next_request.id.source_tick, 100,
            "source_tick preserved for stable identity"
        );

        // Re-enqueue the re-issue at tick 200. Deadline must be
        // 200 + 5 = 205, NOT 100 + 5 = 105 (which is already past).
        let enq = life.try_enqueue(next_request.clone(), 200);
        assert!(
            matches!(enq, EnqueueOutcome::Enqueued),
            "re-enqueue must succeed"
        );

        // Now call expire_overdue at tick 201 — the re-issue is well
        // within its deadline window (deadline_abs = 205) and MUST NOT
        // be re-expired.
        let outcomes = life.expire_overdue(201);
        assert!(
            outcomes.is_empty(),
            "re-issue must not be immediately re-expired; got {outcomes:?}"
        );

        // A reply arriving at tick 204 is still on-time.
        let outcome = life.drain_reply(reply_for(&next_request, Some(Action::NoOp)), 204);
        assert!(
            matches!(outcome, DrainOutcome::Apply { .. }),
            "on-time reply for re-issue must apply; got {outcome:?}"
        );
    }
}
