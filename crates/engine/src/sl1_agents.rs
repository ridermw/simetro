//! SL1 agent runtime (PR 10).
//!
//! Runs after `sl1_observability` so each agent's decision observes
//! the freshest per-tick metric/dashboard/alert state. The runtime
//! iterates agents in stable id order (the scene Vec is sorted at
//! load time), and for each agent whose `interval_ticks` cadence
//! fires on the current tick:
//!
//! 1. asks a [`AgentBackend`] for an optional [`Sl1AgentAction`]
//! 2. validates the action against the agent's `allowed_actions`,
//!    `observation_scope`, `max_cost_per_decision`, and `cooldown_ticks`
//! 3. applies the action's effect to the runtime, OR emits a typed
//!    rejection event
//!
//! PR 10 ships exactly one runtime-effective action variant:
//! [`Sl1AgentAction::ThrottleDemand`]. Other entries in `allowed_actions`
//! are accepted at load time so authors can prepare scenes for later PRs
//! without churn, but if a backend ever proposes one the runtime emits
//! `EffectUnsupportedInThisPr`. Backends shipped here never propose
//! such actions; the rejection path exists only to keep the surface
//! safe against test/future code.
//!
//! All three backends currently return `None`:
//!
//! * [`MockBackend`] — no-op. Used in the deterministic fixture.
//! * [`BuiltinBackend`] — placeholder for the PR-12 GPU Launch Week
//!   heuristics.
//! * [`LlmBackend`] — feature-gated live backend. Always returns
//!   `None` AND emits a one-shot `SimEvent::Sl1AgentLlmDisabled` so
//!   authors can distinguish "agent chose not to act" from "live LLM
//!   not wired".

use simetro_protocol::SimEvent;

use crate::scenario_language_v1::{
    Sl1Agent, Sl1AgentAction, Sl1AgentActionKind, Sl1AgentKind, Sl1AgentObservationTarget,
    Sl1AgentRejectionReason, Sl1AgentRuntimeState, Sl1Scene,
};

/// Decision interface used by [`run`] to obtain an agent's proposed
/// action on a fired tick.
///
/// Backends are stateless from the runtime's point of view — the
/// runtime never persists backend instances. A fresh backend is
/// constructed per agent per call to [`run`]. This keeps the
/// determinism contract honest: deterministic behavior depends only
/// on the agent's static declaration, the scene, and the runtime
/// state — never on hidden mutable backend state.
pub trait AgentBackend {
    /// Returns the agent's proposed action for this tick, or `None`
    /// if it chose not to act. Implementations MUST be deterministic
    /// given the same `(agent, scene, runtime, now)` tuple.
    fn decide(
        &mut self,
        agent: &Sl1Agent,
        scene: &Sl1Scene,
        runtime: &Sl1RuntimeStateRef<'_>,
        now: u64,
    ) -> Option<Sl1AgentAction>;
}

/// Read-only projection of the runtime state passed to backends.
///
/// PR 10 keeps this empty (backends do not yet observe runtime state)
/// to avoid leaking mutable refs through the trait. Later PRs will
/// extend this with the typed observability state declared in each
/// agent's `observation_scope`.
pub struct Sl1RuntimeStateRef<'a> {
    /// Reserved for future PRs; suppresses an unused-lifetime warning.
    pub _phantom: std::marker::PhantomData<&'a ()>,
}

/// No-op backend. Always returns `None`. Used in CI fixtures so
/// determinism baselines do not depend on any heuristic policy.
#[derive(Debug, Default)]
pub struct MockBackend;

impl AgentBackend for MockBackend {
    fn decide(
        &mut self,
        _agent: &Sl1Agent,
        _scene: &Sl1Scene,
        _runtime: &Sl1RuntimeStateRef<'_>,
        _now: u64,
    ) -> Option<Sl1AgentAction> {
        None
    }
}

/// Placeholder for the PR-12 GPU Launch Week heuristics. PR 10
/// returns `None` so the surface compiles end-to-end without shipping
/// behavior the scene cannot yet exercise.
#[derive(Debug, Default)]
pub struct BuiltinBackend;

