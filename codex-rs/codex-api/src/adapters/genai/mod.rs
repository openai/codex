//! Google Generative AI (Gemini) adapter for codex-api.
//!
//! This adapter converts between codex-api's canonical types and the google-genai crate's
//! wire format, enabling Gemini models to be used with the codex agent infrastructure.
//!
//! # Design Notes
//!
//! - **Non-streaming only**: This adapter uses the stateless `generate_content` API.
//! - **ID-based mapping**: FunctionCall/FunctionCallOutput are matched by their `call_id`/`id` fields.
//! - **Reasoning support**: Reasoning items are converted to/from `thought` parts with signatures.

pub mod convert;
mod error;

use super::AdapterConfig;
use super::GenerateResult;
use super::ProviderAdapter;
use crate::common::Prompt;
use crate::error::ApiError;
use async_trait::async_trait;
use google_genai::Client;
use google_genai::ClientConfig;
use google_genai::types::Content;
use google_genai::types::FunctionCallingConfig;
use google_genai::types::FunctionCallingMode;
use google_genai::types::FunctionDeclaration;
use google_genai::types::GenerateContentConfig;
use google_genai::types::Part;
use google_genai::types::Tool;
use google_genai::types::ToolConfig;

pub use error::map_error;

/// Gemini adapter for codex-api.
#[derive(Debug)]
pub struct GeminiAdapter;

impl GeminiAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderAdapter for GeminiAdapter {
    fn name(&self) -> &str {
        "genai"
    }

    async fn generate(
        &self,
        prompt: &Prompt,
        config: &AdapterConfig,
    ) -> Result<GenerateResult, ApiError> {
        // Build client config
        let mut client_config = ClientConfig::default();

        if let Some(api_key) = &config.api_key {
            client_config.api_key = Some(api_key.clone());
        }

        if let Some(base_url) = &config.base_url {
            client_config.base_url = Some(base_url.clone());
        }

        // Create client
        let client = Client::new(client_config).map_err(error::map_error)?;

        // Convert input history to Gemini contents
        let contents = convert::prompt_to_contents(prompt);

        // Build generation config
        let gen_config = build_generation_config(prompt, config);

        // Make API call
        let response = client
            .generate_content(&config.model, contents, Some(gen_config))
            .await
            .map_err(error::map_error)?;

        // Convert response to events (includes Created and Completed)
        let (events, response_id) = convert::response_to_events(&response);

        Ok(GenerateResult {
            events,
            usage: None, // Usage is now included in the Completed event
            response_id: Some(response_id),
        })
    }

    fn supports_response_id(&self) -> bool {
        false
    }
}

/// Build GenerateContentConfig from Prompt and AdapterConfig.
fn build_generation_config(prompt: &Prompt, config: &AdapterConfig) -> GenerateContentConfig {
    let mut gen_config = GenerateContentConfig::default();

    // Set system instruction
    if !prompt.instructions.is_empty() {
        gen_config.system_instruction = Some(Content {
            parts: Some(vec![Part::text(&prompt.instructions)]),
            role: Some("user".to_string()),
        });
    }

    // Convert tools
    if !prompt.tools.is_empty() {
        let function_declarations: Vec<FunctionDeclaration> = prompt
            .tools
            .iter()
            .filter_map(|tool| convert::tool_json_to_declaration(tool))
            .collect();

        if !function_declarations.is_empty() {
            gen_config.tools = Some(vec![Tool::functions(function_declarations)]);

            // Set tool config for parallel calls
            if prompt.parallel_tool_calls {
                gen_config.tool_config = Some(ToolConfig {
                    function_calling_config: Some(FunctionCallingConfig {
                        mode: Some(FunctionCallingMode::Auto),
                        allowed_function_names: None,
                        stream_function_call_arguments: None,
                    }),
                });
            }
        }
    }

    // Apply extra config from adapter config
    if let Some(extra) = &config.extra {
        if let Some(temp) = extra.get("temperature").and_then(|v| v.as_f64()) {
            gen_config.temperature = Some(temp as f32);
        }
        if let Some(max_tokens) = extra.get("max_output_tokens").and_then(|v| v.as_i64()) {
            gen_config.max_output_tokens = Some(max_tokens as i32);
        }
        if let Some(top_p) = extra.get("top_p").and_then(|v| v.as_f64()) {
            gen_config.top_p = Some(top_p as f32);
        }
        if let Some(top_k) = extra.get("top_k").and_then(|v| v.as_i64()) {
            gen_config.top_k = Some(top_k as i32);
        }

        // Handle thinking_level
        if let Some(thinking_level) = extra.get("thinking_level").and_then(|v| v.as_str()) {
            use google_genai::types::ThinkingConfig;
            use google_genai::types::ThinkingLevel;
            let level = match thinking_level {
                "LOW" => ThinkingLevel::Low,
                "HIGH" => ThinkingLevel::High,
                _ => ThinkingLevel::ThinkingLevelUnspecified,
            };
            if level != ThinkingLevel::ThinkingLevelUnspecified {
                gen_config.thinking_config = Some(ThinkingConfig::with_level(level));
            }
        }
    }

    gen_config
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_adapter_name() {
        let adapter = GeminiAdapter::new();
        assert_eq!(adapter.name(), "genai");
    }

    #[test]
    fn test_build_generation_config_with_instructions() {
        let prompt = Prompt {
            instructions: "You are a helpful assistant.".to_string(),
            input: vec![],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };
        let config = AdapterConfig {
            model: "gemini-2.0-flash".to_string(),
            ..Default::default()
        };

        let gen_config = build_generation_config(&prompt, &config);

        assert!(gen_config.system_instruction.is_some());
        let sys = gen_config.system_instruction.unwrap();
        assert_eq!(
            sys.parts.unwrap()[0].text,
            Some("You are a helpful assistant.".to_string())
        );
    }

    #[test]
    fn test_build_generation_config_with_extra() {
        let prompt = Prompt {
            instructions: String::new(),
            input: vec![],
            tools: vec![],
            parallel_tool_calls: false,
            output_schema: None,
            previous_response_id: None,
        };
        let config = AdapterConfig {
            model: "gemini-2.0-flash".to_string(),
            extra: Some(serde_json::json!({
                "temperature": 0.7,
                "max_output_tokens": 1024,
                "top_p": 0.9
            })),
            ..Default::default()
        };

        let gen_config = build_generation_config(&prompt, &config);

        assert!((gen_config.temperature.unwrap() - 0.7).abs() < 0.01);
        assert_eq!(gen_config.max_output_tokens, Some(1024));
        assert!((gen_config.top_p.unwrap() - 0.9).abs() < 0.01);
    }
}
