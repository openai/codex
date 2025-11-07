//! Webhook client with HMAC signing and retry logic

use super::types::{WebhookConfig, WebhookPayload, WebhookService};
use anyhow::{Context, Result};
use reqwest::Client;
use sha2::Sha256;
use std::time::Duration;
use tracing::{debug, warn};

/// Webhook client
pub struct WebhookClient {
    client: Client,
}

impl WebhookClient {
    /// Create a new webhook client
    pub fn new() -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self { client })
    }

    /// Send a webhook with retry logic
    pub async fn send(&self, config: &WebhookConfig, payload: &WebhookPayload) -> Result<()> {
        let mut last_error = None;

        for attempt in 0..=config.max_retries {
            if attempt > 0 {
                // Exponential backoff: 1s, 2s, 4s
                let delay = Duration::from_secs(2u64.pow(attempt - 1));
                debug!(
                    "Retrying webhook after {}s (attempt {})",
                    delay.as_secs(),
                    attempt
                );
                tokio::time::sleep(delay).await;
            }

            match self.send_once(config, payload).await {
                Ok(_) => {
                    debug!("Webhook sent successfully to {}", config.url);
                    return Ok(());
                }
                Err(e) => {
                    warn!("Webhook send failed (attempt {}): {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All webhook retries failed")))
    }

    /// Send webhook once (no retry)
    async fn send_once(&self, config: &WebhookConfig, payload: &WebhookPayload) -> Result<()> {
        let body = match config.service {
            WebhookService::GitHub => self.format_github_payload(payload)?,
            WebhookService::Slack => self.format_slack_payload(payload)?,
            WebhookService::Http => serde_json::to_string(payload)?,
        };

        // Compute HMAC signature
        let signature = self.compute_hmac(&config.secret, &body);

        // Send request
        let response = self
            .client
            .post(&config.url)
            .header("Content-Type", "application/json")
            .header("X-Codex-Signature", format!("sha256={}", signature))
            .header("X-Codex-Event", format!("plan.{}", payload.state))
            .timeout(Duration::from_secs(config.timeout_secs))
            .body(body)
            .send()
            .await
            .context("Failed to send webhook request")?;

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Webhook returned error {}: {}", status, body);
        }

        Ok(())
    }

    /// Compute HMAC-SHA256 signature
    fn compute_hmac(&self, secret: &str, body: &str) -> String {
        use hmac::Mac;
        let mut mac = hmac::Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(body.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    /// Format payload for GitHub
    fn format_github_payload(&self, payload: &WebhookPayload) -> Result<String> {
        let github_payload = serde_json::json!({
            "context": "codex/plan",
            "state": self.map_state_to_github(&payload.state),
            "description": &payload.summary,
            "target_url": format!("https://github.com/zapabob/codex/plans/{}", payload.plan_id),
            "plan_id": &payload.plan_id,
            "title": &payload.title,
            "timestamp": payload.timestamp.to_rfc3339(),
        });

        serde_json::to_string(&github_payload).context("Failed to serialize GitHub payload")
    }

    /// Format payload for Slack
    fn format_slack_payload(&self, payload: &WebhookPayload) -> Result<String> {
        let emoji = match payload.state.as_str() {
            "approved" => ":white_check_mark:",
            "rejected" => ":x:",
            "pending" => ":hourglass:",
            "drafting" => ":pencil:",
            _ => ":information_source:",
        };

        let mut text = format!("{} *{}*\n{}\n\n", emoji, payload.title, payload.summary);

        if let Some(score) = &payload.score {
            text.push_str(&format!(
                "*Competition Result*: Variant {} (Score: {:.1})\n",
                score.variant, score.total
            ));
        }

        if let Some(artifacts) = &payload.artifacts {
            text.push_str(&format!("*Artifacts*: {}\n", artifacts.join(", ")));
        }

        let slack_payload = serde_json::json!({
            "text": text,
            "username": "Codex Blueprint",
            "icon_emoji": ":robot_face:",
        });

        serde_json::to_string(&slack_payload).context("Failed to serialize Slack payload")
    }

    /// Map blueprint state to GitHub commit status
    fn map_state_to_github(&self, state: &str) -> &'static str {
        match state {
            "approved" => "success",
            "rejected" => "failure",
            "pending" => "pending",
            _ => "pending",
        }
    }
}

impl Default for WebhookClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default WebhookClient")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::state::PlanState;

    #[test]
    fn test_compute_hmac() {
        let client = WebhookClient::new().unwrap();
        let signature1 = client.compute_hmac("secret", "test body");
        let signature2 = client.compute_hmac("secret", "test body");

        // Deterministic
        assert_eq!(signature1, signature2);

        // Different input produces different signature
        let signature3 = client.compute_hmac("secret", "different body");
        assert_ne!(signature1, signature3);
    }

    #[test]
    fn test_format_github_payload() {
        let client = WebhookClient::new().unwrap();
        let payload = WebhookPayload::new(
            "bp-123".to_string(),
            "Test".to_string(),
            PlanState::Approved {
                approved_by: "user".to_string(),
                approved_at: chrono::Utc::now(),
            },
            "Summary".to_string(),
        );

        let result = client.format_github_payload(&payload).unwrap();
        assert!(result.contains("codex/plan"));
        assert!(result.contains("success"));
    }

    #[test]
    fn test_format_slack_payload() {
        let client = WebhookClient::new().unwrap();
        let payload = WebhookPayload::new(
            "bp-123".to_string(),
            "Test Blueprint".to_string(),
            PlanState::Approved {
                approved_by: "user".to_string(),
                approved_at: chrono::Utc::now(),
            },
            "Blueprint approved!".to_string(),
        );

        let result = client.format_slack_payload(&payload).unwrap();
        assert!(result.contains("Test Blueprint"));
        assert!(result.contains(":white_check_mark:"));
    }
}
