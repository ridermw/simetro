//! WebSocket text-message framing for the v1 wire protocol.
//!
//! This module intentionally does not open sockets or choose a runtime.
//! It defines the language-neutral payload that a Rust, Python, JS, or
//! other WebSocket implementation sends in each text message: exactly one
//! JSON [`Envelope`] using the current schema version.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

use crate::{Envelope, SCHEMA_VERSION};

pub use crate::capabilities::{CAPABILITY_ACTIONS_V1, CAPABILITY_EXTERNAL_AGENT};

/// WebSocket subprotocol advertised by simetro v1 protocol endpoints.
pub const SUBPROTOCOL: &str = "simetro.v1";

/// Errors found while encoding or decoding a WebSocket text message.
#[derive(Debug, Error)]
pub enum WebSocketProtocolError {
    #[error("websocket protocol JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("schema version mismatch: expected {expected}, found {found}")]
    SchemaMismatch { expected: u32, found: u32 },
}

/// Serialize one protocol envelope as the payload of a WebSocket text message.
///
/// # Errors
/// Returns [`WebSocketProtocolError::SchemaMismatch`] if the caller is about
/// to send a non-current envelope, or [`WebSocketProtocolError::Json`] if
/// serialization fails.
pub fn encode_text<T>(envelope: &Envelope<T>) -> Result<String, WebSocketProtocolError>
where
    T: Serialize,
{
    ensure_current(envelope)?;
    Ok(serde_json::to_string(envelope)?)
}

/// Parse one WebSocket text message into a protocol envelope.
///
/// # Errors
/// Returns [`WebSocketProtocolError::SchemaMismatch`] before the message is
/// accepted by transport code if `schema_version` does not match this build.
/// Invalid JSON or payload shape returns [`WebSocketProtocolError::Json`].
pub fn decode_text<T>(text: &str) -> Result<Envelope<T>, WebSocketProtocolError>
where
    T: DeserializeOwned,
{
    let header: EnvelopeHeader = serde_json::from_str(text)?;
    ensure_schema_version(header.schema_version)?;
    let envelope: Envelope<T> = serde_json::from_str(text)?;
    Ok(envelope)
}

fn ensure_current<T>(envelope: &Envelope<T>) -> Result<(), WebSocketProtocolError> {
    ensure_schema_version(envelope.schema_version)
}

fn ensure_schema_version(schema_version: u32) -> Result<(), WebSocketProtocolError> {
    if schema_version == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(WebSocketProtocolError::SchemaMismatch {
            expected: SCHEMA_VERSION,
            found: schema_version,
        })
    }
}

#[derive(Debug, Deserialize)]
struct EnvelopeHeader {
    schema_version: u32,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::{Action, AgentMessage, SimMessage, WarningPayload};

    #[test]
    fn agent_action_text_roundtrips() {
        let envelope = Envelope::new(
            7,
            AgentMessage::Action(Action::SetSpeed {
                mover: 3,
                speed: 1.25,
            }),
        );

        let text = encode_text(&envelope).expect("encode websocket text");
        let decoded: Envelope<AgentMessage> = decode_text(&text).expect("decode websocket text");

        assert_eq!(decoded.schema_version, SCHEMA_VERSION);
        assert_eq!(decoded.seq, 7);
        assert!(matches!(
            decoded.payload,
            AgentMessage::Action(Action::SetSpeed {
                mover: 3,
                speed: 1.25
            })
        ));
    }

    #[test]
    fn external_agent_connect_shape_is_stable_json() {
        let envelope = Envelope::new(
            1,
            AgentMessage::Connect {
                agent_id: "python-bot".into(),
                capabilities: vec![
                    CAPABILITY_EXTERNAL_AGENT.into(),
                    CAPABILITY_ACTIONS_V1.into(),
                ],
            },
        );

        let text = encode_text(&envelope).expect("encode websocket text");

        assert_eq!(
            text,
            concat!(
                r#"{"schema_version":1,"seq":1,"payload":{"kind":"connect","payload":{"#,
                r#""agent_id":"python-bot","capabilities":["external-agent","actions-v1"]}}}"#
            )
        );
    }

    #[test]
    fn sim_warning_text_roundtrips() {
        let envelope = Envelope::new(
            9,
            SimMessage::Warning(WarningPayload::Behind {
                lag_frames: 2,
                agent_id: None,
            }),
        );

        let text = encode_text(&envelope).expect("encode websocket text");
        let decoded: Envelope<SimMessage> = decode_text(&text).expect("decode websocket text");

        assert_eq!(decoded.seq, 9);
        assert!(matches!(
            decoded.payload,
            SimMessage::Warning(WarningPayload::Behind {
                lag_frames: 2,
                agent_id: None
            })
        ));
    }

    #[test]
    fn encode_rejects_non_current_schema() {
        let envelope = Envelope {
            schema_version: SCHEMA_VERSION + 1,
            seq: 1,
            payload: AgentMessage::Heartbeat,
        };

        let err = encode_text(&envelope).expect_err("mismatched schema should fail");

        assert!(matches!(
            err,
            WebSocketProtocolError::SchemaMismatch {
                expected: SCHEMA_VERSION,
                found
            } if found == SCHEMA_VERSION + 1
        ));
    }

    #[test]
    fn decode_rejects_non_current_schema() {
        let text = concat!(
            r#"{"schema_version":999,"seq":1,"payload":{"kind":"future_agent_message","#,
            r#""payload":{"field":"unknown-to-v1"}}}"#
        );

        let err = decode_text::<AgentMessage>(text).expect_err("mismatched schema should fail");

        assert!(matches!(
            err,
            WebSocketProtocolError::SchemaMismatch {
                expected: SCHEMA_VERSION,
                found: 999
            }
        ));
    }
}
