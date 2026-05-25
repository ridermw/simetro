//! Bridge harness: glue between the engine's wire protocol and an
//! LLM Backend.
//!
//! ```text
//!     engine                bridge                  Backend
//!       │                     │                        │
//!       │  AgentMessage(future)   │                        │
//!       ├────────────────────▶│                        │
//!       │                     │  BackendRequest        │
//!       │                     ├───────────────────────▶│
//!       │                     │  BackendResponse       │
//!       │                     │◀───────────────────────│
//!       │  Action / Warning   │                        │
//!       │◀────────────────────│                        │
//! ```
//!
//! The current bridge owns the invoke/parse path. `parse_tool_call`
//! converts a backend `ToolCall` into a typed `Action` (or a typed
//! `LlmError`); protocol I/O can be layered around it when live
//! provider wiring is enabled.

use std::sync::Arc;

use simetro_protocol::Action;

use crate::backend::{Backend, BackendRequest, BackendResponse, ToolCall};
use crate::error::LlmError;
use crate::tools::{action_tool_specs, names};

/// Owns a configured backend and exposes a high-level
/// `decide(agent_id, prompt)` that walks the full invoke → parse →
/// Action pipeline. Cheap to clone (backend is `Arc`).
#[derive(Clone)]
pub struct Bridge {
    backend: Arc<dyn Backend>,
}

impl Bridge {
    #[must_use]
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self { backend }
    }

    #[must_use]
    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    /// One full decision cycle: invoke backend with the canonical
    /// tool specs, then parse the first tool call into an `Action`.
    ///
    /// # Errors
    /// Returns `LlmError::Refused` if the backend produced no tool
    /// calls; `LlmError::MalformedResponse` if the first tool call
    /// has unparseable arguments or an unknown name; otherwise
    /// propagates the backend's error.
    pub async fn decide(&self, agent_id: &str, prompt: String) -> Result<Action, LlmError> {
        let req = BackendRequest {
            agent_id: agent_id.into(),
            prompt,
            tools: action_tool_specs(),
        };
        let resp = self.backend.invoke(req).await?;
        first_action(&resp, agent_id)
    }
}

/// Pick the first tool call out of a response and parse it into a
/// typed `Action`. Pure function — no IO — used by both `Bridge` and
/// tests directly.
///
/// # Errors
/// - `LlmError::Refused` — the model produced no tool calls AND the
///   raw text indicates a refusal (`"refuse"` / `"can't"` / `"won't"`).
/// - `LlmError::MalformedResponse` — empty tool calls without a
///   refusal cue, unknown tool name, or unparseable arguments.
pub fn first_action(resp: &BackendResponse, agent_id: &str) -> Result<Action, LlmError> {
    let Some(tc) = resp.tool_calls.first() else {
        if looks_like_refusal(&resp.raw) {
            return Err(LlmError::Refused {
                agent_id: agent_id.into(),
                message: resp.raw.clone(),
            });
        }
        return Err(LlmError::MalformedResponse {
            agent_id: agent_id.into(),
            raw: resp.raw.clone(),
        });
    };
    parse_tool_call(tc, agent_id)
}

/// Convert one `ToolCall` into a typed `Action`.
///
/// # Errors
/// `LlmError::MalformedResponse` for unknown tools or invalid JSON.
pub fn parse_tool_call(tc: &ToolCall, agent_id: &str) -> Result<Action, LlmError> {
    let raw = || tc.arguments_json.clone();
    let malformed = |_e: serde_json::Error| LlmError::MalformedResponse {
        agent_id: agent_id.into(),
        raw: raw(),
    };

    match tc.name.as_str() {
        names::NO_OP => Ok(Action::NoOp),
        names::SET_SPEED => {
            #[derive(serde::Deserialize)]
            struct A {
                mover: u32,
                speed: f32,
            }
            let a: A = serde_json::from_str(&tc.arguments_json).map_err(malformed)?;
            Ok(Action::SetSpeed {
                mover: a.mover,
                speed: a.speed,
            })
        }
        names::PLACE_PIECE => {
            #[derive(serde::Deserialize)]
            struct A {
                piece_kind: String,
                pos: [f32; 2],
            }
            let a: A = serde_json::from_str(&tc.arguments_json).map_err(malformed)?;
            Ok(Action::PlacePiece {
                piece_kind: a.piece_kind,
                pos: a.pos,
            })
        }
        names::CONNECT_PIECES => {
            #[derive(serde::Deserialize)]
            struct A {
                from: u32,
                to: u32,
            }
            let a: A = serde_json::from_str(&tc.arguments_json).map_err(malformed)?;
            Ok(Action::ConnectPieces {
                from: a.from,
                to: a.to,
            })
        }
        names::REMOVE_PIECE => {
            #[derive(serde::Deserialize)]
            struct A {
                id: u32,
            }
            let a: A = serde_json::from_str(&tc.arguments_json).map_err(malformed)?;
            Ok(Action::RemovePiece { id: a.id })
        }
        names::DEFINE_RESOURCE => {
            #[derive(serde::Deserialize)]
            struct A {
                name: String,
                color: u8,
            }
            let a: A = serde_json::from_str(&tc.arguments_json).map_err(malformed)?;
            Ok(Action::DefineResource {
                name: a.name,
                color: a.color,
            })
        }
        names::ADD_PRODUCER => {
            #[derive(serde::Deserialize)]
            struct A {
                resource: String,
                amount: u64,
                interval_ticks: u32,
            }
            let a: A = serde_json::from_str(&tc.arguments_json).map_err(malformed)?;
            Ok(Action::AddProducer {
                resource: a.resource,
                amount: a.amount,
                interval_ticks: a.interval_ticks,
            })
        }
        names::ADD_CONSUMER => {
            #[derive(serde::Deserialize)]
            struct A {
                resource: String,
                amount: u64,
                interval_ticks: u32,
            }
            let a: A = serde_json::from_str(&tc.arguments_json).map_err(malformed)?;
            Ok(Action::AddConsumer {
                resource: a.resource,
                amount: a.amount,
                interval_ticks: a.interval_ticks,
            })
        }
        names::SET_GOAL => {
            #[derive(serde::Deserialize)]
            struct A {
                goal: String,
            }
            let a: A = serde_json::from_str(&tc.arguments_json).map_err(malformed)?;
            Ok(Action::SetGoal { goal: a.goal })
        }
        _ => Err(LlmError::MalformedResponse {
            agent_id: agent_id.into(),
            raw: raw(),
        }),
    }
}

