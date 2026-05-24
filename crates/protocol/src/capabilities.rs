//! Stable agent capability strings.
//!
//! These strings are intentionally transport-neutral: WebSocket agents,
//! future WASM plugin agents, and replay/test harnesses can all advertise
//! the same contracts without depending on a provider-specific backend.

/// Agent runs outside the engine process over a transport such as WebSocket.
pub const CAPABILITY_EXTERNAL_AGENT: &str = "external-agent";

/// Agent is loaded as a sandboxed WASM plugin.
pub const CAPABILITY_WASM_PLUGIN_AGENT: &str = "wasm-plugin-agent";

/// Agent accepts the v1 observation JSON contract.
pub const CAPABILITY_OBSERVATIONS_V1: &str = "observations-v1";

/// Agent can emit the v1 [`Action`](crate::Action) contract.
pub const CAPABILITY_ACTIONS_V1: &str = "actions-v1";

/// Agent requests access to authoring actions (`place/connect/remove`).
pub const CAPABILITY_AUTHOR_ACTIONS_V1: &str = "author-actions-v1";

/// Agent can include deterministic decision metadata for AgentLog.
pub const CAPABILITY_AGENT_LOG_V1: &str = "agent-log-v1";
