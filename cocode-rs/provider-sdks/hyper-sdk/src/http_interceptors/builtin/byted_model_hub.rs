//! ByteDance ModelHub interceptor.
//!
//! Adds session_id to the "extra" header as JSON for ByteDance ModelHub
//! session tracking.
//!
//! # Usage
//!
//! Configure in `providers.json`:
//! ```json
//! {
//!   "name": "byted-model-hub",
//!   "type": "openai",
//!   "base_url": "https://ark.cn-beijing.volces.com/api/v3",
//!   "interceptors": ["byted_model_hub"]
//! }
//! ```
//!
//! # Output
//!
//! When conversation_id is available, adds header:
//! ```text
//! extra: {"session_id": "<conversation_id>"}
//! ```

use crate::http_interceptors::HttpInterceptor;
use crate::http_interceptors::HttpInterceptorContext;
use crate::http_interceptors::HttpRequest;
use http::HeaderValue;

/// Interceptor for ByteDance ModelHub session tracking.
///
/// This interceptor adds a `session_id` field to the "extra" header as JSON.
/// ByteDance ModelHub uses this header for session tracking in multi-turn
/// conversations.
///
/// # Example
///
/// ```no_run
/// use hyper_sdk::http_interceptors::{
///     HttpInterceptorChain, HttpInterceptorContext, HttpRequest, BytedModelHubInterceptor
/// };
/// use std::sync::Arc;
///
/// let mut chain = HttpInterceptorChain::new();
/// chain.add(Arc::new(BytedModelHubInterceptor));
///
/// let mut request = HttpRequest::post("https://ark.cn-beijing.volces.com/api/v3/chat");
/// let ctx = HttpInterceptorContext::new().conversation_id("session-123");
///
/// chain.apply(&mut request, &ctx);
/// // Request now has header: extra: {"session_id": "session-123"}
/// ```
#[derive(Debug, Clone, Default)]
pub struct BytedModelHubInterceptor;

impl BytedModelHubInterceptor {
    /// Create a new BytedModelHubInterceptor.
    pub fn new() -> Self {
        Self
    }
}

impl HttpInterceptor for BytedModelHubInterceptor {
    fn name(&self) -> &str {
        "byted_model_hub"
    }

    fn priority(&self) -> i32 {
        50
    }

    fn intercept(&self, request: &mut HttpRequest, ctx: &HttpInterceptorContext) {
        if let Some(session_id) = &ctx.conversation_id {
            let extra_json = serde_json::json!({
                "session_id": session_id
            });
            if let Ok(value) = HeaderValue::from_str(&extra_json.to_string()) {
                request.headers.insert("extra", value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_request() -> HttpRequest {
        HttpRequest::post("https://ark.cn-beijing.volces.com/api/v3/chat")
    }

    #[test]
    fn test_intercept_adds_session_header() {
        let interceptor = BytedModelHubInterceptor::new();
        let mut request = create_test_request();
        let ctx = HttpInterceptorContext {
            conversation_id: Some("test-session-123".to_string()),
            model: Some("deepseek-v3".to_string()),
            provider_name: Some("byted-model-hub".to_string()),
            request_id: None,
            metadata: Default::default(),
        };

        interceptor.intercept(&mut request, &ctx);

        let extra_header = request
            .headers
            .get("extra")
            .expect("extra header should exist");
        let extra_str = extra_header.to_str().unwrap();
        let extra_json: serde_json::Value = serde_json::from_str(extra_str).unwrap();

        assert_eq!(extra_json["session_id"], "test-session-123");
    }

    #[test]
    fn test_intercept_no_session_id() {
        let interceptor = BytedModelHubInterceptor::new();
        let mut request = create_test_request();
        let ctx = HttpInterceptorContext {
            conversation_id: None,
            model: Some("deepseek-v3".to_string()),
            provider_name: Some("byted-model-hub".to_string()),
            request_id: None,
            metadata: Default::default(),
        };

        interceptor.intercept(&mut request, &ctx);

        // Should not add header when conversation_id is None
        assert!(request.headers.get("extra").is_none());
    }

    #[test]
    fn test_interceptor_name() {
        let interceptor = BytedModelHubInterceptor::new();
        assert_eq!(interceptor.name(), "byted_model_hub");
    }

    #[test]
    fn test_interceptor_priority() {
        let interceptor = BytedModelHubInterceptor::new();
        assert_eq!(interceptor.priority(), 50);
    }

    #[test]
    fn test_interceptor_default() {
        let interceptor = BytedModelHubInterceptor::default();
        assert_eq!(interceptor.name(), "byted_model_hub");
    }
}
