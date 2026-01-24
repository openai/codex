//! OpenAI adapter using the openai-sdk crate.
//!
//! This adapter uses the OpenAI Responses API and supports:
//! - previous_response_id for conversation continuity
//! - Extended thinking (thinking_mode + thinking_budget_tokens)
//! - Reasoning effort configuration
//! - Full response round-trip via encrypted_content

pub(crate) mod convert;
mod error;

use async_trait::async_trait;
use openai_sdk::Client;
use openai_sdk::ClientConfig;
use openai_sdk::ReasoningConfig;
use openai_sdk::ReasoningEffort;
use openai_sdk::ResponseCreateParams;
use openai_sdk::ServiceTier;
use openai_sdk::ThinkingConfig;
use serde_json::Value;
use std::sync::Arc;

use crate::adapters::AdapterConfig;
use crate::adapters::GenerateResult;
use crate::adapters::ProviderAdapter;
use crate::adapters::RequestHook as AdapterRequestHook;
use crate::common::ResponseEvent;
use crate::error::ApiError;

// ============================================================================
// Request Hook Wrapper
// ============================================================================

/// Wrapper to bridge codex-api RequestHook to openai-sdk RequestHook.
#[derive(Debug)]
struct RequestHookWrapper {
    inner: Arc<dyn AdapterRequestHook>,
}

impl openai_sdk::RequestHook for RequestHookWrapper {
    fn on_request(&self, request: &mut openai_sdk::HttpRequest) {
        // Convert openai-sdk HttpRequest to codex-api HttpRequest
        let mut adapter_request = crate::adapters::HttpRequest {
            url: request.url.clone(),
            headers: request.headers.clone(),
            body: request.body.clone(),
        };

        // Apply the inner hook
        self.inner.on_request(&mut adapter_request);

        // Copy back the modified values
        request.url = adapter_request.url;
        request.headers = adapter_request.headers;
        request.body = adapter_request.body;
    }
}

/// OpenAI adapter using the openai-sdk crate.
#[derive(Debug)]
pub struct OpenAIAdapter;

impl OpenAIAdapter {
    /// Create a new OpenAI adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenAIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderAdapter for OpenAIAdapter {
    fn name(&self) -> &str {
        "openai"
    }

    fn supports_response_id(&self) -> bool {
        true
    }

    async fn generate(
        &self,
        prompt: &crate::common::Prompt,
        config: &AdapterConfig,
    ) -> Result<GenerateResult, ApiError> {
        // Get API key
        let api_key = config.api_key.as_ref().ok_or_else(|| ApiError::Api {
            status: http::StatusCode::UNAUTHORIZED,
            message: "Missing API key for OpenAI adapter".to_string(),
        })?;

        // Build client config
        let mut client_config = ClientConfig::new(api_key);

        if let Some(base_url) = &config.base_url {
            client_config = client_config.base_url(base_url);
        }

        // Apply request hook if available (wrap to bridge types)
        if let Some(hook) = &config.request_hook {
            let wrapper = Arc::new(RequestHookWrapper {
                inner: hook.clone(),
            });
            client_config = client_config.request_hook(wrapper);
        }

        let client = Client::new(client_config).map_err(error::map_error)?;

        // Get effective base_url and model for context storage
        let effective_base_url = config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1");

        // Convert prompt to OpenAI format
        let (messages, system_prompt) =
            convert::prompt_to_messages(prompt, effective_base_url, &config.model);

        // Build request params
        let mut params = ResponseCreateParams::new(&config.model, messages);

        // Set system instructions
        if let Some(instructions) = system_prompt {
            params = params.instructions(instructions);
        }

        // Set previous_response_id for conversation continuity
        if let Some(prev_id) = &prompt.previous_response_id {
            params = params.previous_response_id(prev_id);
        }

        // Convert tools
        if !prompt.tools.is_empty() {
            let tools = convert::tools_to_openai(&prompt.tools);
            params = params.tools(tools);
        }

        // Apply tool choice from extra config
        if let Some(tool_choice) = convert::parse_tool_choice(&config.extra) {
            params = params.tool_choice(tool_choice);
        }

        // Apply extra config options
        params = apply_extra_config(params, &config.extra);

        // Apply ultrathink config (overrides static extra config)
        if let Some(ut_config) = &config.ultrathink_config {
            let effort = convert_reasoning_effort(ut_config.effort);
            params = params.reasoning(ReasoningConfig::with_effort(effort));
        }

        // Make the API call
        let response = client
            .responses()
            .create(params)
            .await
            .map_err(error::map_error)?;

        // Convert response to events
        let (events, usage) =
            convert::response_to_events(&response, effective_base_url, &config.model)?;

        // Filter out Created event (not needed for non-streaming)
        let events: Vec<ResponseEvent> = events
            .into_iter()
            .filter(|e| !matches!(e, ResponseEvent::Created))
            .collect();

        Ok(GenerateResult {
            events,
            usage,
            response_id: Some(response.id),
        })
    }
}

