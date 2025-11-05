/// Webhook handler for external integrations
///
/// Supports GitHub Actions, Slack, and custom webhook endpoints
use anyhow::Context;
/// Webhook handler for external integrations
///
/// Supports GitHub Actions, Slack, and custom webhook endpoints
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Webhook handler
pub struct WebhookHandler {
    /// Registered webhooks
    webhooks: HashMap<String, WebhookConfig>,
}

impl WebhookHandler {
    /// Create new webhook handler
    pub fn new() -> Self {
        Self {
            webhooks: HashMap::new(),
        }
    }

    /// Register a webhook
    pub fn register(&mut self, name: String, config: WebhookConfig) {
        info!("🔗 Registering webhook: {}", name);
        self.webhooks.insert(name, config);
    }

    /// Trigger webhook by name
    pub async fn trigger(&self, name: &str, payload: WebhookPayload) -> Result<()> {
        let config = self
            .webhooks
            .get(name)
            .with_context(|| format!("Webhook '{}' not found", name))?;

        info!("📤 Triggering webhook '{}' to {}", name, config.url);

        self.send_webhook(config, payload).await
    }

    /// Trigger all webhooks matching event type
    pub async fn trigger_for_event(
        &self,
        event: WebhookEvent,
        payload: WebhookPayload,
    ) -> Result<()> {
        let matching: Vec<_> = self
            .webhooks
            .iter()
            .filter(|(_, config)| config.events.contains(&event))
            .collect();

        if matching.is_empty() {
            debug!("No webhooks registered for event: {:?}", event);
            return Ok(());
        }

        info!(
            "📤 Triggering {} webhooks for event: {:?}",
            matching.len(),
            event
        );

        for (name, config) in matching {
            if let Err(e) = self.send_webhook(config, payload.clone()).await {
                warn!("Failed to send webhook '{}': {}", name, e);
            }
        }

        Ok(())
    }

    /// Send webhook HTTP POST
    async fn send_webhook(&self, config: &WebhookConfig, payload: WebhookPayload) -> Result<()> {
        debug!("Sending webhook to {}", config.url);
        debug!("Headers: {:?}", config.headers);
        debug!("Payload: {:?}", payload);

        // TODO: Actual HTTP POST with reqwest
        // let client = reqwest::Client::new();
        // let response = client.post(&config.url)
        //     .headers(...)
        //     .json(&payload)
        //     .send()
        //     .await?;

        Ok(())
    }
}

impl Default for WebhookHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL
    pub url: String,
    /// Events to trigger on
    pub events: Vec<WebhookEvent>,
    /// Custom headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Authentication method
    pub auth: Option<WebhookAuth>,
}

/// Webhook authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebhookAuth {
    /// Bearer token
    Bearer { token: String },
    /// Basic auth
    Basic { username: String, password: String },
    /// Custom header
    Header { name: String, value: String },
}

/// Webhook event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WebhookEvent {
    /// Agent task started
    AgentStarted,
    /// Agent task completed
    AgentCompleted,
    /// Agent task failed
    AgentFailed,
    /// Research started
    ResearchStarted,
    /// Research completed
    ResearchCompleted,
    /// PR created
    PrCreated,
    /// PR merged
    PrMerged,
    /// Review completed
    ReviewCompleted,
    /// Test results available
    TestResults,
    /// Security audit completed
    SecurityAudit,
}

/// Webhook payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Event type
    pub event: WebhookEvent,
    /// Timestamp
    pub timestamp: String,
    /// Event data
    pub data: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_webhook_handler() {
        let mut handler = WebhookHandler::new();

        let config = WebhookConfig {
            url: "https://example.com/webhook".to_string(),
            events: vec![WebhookEvent::AgentCompleted],
            headers: HashMap::new(),
            auth: None,
        };

        handler.register("test-webhook".to_string(), config);

        let mut data = HashMap::new();
        data.insert("agent".to_string(), serde_json::json!("test-agent"));

        let payload = WebhookPayload {
            event: WebhookEvent::AgentCompleted,
            timestamp: "2025-10-10T18:00:00Z".to_string(),
            data,
        };

        // Should not fail (mock implementation)
        handler.trigger("test-webhook", payload).await.unwrap();
    }

    #[test]
    fn test_webhook_config_serialization() {
        let config = WebhookConfig {
            url: "https://hooks.slack.com/test".to_string(),
            events: vec![WebhookEvent::AgentCompleted, WebhookEvent::PrCreated],
            headers: HashMap::new(),
            auth: Some(WebhookAuth::Bearer {
                token: "secret".to_string(),
            }),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("AgentCompleted"));
        assert!(json.contains("PrCreated"));
    }
}
