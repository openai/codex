use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

pub mod internal_qwen;

pub use internal_qwen::InternalQwenProvider;
pub use internal_qwen::InternalQwenProviderConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_parameters: Option<serde_json::Value>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<serde_json::Value>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: None,
            extra_parameters: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    #[serde(default)]
    pub message: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDelta {
    pub delta: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("operation `{operation}` is not implemented for provider `{provider}`")]
    Unsupported {
        operation: &'static str,
        provider: &'static str,
    },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolMode {
    Strict,
    Json,
    None,
}

impl Default for ToolMode {
    fn default() -> Self {
        Self::None
    }
}

pub trait LlmProvider: Send + Sync {
    type Stream: Iterator<Item = ChatDelta> + Send;

    fn name(&self) -> &'static str;
    fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LlmError>;
    fn chat_stream(&self, req: ChatRequest) -> Result<Self::Stream, LlmError>;
    fn supports_tools(&self) -> bool;
    fn strict_tools(&self) -> bool;
}
