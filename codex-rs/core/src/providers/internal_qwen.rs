use reqwest::Client;

use super::ChatDelta;
use super::ChatRequest;
use super::ChatResponse;
use super::LlmError;
use super::LlmProvider;
use super::ToolMode;

#[derive(Debug, Clone)]
pub struct InternalQwenProviderConfig {
    pub base_url: String,
    pub model: String,
    pub api_key_env: Option<String>,
    pub tool_mode: ToolMode,
}

impl InternalQwenProviderConfig {
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key_env: Option<String>,
        tool_mode: ToolMode,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            api_key_env,
            tool_mode,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InternalQwenProvider {
    client: Client,
    config: InternalQwenProviderConfig,
}

impl InternalQwenProvider {
    const NAME: &'static str = "internal-qwen";

    pub fn new(config: InternalQwenProviderConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn config(&self) -> &InternalQwenProviderConfig {
        &self.config
    }
}

impl Default for InternalQwenProvider {
    fn default() -> Self {
        Self::new(InternalQwenProviderConfig::new(
            "http://localhost:8000/v1",
            "qwen-coder",
            None,
            ToolMode::None,
        ))
    }
}

impl LlmProvider for InternalQwenProvider {
    type Stream = std::vec::IntoIter<ChatDelta>;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn chat(&self, _req: ChatRequest) -> Result<ChatResponse, LlmError> {
        Err(LlmError::Unsupported {
            operation: "chat",
            provider: Self::NAME,
        })
    }

    fn chat_stream(&self, _req: ChatRequest) -> Result<Self::Stream, LlmError> {
        Err(LlmError::Unsupported {
            operation: "chat_stream",
            provider: Self::NAME,
        })
    }

    fn supports_tools(&self) -> bool {
        !matches!(self.config.tool_mode, ToolMode::None)
    }

    fn strict_tools(&self) -> bool {
        matches!(self.config.tool_mode, ToolMode::Strict)
    }
}
