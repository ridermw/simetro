//! Copilot CLI SDK backend.
//!
//! Step 1 stub. Real implementation in Step 15 / Phase 2.
//! Uses the `github-copilot-sdk` Rust crate; expects `gh auth` to be
//! configured, so no secret storage is needed in P1.

use crate::backend::{Backend, BackendRequest, BackendResponse};
use crate::error::LlmError;
use async_trait::async_trait;

pub struct CopilotBackend;

#[async_trait]
impl Backend for CopilotBackend {
    fn name(&self) -> &'static str {
        "copilot"
    }

    async fn invoke(&self, _req: BackendRequest) -> Result<BackendResponse, LlmError> {
        Err(LlmError::NotAuthenticated)
    }
}
