//! AgentLog — append-only JSONL log of agent decisions (PLAN §15).
//!
//! Each agent action produces one line. The log lives on disk under
//! `~/.local/share/simetro/logs/` or a path the embedder picks. When
//! the sink fails (disk full, permission denied) the log falls back
//! to an in-memory ring buffer so the simulation never blocks; the
//! engine surfaces this via `Warning::AgentLogSlow` (PLAN §13 chaos
//! test 3, §17.3).
//!
//! ```text
//!   tick → observation → agent.act() → AgentReport
//!                                          │
//!                                          ▼
//!                                   ┌──────────────┐
//!                                   │  AgentLog    │
//!                                   │              │
//!                       try write ──▶ Sink (file)  │
//!                                   │     │ on err │
//!                                   │     ▼        │
//!                                   │   ring        │
//!                                   │ (bounded VecDeque)
//!                                   └──────────────┘
//! ```
//!
//! Replay (P2) reads the log back: re-emit the parsed action without
//! re-invoking the LLM. Captured `observation_hash` validates that the
//! engine reached the same point on the same seed (PLAN §16).

use std::collections::VecDeque;
use std::io::Write;

use serde::{Deserialize, Serialize};
use simetro_protocol::{Action, WarningPayload};

use crate::agent::Observation;
use crate::components::MoverState;

const DEFAULT_RING_CAP: usize = 4096;

/// One line in the AgentLog. Serializes as a single JSON object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentLogEntry {
    pub tick: u64,
    pub agent_id: String,
    /// Deterministic FNV-1a hash of the observation. Replay uses this
    /// to verify the engine reached the same state on a re-run.
    pub observation_hash: u64,
    /// Raw text from an LLM backend, if any. Native built-in agents
    /// leave this `None`.
    pub raw_response: Option<String>,
    pub parsed_action: Option<Action>,
    pub considered_count: usize,
    pub rationale: String,
}

impl AgentLogEntry {
    /// Build an entry from an agent's observation + report. Computes
    /// the observation hash for replay verification.
    #[must_use]
    pub fn new(
        obs: &Observation,
        agent_id: &str,
        chosen: Option<Action>,
        considered_count: usize,
        rationale: String,
        raw_response: Option<String>,
    ) -> Self {
        Self {
            tick: obs.tick,
            agent_id: agent_id.to_string(),
            observation_hash: observation_hash(obs),
            raw_response,
            parsed_action: chosen,
            considered_count,
            rationale,
        }
    }
}

/// Deterministic 64-bit hash of an observation. FNV-1a (no random
/// seed) so two runs of the same scene + seed produce identical
/// hashes (PLAN §16).
#[must_use]
pub fn observation_hash(obs: &Observation) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mix = |h: &mut u64, x: u64| {
        *h ^= x;
        *h = h.wrapping_mul(0x0000_0100_0000_01B3);
    };
    mix(&mut h, obs.tick);
    for m in &obs.movers {
        mix(&mut h, u64::from(m.id.0));
        mix(&mut h, u64::from(m.speed.to_bits()));
        mix(&mut h, u64::from(m.home_path.0));
        match m.state {
            MoverState::Empty => mix(&mut h, 0xE0),
            MoverState::Waiting { at } => {
                mix(&mut h, 0xE1);
                mix(&mut h, u64::from(at.0));
            }
            MoverState::Traveling { path, progress } => {
                mix(&mut h, 0xE2);
                mix(&mut h, u64::from(path.0));
                mix(&mut h, u64::from(progress.to_bits()));
            }
        }
    }
    h
}

/// Append-only log writer with a ring-buffer fallback.
pub struct AgentLog {
    sink: Box<dyn Write + Send>,
    ring: VecDeque<String>,
    ring_cap: usize,
    /// True once a sink failure has caused us to fall back to the ring.
    degraded: bool,
    /// Number of entries dropped because the ring was full while
    /// degraded.
    dropped: u64,
}

impl AgentLog {
    /// Wrap a writer (typically a `BufWriter<File>`). The ring
    /// fallback holds at most `DEFAULT_RING_CAP` lines.
    pub fn new(sink: Box<dyn Write + Send>) -> Self {
        Self::with_capacity(sink, DEFAULT_RING_CAP)
    }

    pub fn with_capacity(sink: Box<dyn Write + Send>, ring_cap: usize) -> Self {
        Self {
            sink,
            ring: VecDeque::with_capacity(ring_cap.min(64)),
            ring_cap,
            degraded: false,
            dropped: 0,
        }
    }

    /// Open (or create + append to) a file-backed log at `path`. Parent
    /// directories are created if missing.
    ///
    /// # Errors
    /// Propagates any IO error from creating the directory or opening
    /// the file.
    pub fn open_file(path: &std::path::Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let writer = std::io::BufWriter::new(file);
        Ok(Self::new(Box::new(writer)))
    }

    /// Force the log into degraded (ring) mode. Used by chaos tests
    /// (PLAN §17.3 slow_agent_log_disk).
    pub fn force_degrade(&mut self) {
        self.degraded = true;
    }

    /// True iff a sink failure has switched us to the ring buffer.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Count of entries dropped because the ring was full while in
    /// degraded mode.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Snapshot of the ring contents (for tests and replay tooling).
    #[must_use]
    pub fn ring_snapshot(&self) -> Vec<String> {
        self.ring.iter().cloned().collect()
    }

