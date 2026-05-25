//! Copilot CLI SDK backend.
//!
//! Stub backend: returns `LlmError::NotAuthenticated` from every
//! invocation. This keeps the Backend trait honest without making live
//! provider calls part of the deterministic `scenario_language_v1` work.

use crate::backend::{Backend, BackendRequest, BackendResponse};
use crate::error::LlmError;
use async_trait::async_trait;

/// Configuration for the Copilot backend. All fields are optional while
/// live provider wiring remains feature-gated/default-off.
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
        // Explicitly unimplemented until live provider wiring is promoted
        // by the active roadmap. scenario_language_v1 uses simulated agents by default.
        Err(LlmError::NotAuthenticated)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::backend::ToolSpec;

    #[tokio::test]
    async fn stub_returns_not_authenticated() {
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
