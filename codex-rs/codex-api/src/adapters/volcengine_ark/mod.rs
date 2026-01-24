//! Volcengine Ark adapter for codex-api.
//!
//! This adapter converts between codex-api's canonical types and the volcengine-ark-sdk
//! crate's wire format, enabling Doubao models to be used with the codex agent.
//!
//! # Design Notes
//!
//! - **Non-streaming only**: Uses the Response API endpoint.
//! - **Thinking support**: Maps ThinkingConfig to extended thinking mode.
//! - **Tool use**: Maps function calls to Ark's tool calling format.

pub mod convert;
mod error;

use super::AdapterConfig;
use super::GenerateResult;
use super::ProviderAdapter;
use crate::common::Prompt;
use crate::error::ApiError;
use async_trait::async_trait;
use http::StatusCode;
use volcengine_ark_sdk::Client;
use volcengine_ark_sdk::ClientConfig;
use volcengine_ark_sdk::ReasoningEffort;
use volcengine_ark_sdk::ResponseCreateParams;
use volcengine_ark_sdk::ThinkingConfig;
use volcengine_ark_sdk::ToolChoice;

pub use error::map_error;

/// Volcengine Ark adapter for codex-api.
#[derive(Debug)]
pub struct VolcengineArkAdapter;

impl VolcengineArkAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VolcengineArkAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderAdapter for VolcengineArkAdapter {
    fn name(&self) -> &str {
        "volc_ark"
    }

    async fn generate(
        &self,
        prompt: &Prompt,
        config: &AdapterConfig,
    ) -> Result<GenerateResult, ApiError> {
        // Get API key
        let api_key = config.api_key.clone().ok_or_else(|| ApiError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: "API key required for Volcengine Ark adapter".to_string(),
        })?;

        // Build client config
        let mut client_config = ClientConfig::new(&api_key);

        if let Some(base_url) = &config.base_url {
            client_config = client_config.base_url(base_url);
        }

        // Create client
        let client = Client::new(client_config).map_err(error::map_error)?;

        // Get effective base_url for context checking
        let effective_base_url = config.base_url.as_deref().unwrap_or("");

        // Convert prompt to Ark messages (with cross-adapter support)
        let (messages, system_prompt) =
            convert::prompt_to_messages(prompt, effective_base_url, &config.model);

        // Build request parameters
        let mut params = ResponseCreateParams::new(&config.model, messages);

        // Set system prompt
        if let Some(instructions) = system_prompt {
            params = params.instructions(instructions);
        }

        // Convert and set tools
        if !prompt.tools.is_empty() {
            let tools = convert::tools_to_ark(&prompt.tools);
            if !tools.is_empty() {
                params = params.tools(tools);

                // Set tool choice
                if let Some(tool_choice) = convert::parse_tool_choice(&config.extra) {
                    params = params.tool_choice(tool_choice);
                } else {
                    params = params.tool_choice(ToolChoice::Auto);
                }
            }
        }

        // Apply extra configuration
        params = apply_extra_config(params, &config.extra);

        // Set previous response ID from prompt (for conversation continuity)
        if let Some(prev_id) = &prompt.previous_response_id {
            params = params.previous_response_id(prev_id);
        }

        // Make API call
        let response = client
            .responses()
            .create(params)
            .await
            .map_err(error::map_error)?;

        // Convert response to events
        // Pass base_url and model for model switch detection in EncryptedContent
        let effective_base_url = config.base_url.as_deref().unwrap_or("");
        let (events, usage) =
            convert::response_to_events(&response, effective_base_url, &config.model)?;

        Ok(GenerateResult {
            events,
            usage,
            response_id: Some(response.id),
        })
    }

    fn supports_response_id(&self) -> bool {
        true // Ark supports previous_response_id
    }
}

