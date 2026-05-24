//! Scripted backend used by tests and offline dev.
//!
//! `MockBackend` consumes pre-set responses from a queue, in order.
//! Use [`MockBackend::with_responses`] to seed the queue. When the
//! queue is empty, calls return `LlmError::Disconnected` so tests can
//! tell the difference between "ran out" and "haven't yet".

use std::sync::Mutex;

use crate::backend::{Backend, BackendRequest, BackendResponse, ToolCall};
use crate::error::LlmError;
use async_trait::async_trait;

pub struct MockBackend {
    /// FIFO queue of scripted results (responses or canned errors).
    queue: Mutex<std::collections::VecDeque<MockTurn>>,
}

/// One scripted result from `MockBackend`.
#[derive(Debug, Clone)]
pub enum MockTurn {
    Ok(BackendResponse),
    Err(LlmError),
}

impl MockBackend {
    /// Build an empty mock (every call returns `Disconnected`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(std::collections::VecDeque::new()),
        }
    }

    /// Seed the mock with a list of turn results to deliver in order.
    #[must_use]
    pub fn with_responses<I: IntoIterator<Item = MockTurn>>(turns: I) -> Self {
        Self {
            queue: Mutex::new(turns.into_iter().collect()),
        }
    }

    /// Convenience: respond with a single tool call (e.g., NoOp) on
    /// every subsequent invocation until the queue is reseeded.
    #[must_use]
    pub fn with_tool_call_response(name: impl Into<String>, args_json: impl Into<String>) -> Self {
        let resp = BackendResponse {
            raw: String::new(),
            tool_calls: vec![ToolCall {
                name: name.into(),
                arguments_json: args_json.into(),
            }],
        };
        Self::with_responses([MockTurn::Ok(resp)])
    }

    /// Append more scripted turns (lets tests stack expectations
    /// without rebuilding the backend).
    pub fn push(&self, turn: MockTurn) {
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(turn);
        }
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for MockBackend {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn invoke(&self, _req: BackendRequest) -> Result<BackendResponse, LlmError> {
        let next = match self.queue.lock() {
            Ok(mut q) => q.pop_front(),
            Err(_) => None,
        };
        match next {
            Some(MockTurn::Ok(resp)) => Ok(resp),
            Some(MockTurn::Err(e)) => Err(e),
            None => Err(LlmError::Disconnected),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::backend::ToolSpec;

    fn req() -> BackendRequest {
        BackendRequest {
            agent_id: "a".into(),
            prompt: "p".into(),
            tools: vec![ToolSpec {
                name: "no_op".into(),
                json_schema: "{}".into(),
            }],
        }
    }

    #[tokio::test]
    async fn empty_queue_returns_disconnected() {
        let b = MockBackend::new();
        let err = b.invoke(req()).await.unwrap_err();
        assert!(matches!(err, LlmError::Disconnected));
    }

    #[tokio::test]
    async fn returns_scripted_response_in_order() {
        let b = MockBackend::with_responses([
            MockTurn::Ok(BackendResponse {
                raw: "first".into(),
                tool_calls: vec![],
            }),
            MockTurn::Ok(BackendResponse {
                raw: "second".into(),
                tool_calls: vec![],
            }),
        ]);
        assert_eq!(b.invoke(req()).await.unwrap().raw, "first");
        assert_eq!(b.invoke(req()).await.unwrap().raw, "second");
        assert!(matches!(
            b.invoke(req()).await.unwrap_err(),
            LlmError::Disconnected
        ));
    }

    #[tokio::test]
    async fn surfaces_canned_errors() {
        let b = MockBackend::with_responses([MockTurn::Err(LlmError::RateLimited {
            retry_after_ms: 250,
        })]);
        let err = b.invoke(req()).await.unwrap_err();
        assert!(matches!(
            err,
            LlmError::RateLimited {
                retry_after_ms: 250
            }
        ));
    }

    #[tokio::test]
    async fn convenience_tool_call_response_is_emitted_once() {
        let b = MockBackend::with_tool_call_response("no_op", "{}");
        let resp = b.invoke(req()).await.unwrap();
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "no_op");
    }
}
