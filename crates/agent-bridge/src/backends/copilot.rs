//! Copilot CLI SDK backend.
//!
//! Phase-1 stub: returns `LlmError::NotAuthenticated` from every
//! invocation. Phase-2 will wire the `github-copilot-sdk` JSON-RPC
//! `Session` client; until then this exists to:
//!   - keep the Backend trait honest (Mock isn't the only impl),
//!   - reserve the configuration surface so P2 wiring is a delta only.

use crate::backend::{Backend, BackendRequest, BackendResponse};
use crate::error::LlmError;
use async_trait::async_trait;

/// Configuration for the Copilot backend. All fields are optional in
/// P1 because we never actually authenticate.
#[derive(Debug, Clone, Default)]
pub struct CopilotConfig {
    /// Model id (e.g. "gpt-4o", "claude-opus-4"). None → backend
    /// default.
    pub model: Option<String>,
    /// Override the system prompt baked into the bridge.
    pub system_prompt: Option<String>,
    /// Request timeout in milliseconds. None → 60_000.
    pub timeout_ms: Option<u32>,
}

pub struct CopilotBackend {
    cfg: CopilotConfig,
}

impl CopilotBackend {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cfg: CopilotConfig::default(),
        }
    }

    #[must_use]
    pub fn with_config(cfg: CopilotConfig) -> Self {
        Self { cfg }
    }

    #[must_use]
    pub fn config(&self) -> &CopilotConfig {
        &self.cfg
    }
}

impl Default for CopilotBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for CopilotBackend {
    fn name(&self) -> &'static str {
        "copilot"
    }

    async fn invoke(&self, _req: BackendRequest) -> Result<BackendResponse, LlmError> {
        // P1: explicitly unimplemented; P2 wires up github-copilot-sdk.
        Err(LlmError::NotAuthenticated)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::backend::ToolSpec;

    #[tokio::test]
    async fn p1_stub_returns_not_authenticated() {
        let b = CopilotBackend::new();
        let err = b
            .invoke(BackendRequest {
                agent_id: "x".into(),
                prompt: "p".into(),
                tools: vec![ToolSpec {
                    name: "no_op".into(),
                    json_schema: "{}".into(),
                }],
            })
            .await
            .unwrap_err();
        assert!(matches!(err, LlmError::NotAuthenticated));
        assert_eq!(b.name(), "copilot");
    }

    #[test]
    fn config_is_stored() {
        let cfg = CopilotConfig {
            model: Some("gpt-4o".into()),
            ..Default::default()
        };
        let b = CopilotBackend::with_config(cfg.clone());
        assert_eq!(b.config().model.as_deref(), Some("gpt-4o"));
    }
}
