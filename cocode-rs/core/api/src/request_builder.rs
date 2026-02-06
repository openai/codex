//! Unified request builder for LLM inference.
//!
//! `RequestBuilder` provides a unified way to construct `GenerateRequest` from
//! an `InferenceContext`. It handles:
//!
//! - Applying model-specific parameters (temperature, top_p, max_tokens)
//! - Converting thinking configuration to provider-specific options
//! - Message sanitization for target provider
//!
//! # Example
//!
//! ```ignore
//! use cocode_api::RequestBuilder;
//! use cocode_protocol::execution::InferenceContext;
//!
//! // Context from ModelHub (selections passed as parameter)
//! let (ctx, model) = hub.prepare_main_with_selections(&selections, "session", 1)?;
//!
//! // Build request with messages and tools
//! let request = RequestBuilder::new(ctx)
//!     .messages(messages)
//!     .tools(tools)
//!     .build();
//!
//! // Use with model
//! model.stream(request).await?;
//! ```

use cocode_protocol::execution::InferenceContext;
use hyper_sdk::GenerateRequest;
use hyper_sdk::Message;
use hyper_sdk::ToolChoice;
use hyper_sdk::ToolDefinition;

use crate::request_options_merge;
use crate::thinking_convert;

/// Builder for constructing `GenerateRequest` from `InferenceContext`.
///
/// This centralizes all the parameter assembly that was previously scattered
/// across different parts of the codebase.
pub struct RequestBuilder {
    context: InferenceContext,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    tool_choice: Option<ToolChoice>,

    // Optional overrides (take precedence over context values)
    temperature_override: Option<f64>,
    max_tokens_override: Option<i32>,
    top_p_override: Option<f64>,
}

impl RequestBuilder {
    /// Create a new request builder with the given inference context.
    pub fn new(context: InferenceContext) -> Self {
        Self {
            context,
            messages: Vec::new(),
            tools: None,
            tool_choice: None,
            temperature_override: None,
            max_tokens_override: None,
            top_p_override: None,
        }
    }

    /// Set the messages for the request.
    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    /// Set the tools for the request.
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set the tool choice for the request.
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Override the temperature from context.
    pub fn temperature(mut self, temp: f64) -> Self {
        self.temperature_override = Some(temp);
        self
    }

    /// Override the max tokens from context.
    pub fn max_tokens(mut self, tokens: i32) -> Self {
        self.max_tokens_override = Some(tokens);
        self
    }

    /// Override the top_p from context.
    pub fn top_p(mut self, p: f64) -> Self {
        self.top_p_override = Some(p);
        self
    }

    /// Build the final `GenerateRequest`.
    ///
    /// This method:
    /// 1. Sets sampling parameters from context (temperature, top_p, max_tokens)
    /// 2. Converts thinking level to provider-specific options
    /// 3. Applies any overrides
    pub fn build(self) -> GenerateRequest {
        let mut request = GenerateRequest::new(self.messages);

        // Apply temperature (override > context > default None)
        request.temperature = self
            .temperature_override
            .or_else(|| self.context.temperature().map(|t| t as f64));

        // Apply max_tokens (override > context > default None)
        request.max_tokens = self
            .max_tokens_override
            .or_else(|| self.context.max_output_tokens().map(|t| t as i32));

        // Apply top_p (override > context > default None)
        request.top_p = self
            .top_p_override
            .or_else(|| self.context.top_p().map(|p| p as f64));

        // Apply tools and tool choice
        request.tools = self.tools;
        request.tool_choice = self.tool_choice;

        // Step 1: Build provider options from thinking config
        let mut provider_options =
            if let Some(thinking_level) = self.context.effective_thinking_level() {
                thinking_convert::to_provider_options(
                    thinking_level,
                    &self.context.model_info,
                    self.context.model_spec.provider_type,
                )
            } else {
                None
            };

        // Step 2: Merge request_options into provider_options
        if let Some(req_opts) = &self.context.request_options {
            if !req_opts.is_empty() {
                provider_options = Some(request_options_merge::merge_into_provider_options(
                    provider_options,
                    req_opts,
                    self.context.model_spec.provider_type,
                ));
            }
        }

        request.provider_options = provider_options;

        request
    }

