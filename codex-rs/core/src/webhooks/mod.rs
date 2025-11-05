//! Webhook integration module
//!
//! Supports GitHub, Slack, and generic HTTP webhooks with HMAC signing and retry logic.

pub mod client;
pub mod types;

pub use client::WebhookClient;
pub use types::{CompetitionScore, WebhookConfig, WebhookPayload, WebhookService};

use anyhow::Result;
use std::sync::Arc;

/// Global webhook client (lazy-initialized)
static WEBHOOK_CLIENT: once_cell::sync::OnceCell<Arc<WebhookClient>> =
    once_cell::sync::OnceCell::new();

/// Initialize the global webhook client
pub fn init() -> Result<()> {
    let client = Arc::new(WebhookClient::new()?);
    WEBHOOK_CLIENT
        .set(client)
        .map_err(|_| anyhow::anyhow!("Webhook client already initialized"))?;
    Ok(())
}

/// Get the global webhook client
pub fn instance() -> Option<Arc<WebhookClient>> {
    WEBHOOK_CLIENT.get().cloned()
}

/// Send a webhook (convenience function)
pub async fn send(config: &WebhookConfig, payload: &WebhookPayload) -> Result<()> {
    let client = instance()
        .unwrap_or_else(|| Arc::new(WebhookClient::new().expect("Failed to create WebhookClient")));

    client.send(config, payload).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init().unwrap();
        assert!(instance().is_some());
    }
}