fn looks_like_refusal(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("refuse")
        || lower.contains("can't help")
        || lower.contains("won't help")
        || lower.contains("cannot help")
        || lower.contains("i'm sorry")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::backends::mock::{MockBackend, MockTurn};

    fn bridge_with(turns: Vec<MockTurn>) -> Bridge {
        Bridge::new(Arc::new(MockBackend::with_responses(turns)))
    }

    #[tokio::test]
    async fn decide_returns_set_speed_action() {
        let b = bridge_with(vec![MockTurn::Ok(BackendResponse {
            raw: "ok".into(),
            tool_calls: vec![ToolCall {
                name: "set_speed".into(),
                arguments_json: r#"{"mover":1,"speed":1.5}"#.into(),
            }],
        })]);
        let action = b.decide("a", "p".into()).await.unwrap();
        assert_eq!(
            action,
            Action::SetSpeed {
                mover: 1,
                speed: 1.5
            }
        );
    }

    #[tokio::test]
    async fn decide_returns_no_op() {
        let b = bridge_with(vec![MockTurn::Ok(BackendResponse {
            raw: "".into(),
            tool_calls: vec![ToolCall {
                name: "no_op".into(),
                arguments_json: "{}".into(),
            }],
        })]);
        assert_eq!(b.decide("a", "p".into()).await.unwrap(), Action::NoOp);
    }

    #[tokio::test]
    async fn malformed_json_returns_malformed_error() {
        let b = bridge_with(vec![MockTurn::Ok(BackendResponse {
            raw: "".into(),
            tool_calls: vec![ToolCall {
                name: "set_speed".into(),
                arguments_json: "{not json".into(),
            }],
        })]);
        let err = b.decide("a", "p".into()).await.unwrap_err();
        assert!(matches!(err, LlmError::MalformedResponse { .. }));
    }

    #[tokio::test]
    async fn unknown_tool_name_is_malformed() {
        let b = bridge_with(vec![MockTurn::Ok(BackendResponse {
            raw: "".into(),
            tool_calls: vec![ToolCall {
                name: "fly_to_the_moon".into(),
                arguments_json: "{}".into(),
            }],
        })]);
        let err = b.decide("a", "p".into()).await.unwrap_err();
        assert!(matches!(err, LlmError::MalformedResponse { .. }));
    }

    #[tokio::test]
    async fn refusal_text_is_classified_as_refused() {
        let b = bridge_with(vec![MockTurn::Ok(BackendResponse {
            raw: "I refuse to do that.".into(),
            tool_calls: vec![],
        })]);
        let err = b.decide("a", "p".into()).await.unwrap_err();
        assert!(matches!(err, LlmError::Refused { .. }));
    }

    #[tokio::test]
    async fn empty_response_without_refusal_cue_is_malformed() {
        let b = bridge_with(vec![MockTurn::Ok(BackendResponse {
            raw: "ok".into(),
            tool_calls: vec![],
        })]);
        let err = b.decide("a", "p".into()).await.unwrap_err();
        assert!(matches!(err, LlmError::MalformedResponse { .. }));
    }

    #[tokio::test]
    async fn backend_error_propagates() {
        let b = bridge_with(vec![MockTurn::Err(LlmError::RateLimited {
            retry_after_ms: 100,
        })]);
        let err = b.decide("a", "p".into()).await.unwrap_err();
        assert!(matches!(err, LlmError::RateLimited { .. }));
    }

    #[test]
    fn parse_place_piece_roundtrips() {
        let tc = ToolCall {
            name: "place_piece".into(),
            arguments_json: r#"{"piece_kind":"node","pos":[1.0,2.0]}"#.into(),
        };
        let a = parse_tool_call(&tc, "a").unwrap();
        assert_eq!(
            a,
            Action::PlacePiece {
                piece_kind: "node".into(),
                pos: [1.0, 2.0],
            }
        );
    }

    #[test]
    fn parse_remove_piece_roundtrips() {
        let tc = ToolCall {
            name: "remove_piece".into(),
            arguments_json: r#"{"id":7}"#.into(),
        };
        let a = parse_tool_call(&tc, "a").unwrap();
        assert_eq!(a, Action::RemovePiece { id: 7 });
    }
}
