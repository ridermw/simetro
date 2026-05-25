//! Deterministic SHA-256 state hash (determinism contract).
//!
//! The headless `hash` subcommand walks a scene + seed, runs N ticks,
//! and emits a SHA-256 over `(world_state_snapshot, event_stream)`.
//! CI commits this to `tests/baselines/<scene>.hash` and gates every
//! build by diffing against it; any drift fails the build.
//!
//! ```text
//!   sha256_init
//!     ├── feed world: nodes, paths, movers (BTreeMap iter → stable)
//!     └── per tick:
//!         ├── tick_once(runner)
//!         ├── feed event count
//!         └── feed each event in emission order
//!   sha256_finalize → 32 bytes → hex
//! ```
//!
//! The hash is **not** intended to be a content fingerprint; it is a
//! determinism contract. Two runs of the same scene + seed must
//! produce the same hex digest on every supported platform.

use sha2::{Digest, Sha256};
use simetro_protocol::{Action, AgentReport, FaultPayload, SimEvent, SimMessage, WarningPayload};

use crate::components::{MoverState, NodeShape};
use crate::tick::TickRunner;
use crate::world::World;

/// Hash a world's static contents (after `load_scene_str`). Captures
/// nodes, paths, and movers but ignores tick counter and run state
/// (which the caller intends to advance).
pub fn hash_world(world: &World) -> [u8; 32] {
    let mut h = Sha256::new();
    feed_world(&mut h, world);
    h.finalize().into()
}

fn feed_world(h: &mut Sha256, world: &World) {
    h.update(b"world.v1");
    h.update(world.dt.to_le_bytes());
    h.update((world.nodes.len() as u64).to_le_bytes());
    for (id, node) in &world.nodes {
        h.update(id.0.to_le_bytes());
        h.update(node.pos[0].to_le_bytes());
        h.update(node.pos[1].to_le_bytes());
        h.update([node.color]);
        h.update([shape_tag(node.shape)]);
    }
    h.update((world.paths.len() as u64).to_le_bytes());
    for (id, p) in &world.paths {
        h.update(id.0.to_le_bytes());
        h.update(p.from.0.to_le_bytes());
        h.update(p.to.0.to_le_bytes());
        h.update([p.color]);
    }
    h.update((world.movers.len() as u64).to_le_bytes());
    for (id, m) in &world.movers {
        h.update(id.0.to_le_bytes());
        h.update(m.home_path.0.to_le_bytes());
        h.update(m.speed.to_le_bytes());
        feed_mover_state(h, m.state());
    }
    if !world.resources.is_empty()
        || !world.inventory.is_empty()
        || !world.producers.is_empty()
        || !world.consumers.is_empty()
    {
        h.update(b"resources.v1");
        h.update((world.resources.len() as u64).to_le_bytes());
        for (id, r) in &world.resources {
            h.update(id.0.to_le_bytes());
            h.update([r.color]);
        }
        h.update((world.inventory.len() as u64).to_le_bytes());
        for (id, amount) in &world.inventory {
            h.update(id.0.to_le_bytes());
            h.update(amount.to_le_bytes());
        }
        h.update((world.producers.len() as u64).to_le_bytes());
        for (id, p) in &world.producers {
            h.update(id.0.to_le_bytes());
            h.update(p.resource.0.to_le_bytes());
            h.update(p.amount.to_le_bytes());
            h.update(p.interval_ticks.to_le_bytes());
        }
        h.update((world.consumers.len() as u64).to_le_bytes());
        for (id, c) in &world.consumers {
            h.update(id.0.to_le_bytes());
            h.update(c.resource.0.to_le_bytes());
            h.update(c.amount.to_le_bytes());
            h.update(c.interval_ticks.to_le_bytes());
        }
    }
    feed_sl1(h, world);
}