impl AgentBackend for BuiltinBackend {
    fn decide(
        &mut self,
        _agent: &Sl1Agent,
        _scene: &Sl1Scene,
        _runtime: &Sl1RuntimeStateRef<'_>,
        _now: u64,
    ) -> Option<Sl1AgentAction> {
        None
    }
}

/// Feature-gated live LLM backend. PR 10 always returns `None` and
/// signals to the caller (via [`run`]) that
/// `Sl1AgentLlmDisabled` should be emitted exactly once per scene
/// run for any `kind: llm` agent.
#[derive(Debug, Default)]
pub struct LlmBackend;

impl AgentBackend for LlmBackend {
    fn decide(
        &mut self,
        _agent: &Sl1Agent,
        _scene: &Sl1Scene,
        _runtime: &Sl1RuntimeStateRef<'_>,
        _now: u64,
    ) -> Option<Sl1AgentAction> {
        None
    }
}

/// Construct the default backend for an agent's declared kind. PR 10
/// ships only the three built-in backends; test code uses
/// [`run_with_backend`] to inject a [`ScriptedBackend`] for
/// deterministic action coverage.
fn default_backend(kind: Sl1AgentKind) -> Box<dyn AgentBackend> {
    match kind {
        Sl1AgentKind::Mock => Box::new(MockBackend),
        Sl1AgentKind::Builtin => Box::new(BuiltinBackend),
        Sl1AgentKind::Llm => Box::new(LlmBackend),
    }
}

/// Drive every agent for one tick. See module docs.
pub fn run(
    scene: &Sl1Scene,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
) {
    run_with_factory(scene, runtime, now, events, |kind| default_backend(kind));
}

/// Test seam: drive every agent for one tick using a caller-supplied
/// per-agent backend factory.
///
/// The factory is invoked once per agent on every tick (regardless of
/// cadence) so the per-agent backend identity is deterministic.
/// Backends still only `decide` on cadence ticks.
pub fn run_with_factory(
    scene: &Sl1Scene,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
    mut factory: impl FnMut(Sl1AgentKind) -> Box<dyn AgentBackend>,
) {
    if scene.agents.is_empty() {
        return;
    }
    for agent in &scene.agents {
        let mut backend = factory(agent.kind);
        decide_one(agent, scene, runtime, now, events, backend.as_mut());
    }
}

