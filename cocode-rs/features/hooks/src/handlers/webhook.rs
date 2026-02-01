//! Webhook handler: sends hook context to an HTTP endpoint.
//!
//! The webhook receives the full `HookContext` as JSON via HTTP POST and returns
//! a JSON response. The response format supports both:
//!
//! 1. `HookResult` (legacy format with `action` tag):
//!    ```json
//!    { "action": "continue" }
//!    { "action": "reject", "reason": "..." }
//!    { "action": "modify_input", "new_input": {...} }
//!    ```
//!
//! 2. `HookOutput` (Claude Code v2.1.7 format):
//!    ```json
//!    { "continue_execution": true }
//!    { "continue_execution": false, "stop_reason": "..." }
//!    { "continue_execution": true, "updated_input": {...} }
//!    ```
//!
//! ## Request Headers
//!
//! - `Content-Type: application/json`
//! - `User-Agent: cocode-hooks/1.0`
//! - `X-Hook-Event: <event_type>` (e.g., "pre_tool_use")
//! - `X-Hook-Tool-Name: <tool_name>` (if applicable)
//! - `X-Hook-Session-Id: <session_id>`
//!
//! ## Error Handling
//!
//! On any error (network, timeout, invalid response), the handler returns
//! `Continue` to allow execution to proceed. Errors are logged at warn level.

use std::time::Duration;

use tracing::debug;
use tracing::warn;

use super::command::HookOutput;
use crate::context::HookContext;
use crate::result::HookResult;

/// Default timeout for webhook requests (10 seconds).
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Handles hooks that call external webhooks via HTTP POST.
pub struct WebhookHandler;

impl WebhookHandler {
    /// Sends the `HookContext` as JSON to the given URL and parses the response.
    ///
    /// The request includes headers with event metadata for routing/filtering.
    /// On any error, returns `Continue` to avoid blocking execution.
    pub async fn execute(url: &str, context: &HookContext) -> HookResult {
        Self::execute_with_timeout(url, context, DEFAULT_TIMEOUT_SECS).await
    }

    /// Execute webhook with custom timeout (for testing).
    pub async fn execute_with_timeout(
        url: &str,
        context: &HookContext,
        timeout_secs: u64,
    ) -> HookResult {
        debug!(url, event_type = %context.event_type, "Executing webhook hook");

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!(url, error = %e, "Failed to create HTTP client for webhook");
                return HookResult::Continue;
            }
        };

        let response = match client
            .post(url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "cocode-hooks/1.0")
            .header("X-Hook-Event", context.event_type.as_str())
            .header(
                "X-Hook-Tool-Name",
                context.tool_name.as_deref().unwrap_or(""),
            )
            .header("X-Hook-Session-Id", &context.session_id)
            .json(context)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(url, error = %e, "Webhook request failed");
                return HookResult::Continue;
            }
        };

        let status = response.status();
        if !status.is_success() {
            warn!(
                url,
                status = %status,
                "Webhook returned non-success status"
            );
            return HookResult::Continue;
        }

        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => {
                warn!(url, error = %e, "Failed to read webhook response body");
                return HookResult::Continue;
            }
        };

        if body.trim().is_empty() {
            return HookResult::Continue;
        }

        parse_webhook_response(url, body.trim())
    }
}

/// Parses webhook response, supporting both `HookResult` and `HookOutput` formats.
fn parse_webhook_response(url: &str, body: &str) -> HookResult {
    // Try parsing as HookResult first (has "action" field)
    if let Ok(result) = serde_json::from_str::<HookResult>(body) {
        return result;
    }

    // Try parsing as HookOutput (Claude Code v2.1.7 format with "continue_execution" field)
    if let Ok(output) = serde_json::from_str::<HookOutput>(body) {
        return output.into();
    }

    warn!(
        url,
        body = %body,
        "Failed to parse webhook response as HookResult or HookOutput"
    );
    HookResult::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::HookEventType;
    use std::path::PathBuf;

    fn make_ctx() -> HookContext {
        HookContext::new(
            HookEventType::PreToolUse,
            "test-session".to_string(),
            PathBuf::from("/tmp"),
        )
        .with_tool_name("Write")
    }

    #[test]
    fn test_parse_webhook_response_hook_result() {
        let json = r#"{"action":"continue"}"#;
        let result = parse_webhook_response("http://test", json);
        assert!(matches!(result, HookResult::Continue));

        let json = r#"{"action":"reject","reason":"blocked by policy"}"#;
        let result = parse_webhook_response("http://test", json);
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "blocked by policy");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_parse_webhook_response_hook_output() {
        let json = r#"{"continue_execution":true}"#;
        let result = parse_webhook_response("http://test", json);
        assert!(matches!(result, HookResult::Continue));

        let json = r#"{"continue_execution":false,"stop_reason":"denied"}"#;
        let result = parse_webhook_response("http://test", json);
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "denied");
        } else {
            panic!("Expected Reject");
        }

        let json = r#"{"continue_execution":true,"updated_input":{"modified":true}}"#;
        let result = parse_webhook_response("http://test", json);
        if let HookResult::ModifyInput { new_input } = result {
            assert_eq!(new_input["modified"], true);
        } else {
            panic!("Expected ModifyInput");
        }
    }

    #[test]
    fn test_parse_webhook_response_invalid() {
        let result = parse_webhook_response("http://test", "not json");
        assert!(matches!(result, HookResult::Continue));

        let result = parse_webhook_response("http://test", r#"{"unknown":"format"}"#);
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_execute_nonexistent_url() {
        let ctx = make_ctx();
        // Use a non-routable IP to ensure quick failure
        let result =
            WebhookHandler::execute_with_timeout("http://192.0.2.1:9999/hook", &ctx, 1).await;
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_execute_invalid_url() {
        let ctx = make_ctx();
        let result = WebhookHandler::execute("not-a-valid-url", &ctx).await;
        assert!(matches!(result, HookResult::Continue));
    }
}
