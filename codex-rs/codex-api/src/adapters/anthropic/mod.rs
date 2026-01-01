//! Anthropic Claude adapter for codex-api.
//!
//! This adapter converts between codex-api's canonical types and the anthropic-sdk
//! crate's wire format, enabling Claude models to be used with the codex agent.
//!
//! # Design Notes
//!
//! - **Non-streaming only**: Uses the `messages.create()` endpoint.
//! - **Thinking support**: Maps Thinking/RedactedThinking to Reasoning items.
//! - **Tool use**: Maps ToolUse/ToolResult to FunctionCall/FunctionCallOutput.

pub mod convert;
mod error;

use super::AdapterConfig;
use super::GenerateResult;
use super::ProviderAdapter;
use crate::common::Prompt;
use crate::error::ApiError;
use anthropic_sdk::Client;
use anthropic_sdk::ClientConfig;
use anthropic_sdk::MessageCreateParams;
use anthropic_sdk::ThinkingConfig;
use anthropic_sdk::ToolChoice;
use async_trait::async_trait;
use http::StatusCode;

pub use error::map_error;

/// Anthropic Claude adapter for codex-api.
#[derive(Debug)]
pub struct AnthropicAdapter;

impl AnthropicAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AnthropicAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn generate(
        &self,
        prompt: &Prompt,
        config: &AdapterConfig,
    ) -> Result<GenerateResult, ApiError> {
        // Get API key
        let api_key = config.api_key.clone().ok_or_else(|| ApiError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: "API key required for Anthropic adapter".to_string(),
        })?;

        // Build client config
        let mut client_config = ClientConfig::new(&api_key);

        if let Some(base_url) = &config.base_url {
            client_config.base_url = base_url.clone();
        }

        // Create client
        let client = Client::new(client_config).map_err(error::map_error)?;

        // Convert prompt to Anthropic messages
        let (messages, system_prompt) = convert::prompt_to_messages(prompt);

        // Extract max_tokens from extra config (required for Anthropic)
        let max_tokens = extract_max_tokens(&config.extra);

        // Build message parameters
        let mut params = MessageCreateParams::new(&config.model, max_tokens, messages);

        // Set system prompt
        if let Some(system) = system_prompt {
            params.system = Some(system);
        }

        // Convert and set tools
        if !prompt.tools.is_empty() {
            let tools = convert::tools_to_anthropic(&prompt.tools);
            if !tools.is_empty() {
                params = params.tools(tools);

                // Always set tool choice to explicitly control parallel behavior
                params = params.tool_choice(ToolChoice::Auto {
                    disable_parallel_tool_use: Some(!prompt.parallel_tool_calls),
                });
            }
        }

        // Apply extra configuration
        apply_extra_config(&mut params, &config.extra);

        // Make API call
        let message = client
            .messages()
            .create(params)
            .await
            .map_err(error::map_error)?;

        // Convert response to events
        let (events, _usage) = convert::message_to_events(&message);

        Ok(GenerateResult {
            events,
            usage: None, // Usage is included in the Completed event
            response_id: Some(message.id),
        })
    }

    fn supports_response_id(&self) -> bool {
        false // Anthropic doesn't support previous_response_id like OpenAI
    }
}

/// Extract max_tokens from extra config, with a default value.
fn extract_max_tokens(extra: &Option<serde_json::Value>) -> i32 {
    match extra
        .as_ref()
        .and_then(|e| e.get("max_tokens"))
        .and_then(|v| v.as_i64())
    {
        Some(v) => (v.max(1)) as i32,
        None => {
            tracing::debug!("max_tokens not specified, using default 4096");
            4096
        }
    }
}

/// Apply extra configuration to MessageCreateParams.
fn apply_extra_config(params: &mut MessageCreateParams, extra: &Option<serde_json::Value>) {
    let Some(extra) = extra else { return };

    // Temperature
    if let Some(temp) = extra.get("temperature").and_then(|v| v.as_f64()) {
        params.temperature = Some(temp);
    }

    // Top P
    if let Some(top_p) = extra.get("top_p").and_then(|v| v.as_f64()) {
        params.top_p = Some(top_p);
    }

    // Top K
    if let Some(top_k) = extra.get("top_k").and_then(|v| v.as_i64()) {
        params.top_k = Some(top_k as i32);
    }

    // Stop sequences
    if let Some(stop) = extra.get("stop_sequences").and_then(|v| v.as_array()) {
        let sequences: Vec<String> = stop
            .iter()
            .filter_map(|s| s.as_str().map(String::from))
            .collect();
        if !sequences.is_empty() {
            params.stop_sequences = Some(sequences);
        }
    }

    // Thinking configuration with validation
    if let Some(budget) = extra.get("thinking_budget_tokens").and_then(|v| v.as_i64()) {
        match ThinkingConfig::enabled_checked(budget as i32) {
            Ok(config) => params.thinking = Some(config),
            Err(e) => tracing::warn!("Invalid thinking budget: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_adapter_name() {
        let adapter = AnthropicAdapter::new();
        assert_eq!(adapter.name(), "anthropic");
    }

    #[test]
    fn test_extract_max_tokens_default() {
        assert_eq!(extract_max_tokens(&None), 4096);
    }

    #[test]
    fn test_extract_max_tokens_from_extra() {
        let extra = Some(serde_json::json!({"max_tokens": 2048}));
        assert_eq!(extract_max_tokens(&extra), 2048);
    }

    #[test]
    fn test_apply_extra_config() {
        let mut params = MessageCreateParams::new("claude-3-5-sonnet", 1000, vec![]);
        let extra = Some(serde_json::json!({
            "temperature": 0.7,
            "top_p": 0.9,
            "top_k": 40,
            "thinking_budget_tokens": 5000
        }));

        apply_extra_config(&mut params, &extra);

        assert!((params.temperature.unwrap() - 0.7).abs() < 0.01);
        assert!((params.top_p.unwrap() - 0.9).abs() < 0.01);
        assert_eq!(params.top_k, Some(40));
        assert!(params.thinking.is_some());
    }

    #[test]
    fn test_supports_response_id() {
        let adapter = AnthropicAdapter::new();
        assert!(!adapter.supports_response_id());
    }
}
