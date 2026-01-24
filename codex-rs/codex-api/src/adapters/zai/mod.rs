//! Z.AI adapter for codex-api.
//!
//! This adapter converts between codex-api's canonical types and the z-ai-sdk
//! crate's wire format, enabling Z.AI and ZhipuAI models to be used with the codex agent.
//!
//! # Design Notes
//!
//! - **Non-streaming only**: Uses the `chat.completions.create()` endpoint.
//! - **Thinking support**: Maps reasoning_content to Reasoning items (ultrathink).
//! - **Tool use**: Maps tool_calls to FunctionCall events.
//! - **Multi-region**: Supports both Z.AI (api.z.ai) and ZhipuAI (open.bigmodel.cn) endpoints.

pub mod convert;
mod error;

use super::AdapterConfig;
use super::GenerateResult;
use super::ProviderAdapter;
use crate::common::Prompt;
use crate::error::ApiError;
use async_trait::async_trait;
use http::StatusCode;
use z_ai_sdk::ChatCompletionsCreateParams;
use z_ai_sdk::ClientConfig;
use z_ai_sdk::MessageParam;
use z_ai_sdk::ThinkingConfig;
use z_ai_sdk::ToolChoice;
use z_ai_sdk::ZaiClient;

pub use error::map_error;

/// Z.AI adapter for codex-api.
#[derive(Debug)]
pub struct ZaiAdapter;

impl ZaiAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ZaiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderAdapter for ZaiAdapter {
    fn name(&self) -> &str {
        "zai"
    }

    async fn generate(
        &self,
        prompt: &Prompt,
        config: &AdapterConfig,
    ) -> Result<GenerateResult, ApiError> {
        // Get API key
        let api_key = config.api_key.clone().ok_or_else(|| ApiError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: "API key required for Z.AI adapter".to_string(),
        })?;

        // Build client config - use zai() by default
        let mut client_config = ClientConfig::zai(&api_key);

        if let Some(base_url) = &config.base_url {
            client_config.base_url = base_url.clone();
        }

        // Create client
        let client = ZaiClient::new(client_config).map_err(error::map_error)?;

        // Get effective base_url for context checking
        let effective_base_url = config.base_url.as_deref().unwrap_or("");

        // Convert prompt to Z.AI messages (with cross-adapter support)
        let (mut messages, system_prompt) =
            convert::prompt_to_messages(prompt, effective_base_url, &config.model);

        // Prepend system prompt as a system message
        if let Some(system) = system_prompt {
            messages.insert(0, MessageParam::system(system));
        }

        // Build chat completion parameters
        let mut params = ChatCompletionsCreateParams::new(&config.model, messages);

        // Convert and set tools
        if !prompt.tools.is_empty() {
            let tools = convert::tools_to_zai(&prompt.tools);
            if !tools.is_empty() {
                params = params.tools(tools);

                // Set tool choice based on parallel_tool_calls
                // Note: Z.AI doesn't have explicit parallel control, use "auto"
                params = params.tool_choice(ToolChoice::auto());
            }
        }

        // Apply extra configuration (temperature, max_tokens, thinking, etc.)
        apply_extra_config(&mut params, &config.extra);

        // Make API call
        let completion = client
            .chat()
            .completions()
            .create(params)
            .await
            .map_err(error::map_error)?;

        // Convert response to events
        // Pass base_url and model for model switch detection in EncryptedContent
        let effective_base_url = config.base_url.as_deref().unwrap_or("");
        let events = convert::completion_to_events(&completion, effective_base_url, &config.model)?;

        Ok(GenerateResult {
            events,
            usage: Some(convert::extract_usage(&completion.usage)),
            response_id: completion.id,
        })
    }

    fn supports_response_id(&self) -> bool {
        false // Z.AI doesn't support previous_response_id like OpenAI
    }
}

/// Apply extra configuration to ChatCompletionsCreateParams.
fn apply_extra_config(params: &mut ChatCompletionsCreateParams, extra: &Option<serde_json::Value>) {
    let Some(extra) = extra else { return };

    // Max tokens
    if let Some(max_tokens) = extra.get("max_tokens").and_then(|v| v.as_i64()) {
        params.max_tokens = Some(max_tokens as i32);
    }

    // Temperature
    if let Some(temp) = extra.get("temperature").and_then(|v| v.as_f64()) {
        params.temperature = Some(temp);
    }

    // Top P
    if let Some(top_p) = extra.get("top_p").and_then(|v| v.as_f64()) {
        params.top_p = Some(top_p);
    }

    // Stop sequences
    if let Some(stop) = extra.get("stop").and_then(|v| v.as_array()) {
        let sequences: Vec<String> = stop
            .iter()
            .filter_map(|s| s.as_str().map(String::from))
            .collect();
        if !sequences.is_empty() {
            params.stop = Some(sequences);
        }
    }

    // User ID
    if let Some(user_id) = extra.get("user_id").and_then(|v| v.as_str()) {
        params.user_id = Some(user_id.to_string());
    }

    // Thinking configuration (ultrathink support)
    if let Some(budget) = extra.get("thinking_budget_tokens").and_then(|v| v.as_i64()) {
        params.thinking = Some(ThinkingConfig::enabled_with_budget(budget as i32));
    } else if extra.get("enable_thinking").and_then(|v| v.as_bool()) == Some(true) {
        params.thinking = Some(ThinkingConfig::enabled());
    }

    // Request ID for tracing
    if let Some(request_id) = extra.get("request_id").and_then(|v| v.as_str()) {
        params.request_id = Some(request_id.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_adapter_name() {
        let adapter = ZaiAdapter::new();
        assert_eq!(adapter.name(), "zai");
    }

    #[test]
    fn test_apply_extra_config_basic() {
        let mut params = ChatCompletionsCreateParams::new("glm-4", vec![]);
        let extra = Some(serde_json::json!({
            "temperature": 0.7,
            "top_p": 0.9,
            "max_tokens": 2048
        }));

        apply_extra_config(&mut params, &extra);

        assert!((params.temperature.unwrap() - 0.7).abs() < 0.01);
        assert!((params.top_p.unwrap() - 0.9).abs() < 0.01);
        assert_eq!(params.max_tokens, Some(2048));
    }

    #[test]
    fn test_apply_extra_config_thinking() {
        let mut params = ChatCompletionsCreateParams::new("glm-4", vec![]);
        let extra = Some(serde_json::json!({
            "thinking_budget_tokens": 8192
        }));

        apply_extra_config(&mut params, &extra);

        assert!(params.thinking.is_some());
    }

    #[test]
    fn test_apply_extra_config_thinking_enabled() {
        let mut params = ChatCompletionsCreateParams::new("glm-4", vec![]);
        let extra = Some(serde_json::json!({
            "enable_thinking": true
        }));

        apply_extra_config(&mut params, &extra);

        assert!(params.thinking.is_some());
    }

    #[test]
    fn test_supports_response_id() {
        let adapter = ZaiAdapter::new();
        assert!(!adapter.supports_response_id());
    }
}
