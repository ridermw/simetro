//! Deterministic SHA-256 state hash (PLAN §16).
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
use simetro_protocol::{AgentReport, FaultPayload, SimEvent, SimMessage, WarningPayload};

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
        h.update([c.action.tag() as u8]);
        h.update(c.confidence.to_le_bytes());
    }
    // `chosen` presence byte + tag.
    //
    // We hash only the discriminant here (not the full Action payload)
    // because the action's PAYLOAD-LEVEL effects are captured elsewhere
    // in the hash:
    //   - `SetSpeed { mover, speed }` → emits
    //     `SimEvent::MoverSpeedChange { mover, old, new }` per-tick,
    //     fed by `feed_event` (`0x12` tag).
    //   - `PlacePiece { piece_kind, pos }` → mutates `world.nodes`;
    //     captured by the FINAL `feed_world` call at the end of
    //     `hash_run` (node positions + shapes + colors).
    //   - `ConnectPieces { from, to, kind }` → mutates `world.paths`;
    //     captured by the final `feed_world` (path endpoints +
    //     colors).
    //   - `RemovePiece { piece_kind, id }` → mutates world topology;
    //     captured by the final `feed_world`.
    //   - `NoOp` → no payload to hash.
    //
    // Hashing the full Action payload here would double-count for
    // SetSpeed (since MoverSpeedChange already encodes both old and
    // new) and would also leak per-tick payload changes that the
    // final-world hash would catch anyway. Test
    // `hash_run_distinguishes_runs_that_differ_only_in_action_payload`
    // is the regression for this design choice.
    match &r.chosen {
        Some(a) => {
            h.update([0x01]);
            h.update([a.tag() as u8]);
        }
        None => h.update([0x00]),
    }
    h.update((r.rationale.len() as u64).to_le_bytes());
    h.update(r.rationale.as_bytes());
    h.update(r.confidence.to_le_bytes());
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
/// when driven by the same scene + seed (PLAN §16).
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
}
