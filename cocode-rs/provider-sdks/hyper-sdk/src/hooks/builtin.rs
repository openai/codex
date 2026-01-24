//! Built-in hooks for common functionality.

use super::HookContext;
use super::RequestHook;
use super::ResponseHook;
use super::StreamHook;
use crate::error::HyperError;
use crate::options::OpenAIOptions;
use crate::options::VolcengineOptions;
use crate::options::downcast_options;
use crate::request::GenerateRequest;
use crate::response::GenerateResponse;
use crate::response::TokenUsage;
use crate::stream::StreamEvent;
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::Mutex;

/// Hook that injects `previous_response_id` for conversation continuity.
///
/// This hook reads `previous_response_id` from the context and injects it
/// into provider-specific options for providers that support conversation
/// continuity (e.g., OpenAI Responses API).
///
/// # Priority
///
/// This hook has priority 10 (early execution) to ensure the response ID
/// is set before other hooks process the request.
#[derive(Debug, Default)]
pub struct ResponseIdHook;

impl ResponseIdHook {
    /// Create a new ResponseIdHook.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RequestHook for ResponseIdHook {
    async fn on_request(
        &self,
        request: &mut GenerateRequest,
        context: &mut HookContext,
    ) -> Result<(), HyperError> {
        if let Some(ref prev_id) = context.previous_response_id {
            // For OpenAI provider, inject into OpenAI options
            if context.provider == "openai" {
                let mut options = request
                    .provider_options
                    .as_ref()
                    .and_then(|opts| downcast_options::<OpenAIOptions>(opts))
                    .cloned()
                    .unwrap_or_default();

                if options.previous_response_id.is_none() {
                    options.previous_response_id = Some(prev_id.clone());
                }

                request.provider_options = Some(Box::new(options));
            }
            // For Volcengine provider, inject into Volcengine options
            else if context.provider == "volcengine" {
                let mut options = request
                    .provider_options
                    .as_ref()
                    .and_then(|opts| downcast_options::<VolcengineOptions>(opts))
                    .cloned()
                    .unwrap_or_default();

                if options.previous_response_id.is_none() {
                    options.previous_response_id = Some(prev_id.clone());
                }

                request.provider_options = Some(Box::new(options));
            }
        }
        Ok(())
    }

    fn priority(&self) -> i32 {
        10
    }

    fn name(&self) -> &str {
        "response_id"
    }
}

/// Hook that logs requests and responses.
///
/// # Log Levels
///
/// - `Debug`: Log all requests and responses with full details
/// - `Info`: Log request/response summaries
/// - `Warn`: Log only errors
#[derive(Debug)]
pub struct LoggingHook {
    level: LogLevel,
}

#[derive(Debug, Clone, Copy)]
enum LogLevel {
    Debug,
    Info,
    Warn,
}

impl LoggingHook {
    /// Create a logging hook with debug level.
    pub fn debug() -> Self {
        Self {
            level: LogLevel::Debug,
        }
    }

    /// Create a logging hook with info level.
    pub fn info() -> Self {
        Self {
            level: LogLevel::Info,
        }
    }

    /// Create a logging hook with warn level.
    pub fn warn() -> Self {
        Self {
            level: LogLevel::Warn,
        }
    }
}

impl Default for LoggingHook {
    fn default() -> Self {
        Self::info()
    }
}

#[async_trait]
impl RequestHook for LoggingHook {
    async fn on_request(
        &self,
        request: &mut GenerateRequest,
        context: &mut HookContext,
    ) -> Result<(), HyperError> {
        match self.level {
            LogLevel::Debug => {
                tracing::debug!(
                    provider = %context.provider,
                    model = %context.model_id,
                    messages = request.messages.len(),
                    temperature = ?request.temperature,
                    max_tokens = ?request.max_tokens,
                    has_tools = request.has_tools(),
                    "Sending request"
                );
            }
            LogLevel::Info => {
                tracing::info!(
                    provider = %context.provider,
                    model = %context.model_id,
                    messages = request.messages.len(),
                    "Sending request"
                );
            }
            LogLevel::Warn => {
                // No logging at warn level for normal requests
            }
        }
        Ok(())
    }

