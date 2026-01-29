//! Webhook handler stub.
//!
//! Sends hook context to an HTTP endpoint. Currently a stub that returns
//! `Continue` with a warning.

use tracing::warn;

use crate::context::HookContext;
use crate::result::HookResult;

/// Handles hooks that call external webhooks.
pub struct WebhookHandler;

impl WebhookHandler {
    /// Stub implementation. Logs a warning and returns `Continue`.
    ///
    /// In the future this will POST the `HookContext` as JSON to the given URL
    /// and parse the response as a `HookResult`.
    pub async fn execute(url: &str, _context: &HookContext) -> HookResult {
        warn!(url, "Webhook hook handler not yet implemented");
        HookResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::HookEventType;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_stub_returns_continue() {
        let ctx = HookContext::new(
            HookEventType::SessionStart,
            "s1".to_string(),
            PathBuf::from("/tmp"),
        );
        let result = WebhookHandler::execute("https://example.com/hook", &ctx).await;
        assert!(matches!(result, HookResult::Continue));
    }
}
