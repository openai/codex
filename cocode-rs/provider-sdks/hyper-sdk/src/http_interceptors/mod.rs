//! HTTP interceptor system for modifying requests before sending.
//!
//! This module provides an HTTP-level interceptor system that allows modifying
//! requests before they are sent to the provider. Unlike the higher-level
//! `RequestHook` which operates on `GenerateRequest`, HTTP interceptors work
//! directly with HTTP request details (headers, URL, body).
//!
//! # Architecture
//!
//! ```text
//! config (provider.interceptors = ["byted_model_hub"])
//!   └── HttpInterceptorChain
//!       └── interceptor.intercept(&mut HttpRequest, &HttpInterceptorContext)
//! ```
//!
//! # Built-in Interceptors
//!
//! - `byted_model_hub` - Adds session_id to "extra" header as JSON for ByteDance ModelHub
//!
//! # Example
//!
//! ```no_run
//! use hyper_sdk::http_interceptors::{HttpInterceptor, HttpInterceptorContext, HttpRequest};
//!
//! #[derive(Debug)]
//! struct MyInterceptor;
//!
//! impl HttpInterceptor for MyInterceptor {
//!     fn name(&self) -> &str {
//!         "my_interceptor"
//!     }
//!
//!     fn intercept(&self, request: &mut HttpRequest, ctx: &HttpInterceptorContext) {
//!         // Add custom header
//!         let value = http::HeaderValue::from_static("custom-value");
//!         request.headers.insert("X-Custom-Header", value);
//!     }
//! }
//! ```

pub mod builtin;
mod chain;

pub use builtin::BytedModelHubInterceptor;
pub use chain::HttpInterceptorChain;

use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;

/// HTTP request that can be modified by interceptors.
///
/// This represents the HTTP-level request details that interceptors
/// can inspect and modify before the request is sent.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// HTTP method (GET, POST, etc.).
    pub method: http::Method,
    /// Full URL including query parameters.
    pub url: String,
    /// HTTP headers.
    pub headers: http::HeaderMap,
    /// Request body as JSON (for JSON APIs).
    pub body: Option<Value>,
}

impl HttpRequest {
    /// Create a new HTTP request with the given method and URL.
    pub fn new(method: http::Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: http::HeaderMap::new(),
            body: None,
        }
    }

    /// Create a POST request with the given URL.
    pub fn post(url: impl Into<String>) -> Self {
        Self::new(http::Method::POST, url)
    }

    /// Set the request body.
    pub fn with_body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }

    /// Add a header to the request.
    pub fn with_header(mut self, name: &'static str, value: &str) -> Self {
        if let Ok(header_value) = http::HeaderValue::from_str(value) {
            self.headers.insert(name, header_value);
        }
        self
    }
}

/// Context passed to HTTP interceptors.
///
/// Contains request metadata that interceptors can use to make decisions
/// about how to modify the request.
#[derive(Debug, Clone, Default)]
pub struct HttpInterceptorContext {
    /// Conversation/session ID for tracking multi-turn conversations.
    pub conversation_id: Option<String>,
    /// Model being used (e.g., "gpt-4o", "claude-sonnet-4-20250514").
    pub model: Option<String>,
    /// Provider name (e.g., "openai", "anthropic").
    pub provider_name: Option<String>,
    /// Unique request ID for this specific request.
    pub request_id: Option<String>,
    /// Custom metadata that can be used by interceptors.
    pub metadata: HashMap<String, Value>,
}

impl HttpInterceptorContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with provider and model.
    pub fn with_provider(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider_name: Some(provider.into()),
            model: Some(model.into()),
            ..Default::default()
        }
    }

    /// Set the conversation ID.
    pub fn conversation_id(mut self, id: impl Into<String>) -> Self {
        self.conversation_id = Some(id.into());
        self
    }

    /// Set the request ID.
    pub fn request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
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

/// Trait for HTTP request interceptors.
///
/// Interceptors can modify the HTTP request before it's sent to the provider.
/// This includes headers, URL, and body modifications.
///
/// # Implementors
///
/// To create a custom interceptor:
/// 1. Implement this trait on a struct
/// 2. Return a unique name from `name()`
/// 3. Modify the request in `intercept()`
/// 4. Optionally set priority (lower = earlier execution)
pub trait HttpInterceptor: Send + Sync + Debug {
    /// Unique name identifying this interceptor.
    ///
    /// This name is used in configuration to enable the interceptor.
    fn name(&self) -> &str;

    /// Interceptor priority (lower = earlier execution).
    ///
    /// Default priority is 100. Built-in interceptors typically use 50.
    fn priority(&self) -> i32 {
        100
    }

    /// Modify the request.
    ///
    /// Called before the request is sent. The interceptor can modify
    /// any field of the request: method, url, headers, body.
    fn intercept(&self, request: &mut HttpRequest, ctx: &HttpInterceptorContext);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_builder() {
        let request = HttpRequest::post("https://api.example.com/v1/chat")
            .with_header("Authorization", "Bearer token")
            .with_body(serde_json::json!({"message": "hello"}));

        assert_eq!(request.method, http::Method::POST);
        assert_eq!(request.url, "https://api.example.com/v1/chat");
        assert!(request.headers.contains_key("Authorization"));
        assert!(request.body.is_some());
    }

    #[test]
    fn test_http_interceptor_context_builder() {
        let ctx = HttpInterceptorContext::with_provider("openai", "gpt-4o")
            .conversation_id("conv_123")
            .request_id("req_456");

        assert_eq!(ctx.provider_name, Some("openai".to_string()));
        assert_eq!(ctx.model, Some("gpt-4o".to_string()));
        assert_eq!(ctx.conversation_id, Some("conv_123".to_string()));
        assert_eq!(ctx.request_id, Some("req_456".to_string()));
    }

    #[test]
    fn test_http_interceptor_context_metadata() {
        let mut ctx = HttpInterceptorContext::new();
        ctx.set_metadata("key", serde_json::json!("value"));

        assert_eq!(ctx.get_metadata("key"), Some(&serde_json::json!("value")));
        assert_eq!(ctx.get_metadata("nonexistent"), None);
    }
}