    fn priority(&self) -> i32 {
        0
    }

    fn name(&self) -> &str {
        "logging"
    }
}

#[async_trait]
impl ResponseHook for LoggingHook {
    async fn on_response(
        &self,
        response: &mut GenerateResponse,
        context: &HookContext,
    ) -> Result<(), HyperError> {
        match self.level {
            LogLevel::Debug => {
                tracing::debug!(
                    provider = %context.provider,
                    model = %response.model,
                    response_id = %response.id,
                    finish_reason = ?response.finish_reason,
                    has_tool_calls = response.has_tool_calls(),
                    usage = ?response.usage,
                    "Received response"
                );
            }
            LogLevel::Info => {
                tracing::info!(
                    provider = %context.provider,
                    response_id = %response.id,
                    finish_reason = ?response.finish_reason,
                    "Received response"
                );
            }
            LogLevel::Warn => {
                // No logging at warn level for normal responses
            }
        }
        Ok(())
    }

    fn priority(&self) -> i32 {
        0
    }

    fn name(&self) -> &str {
        "logging"
    }
}

#[async_trait]
impl StreamHook for LoggingHook {
    async fn on_event(&self, event: &StreamEvent, context: &HookContext) -> Result<(), HyperError> {
        if matches!(self.level, LogLevel::Debug) {
            match event {
                StreamEvent::ResponseCreated { id } => {
                    tracing::debug!(provider = %context.provider, response_id = %id, "Stream started");
                }
                StreamEvent::ResponseDone {
                    id, finish_reason, ..
                } => {
                    tracing::debug!(
                        provider = %context.provider,
                        response_id = %id,
                        finish_reason = ?finish_reason,
                        "Stream completed"
                    );
                }
                StreamEvent::Error(err) => {
                    tracing::warn!(
                        provider = %context.provider,
                        error = %err.message,
                        "Stream error"
                    );
                }
                _ => {
                    // Don't log every delta
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "logging"
    }
}

/// Hook that tracks cumulative token usage across requests.
///
/// This hook accumulates token usage from each response, useful for
/// monitoring total token consumption in a conversation or session.
#[derive(Debug)]
pub struct UsageTrackingHook {
    usage: Arc<Mutex<TokenUsage>>,
}

impl UsageTrackingHook {
    /// Create a new usage tracking hook with its own counter.
    pub fn new() -> Self {
        Self {
            usage: Arc::new(Mutex::new(TokenUsage::default())),
        }
    }

    /// Create a usage tracking hook with a shared counter.
    pub fn with_shared_usage(usage: Arc<Mutex<TokenUsage>>) -> Self {
        Self { usage }
    }

    /// Get the current accumulated usage.
    pub fn get_usage(&self) -> TokenUsage {
        self.usage.lock().unwrap().clone()
    }

    /// Reset the usage counter.
    pub fn reset(&self) {
        let mut usage = self.usage.lock().unwrap();
        *usage = TokenUsage::default();
    }

    /// Get a reference to the shared usage counter.
    pub fn usage_ref(&self) -> Arc<Mutex<TokenUsage>> {
        self.usage.clone()
    }
}

impl Default for UsageTrackingHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ResponseHook for UsageTrackingHook {
    async fn on_response(
        &self,
        response: &mut GenerateResponse,
        _context: &HookContext,
    ) -> Result<(), HyperError> {
        if let Some(ref response_usage) = response.usage {
            let mut total = self.usage.lock().unwrap();
            total.prompt_tokens += response_usage.prompt_tokens;
            total.completion_tokens += response_usage.completion_tokens;
            total.total_tokens += response_usage.total_tokens;
            if let Some(cached) = response_usage.cache_read_tokens {
                *total.cache_read_tokens.get_or_insert(0) += cached;
            }
            if let Some(cache_creation) = response_usage.cache_creation_tokens {
                *total.cache_creation_tokens.get_or_insert(0) += cache_creation;
            }
            if let Some(reasoning) = response_usage.reasoning_tokens {
                *total.reasoning_tokens.get_or_insert(0) += reasoning;
            }
        }
        Ok(())
    }

    fn priority(&self) -> i32 {
        200 // Run late to capture final usage
    }

    fn name(&self) -> &str {
        "usage_tracking"
    }
}

/// Hook that sanitizes history messages when switching providers.
///
/// This hook automatically converts messages from other providers to be
/// compatible with the target provider. It:
/// - Strips thinking signatures from other providers
/// - Clears provider-specific options
/// - Preserves source tracking in metadata for debugging
#[derive(Debug, Default)]
pub struct CrossProviderSanitizationHook;

impl CrossProviderSanitizationHook {
    /// Create a new CrossProviderSanitizationHook.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RequestHook for CrossProviderSanitizationHook {
    async fn on_request(
        &self,
        request: &mut GenerateRequest,
        context: &mut HookContext,
    ) -> Result<(), HyperError> {
        // Sanitize all messages for target provider
        for msg in &mut request.messages {
            // Only convert if message came from different provider
            if !msg.metadata.is_from_provider(&context.provider) {
                // Has source info and it's different from target
                if msg.metadata.source_provider.is_some() {
                    msg.convert_for_provider(&context.provider, &context.model_id);
                }
            }
        }
        Ok(())
    }

    fn priority(&self) -> i32 {
        5 // Run very early, before ResponseIdHook (priority 10)
    }

    fn name(&self) -> &str {
        "cross_provider_sanitization"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::ContentBlock;
    use crate::messages::Message;
    use crate::response::FinishReason;

    #[tokio::test]
    async fn test_response_id_hook_openai() {
        let hook = ResponseIdHook::new();
        let mut request = GenerateRequest::new(vec![Message::user("Hello")]);
        let mut context =
            HookContext::with_provider("openai", "gpt-4o").previous_response_id("resp_prev_123");

        hook.on_request(&mut request, &mut context).await.unwrap();

        // Check that provider options were set
        let options = request
            .provider_options
            .as_ref()
            .and_then(|opts| downcast_options::<OpenAIOptions>(opts))
            .unwrap();
        assert_eq!(
            options.previous_response_id,
            Some("resp_prev_123".to_string())
        );
    }

    #[tokio::test]
    async fn test_response_id_hook_other_provider() {
        let hook = ResponseIdHook::new();
        let mut request = GenerateRequest::new(vec![Message::user("Hello")]);
        let mut context = HookContext::with_provider("anthropic", "claude-3")
            .previous_response_id("resp_prev_123");

        hook.on_request(&mut request, &mut context).await.unwrap();

        // For non-supported providers, options should not be set
        assert!(request.provider_options.is_none());
    }

    #[tokio::test]
    async fn test_response_id_hook_volcengine() {
        let hook = ResponseIdHook::new();
        let mut request = GenerateRequest::new(vec![Message::user("Hello")]);
        let mut context = HookContext::with_provider("volcengine", "doubao-pro-32k")
            .previous_response_id("resp_prev_456");

        hook.on_request(&mut request, &mut context).await.unwrap();

        // Check that provider options were set
        let options = request
            .provider_options
            .as_ref()
            .and_then(|opts| downcast_options::<VolcengineOptions>(opts))
            .unwrap();
        assert_eq!(
            options.previous_response_id,
            Some("resp_prev_456".to_string())
        );
    }

    #[tokio::test]
    async fn test_usage_tracking_hook() {
        let hook = UsageTrackingHook::new();

        // First response
        let mut response1 = GenerateResponse::new("resp_1", "gpt-4o")
            .with_content(vec![ContentBlock::text("Hello")])
            .with_usage(TokenUsage::new(100, 50))
            .with_finish_reason(FinishReason::Stop);

        let context = HookContext::with_provider("openai", "gpt-4o");
        hook.on_response(&mut response1, &context).await.unwrap();

        // Second response
        let mut response2 = GenerateResponse::new("resp_2", "gpt-4o")
            .with_content(vec![ContentBlock::text("World")])
            .with_usage(TokenUsage::new(80, 40))
            .with_finish_reason(FinishReason::Stop);

        hook.on_response(&mut response2, &context).await.unwrap();

        // Check accumulated usage
        let usage = hook.get_usage();
        assert_eq!(usage.prompt_tokens, 180);
        assert_eq!(usage.completion_tokens, 90);
        assert_eq!(usage.total_tokens, 270);

        // Test reset
        hook.reset();
        let usage = hook.get_usage();
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_usage_tracking_shared_counter() {
        let shared = Arc::new(Mutex::new(TokenUsage::default()));
        let hook1 = UsageTrackingHook::with_shared_usage(shared.clone());
        let hook2 = UsageTrackingHook::with_shared_usage(shared);

        // Modifying through one hook affects the other
        {
            let mut usage = hook1.usage.lock().unwrap();
            usage.prompt_tokens = 100;
        }

        assert_eq!(hook2.get_usage().prompt_tokens, 100);
    }

    // ============================================================
    // CrossProviderSanitizationHook Tests
    // ============================================================

    #[tokio::test]
    async fn test_cross_provider_sanitization_hook_strips_signatures() {
        let hook = CrossProviderSanitizationHook::new();

        // Create request with messages from different providers
        let anthropic_msg = Message::new(
            crate::messages::Role::Assistant,
            vec![
                ContentBlock::Thinking {
                    content: "Thinking content".to_string(),
                    signature: Some("anthropic-signature".to_string()),
                },
                ContentBlock::text("Response"),
            ],
        )
        .with_source("anthropic", "claude-sonnet-4-20250514");

        let mut request = GenerateRequest::new(vec![
            Message::user("Hello"),
            anthropic_msg,
            Message::user("Follow up"),
        ]);

        // Target provider is OpenAI
        let mut context = HookContext::with_provider("openai", "gpt-4o");

        hook.on_request(&mut request, &mut context).await.unwrap();

        // Anthropic signature should be stripped
        if let ContentBlock::Thinking { signature, .. } = &request.messages[1].content[0] {
            assert!(
                signature.is_none(),
                "Signature should be stripped when switching to different provider"
            );
        } else {
            panic!("Expected Thinking block");
        }
    }

    #[tokio::test]
    async fn test_cross_provider_sanitization_hook_preserves_same_provider() {
        let hook = CrossProviderSanitizationHook::new();

        // Create request with messages from the same provider
        let anthropic_msg = Message::new(
            crate::messages::Role::Assistant,
            vec![ContentBlock::Thinking {
                content: "Thinking content".to_string(),
                signature: Some("anthropic-signature".to_string()),
            }],
        )
        .with_source("anthropic", "claude-sonnet-4-20250514");

        let mut request = GenerateRequest::new(vec![Message::user("Hello"), anthropic_msg]);

        // Target provider is the same (Anthropic) - hook should NOT modify
        // Note: Same-provider, different-model sanitization is handled by Message::sanitize_for_target
        let mut context = HookContext::with_provider("anthropic", "claude-opus-4-20250514");

        hook.on_request(&mut request, &mut context).await.unwrap();

        // Signature should be preserved - hook only handles cross-PROVIDER sanitization
        if let ContentBlock::Thinking { signature, .. } = &request.messages[1].content[0] {
            assert!(
                signature.is_some(),
                "Signature should be preserved for same provider (cross-model sanitization is separate)"
            );
        }
    }

    #[tokio::test]
    async fn test_cross_provider_sanitization_hook_skips_no_source() {
        let hook = CrossProviderSanitizationHook::new();

        // Message without source info (e.g., user messages)
        let mut request = GenerateRequest::new(vec![Message::user("Hello")]);

        let mut context = HookContext::with_provider("openai", "gpt-4o");

        // Should not panic or modify user messages
        hook.on_request(&mut request, &mut context).await.unwrap();

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].text(), "Hello");
    }

    #[test]
    fn test_cross_provider_sanitization_hook_priority() {
        let hook = CrossProviderSanitizationHook::new();
        assert_eq!(hook.priority(), 5);
        assert_eq!(hook.name(), "cross_provider_sanitization");
    }
}