/// Apply extra configuration to ResponseCreateParams.
fn apply_extra_config(
    mut params: ResponseCreateParams,
    extra: &Option<Value>,
) -> ResponseCreateParams {
    let Some(extra) = extra else {
        return params;
    };

    // max_tokens / max_output_tokens
    if let Some(max_tokens) = extra
        .get("max_tokens")
        .or(extra.get("max_output_tokens"))
        .and_then(|v| v.as_i64())
    {
        params = params.max_output_tokens(max_tokens as i32);
    }

    // temperature
    if let Some(temperature) = extra.get("temperature").and_then(|v| v.as_f64()) {
        params = params.temperature(temperature);
    }

    // top_p
    if let Some(top_p) = extra.get("top_p").and_then(|v| v.as_f64()) {
        params = params.top_p(top_p);
    }

    // stop / stop_sequences
    if let Some(stop) = extra
        .get("stop")
        .or(extra.get("stop_sequences"))
        .and_then(|v| v.as_array())
    {
        let stop_seqs: Vec<String> = stop
            .iter()
            .filter_map(|s| s.as_str().map(String::from))
            .collect();
        if !stop_seqs.is_empty() {
            params = params.stop(stop_seqs);
        }
    }

    // Extended thinking (thinking_mode + thinking_budget_tokens)
    if let Some(thinking_mode) = extra.get("thinking_mode").and_then(|v| v.as_str()) {
        match thinking_mode {
            "enabled" => {
                let budget = extra
                    .get("thinking_budget_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(10000) as i32;
                params = params.thinking(ThinkingConfig::enabled(budget));
            }
            "disabled" => {
                params = params.thinking(ThinkingConfig::Disabled);
            }
            "auto" => {
                params = params.thinking(ThinkingConfig::Auto);
            }
            _ => {}
        }
    }

    // Reasoning effort (for o-series models)
    if let Some(effort) = extra.get("reasoning_effort").and_then(|v| v.as_str()) {
        let reasoning_effort = match effort {
            "low" | "LOW" => Some(ReasoningEffort::Low),
            "medium" | "MEDIUM" => Some(ReasoningEffort::Medium),
            "high" | "HIGH" => Some(ReasoningEffort::High),
            "none" | "NONE" => Some(ReasoningEffort::None),
            _ => None,
        };
        if let Some(effort) = reasoning_effort {
            params = params.reasoning(ReasoningConfig::with_effort(effort));
        }
    }

    // Service tier
    if let Some(tier) = extra.get("service_tier").and_then(|v| v.as_str()) {
        let service_tier = match tier {
            "auto" => Some(ServiceTier::Auto),
            "default" => Some(ServiceTier::Default),
            "flex" => Some(ServiceTier::Flex),
            "scale" => Some(ServiceTier::Scale),
            "priority" => Some(ServiceTier::Priority),
            _ => None,
        };
        if let Some(tier) = service_tier {
            params = params.service_tier(tier);
        }
    }

    // Store (whether to store the response on server)
    if let Some(store) = extra.get("store").and_then(|v| v.as_bool()) {
        params = params.store(store);
    }

    params
}

/// Convert protocol ReasoningEffort to openai-sdk ReasoningEffort.
fn convert_reasoning_effort(
    effort: codex_protocol::openai_models::ReasoningEffort,
) -> ReasoningEffort {
    use codex_protocol::openai_models::ReasoningEffort as ProtocolEffort;
    match effort {
        ProtocolEffort::None => ReasoningEffort::None,
        ProtocolEffort::Minimal | ProtocolEffort::Low => ReasoningEffort::Low,
        ProtocolEffort::Medium => ReasoningEffort::Medium,
        ProtocolEffort::High | ProtocolEffort::XHigh => ReasoningEffort::High,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_name() {
        let adapter = OpenAIAdapter::new();
        assert_eq!(adapter.name(), "openai");
    }

    #[test]
    fn test_supports_response_id() {
        let adapter = OpenAIAdapter::new();
        assert!(adapter.supports_response_id());
    }

    #[test]
    fn test_apply_extra_config_temperature() {
        let params = ResponseCreateParams::new("gpt-4o", vec![]);
        let extra = Some(serde_json::json!({
            "temperature": 0.7
        }));
        let params = apply_extra_config(params, &extra);
        // Note: We can't easily verify the internal state, but we can ensure it doesn't panic
        let _ = params;
    }

    #[test]
    fn test_apply_extra_config_thinking() {
        let params = ResponseCreateParams::new("gpt-4o", vec![]);
        let extra = Some(serde_json::json!({
            "thinking_mode": "enabled",
            "thinking_budget_tokens": 5000
        }));
        let params = apply_extra_config(params, &extra);
        let _ = params;
    }

    #[test]
    fn test_apply_extra_config_reasoning_effort() {
        let params = ResponseCreateParams::new("o1", vec![]);
        let extra = Some(serde_json::json!({
            "reasoning_effort": "high"
        }));
        let params = apply_extra_config(params, &extra);
        let _ = params;
    }

    #[test]
    fn test_apply_extra_config_stop_sequences() {
        let params = ResponseCreateParams::new("gpt-4o", vec![]);
        let extra = Some(serde_json::json!({
            "stop_sequences": ["END", "STOP"]
        }));
        let params = apply_extra_config(params, &extra);
        let _ = params;
    }

    #[test]
    fn test_apply_extra_config_service_tier() {
        let params = ResponseCreateParams::new("gpt-4o", vec![]);
        let extra = Some(serde_json::json!({
            "service_tier": "scale"
        }));
        let params = apply_extra_config(params, &extra);
        let _ = params;
    }
}
