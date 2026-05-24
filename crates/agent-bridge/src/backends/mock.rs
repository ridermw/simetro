//! Scripted backend used by tests and offline dev.
//!
//! Step 1 stub. Real implementation in Step 15.

use crate::backend::{Backend, BackendRequest, BackendResponse};
use crate::error::LlmError;
use async_trait::async_trait;

pub struct MockBackend;

#[async_trait]
impl Backend for MockBackend {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn invoke(&self, _req: BackendRequest) -> Result<BackendResponse, LlmError> {
        Ok(BackendResponse {
            raw: String::new(),
            tool_calls: Vec::new(),
        })
    }
}
