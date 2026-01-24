//! Hooks system for intercepting and transforming requests/responses.
//!
//! The hooks system provides a flexible way to modify requests before sending,
//! transform responses after receiving, and observe stream events.
//!
//! # Hook Types
//!
//! - [`RequestHook`]: Intercept and modify requests before sending
//! - [`ResponseHook`]: Transform responses after receiving
//! - [`StreamHook`]: Observe stream events
//!
//! # Example
//!
//! ```no_run
//! use hyper_sdk::hooks::{RequestHook, HookContext, HookChain};
//! use hyper_sdk::{GenerateRequest, HyperError};
//! use async_trait::async_trait;
//!
//! #[derive(Debug)]
//! struct MyRequestHook;
//!
//! #[async_trait]
//! impl RequestHook for MyRequestHook {
//!     async fn on_request(
//!         &self,
//!         request: &mut GenerateRequest,
//!         context: &mut HookContext,
//!     ) -> Result<(), HyperError> {
//!         // Modify the request
//!         request.temperature = Some(0.7);
//!         Ok(())
//!     }
//!
//!     fn name(&self) -> &str {
//!         "my_request_hook"
//!     }
//! }
//! ```

mod builtin;
mod chain;

pub use builtin::CrossProviderSanitizationHook;
pub use builtin::LoggingHook;
pub use builtin::ResponseIdHook;
pub use builtin::UsageTrackingHook;
pub use chain::HookChain;

use crate::error::HyperError;
use crate::request::GenerateRequest;
use crate::response::GenerateResponse;
use crate::stream::StreamEvent;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;

/// Context passed to hooks containing request/response metadata.
#[derive(Debug, Clone, Default)]
pub struct HookContext {
    /// Provider name (e.g., "openai", "anthropic").
    pub provider: String,
    /// Model ID being used.
    pub model_id: String,
    /// Unique conversation ID for multi-turn conversations.
    pub conversation_id: Option<String>,
    /// Previous response ID for conversation continuity.
    pub previous_response_id: Option<String>,
    /// Request ID (set after request is sent).
    pub request_id: Option<String>,
    /// Custom metadata that can be used by hooks.
    pub metadata: HashMap<String, Value>,
}

impl HookContext {
    /// Create a new empty hook context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with provider and model.
    pub fn with_provider(provider: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model_id: model_id.into(),
            ..Default::default()
        }
    }

    /// Set the conversation ID.
    pub fn conversation_id(mut self, id: impl Into<String>) -> Self {
        self.conversation_id = Some(id.into());
        self
    }

    /// Set the previous response ID.
    pub fn previous_response_id(mut self, id: impl Into<String>) -> Self {
        self.previous_response_id = Some(id.into());
        self
    }

    /// Set a metadata value.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: Value) {
        self.metadata.insert(key.into(), value);
    }

    /// Get a metadata value.
    pub fn get_metadata(&self, key: &str) -> Option<&Value> {
        self.metadata.get(key)
    }
}

/// Hook for intercepting requests before sending.
///
/// Request hooks are executed in priority order before a request is sent to the provider.
/// They can modify the request or add metadata to the context.
#[async_trait]
pub trait RequestHook: Send + Sync + Debug {
    /// Called before request is sent.
    ///
    /// The hook can modify both the request and the context.
    async fn on_request(
        &self,
        request: &mut GenerateRequest,
        context: &mut HookContext,
    ) -> Result<(), HyperError>;

    /// Hook priority (lower = earlier execution).
    ///
    /// Default priority is 100. Built-in hooks typically use priorities 0-50.
    fn priority(&self) -> i32 {
        100
    }

    /// Hook name for debugging and logging.
    fn name(&self) -> &str;
}

/// Hook for transforming responses after receiving.
///
/// Response hooks are executed in priority order after a response is received.
/// They can modify the response or extract information into the context.
#[async_trait]
pub trait ResponseHook: Send + Sync + Debug {
    /// Called after response is received.
    ///
    /// The hook can modify the response and read/write to the context.
    async fn on_response(
        &self,
        response: &mut GenerateResponse,
        context: &HookContext,
    ) -> Result<(), HyperError>;

    /// Hook priority (lower = earlier execution).
    fn priority(&self) -> i32 {
        100
    }

    /// Hook name for debugging and logging.
    fn name(&self) -> &str;
}

/// Hook for observing stream events.
///
/// Stream hooks are called for each event in a streaming response.
/// They cannot modify events but can observe and react to them.
#[async_trait]
pub trait StreamHook: Send + Sync + Debug {
    /// Called for each stream event.
    async fn on_event(&self, event: &StreamEvent, context: &HookContext) -> Result<(), HyperError>;

    /// Hook priority (lower = earlier execution).
    ///
    /// Default priority is 100. Built-in hooks typically use priorities 0-50.
    fn priority(&self) -> i32 {
        100
    }

    /// Hook name for debugging and logging.
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::Message;

    #[derive(Debug)]
    struct TestRequestHook {
        name: String,
        priority: i32,
    }

    impl TestRequestHook {
        fn new(name: &str, priority: i32) -> Self {
            Self {
                name: name.to_string(),
                priority,
            }
        }
    }

    #[async_trait]
    impl RequestHook for TestRequestHook {
        async fn on_request(
            &self,
            request: &mut GenerateRequest,
            _context: &mut HookContext,
        ) -> Result<(), HyperError> {
            request.temperature = Some(0.5);
            Ok(())
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_hook_context_builder() {
        let context = HookContext::with_provider("openai", "gpt-4o").conversation_id("conv_123");

        assert_eq!(context.provider, "openai");
        assert_eq!(context.model_id, "gpt-4o");
        assert_eq!(context.conversation_id, Some("conv_123".to_string()));
    }

    #[test]
    fn test_hook_context_metadata() {
        let mut context = HookContext::new();
        context.set_metadata("key", serde_json::json!("value"));

        assert_eq!(
            context.get_metadata("key"),
            Some(&serde_json::json!("value"))
        );
        assert_eq!(context.get_metadata("nonexistent"), None);
    }

    #[tokio::test]
    async fn test_request_hook() {
        let hook = TestRequestHook::new("test", 50);
        let mut request = GenerateRequest::new(vec![Message::user("Hello")]);
        let mut context = HookContext::new();

        hook.on_request(&mut request, &mut context).await.unwrap();
        assert_eq!(request.temperature, Some(0.5));
        assert_eq!(hook.priority(), 50);
        assert_eq!(hook.name(), "test");
    }
}
