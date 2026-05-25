//! # Bridge stdio wire protocol (P2.A task 6)
//!
//! `simetro-bridge` runs as a separate process spawned by the
//! `simetro-tauri-app` shell (or `simetro-headless`). The engine and
//! bridge communicate over the child's stdin / stdout using
//! **newline-delimited JSON** envelopes — one message per line.
//!
//! ```text
//!   parent (engine host)             child (simetro-bridge)
//!         │                                  │
//!         │  Envelope<BridgeMessage>         │
//!         │ ──────── stdin (NDJSON) ───────▶ │
//!         │                                  │  parse + dispatch
//!         │                                  │  to Backend::invoke
//!         │ ◀─────── stdout (NDJSON) ──────  │
//!         │  Envelope<BridgeMessage>         │
//! ```
//!
//! ## Why NDJSON instead of length-prefix?
//!
//! - Trivial to inspect / `cat` / `grep` / replay from a recorded
//!   transcript. Bridge fixture-suite (task 11) reads NDJSON files.
//! - No framing edge cases at EOF or partial writes.
//! - JSON itself cannot contain a bare `\n` (it must be escaped), so a
//!   single newline is a safe record separator.
//!
//! ## Schema version
//!
//! Every envelope carries [`simetro_protocol::SCHEMA_VERSION`]. The
//! receiver MUST reject mismatched versions ([`is_compatible`]) — see
//! the `Hello` handshake below for negotiation. Per spec §10.1 there's
//! no silent migration; a version bump requires both ends to update.
//!
//! [`is_compatible`]: simetro_protocol::Envelope::is_compatible

use serde::{Deserialize, Serialize};
use simetro_engine::lifecycle::{AgentReply, AgentRequest};
use simetro_protocol::Envelope;
use std::io::{self, BufRead, Write};

/// Everything the bridge stdio loop can send or receive. Tagged enum
/// so a `Shutdown` line is unambiguous from a `Reply`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeMessage {
    /// Sent ONCE by either side as the first envelope on each pipe.
    /// `schema_version` lets the receiver bail early on mismatch.
    Hello {
        bridge_version: String,
        schema_version: u32,
    },
    /// Engine → bridge: process this request and return a `Reply`.
    Request(AgentRequest),
    /// Bridge → engine: response to a previous `Request` (correlated
    /// via [`AgentReply::id`]).
    Reply(AgentReply),
    /// Engine → bridge: shut down cleanly. Bridge SHOULD flush its
    /// inflight queue, write a final `Hello` (optional) or just exit
    /// with code 0.
    Shutdown,
    /// Either direction: bridge-emitted log / telemetry line. Free-
    /// form payload. Kept out of the lifecycle path so a noisy bridge
    /// can't perturb engine determinism.
    Log { level: String, message: String },
}

/// Read ONE [`Envelope<BridgeMessage>`] line from `reader`. Returns
/// `Ok(None)` on clean EOF; `Err(_)` on IO error or malformed JSON.
///
/// Blocking — async stdio is not needed because the bridge dispatches
/// one request at a time via `tokio::runtime::block_on`.
pub fn read_envelope<R: BufRead>(reader: &mut R) -> io::Result<Option<Envelope<BridgeMessage>>> {
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Ok(None);
    }
    let trimmed = line.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() {
        // Treat blank lines as a no-op so a partial flush + newline
        // doesn't crash the loop.
        return Ok(Some(Envelope::new(
            0,
            BridgeMessage::Log {
                level: "trace".to_string(),
                message: "blank line".to_string(),
            },
        )));
    }
    serde_json::from_str(trimmed).map(Some).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("malformed bridge envelope: {err}"),
        )
    })
}

/// Write ONE [`Envelope<BridgeMessage>`] line to `writer`, terminated
/// with `\n`. Flushes after each write so the parent process sees
/// every reply as soon as it's produced.
pub fn write_envelope<W: Write>(
    writer: &mut W,
    envelope: &Envelope<BridgeMessage>,
) -> io::Result<()> {
    let s = serde_json::to_string(envelope).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("envelope serialization failed: {err}"),
        )
    })?;
    writeln!(writer, "{s}")?;
    writer.flush()
}

/// Build a `Hello` envelope advertising this build's schema version.
/// The receiver MUST verify the version matches its own before
/// processing any further envelopes.
pub fn hello_envelope(seq: u64, bridge_version: impl Into<String>) -> Envelope<BridgeMessage> {
    Envelope::new(
        seq,
        BridgeMessage::Hello {
            bridge_version: bridge_version.into(),
            schema_version: simetro_protocol::SCHEMA_VERSION,
        },
    )
}

