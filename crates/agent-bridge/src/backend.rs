//! `Backend` trait: the boundary every LLM provider implements.

use crate::error::LlmError;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct BackendRequest {
    pub prompt: String,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone)]
pub struct BackendResponse {
    pub raw: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub json_schema: String,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub arguments_json: String,
}

#[async_trait]
pub trait Backend: Send + Sync {
    fn name(&self) -> &'static str;

    async fn invoke(&self, req: BackendRequest) -> Result<BackendResponse, LlmError>;
}