fn decide_one(
    agent: &Sl1Agent,
    scene: &Sl1Scene,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    now: u64,
    events: &mut Vec<SimEvent>,
    backend: &mut dyn AgentBackend,
) {
    // Agent's runtime state must exist; from_scene seeds it for every
    // declared agent at load time.
    let Some(state) = runtime.agents.get(&agent.id).cloned() else {
        return;
    };

    if !should_fire(agent, &state, now) {
        return;
    }

    // For `kind: llm`, the live backend is feature-gated off in PR 10.
    // Emit the one-shot disabled event the first time the agent's
    // cadence would have fired, then DO NOT call the backend — this
    // matches the spec: "agent observed nothing this tick" is
    // indistinguishable from "feature disabled" without this signal.
    if agent.kind == Sl1AgentKind::Llm {
        update_decision_clock(runtime, &agent.id, now);
        let mut emitted = state.llm_disabled_emitted;
        if !emitted {
            events.push(SimEvent::Sl1AgentLlmDisabled {
                agent_id: agent.id.clone(),
                tick: now,
            });
            emitted = true;
        }
        if let Some(s) = runtime.agents.get_mut(&agent.id) {
            s.llm_disabled_emitted = emitted;
        }
        return;
    }

    let world_ref = Sl1RuntimeStateRef {
        _phantom: std::marker::PhantomData,
    };
    let proposed = backend.decide(agent, scene, &world_ref, now);
    update_decision_clock(runtime, &agent.id, now);

    let Some(action) = proposed else {
        return;
    };

    // Validate. On any rejection, emit Sl1AgentActionRejected and
    // return WITHOUT applying the effect or starting cooldown.
    let action_kind = action.kind();
    let target_id = action_target_id(&action);

    // 1. Cooldown.
    if let Some(until) = state.cooldown_until_tick {
        if now < until {
            emit_rejection(
                events,
                &agent.id,
                action_kind,
                target_id.clone(),
                Sl1AgentRejectionReason::Cooldown,
                now,
            );
            return;
        }
    }
    // 2. Allowed-actions membership.
    if !agent.allowed_actions.contains(&action_kind) {
        emit_rejection(
            events,
            &agent.id,
            action_kind,
            target_id.clone(),
            Sl1AgentRejectionReason::ActionNotAllowed,
            now,
        );
        return;
    }
    // 3. Target resolves to a declared scene element.
    if let Some(reason) = validate_target_exists(scene, action_kind, target_id.as_deref()) {
        emit_rejection(
            events,
            &agent.id,
            action_kind,
            target_id.clone(),
            reason,
            now,
        );
        return;
    }
    // 4. Target is within the agent's observation_scope.
    if let Some(tid) = target_id.as_deref() {
        if !target_in_scope(agent, action_kind, tid) {
            emit_rejection(
                events,
                &agent.id,
                action_kind,
                target_id.clone(),
                Sl1AgentRejectionReason::ActionTargetOutOfScope,
                now,
            );
            return;
        }
    }
    // 5. Cost.
    let cost = action.cost();
    if cost > agent.max_cost_per_decision {
        emit_rejection(
            events,
            &agent.id,
            action_kind,
            target_id.clone(),
            Sl1AgentRejectionReason::CostExceedsBudget,
            now,
        );
        return;
    }
    // 6. Action-specific parameter validation.
    if let Some(reason) = validate_parameters(&action) {
        emit_rejection(
            events,
            &agent.id,
            action_kind,
            target_id.clone(),
            reason,
            now,
        );
        return;
    }

    // Apply.
    apply_action(&action, runtime, now);
    if let Some(s) = runtime.agents.get_mut(&agent.id) {
        if agent.cooldown_ticks > 0 {
            s.cooldown_until_tick = Some(now.saturating_add(agent.cooldown_ticks));
        }
    }
    events.push(SimEvent::Sl1AgentActionApplied {
        agent_id: agent.id.clone(),
        action_kind: action_kind.as_str().to_string(),
        target_id: target_id.unwrap_or_default(),
        cost,
        tick: now,
    });
}

fn should_fire(agent: &Sl1Agent, state: &Sl1AgentRuntimeState, now: u64) -> bool {
    // Schedules begin at tick 1 (tick 0 is the pre-tick world).
    if now == 0 {
        return false;
    }
    match state.last_decision_tick {
        None => now % agent.interval_ticks == 0,
        Some(last) => now >= last.saturating_add(agent.interval_ticks),
    }
}

fn update_decision_clock(
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    agent_id: &str,
    now: u64,
) {
    if let Some(s) = runtime.agents.get_mut(agent_id) {
        s.last_decision_tick = Some(now);
    }
}

fn action_target_id(action: &Sl1AgentAction) -> Option<String> {
    match action {
        Sl1AgentAction::ThrottleDemand { demand_id, .. } => Some(demand_id.clone()),
    }
}

fn validate_target_exists(
    scene: &Sl1Scene,
    kind: Sl1AgentActionKind,
    target_id: Option<&str>,
) -> Option<Sl1AgentRejectionReason> {
    match kind {
        Sl1AgentActionKind::ThrottleDemand => {
            let Some(tid) = target_id else {
                return Some(Sl1AgentRejectionReason::ActionTargetUnknown);
            };
            if scene.demand.iter().any(|d| d.id == tid) {
                None
            } else {
                Some(Sl1AgentRejectionReason::ActionTargetUnknown)
            }
        }
        // Future variants — runtime can't apply them in PR 10.
        Sl1AgentActionKind::SetJobPriority
        | Sl1AgentActionKind::ScalePlaceCapacity
        | Sl1AgentActionKind::WarmCache
        | Sl1AgentActionKind::PrioritizeTransform
        | Sl1AgentActionKind::PauseReportRefresh => {
            Some(Sl1AgentRejectionReason::EffectUnsupportedInThisPr)
        }
    }
}