    /// Get a reference to the inference context.
    pub fn context(&self) -> &InferenceContext {
        &self.context
    }
}

/// Convenience function to build a request directly from context and messages.
pub fn build_request(
    context: InferenceContext,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
) -> GenerateRequest {
    let mut builder = RequestBuilder::new(context).messages(messages);
    if let Some(t) = tools {
        builder = builder.tools(t);
    }
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::execution::AgentKind;
    use cocode_protocol::execution::ExecutionIdentity;
    use cocode_protocol::model::ModelInfo;
    use cocode_protocol::model::ModelSpec;
    use cocode_protocol::thinking::ThinkingLevel;

    fn sample_context() -> InferenceContext {
        let spec = ModelSpec::new("anthropic", "claude-opus-4");
        let info = ModelInfo {
            slug: "claude-opus-4".to_string(),
            context_window: Some(200000),
            max_output_tokens: Some(16384),
            temperature: Some(1.0),
            top_p: Some(0.9),
            ..Default::default()
        };

        InferenceContext::new(
            "call-123",
            "session-456",
            1,
            spec,
            info,
            AgentKind::Main,
            ExecutionIdentity::main(),
        )
    }

    #[test]
    fn test_basic_build() {
        let ctx = sample_context();
        let messages = vec![Message::user("Hello")];

        let request = RequestBuilder::new(ctx).messages(messages).build();

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.temperature, Some(1.0));
        // top_p comparison with tolerance due to f32->f64 conversion
        assert!((request.top_p.unwrap() - 0.9).abs() < 0.001);
        assert_eq!(request.max_tokens, Some(16384));
    }

    #[test]
    fn test_override_temperature() {
        let ctx = sample_context();
        let messages = vec![Message::user("Hello")];

        let request = RequestBuilder::new(ctx)
            .messages(messages)
            .temperature(0.5)
            .build();

        assert_eq!(request.temperature, Some(0.5));
    }

    #[test]
    fn test_override_max_tokens() {
        let ctx = sample_context();
        let messages = vec![Message::user("Hello")];

        let request = RequestBuilder::new(ctx)
            .messages(messages)
            .max_tokens(1000)
            .build();

        assert_eq!(request.max_tokens, Some(1000));
    }

    #[test]
    fn test_with_tools() {
        let ctx = sample_context();
        let messages = vec![Message::user("Hello")];
        let tools = vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            parameters: serde_json::json!({"type": "object"}),
        }];

        let request = RequestBuilder::new(ctx)
            .messages(messages)
            .tools(tools)
            .build();

        assert!(request.tools.is_some());
        assert_eq!(request.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_with_thinking() {
        let spec = ModelSpec::new("openai", "o1");
        let info = ModelInfo {
            slug: "o1".to_string(),
            context_window: Some(128000),
            max_output_tokens: Some(32768),
            default_thinking_level: Some(ThinkingLevel::high()),
            ..Default::default()
        };

        let ctx = InferenceContext::new(
            "call-123",
            "session-456",
            1,
            spec,
            info,
            AgentKind::Main,
            ExecutionIdentity::main(),
        );

        let messages = vec![Message::user("Hello")];
        let request = RequestBuilder::new(ctx).messages(messages).build();

        // Should have provider options for thinking
        assert!(request.provider_options.is_some());
    }

    #[test]
    fn test_context_accessor() {
        let ctx = sample_context();
        let builder = RequestBuilder::new(ctx);

        assert_eq!(builder.context().session_id, "session-456");
        assert_eq!(builder.context().turn_number, 1);
    }

    #[test]
    fn test_build_request_helper() {
        let ctx = sample_context();
        let messages = vec![Message::user("Hello")];

        let request = build_request(ctx, messages, None);

        assert_eq!(request.messages.len(), 1);
        assert!(request.tools.is_none());
    }
}