/// Apply extra configuration to ResponseCreateParams.
fn apply_extra_config(
    mut params: ResponseCreateParams,
    extra: &Option<serde_json::Value>,
) -> ResponseCreateParams {
    let Some(extra) = extra else {
        return params;
    };

    // Max output tokens
    if let Some(max_tokens) = extra.get("max_tokens").and_then(|v| v.as_i64()) {
        params = params.max_output_tokens(max_tokens as i32);
    }

    // Temperature
    if let Some(temp) = extra.get("temperature").and_then(|v| v.as_f64()) {
        params = params.temperature(temp);
    }

    // Top P
    if let Some(top_p) = extra.get("top_p").and_then(|v| v.as_f64()) {
        params = params.top_p(top_p);
    }

    // Stop sequences
    if let Some(stop) = extra.get("stop_sequences").and_then(|v| v.as_array()) {
        let sequences: Vec<String> = stop
            .iter()
            .filter_map(|s| s.as_str().map(String::from))
            .collect();
        if !sequences.is_empty() {
            params = params.stop_sequences(sequences);
        }
    }

    // Thinking configuration
    // First check for thinking_mode (auto/enabled/disabled), then budget
    if let Some(mode) = extra.get("thinking_mode").and_then(|v| v.as_str()) {
        match mode {
            "auto" => params = params.thinking(ThinkingConfig::auto()),
            "disabled" => params = params.thinking(ThinkingConfig::disabled()),
            "enabled" => {
                // If enabled, require budget_tokens
                if let Some(budget) = extra.get("thinking_budget_tokens").and_then(|v| v.as_i64()) {
                    match ThinkingConfig::enabled_checked(budget as i32) {
                        Ok(config) => params = params.thinking(config),
                        Err(e) => tracing::warn!("Invalid thinking budget: {e}"),
                    }
                } else {
                    tracing::warn!("thinking_mode=enabled requires thinking_budget_tokens");
                }
            }
            _ => tracing::warn!("Invalid thinking_mode: {mode}, expected auto/enabled/disabled"),
        }
    } else if let Some(budget) = extra.get("thinking_budget_tokens").and_then(|v| v.as_i64()) {
        // Legacy: just budget implies enabled
        match ThinkingConfig::enabled_checked(budget as i32) {
            Ok(config) => params = params.thinking(config),
            Err(e) => tracing::warn!("Invalid thinking budget: {e}"),
        }
    }

    // Reasoning effort
    if let Some(effort) = extra.get("reasoning_effort").and_then(|v| v.as_str()) {
        match effort {
            "minimal" => params = params.reasoning_effort(ReasoningEffort::Minimal),
            "low" => params = params.reasoning_effort(ReasoningEffort::Low),
            "medium" => params = params.reasoning_effort(ReasoningEffort::Medium),
            "high" => params = params.reasoning_effort(ReasoningEffort::High),
            _ => tracing::warn!(
                "Invalid reasoning_effort: {effort}, expected minimal/low/medium/high"
            ),
        }
    }

    // Store
    if let Some(store) = extra.get("store").and_then(|v| v.as_bool()) {
        params = params.store(store);
    }

    // Previous response ID
    if let Some(prev_id) = extra.get("previous_response_id").and_then(|v| v.as_str()) {
        params = params.previous_response_id(prev_id);
    }

    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use volcengine_ark_sdk::InputMessage;

    #[test]
    fn test_adapter_name() {
        let adapter = VolcengineArkAdapter::new();
        assert_eq!(adapter.name(), "volc_ark");
    }

    #[test]
    fn test_supports_response_id() {
        let adapter = VolcengineArkAdapter::new();
        assert!(adapter.supports_response_id());
    }

    #[test]
    fn test_apply_extra_config() {
        let params = ResponseCreateParams::new("ep-xxx", vec![InputMessage::user_text("test")]);
        let extra = Some(serde_json::json!({
            "max_tokens": 2048,
            "temperature": 0.7,
            "top_p": 0.9,
            "thinking_budget_tokens": 5000,
            "store": true
        }));

        let params = apply_extra_config(params, &extra);

        assert_eq!(params.max_output_tokens, Some(2048));
        assert!((params.temperature.unwrap() - 0.7).abs() < 0.01);
        assert!((params.top_p.unwrap() - 0.9).abs() < 0.01);
        assert!(params.thinking.is_some());
        assert_eq!(params.store, Some(true));
    }

    #[test]
    fn test_apply_extra_config_thinking_mode_auto() {
        let params = ResponseCreateParams::new("ep-xxx", vec![InputMessage::user_text("test")]);
        let extra = Some(serde_json::json!({
            "thinking_mode": "auto"
        }));

        let params = apply_extra_config(params, &extra);

        assert!(params.thinking.is_some());
        let json = serde_json::to_string(&params.thinking).unwrap();
        assert!(json.contains(r#""type":"auto""#));
    }

    #[test]
    fn test_apply_extra_config_thinking_mode_disabled() {
        let params = ResponseCreateParams::new("ep-xxx", vec![InputMessage::user_text("test")]);
        let extra = Some(serde_json::json!({
            "thinking_mode": "disabled"
        }));

        let params = apply_extra_config(params, &extra);

        assert!(params.thinking.is_some());
        let json = serde_json::to_string(&params.thinking).unwrap();
        assert!(json.contains(r#""type":"disabled""#));
    }

    #[test]
    fn test_apply_extra_config_thinking_mode_enabled() {
        let params = ResponseCreateParams::new("ep-xxx", vec![InputMessage::user_text("test")]);
        let extra = Some(serde_json::json!({
            "thinking_mode": "enabled",
            "thinking_budget_tokens": 2048
        }));

        let params = apply_extra_config(params, &extra);

        assert!(params.thinking.is_some());
        let json = serde_json::to_string(&params.thinking).unwrap();
        assert!(json.contains(r#""type":"enabled""#));
        assert!(json.contains(r#""budget_tokens":2048"#));
    }

    #[test]
    fn test_apply_extra_config_reasoning_effort() {
        let params = ResponseCreateParams::new("ep-xxx", vec![InputMessage::user_text("test")]);
        let extra = Some(serde_json::json!({
            "reasoning_effort": "high"
        }));

        let params = apply_extra_config(params, &extra);

        assert!(params.reasoning_effort.is_some());
        let json = serde_json::to_string(&params.reasoning_effort).unwrap();
        assert!(json.contains(r#""high""#));
    }

    #[test]
    fn test_apply_extra_config_reasoning_effort_all_levels() {
        for (level, expected) in [
            ("minimal", "minimal"),
            ("low", "low"),
            ("medium", "medium"),
            ("high", "high"),
        ] {
            let params = ResponseCreateParams::new("ep-xxx", vec![InputMessage::user_text("test")]);
            let extra = Some(serde_json::json!({ "reasoning_effort": level }));
            let params = apply_extra_config(params, &extra);

            assert!(params.reasoning_effort.is_some());
            let json = serde_json::to_string(&params.reasoning_effort).unwrap();
            assert!(json.contains(expected));
        }
    }
}