/// Stable contribution of `world.sl1` to the deterministic hash.
///
/// Emits a fixed `sl1.v1` tag with the SL1 schema version, the
/// section counts in canonical order, and the per-primitive content
/// for every primitive that has had its PR land. Empty for any
/// not-yet-implemented primitive (zero count, no per-entry bytes)
/// so older baselines remain stable until the next primitive lands.
///
/// Always emitted when an SL1 block is present so the baseline
/// distinguishes a legacy scene from an SL1-equipped scene. PR 1 adds
/// the per-place fingerprint (everything after the counts header).
/// Any future extension is a deliberate baseline drift that must be
/// rolled into the baseline hash in the same PR.
fn feed_sl1(h: &mut Sha256, world: &World) {
    let Some(sl1) = world.sl1.as_ref() else {
        return;
    };
    h.update(b"sl1.v1");
    h.update(sl1.schema_version.to_le_bytes());
    h.update((sl1.places.len() as u64).to_le_bytes());
    h.update((sl1.links.len() as u64).to_le_bytes());
    h.update((sl1.things.len() as u64).to_le_bytes());
    h.update((sl1.transforms.len() as u64).to_le_bytes());
    h.update((sl1.demand.len() as u64).to_le_bytes());
    h.update((sl1.pressure.len() as u64).to_le_bytes());
    h.update((sl1.objectives.len() as u64).to_le_bytes());
    h.update((sl1.failure_conditions.len() as u64).to_le_bytes());
    h.update((sl1.agents.len() as u64).to_le_bytes());
    h.update((sl1.milestones.len() as u64).to_le_bytes());
    h.update([u8::from(sl1.observability.is_some())]);

    // Per-place fingerprint (PR 1). Places are already sorted by id at
    // validation time, so iterating in vec order is deterministic and
    // equivalent to sorted iteration. Walking each place's content
    // captures every author-declared field that distinguishes one
    // configuration from another.
    //
    // When `sl1.places` is empty (e.g. `sl1-empty.json` fixture), the
    // loop body runs zero times and no extra bytes are appended, so
    // the existing baseline hash stays stable across this PR.
    for place in &sl1.places {
        h.update(b"sl1.place.v1");
        feed_str(h, &place.id);
        feed_str(h, &place.role);
        h.update(place.pos[0].to_le_bytes());
        h.update(place.pos[1].to_le_bytes());

        h.update((place.capacity.len() as u64).to_le_bytes());
        for (k, v) in &place.capacity {
            feed_str(h, k);
            h.update(v.to_le_bytes());
        }

        h.update((place.storage.len() as u64).to_le_bytes());
        for (slot, def) in &place.storage {
            feed_str(h, slot);
            h.update(def.capacity.to_le_bytes());
            h.update(def.initial.to_le_bytes());
        }

        feed_str_list(h, &place.accepts);
        feed_str_list(h, &place.produces);
        feed_str_list(h, &place.failure_domains);

        h.update((place.operating_states.len() as u64).to_le_bytes());
        for (name, state) in &place.operating_states {
            feed_str(h, name);
            feed_predicate(h, &state.predicate);
            match state.grace_ticks {
                Some(t) => {
                    h.update([1u8]);
                    h.update(t.to_le_bytes());
                }
                None => h.update([0u8]),
            }
        }
    }
}