fn target_in_scope(agent: &Sl1Agent, kind: Sl1AgentActionKind, target_id: &str) -> bool {
    // PR 10 only emits ThrottleDemand which targets a demand. The
    // agent must declare that demand in its observation_scope.
    let want_kind = match kind {
        Sl1AgentActionKind::ThrottleDemand => "demand",
        // Other kinds are rejected as EffectUnsupportedInThisPr
        // before this function runs.
        _ => return true,
    };
    agent.observation_scope.iter().any(|t| {
        t.kind_str() == want_kind
            && match t {
                Sl1AgentObservationTarget::Demand(id) => id == target_id,
                _ => false,
            }
    })
}

fn validate_parameters(action: &Sl1AgentAction) -> Option<Sl1AgentRejectionReason> {
    match action {
        Sl1AgentAction::ThrottleDemand { pause_ticks, .. } => {
            if *pause_ticks == 0
                || *pause_ticks > crate::scenario_language_v1::SL1_AGENT_MAX_COOLDOWN_TICKS
            {
                Some(Sl1AgentRejectionReason::InvalidActionParameter)
            } else {
                None
            }
        }
    }
}

fn apply_action(
    action: &Sl1AgentAction,
    runtime: &mut crate::scenario_language_v1::Sl1RuntimeState,
    now: u64,
) {
    match action {
        Sl1AgentAction::ThrottleDemand {
            demand_id,
            pause_ticks,
        } => {
            // Pause is exclusive of `pause_until_tick`. Pausing on
            // tick `now` for `pause_ticks=3` blocks spawns on ticks
            // `now+1, now+2, now+3`; spawning resumes on `now+1+3 =
            // now+pause_ticks+1`, which equals
            // `pause_until_tick = now + 1 + pause_ticks`.
            //
            // The +1 anchor reflects that the agent decides AFTER
            // demand spawn for tick `now` has already run; the next
            // possible spawn tick is `now+1`.
            let pause_until = now.saturating_add(1).saturating_add(*pause_ticks);
            // Monotonic conflict resolution: if two agents pause the
            // same demand on the same tick (or a stale longer pause
            // is still active), keep the later expiry. Last-writer
            // shortening is surprising even when deterministic.
            let entry = runtime
                .agent_demand_pauses
                .entry(demand_id.clone())
                .or_insert(0);
            *entry = (*entry).max(pause_until);
        }
    }
}

fn emit_rejection(
    events: &mut Vec<SimEvent>,
    agent_id: &str,
    action_kind: Sl1AgentActionKind,
    target_id: Option<String>,
    reason: Sl1AgentRejectionReason,
    now: u64,
) {
    events.push(SimEvent::Sl1AgentActionRejected {
        agent_id: agent_id.to_string(),
        action_kind: action_kind.as_str().to_string(),
        target_id,
        reason: reason.as_str().to_string(),
        tick: now,
    });
}

// ---------------------------------------------------------------------------
// Test-only scripted backend.
// ---------------------------------------------------------------------------

/// A scripted backend useful for fixture-driven tests: each entry in
/// the script is `(tick, action)`; whenever the agent fires on a
/// matching tick, the matching action is proposed. Multiple actions
/// for the same tick are NOT supported (only the first match is
/// consumed). Unmatched fired ticks return `None`.
#[derive(Debug, Default, Clone)]
pub struct ScriptedBackend {
    pub script: Vec<(u64, Sl1AgentAction)>,
}

impl AgentBackend for ScriptedBackend {
    fn decide(
        &mut self,
        _agent: &Sl1Agent,
        _scene: &Sl1Scene,
        _runtime: &Sl1RuntimeStateRef<'_>,
        now: u64,
    ) -> Option<Sl1AgentAction> {
        if let Some(idx) = self.script.iter().position(|(t, _)| *t == now) {
            Some(self.script.remove(idx).1)
        } else {
            None
        }
    }
}