    /// Append one entry.
    ///
    /// Returns `Some(WarningPayload::AgentLogSlow)` the first time we
    /// fall back to the ring so the caller can surface it via
    /// `SimMessage::Warning`. Subsequent failures while already
    /// degraded return `None` (caller already knows).
    pub fn append(&mut self, entry: &AgentLogEntry) -> Option<WarningPayload> {
        let line = match serde_json::to_string(entry) {
            Ok(s) => s,
            Err(_) => {
                // Serialization failure is engine-internal and should
                // never happen with our types; record and degrade.
                self.push_ring(format!(
                    "{{\"error\":\"serialize\",\"agent_id\":\"{}\"}}",
                    entry.agent_id
                ));
                return self.first_degrade();
            }
        };

        if self.degraded {
            self.push_ring(line);
            return None;
        }

        // Try sink. On error, switch to ring and signal once.
        let res = (|| -> std::io::Result<()> {
            self.sink.write_all(line.as_bytes())?;
            self.sink.write_all(b"\n")?;
            Ok(())
        })();

        if res.is_err() {
            self.push_ring(line);
            return self.first_degrade();
        }

        None
    }

    /// Flush the underlying sink. Returns the underlying io error
    /// without degrading (caller decides).
    ///
    /// # Errors
    /// Whatever the sink's flush returned.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }

    fn push_ring(&mut self, line: String) {
        if self.ring.len() >= self.ring_cap {
            self.ring.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.ring.push_back(line);
    }

    fn first_degrade(&mut self) -> Option<WarningPayload> {
        if !self.degraded {
            self.degraded = true;
            Some(WarningPayload::AgentLogSlow)
        } else {
            None
        }
    }
}

impl std::fmt::Debug for AgentLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentLog")
            .field("degraded", &self.degraded)
            .field("ring_len", &self.ring.len())
            .field("ring_cap", &self.ring_cap)
            .field("dropped", &self.dropped)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::agent::MoverObservation;
    use crate::components::{MoverId, NodeId, PathId};
    use std::io;

    fn obs() -> Observation {
        Observation {
            tick: 7,
            movers: vec![MoverObservation {
                id: MoverId(1),
                state: MoverState::Waiting { at: NodeId(2) },
                speed: 1.5,
                home_path: PathId(0),
            }],
        }
    }

    #[test]
    fn observation_hash_is_deterministic() {
        let a = observation_hash(&obs());
        let b = observation_hash(&obs());
        assert_eq!(a, b);
    }

    #[test]
    fn observation_hash_changes_with_state() {
        let mut o1 = obs();
        let mut o2 = obs();
        o2.tick = 8;
        assert_ne!(observation_hash(&o1), observation_hash(&o2));
        o2 = obs();
        o2.movers[0].speed = 1.6;
        assert_ne!(observation_hash(&o1), observation_hash(&o2));
        o1.movers.push(MoverObservation {
            id: MoverId(2),
            state: MoverState::Empty,
            speed: 1.0,
            home_path: PathId(1),
        });
        assert_ne!(observation_hash(&o1), observation_hash(&obs()));
    }

    #[test]
    fn append_writes_one_line_per_entry() {
        struct Counter {
            lines: usize,
            bytes: usize,
        }
        impl io::Write for Counter {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.bytes += b.len();
                if b == b"\n" {
                    self.lines += 1;
                }
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let counter = Counter { lines: 0, bytes: 0 };
        let mut log = AgentLog::new(Box::new(counter));
        let entry = AgentLogEntry::new(&obs(), "a", Some(Action::NoOp), 1, "r".into(), None);
        assert!(log.append(&entry).is_none());
        assert!(log.append(&entry).is_none());
        assert!(!log.is_degraded());
    }

    struct AlwaysErr;
    impl io::Write for AlwaysErr {
        fn write(&mut self, _b: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("boom"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn write_failure_falls_back_to_ring_and_warns_once() {
        let mut log = AgentLog::with_capacity(Box::new(AlwaysErr), 8);
        let entry = AgentLogEntry::new(&obs(), "a", Some(Action::NoOp), 1, "r".into(), None);
        let first = log.append(&entry);
        assert!(matches!(first, Some(WarningPayload::AgentLogSlow)));
        assert!(log.is_degraded());
        // Subsequent failures don't re-warn.
        let second = log.append(&entry);
        assert!(second.is_none());
        assert_eq!(log.ring_snapshot().len(), 2);
    }

    #[test]
    fn ring_is_bounded_and_counts_drops() {
        let mut log = AgentLog::with_capacity(Box::new(AlwaysErr), 3);
        let entry = AgentLogEntry::new(&obs(), "a", Some(Action::NoOp), 1, "r".into(), None);
        for _ in 0..10 {
            let _ = log.append(&entry);
        }
        assert_eq!(log.ring_snapshot().len(), 3);
        assert_eq!(log.dropped(), 7);
    }

    #[test]
    fn force_degrade_skips_sink_immediately() {
        struct NeverCalled;
        impl io::Write for NeverCalled {
            fn write(&mut self, _b: &[u8]) -> io::Result<usize> {
                panic!("sink should not be called after force_degrade")
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut log = AgentLog::new(Box::new(NeverCalled));
        log.force_degrade();
        let entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        assert!(log.append(&entry).is_none());
        assert_eq!(log.ring_snapshot().len(), 1);
    }

    #[test]
    fn entry_roundtrips_through_json() {
        let entry = AgentLogEntry::new(
            &obs(),
            "speed_tuner_0",
            Some(Action::SetSpeed {
                mover: 1,
                speed: 1.5,
            }),
            3,
            "nudge".into(),
            Some("raw llm text".into()),
        );
        let s = serde_json::to_string(&entry).unwrap();
        let back: AgentLogEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(back, entry);
    }
}