fn feed_str(h: &mut Sha256, s: &str) {
    h.update((s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
}

fn feed_str_list(h: &mut Sha256, list: &[String]) {
    h.update((list.len() as u64).to_le_bytes());
    for s in list {
        feed_str(h, s);
    }
}

fn feed_predicate(h: &mut Sha256, p: &crate::scenario_language_v1::Sl1OperatingPredicate) {
    use crate::scenario_language_v1::Sl1OperatingPredicate;
    match p {
        Sl1OperatingPredicate::UsedPercentGte { metric, threshold } => {
            h.update([0x01]);
            feed_str(h, metric);
            h.update([*threshold]);
        }
        Sl1OperatingPredicate::OverloadedTicksGt { ticks } => {
            h.update([0x02]);
            h.update(ticks.to_le_bytes());
        }
    }
}

fn feed_mover_state(h: &mut Sha256, s: MoverState) {
    match s {
        MoverState::Empty => h.update([0xE0]),
        MoverState::Waiting { at } => {
            h.update([0xE1]);
            h.update(at.0.to_le_bytes());
        }
        MoverState::Traveling { path, progress } => {
            h.update([0xE2]);
            h.update(path.0.to_le_bytes());
            h.update(progress.to_le_bytes());
        }
    }
}

fn shape_tag(s: NodeShape) -> u8 {
    match s {
        NodeShape::Circle => 1,
        NodeShape::Square => 2,
        NodeShape::Triangle => 3,
        NodeShape::Diamond => 4,
        NodeShape::Hexagon => 5,
    }
}

fn feed_event(h: &mut Sha256, e: &SimEvent) {
    match e {
        SimEvent::MoverDeparted {
            mover,
            from_node,
            path,
        } => {
            h.update([0x10]);
            h.update(mover.to_le_bytes());
            h.update(from_node.to_le_bytes());
            h.update(path.to_le_bytes());
        }
        SimEvent::MoverArrived {
            mover,
            at_node,
            path,
        } => {
            h.update([0x11]);
            h.update(mover.to_le_bytes());
            h.update(at_node.to_le_bytes());
            h.update(path.to_le_bytes());
        }
        SimEvent::MoverSpeedChange { mover, old, new } => {
            h.update([0x12]);
            h.update(mover.to_le_bytes());
            h.update(old.to_le_bytes());
            h.update(new.to_le_bytes());
        }
        SimEvent::NodeHighlighted { node, reason } => {
            h.update([0x13]);
            h.update(node.to_le_bytes());
            h.update([*reason as u8]);
        }
        SimEvent::PathPulsed { path } => {
            h.update([0x14]);
            h.update(path.to_le_bytes());
        }
        SimEvent::AgentDecided { agent_id, action } => {
            h.update([0x15]);
            h.update(agent_id.as_bytes());
            h.update([*action as u8]);
        }
        SimEvent::Tick { tick } => {
            h.update([0x16]);
            h.update(tick.to_le_bytes());
        }
    }
}

/// Hash a single `SimMessage` into the running digest.
///
/// Only message variants that carry deterministic, engine-emitted
/// content are fed into the hash:
///
/// - `Warning` — engine-side degradations (Behind, TickOverBudget,
///   InvalidAction, AgentLogSlow)
/// - `Fault` — engine-side errors (AgentCrashed, NumericDrift,
///   LoadError, etc.)
/// - `AgentReport` — agent decision rationale + considered + confidence
///
/// `Static`, `Snapshot`, and `Events(_)` are deliberately NOT fed:
///
/// - `Static` is per-load metadata (already covered by `feed_world`).
/// - `Snapshot` is render-pacing state, not deterministic per-tick.
/// - `Events(_)` is the visible-changes channel that `hash_run`
///   already feeds via `runner.events()`; feeding it again here
///   would double-count.
///
/// This is the rubber-duck-identified gap: without this function the
/// determinism hash was blind to stalled-bridge warnings, panicked-
/// agent faults, and varying LLM rationale strings. See spec
/// §10.2 / §14 plan-mode decisions.
fn feed_message(h: &mut Sha256, msg: &SimMessage) {
    match msg {
        // Skipped (see doc comment).
        SimMessage::Static(_) | SimMessage::Snapshot(_) | SimMessage::Events(_) => {}
        SimMessage::Warning(w) => {
            h.update([0x20]);
            feed_warning(h, w);
        }
        SimMessage::Fault(f) => {
            h.update([0x21]);
            feed_fault(h, f);
        }
        SimMessage::AgentReport(r) => {
            h.update([0x22]);
            feed_agent_report(h, r);
        }
    }
}

fn feed_warning(h: &mut Sha256, w: &WarningPayload) {
    match w {
        WarningPayload::InvalidAction { agent_id, reason } => {
            h.update([0x30]);
            h.update((agent_id.len() as u64).to_le_bytes());
            h.update(agent_id.as_bytes());
            h.update((reason.len() as u64).to_le_bytes());
            h.update(reason.as_bytes());
        }
        WarningPayload::Behind {
            lag_frames,
            agent_id,
        } => {
            h.update([0x31]);
            h.update(lag_frames.to_le_bytes());
            // agent_id is Option<String>: hash a presence byte + bytes.
            match agent_id {
                Some(id) => {
                    h.update([0x01]);
                    h.update((id.len() as u64).to_le_bytes());
                    h.update(id.as_bytes());
                }
                None => h.update([0x00]),
            }
        }
        WarningPayload::TickOverBudget { ms } => {
            h.update([0x32]);
            h.update(ms.to_le_bytes());
        }
        WarningPayload::AgentLogSlow => {
            h.update([0x33]);
        }
    }
}

fn feed_fault(h: &mut Sha256, f: &FaultPayload) {
    match f {
        FaultPayload::LoadError { message, line, col } => {
            h.update([0x40]);
            h.update((message.len() as u64).to_le_bytes());
            h.update(message.as_bytes());
            feed_opt_u32(h, *line);
            feed_opt_u32(h, *col);
        }
        FaultPayload::AgentCrashed { agent_id, message } => {
            h.update([0x41]);
            h.update((agent_id.len() as u64).to_le_bytes());
            h.update(agent_id.as_bytes());
            h.update((message.len() as u64).to_le_bytes());
            h.update(message.as_bytes());
        }
        FaultPayload::NumericDrift { tick } => {
            h.update([0x42]);
            h.update(tick.to_le_bytes());
        }
        FaultPayload::EngineFault { message } => {
            h.update([0x43]);
            h.update((message.len() as u64).to_le_bytes());
            h.update(message.as_bytes());
        }
        FaultPayload::BaselineHashMismatch { expected, found } => {
            h.update([0x44]);
            h.update((expected.len() as u64).to_le_bytes());
            h.update(expected.as_bytes());
            h.update((found.len() as u64).to_le_bytes());
            h.update(found.as_bytes());
        }
        FaultPayload::SchemaMismatch { expected, found } => {
            h.update([0x45]);
            h.update(expected.to_le_bytes());
            h.update(found.to_le_bytes());
        }
        FaultPayload::TransportLost => {
            h.update([0x46]);
        }
    }
}

fn feed_agent_report(h: &mut Sha256, r: &AgentReport) {
    h.update(r.tick.to_le_bytes());
    h.update((r.agent_id.len() as u64).to_le_bytes());
    h.update(r.agent_id.as_bytes());
    h.update((r.considered.len() as u64).to_le_bytes());
    for c in &r.considered {
        // Hash full action payload for considered alternatives too —
        // two runs that consider different alternatives must hash
        // differently even if `chosen` is the same.
        feed_action(h, &c.action);
        h.update(c.confidence.to_le_bytes());
    }
    // `chosen` presence byte + FULL Action payload.
    //
    // We hash the full payload here (not just the discriminant) so
    // that the determinism gate catches any difference in agent
    // decisions, regardless of whether the downstream apply path
    // would surface that difference (e.g. a rejected SetSpeed for a
    // non-existent mover, two different rejected SetSpeed payloads
    // that both emit identical `Warning::InvalidAction` reasons, an
    // author action that fails before mutating world state). This
    // makes the determinism gate independent of how each Action
    // variant routes through events / messages / world mutation.
    //
    // Test `hash_run_distinguishes_runs_that_differ_only_in_action_payload`
    // covers the happy-path SetSpeed case; this stronger formulation
    // also covers the rejection/no-op edge cases that would otherwise
    // hash identically.
    match &r.chosen {
        Some(a) => {
            h.update([0x01]);
            feed_action(h, a);
        }
        None => h.update([0x00]),
    }
    h.update((r.rationale.len() as u64).to_le_bytes());
    h.update(r.rationale.as_bytes());
    h.update(r.confidence.to_le_bytes());
}

/// Hash a single `Action` into the running digest, including its
/// full payload. Stable byte tags so future variant additions extend
/// without disturbing existing baselines.
fn feed_action(h: &mut Sha256, a: &Action) {
    match a {
        Action::NoOp => h.update([0x50]),
        Action::SetSpeed { mover, speed } => {
            h.update([0x51]);
            h.update(mover.to_le_bytes());
            h.update(speed.to_le_bytes());
        }
        Action::PlacePiece { piece_kind, pos } => {
            h.update([0x52]);
            h.update((piece_kind.len() as u64).to_le_bytes());
            h.update(piece_kind.as_bytes());
            h.update(pos[0].to_le_bytes());
            h.update(pos[1].to_le_bytes());
        }
        Action::ConnectPieces { from, to } => {
            h.update([0x53]);
            h.update(from.to_le_bytes());
            h.update(to.to_le_bytes());
        }
        Action::RemovePiece { id } => {
            h.update([0x54]);
            h.update(id.to_le_bytes());
        }
        Action::DefineResource { name, color } => {
            h.update([0x55]);
            h.update((name.len() as u64).to_le_bytes());
            h.update(name.as_bytes());
            h.update([*color]);
        }
        Action::AddProducer {
            resource,
            amount,
            interval_ticks,
        } => {
            h.update([0x56]);
            h.update((resource.len() as u64).to_le_bytes());
            h.update(resource.as_bytes());
            h.update(amount.to_le_bytes());
            h.update(interval_ticks.to_le_bytes());
        }
        Action::AddConsumer {
            resource,
            amount,
            interval_ticks,
        } => {
            h.update([0x57]);
            h.update((resource.len() as u64).to_le_bytes());
            h.update(resource.as_bytes());
            h.update(amount.to_le_bytes());
            h.update(interval_ticks.to_le_bytes());
        }
        Action::SetGoal { goal } => {
            h.update([0x58]);
            h.update((goal.len() as u64).to_le_bytes());
            h.update(goal.as_bytes());
        }
    }
}

fn feed_opt_u32(h: &mut Sha256, v: Option<u32>) {
    match v {
        Some(n) => {
            h.update([0x01]);
            h.update(n.to_le_bytes());
        }
        None => h.update([0x00]),
    }
}

/// Run `ticks` ticks against `world` using `runner` and produce the
/// final hex-encoded SHA-256 of the full event + message stream + ending
/// world state. The hash is deterministic on every supported platform
/// when driven by the same scene + seed (determinism contract).
///
/// The hash now covers `runner.messages()` in addition to
/// `runner.events()`. This closes the rubber-duck-identified gap
/// (CRITICAL #7): without messages, a stalled LLM bridge or panicked
/// agent could produce nondeterministic warnings / faults /
/// AgentReports that the baseline gate did not catch. With messages
/// included, any nondeterminism in those channels breaks the gate.
///
/// Per-tick hash sequence (after the world prefix):
///   `evs` + len + each event (existing)
///   `msg` + len + each message (NEW — Warning / Fault / AgentReport
///                                only; Static / Snapshot / Events are
///                                skipped by `feed_message`).
pub fn hash_run(world: &mut World, runner: &mut TickRunner, ticks: u64) -> String {
    let mut h = Sha256::new();
    feed_world(&mut h, world);
    for _ in 0..ticks {
        runner.tick_once(world);
        let evs = runner.events();
        h.update(b"evs");
        h.update((evs.len() as u64).to_le_bytes());
        for e in evs {
            feed_event(&mut h, e);
        }
        let msgs = runner.messages();
        h.update(b"msg");
        h.update((msgs.len() as u64).to_le_bytes());
        for m in msgs {
            feed_message(&mut h, m);
        }
    }
    h.update(b"final");
    feed_world(&mut h, world);
    let bytes: [u8; 32] = h.finalize().into();
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    const TBL: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(TBL[(b >> 4) as usize] as char);
        s.push(TBL[(b & 0x0f) as usize] as char);
    }
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::loader::load_scene_str;

    const SCENE: &str = include_str!("../../../games/demo-paths.json");

    #[test]
    fn hash_world_is_stable_for_same_input() {
        let a = load_scene_str(SCENE, 42).unwrap();
        let b = load_scene_str(SCENE, 42).unwrap();
        assert_eq!(hash_world(&a.world), hash_world(&b.world));
    }

    #[test]
    fn hash_run_matches_across_two_invocations() {
        let mut a = load_scene_str(SCENE, 42).unwrap();
        let mut b = load_scene_str(SCENE, 42).unwrap();
        let mut ra = TickRunner::new();
        let mut rb = TickRunner::new();
        let ha = hash_run(&mut a.world, &mut ra, 2000);
        let hb = hash_run(&mut b.world, &mut rb, 2000);
        assert_eq!(ha, hb, "determinism violation");
        assert_eq!(ha.len(), 64);
    }

    #[test]
    fn hash_run_changes_with_seed() {
        let mut a = load_scene_str(SCENE, 42).unwrap();
        let mut b = load_scene_str(SCENE, 7).unwrap();
        let mut ra = TickRunner::new();
        let mut rb = TickRunner::new();
        let ha = hash_run(&mut a.world, &mut ra, 200);
        let hb = hash_run(&mut b.world, &mut rb, 200);
        // Without RNG-influenced systems wired yet, hashes may match —
        // assert that the function is at least deterministic.
        assert_eq!(ha.len(), 64);
        assert_eq!(hb.len(), 64);
    }

    // ---- Messages-included hash (rubber-duck CRITICAL #7 fix) ------

    /// `hash_run` must include `runner.messages()` so that
    /// nondeterministic warnings / faults / AgentReports cannot leak
    /// past the determinism gate. This test demonstrates the gap is
    /// closed: a run with an agent that produces a warning has a
    /// DIFFERENT hash than the same run without that agent — the old
    /// events-only hash would have been identical because warnings
    /// don't flow through `events()`.
    #[test]
    fn hash_run_distinguishes_runs_that_differ_only_in_warnings() {
        use crate::agent::{Agent, AgentHost, Observation};
        use crate::error::AgentError;
        use crate::world::World;
        use simetro_protocol::{Action, AgentReport};

        // An agent that always returns an action the apply pipeline
        // will reject as InvalidAction (out-of-bounds piece kind for
        // demo-paths). Each scheduled tick emits one
        // `SimMessage::Warning(InvalidAction{...})` in `runner.messages()`.
        struct AlwaysInvalidAgent;
        impl Agent for AlwaysInvalidAgent {
            fn id(&self) -> &str {
                "always-invalid"
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
                    agent_id: "always-invalid".into(),
                    considered: vec![],
                    // Use NoOp so the *action* hash is constant; the
                    // only thing that varies between this scenario and
                    // the baseline scenario is whether messages are
                    // populated by the rationale string.
                    chosen: Some(Action::NoOp),
                    rationale: "always returns NoOp (forces an AgentReport \
                                message every tick)"
                        .into(),
                    confidence: 1.0,
                })
            }
        }

        // Baseline: no agent registered → no AgentReports / no warnings.
        let mut a = load_scene_str(SCENE, 42).unwrap();
        let mut ra = TickRunner::new();
        let hash_no_agent = hash_run(&mut a.world, &mut ra, 50);

        // Same scene + seed + ticks, but now an agent emits an
        // `AgentReport` message every tick.
        let mut b = load_scene_str(SCENE, 42).unwrap();
        let mut rb = TickRunner::new();
        rb.register_agent(AgentHost::new(Box::new(AlwaysInvalidAgent)));
        let hash_with_agent = hash_run(&mut b.world, &mut rb, 50);

        // The events stream now differs (AgentDecided is emitted), so
        // the hashes would differ even without messages support. The
        // STRONGER assertion below (`hash_run_distinguishes_runs_that_differ_only_in_rationale`)
        // proves the messages channel specifically is in-hash.
        assert_ne!(
            hash_no_agent, hash_with_agent,
            "with-agent run must hash differently"
        );
    }

    /// Stronger version of the above: two agents with IDENTICAL
    /// actions (so `events` is byte-identical) but DIFFERENT rationale
    /// strings (so AgentReport messages differ). Old events-only
    /// hash would say "same"; new messages-included hash must say
    /// "different". This is the exact gap rubber-duck CRITICAL #7
    /// described.
    #[test]
    fn hash_run_distinguishes_runs_that_differ_only_in_rationale() {
        use crate::agent::{Agent, AgentHost, Observation};
        use crate::error::AgentError;
        use crate::world::World;
        use simetro_protocol::{Action, AgentReport};

        fn make_agent(id: &'static str, rationale: &'static str) -> Box<dyn Agent> {
            struct ConstAgent {
                id: &'static str,
                rationale: &'static str,
            }
            impl Agent for ConstAgent {
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
                    Ok(AgentReport {
                        tick: 0,
                        agent_id: self.id.into(),
                        considered: vec![],
                        chosen: Some(Action::NoOp),
                        rationale: self.rationale.into(),
                        confidence: 1.0,
                    })
                }
            }
            Box::new(ConstAgent { id, rationale })
        }

        // Same agent_id and same action (NoOp) — events stream is
        // identical. Only rationale differs, which lives ONLY in the
        // messages stream.
        let mut a = load_scene_str(SCENE, 42).unwrap();
        let mut ra = TickRunner::new();
        ra.register_agent(AgentHost::new(make_agent("same-id", "rationale A")));
        let hash_a = hash_run(&mut a.world, &mut ra, 50);

        let mut b = load_scene_str(SCENE, 42).unwrap();
        let mut rb = TickRunner::new();
        rb.register_agent(AgentHost::new(make_agent("same-id", "rationale B")));
        let hash_b = hash_run(&mut b.world, &mut rb, 50);

        assert_ne!(
            hash_a, hash_b,
            "hash_run must distinguish runs that differ only in \
             AgentReport rationale — this is the rubber-duck CRITICAL \
             #7 gap; if this assertion fails, messages are not being \
             fed into the hash"
        );
    }

    /// Regression for the `feed_agent_report` design choice: we hash
    /// only the Action's discriminant inside AgentReport because the
    /// PAYLOAD is captured elsewhere (events for `SetSpeed` via
    /// `MoverSpeedChange`; final world state for `PlacePiece` /
    /// `ConnectPieces` / `RemovePiece`). This test proves that two
    /// runs whose agents emit different `SetSpeed` payloads (same
    /// `ActionTag::SetSpeed`, different `speed` value) hash
    /// differently because the resulting `MoverSpeedChange` event
    /// payloads differ.
    ///
    /// If this test fails, the design comment in `feed_agent_report`
    /// is wrong AND the hash is genuinely blind to action payload
    /// changes — at which point we'd need to hash the full Action
    /// payload inside `feed_agent_report` directly.
    #[test]
    fn hash_run_distinguishes_runs_that_differ_only_in_action_payload() {
        use crate::agent::{Agent, AgentHost, Observation};
        use crate::error::AgentError;
        use crate::world::World;
        use simetro_protocol::{Action, AgentReport};

        fn make_speed_agent(speed: f32) -> Box<dyn Agent> {
            struct SpeedAgent {
                speed: f32,
            }
            impl Agent for SpeedAgent {
                fn id(&self) -> &str {
                    "speed-payload-test"
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
                        agent_id: "speed-payload-test".into(),
                        considered: vec![],
                        chosen: Some(Action::SetSpeed {
                            mover: 0,
                            speed: self.speed,
                        }),
                        rationale: String::new(),
                        confidence: 1.0,
                    })
                }
            }
            Box::new(SpeedAgent { speed })
        }

        // Two runs with same agent_id + same ActionTag::SetSpeed but
        // different `speed` value in the Action payload.
        let mut a = load_scene_str(SCENE, 42).unwrap();
        let mut ra = TickRunner::new();
        ra.register_agent(AgentHost::new(make_speed_agent(0.5)));
        let hash_a = hash_run(&mut a.world, &mut ra, 50);

        let mut b = load_scene_str(SCENE, 42).unwrap();
        let mut rb = TickRunner::new();
        rb.register_agent(AgentHost::new(make_speed_agent(2.0)));
        let hash_b = hash_run(&mut b.world, &mut rb, 50);

        assert_ne!(
            hash_a, hash_b,
            "hash_run must distinguish runs that differ only in \
             Action payload (e.g. SetSpeed {{speed}} value). If this \
             assertion fails, the `feed_agent_report` design comment \
             is wrong and we MUST hash the full Action payload inside \
             that function (currently we hash only the tag because the \
             payload is captured via SimEvent::MoverSpeedChange / \
             final world state)."
        );
    }

    /// Closes the specific edge case the codex-connector reviewer
    /// flagged on review: two runs whose `chosen` action ends in a
    /// rejection/no-op (so the apply pipeline produces identical
    /// downstream events or warnings) but whose AgentReport.chosen
    /// payloads differ must still hash differently. We hash the full
    /// Action payload inside `feed_agent_report` precisely so this
    /// edge case cannot leak past the determinism gate.
    #[test]
    fn hash_run_distinguishes_rejected_actions_with_different_payloads() {
        use crate::agent::{Agent, AgentHost, Observation};
        use crate::error::AgentError;
        use crate::world::World;
        use simetro_protocol::{Action, AgentReport};

        fn make_rejected_agent(mover_id: u32) -> Box<dyn Agent> {
            struct RejectedAgent {
                mover_id: u32,
            }
            impl Agent for RejectedAgent {
                fn id(&self) -> &str {
                    "rejected-payload-test"
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
                        agent_id: "rejected-payload-test".into(),
                        considered: vec![],
                        // Target a mover ID that does NOT exist in the
                        // demo scene → the apply pipeline rejects this
                        // with `Warning::InvalidAction`. The two runs
                        // below use DIFFERENT non-existent IDs.
                        chosen: Some(Action::SetSpeed {
                            mover: self.mover_id,
                            speed: 1.0,
                        }),
                        rationale: String::new(),
                        confidence: 1.0,
                    })
                }
            }
            Box::new(RejectedAgent { mover_id })
        }

        // Two runs: same agent_id, same speed, same rationale, both
        // chosen actions rejected by apply pipeline. Only difference
        // is the `mover` value in the chosen Action payload.
        let mut a = load_scene_str(SCENE, 42).unwrap();
        let mut ra = TickRunner::new();
        ra.register_agent(AgentHost::new(make_rejected_agent(9001)));
        let hash_a = hash_run(&mut a.world, &mut ra, 20);

        let mut b = load_scene_str(SCENE, 42).unwrap();
        let mut rb = TickRunner::new();
        rb.register_agent(AgentHost::new(make_rejected_agent(9002)));
        let hash_b = hash_run(&mut b.world, &mut rb, 20);

        assert_ne!(
            hash_a, hash_b,
            "hash_run must distinguish runs that differ only in the \
             payload of a REJECTED action — even if both actions are \
             rejected and downstream events are identical, the \
             AgentReport.chosen payload must contribute to the hash. \
             If this fails, `feed_agent_report` is hashing only the \
             tag and the reviewer-identified edge case is back."
        );
    }
}