/// Build a `Shutdown` envelope.
pub fn shutdown_envelope(seq: u64) -> Envelope<BridgeMessage> {
    Envelope::new(seq, BridgeMessage::Shutdown)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use simetro_engine::lifecycle::RequestId;
    use simetro_protocol::Action;
    use std::io::Cursor;

    fn sample_request() -> AgentRequest {
        AgentRequest {
            id: RequestId {
                timeline_id: 7,
                agent_id: "trafficker".to_string(),
                source_tick: 100,
                attempt: 0,
            },
            deadline_ticks: 60,
            observation_json: "{\"tick\":100,\"movers\":[]}".to_string(),
        }
    }

    fn sample_reply(req: &AgentRequest) -> AgentReply {
        AgentReply {
            id: req.id.clone(),
            chosen: Some(Action::NoOp),
            rationale: "did the thing".to_string(),
            confidence: 0.9,
        }
    }

    #[test]
    fn hello_envelope_carries_current_schema_version() {
        let env = hello_envelope(1, "0.1.0");
        assert!(env.is_compatible());
        match &env.payload {
            BridgeMessage::Hello {
                bridge_version,
                schema_version,
            } => {
                assert_eq!(bridge_version, "0.1.0");
                assert_eq!(*schema_version, simetro_protocol::SCHEMA_VERSION);
            }
            other => panic!("expected Hello, got {other:?}"),
        }
    }

    #[test]
    fn round_trip_request_through_ndjson() {
        let env = Envelope::new(1, BridgeMessage::Request(sample_request()));
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        // Must end with a newline so peer can split on '\n'.
        assert!(buf.ends_with(b"\n"));
        // And must NOT contain a stray newline mid-payload.
        assert_eq!(buf.iter().filter(|&&b| b == b'\n').count(), 1);

        let mut reader = Cursor::new(buf);
        let parsed = read_envelope(&mut reader).unwrap().unwrap();
        assert_eq!(parsed.seq, 1);
        assert_eq!(parsed.payload, env.payload);
    }

    #[test]
    fn round_trip_reply_preserves_request_id() {
        let req = sample_request();
        let reply = sample_reply(&req);
        let env = Envelope::new(2, BridgeMessage::Reply(reply.clone()));
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let parsed = read_envelope(&mut Cursor::new(buf)).unwrap().unwrap();
        match parsed.payload {
            BridgeMessage::Reply(got) => {
                assert_eq!(got.id, reply.id);
                assert_eq!(got.chosen, Some(Action::NoOp));
                assert_eq!(got.rationale, "did the thing");
                assert!((got.confidence - 0.9).abs() < 1e-6);
            }
            other => panic!("expected Reply, got {other:?}"),
        }
    }

    #[test]
    fn shutdown_round_trips() {
        let env = shutdown_envelope(42);
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let parsed = read_envelope(&mut Cursor::new(buf)).unwrap().unwrap();
        assert!(matches!(parsed.payload, BridgeMessage::Shutdown));
        assert_eq!(parsed.seq, 42);
    }

    #[test]
    fn eof_yields_none() {
        let mut empty = Cursor::new(Vec::<u8>::new());
        assert!(read_envelope(&mut empty).unwrap().is_none());
    }

    #[test]
    fn malformed_line_is_invalid_data_error() {
        let mut reader = Cursor::new(b"not json\n".to_vec());
        let err = read_envelope(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn multi_message_stream_decodes_in_order() {
        let mut buf = Vec::new();
        write_envelope(
            &mut buf,
            &Envelope::new(1, BridgeMessage::Request(sample_request())),
        )
        .unwrap();
        write_envelope(
            &mut buf,
            &Envelope::new(2, BridgeMessage::Reply(sample_reply(&sample_request()))),
        )
        .unwrap();
        write_envelope(&mut buf, &shutdown_envelope(3)).unwrap();

        let mut reader = Cursor::new(buf);
        let seqs: Vec<u64> = std::iter::from_fn(|| read_envelope(&mut reader).ok().flatten())
            .map(|e| e.seq)
            .collect();
        assert_eq!(seqs, vec![1, 2, 3]);
    }

    #[test]
    fn log_envelope_is_skippable_metadata() {
        let env = Envelope::new(
            1,
            BridgeMessage::Log {
                level: "info".to_string(),
                message: "starting".to_string(),
            },
        );
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let parsed = read_envelope(&mut Cursor::new(buf)).unwrap().unwrap();
        match parsed.payload {
            BridgeMessage::Log { level, message } => {
                assert_eq!(level, "info");
                assert_eq!(message, "starting");
            }
            other => panic!("expected Log, got {other:?}"),
        }
    }
}
