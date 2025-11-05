//! Generic webhook and external API integration client.

use anyhow::Context;
use anyhow::Result;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Supported webhook/API services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebhookService {
    /// GitHub API
    GitHub,
    /// Slack Webhook
    Slack,
    /// Custom webhook
    Custom,
}

/// Payload for webhook/API calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Service to call
    pub service: WebhookService,
    /// Action/endpoint to invoke
    pub action: String,
    /// Payload data
    pub data: Value,
    /// Optional custom headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Map<String, Value>>,
}

/// Response from webhook/API call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResponse {
    /// HTTP status code
    pub status: u16,
    /// Response body (if JSON)
    pub body: Option<Value>,
    /// Raw text response
    pub text: String,
    /// Whether the call succeeded
    pub success: bool,
}

/// Client for making webhook and API calls.
pub struct WebhookClient {
    /// HTTP client
    client: reqwest::Client,
    /// GitHub API token (from env)
    github_token: Option<String>,
    /// Slack webhook URL (from env)
    slack_webhook_url: Option<String>,
}

impl WebhookClient {
    /// Create a new webhook client.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("codex-webhook-client/1.0")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let github_token = std::env::var("GITHUB_TOKEN").ok();
        let slack_webhook_url = std::env::var("SLACK_WEBHOOK_URL").ok();

        Self {
            client,
            github_token,
            slack_webhook_url,
        }
    }

    /// Execute a webhook call.
    pub async fn execute(&self, payload: WebhookPayload) -> Result<WebhookResponse> {
        match payload.service {
            WebhookService::GitHub => self.execute_github(&payload).await,
            WebhookService::Slack => self.execute_slack(&payload).await,
            WebhookService::Custom => self.execute_custom(&payload).await,
        }
    }

    /// Execute a GitHub API call.
    async fn execute_github(&self, payload: &WebhookPayload) -> Result<WebhookResponse> {
        let token = self
            .github_token
            .as_ref()
            .context("GITHUB_TOKEN environment variable not set")?;

        let base_url = "https://api.github.com";
        let url = format!("{}/{}", base_url, payload.action.trim_start_matches('/'));

        info!("🔗 GitHub API call: {}", url);
        debug!("Payload: {:?}", payload.data);

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).context("Invalid GitHub token")?,
        );
        headers.insert(
            "Accept",
            HeaderValue::from_static("application/vnd.github.v3+json"),
        );

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&payload.data)
            .send()
            .await
            .context("GitHub API request failed")?;

        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .context("Failed to read GitHub API response")?;

        let body = serde_json::from_str(&text).ok();
        let success = (200..300).contains(&status);

        if !success {
            warn!("GitHub API call failed with status {}: {}", status, text);
        }

        Ok(WebhookResponse {
            status,
            body,
            text,
            success,
        })
    }

    /// Execute a Slack webhook call.
    async fn execute_slack(&self, payload: &WebhookPayload) -> Result<WebhookResponse> {
        let webhook_url = self
            .slack_webhook_url
            .as_ref()
            .context("SLACK_WEBHOOK_URL environment variable not set")?;

        info!("🔗 Slack webhook call");
        debug!("Payload: {:?}", payload.data);

        let response = self
            .client
            .post(webhook_url)
            .json(&payload.data)
            .send()
            .await
            .context("Slack webhook request failed")?;

        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .context("Failed to read Slack webhook response")?;

        let success = (200..300).contains(&status);

        if !success {
            warn!("Slack webhook call failed with status {}: {}", status, text);
        }

        Ok(WebhookResponse {
            status,
            body: None,
            text,
            success,
        })
    }

    /// Execute a custom webhook call.
    async fn execute_custom(&self, payload: &WebhookPayload) -> Result<WebhookResponse> {
        // For custom webhooks, the "action" field should be the full URL
        let url = &payload.action;

        info!("🔗 Custom webhook call: {}", url);
        debug!("Payload: {:?}", payload.data);

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Add custom headers if provided
        if let Some(custom_headers) = &payload.headers {
            for (key, value) in custom_headers {
                if let Some(value_str) = value.as_str()
                    && let Ok(header_value) = HeaderValue::from_str(value_str)
                {
                    headers.insert(key.parse().unwrap_or(CONTENT_TYPE), header_value);
                }
            }
        }

        let response = self
            .client
            .post(url)
            .headers(headers)
            .json(&payload.data)
            .send()
            .await
            .context("Custom webhook request failed")?;

        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .context("Failed to read custom webhook response")?;

        let body = serde_json::from_str(&text).ok();
        let success = (200..300).contains(&status);

        if !success {
            warn!(
                "Custom webhook call failed with status {}: {}",
                status, text
            );
        }

        Ok(WebhookResponse {
            status,
            body,
            text,
            success,
        })
    }

    /// Helper: Create a GitHub PR.
    pub async fn create_github_pr(
        &self,
        repo: &str,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<WebhookResponse> {
        let payload = WebhookPayload {
            service: WebhookService::GitHub,
            action: format!("repos/{repo}/pulls"),
            data: serde_json::json!({
                "title": title,
                "body": body,
                "head": head,
                "base": base,
            }),
            headers: None,
        };

        self.execute(payload).await
    }

    /// Helper: Post a message to Slack.
    pub async fn post_slack_message(
        &self,
        text: &str,
        channel: Option<&str>,
    ) -> Result<WebhookResponse> {
        let mut data = serde_json::json!({
            "text": text,
        });

        if let Some(ch) = channel {
            data["channel"] = serde_json::Value::String(ch.to_string());
        }

        let payload = WebhookPayload {
            service: WebhookService::Slack,
            action: String::new(), // Not used for Slack
            data,
            headers: None,
        };

        self.execute(payload).await
    }
}

impl Default for WebhookClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_payload_serialization() {
        let payload = WebhookPayload {
            service: WebhookService::GitHub,
            action: "repos/test/pulls".to_string(),
            data: serde_json::json!({"title": "Test PR"}),
            headers: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("github"));
        assert!(json.contains("repos/test/pulls"));
    }

    #[test]
    fn test_webhook_service_serde() {
        let service = WebhookService::Slack;
        let json = serde_json::to_string(&service).unwrap();
        assert_eq!(json, "\"slack\"");

        let deserialized: WebhookService = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, WebhookService::Slack);
    }
}
