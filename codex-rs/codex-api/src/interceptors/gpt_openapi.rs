//! GPT OpenAPI interceptor.
//!
//! Adds session_id to the "extra" header as JSON.
//!
//! # Usage
//!
//! Configure in `config.toml`:
//! ```toml
//! [model_providers.my_provider]
//! interceptors = ["gpt_openapi"]
//! ```
//!
//! # Output
//!
//! When conversation_id is available, adds header:
//! ```text
//! extra: {"session_id": "<conversation_id>"}
//! ```

use super::Interceptor;
use super::InterceptorContext;
use codex_client::Request;

/// Interceptor that adds session context to the "extra" header.
///
/// This interceptor is designed for GPT OpenAPI compatible providers
/// that need session tracking via a custom header.
#[derive(Debug)]
pub struct GptOpenapiInterceptor;

impl Interceptor for GptOpenapiInterceptor {
    fn name(&self) -> &str {
        "gpt_openapi"
    }

    fn intercept(&self, request: &mut Request, ctx: &InterceptorContext) {
        if let Some(session_id) = &ctx.conversation_id {
            let extra_json = serde_json::json!({
                "session_id": session_id
            });
            if let Ok(value) = http::HeaderValue::from_str(&extra_json.to_string()) {
                request.headers.insert("extra", value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;
    use http::Method;

    fn create_test_request() -> Request {
        Request {
            method: Method::POST,
            url: "https://api.example.com/v1/chat".to_string(),
            headers: HeaderMap::new(),
            body: None,
            timeout: None,
        }
    }

    #[test]
    fn test_intercept_adds_session_header() {
        let interceptor = GptOpenapiInterceptor;
        let mut request = create_test_request();
        let ctx = InterceptorContext {
            conversation_id: Some("test-session-123".to_string()),
            model: Some("gpt-4".to_string()),
            provider_name: Some("test-provider".to_string()),
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
        let interceptor = GptOpenapiInterceptor;
        let mut request = create_test_request();
        let ctx = InterceptorContext {
            conversation_id: None,
            model: Some("gpt-4".to_string()),
            provider_name: Some("test-provider".to_string()),
        };

        interceptor.intercept(&mut request, &ctx);

        // Should not add header when conversation_id is None
        assert!(request.headers.get("extra").is_none());
    }

    #[test]
    fn test_interceptor_name() {
        let interceptor = GptOpenapiInterceptor;
        assert_eq!(interceptor.name(), "gpt_openapi");
    }
}
