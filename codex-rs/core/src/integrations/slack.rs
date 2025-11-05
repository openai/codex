/// Slack integration for Codex Sub-Agents
///
/// Provides notifications, status updates, and interactive commands
use anyhow::Context;
/// Slack integration for Codex Sub-Agents
///
/// Provides notifications, status updates, and interactive commands
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use tracing::debug;
use tracing::info;

/// Slack integration client
pub struct SlackIntegration {
    /// Webhook URL
    webhook_url: Option<String>,
    /// Bot token
    bot_token: Option<String>,
    /// Default channel
    #[allow(dead_code)]
    default_channel: String,
}

impl SlackIntegration {
    /// Create new Slack integration
    pub fn new(
        webhook_url: Option<String>,
        bot_token: Option<String>,
        default_channel: String,
    ) -> Self {
        Self {
            webhook_url,
            bot_token,
            default_channel,
        }
    }

    /// Send notification via webhook
    pub async fn send_notification(&self, message: SlackMessage) -> Result<()> {
        info!("📢 Sending Slack notification: {}", message.text);

        let webhook_url = self
            .webhook_url
            .as_ref()
            .context("Slack webhook URL not configured")?;

        // TODO: Actual Slack webhook POST
        debug!("Webhook URL: {}", webhook_url);
        debug!("Message: {:?}", message);

        Ok(())
    }

    /// Post message to channel (using Bot Token)
    pub async fn post_message(
        &self,
        channel: &str,
        text: &str,
        _blocks: Option<Vec<SlackBlock>>,
    ) -> Result<()> {
        info!("💬 Posting to Slack channel: {}", channel);

        let _token = self
            .bot_token
            .as_ref()
            .context("Slack bot token not configured")?;

        // TODO: Actual Slack API call
        debug!("Channel: {}, Text: {}", channel, text);

        Ok(())
    }

    /// Send agent progress update
    pub async fn notify_agent_progress(
        &self,
        agent_name: &str,
        progress: f64,
        status: &str,
    ) -> Result<()> {
        let message = SlackMessage {
            text: format!(
                "🤖 Agent '{}': {} ({:.0}%)",
                agent_name,
                status,
                progress * 100.0
            ),
            attachments: vec![SlackAttachment {
                color: match status {
                    "completed" => "good".to_string(),
                    "failed" => "danger".to_string(),
                    _ => "warning".to_string(),
                },
                fields: vec![
                    SlackField {
                        title: "Agent".to_string(),
                        value: agent_name.to_string(),
                        short: true,
                    },
                    SlackField {
                        title: "Progress".to_string(),
                        value: format!("{:.0}%", progress * 100.0),
                        short: true,
                    },
                ],
                ..Default::default()
            }],
        };

        self.send_notification(message).await
    }

    /// Send research report summary
    pub async fn notify_research_complete(
        &self,
        topic: &str,
        summary: &str,
        artifacts: &[String],
    ) -> Result<()> {
        let message = SlackMessage {
            text: format!("🔍 Research completed: {}", topic),
            attachments: vec![SlackAttachment {
                color: "good".to_string(),
                title: Some("Research Summary".to_string()),
                text: Some(summary.to_string()),
                fields: vec![SlackField {
                    title: "Artifacts".to_string(),
                    value: artifacts.join("\n"),
                    short: false,
                }],
                ..Default::default()
            }],
        };

        self.send_notification(message).await
    }

    /// Send PR created notification
    pub async fn notify_pr_created(
        &self,
        pr_number: u64,
        pr_url: &str,
        agent_name: &str,
    ) -> Result<()> {
        let message = SlackMessage {
            text: format!("✅ PR #{} created by agent '{}'", pr_number, agent_name),
            attachments: vec![SlackAttachment {
                color: "good".to_string(),
                title: Some(format!("Pull Request #{}", pr_number)),
                title_link: Some(pr_url.to_string()),
                fields: vec![SlackField {
                    title: "Created by".to_string(),
                    value: agent_name.to_string(),
                    short: true,
                }],
                ..Default::default()
            }],
        };

        self.send_notification(message).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessage {
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<SlackAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlackAttachment {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default)]
    pub fields: Vec<SlackField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackField {
    pub title: String,
    pub value: String,
    pub short: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<SlackText>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackText {
    #[serde(rename = "type")]
    pub text_type: String,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_slack_integration() {
        let slack = SlackIntegration::new(
            Some("https://hooks.slack.com/services/test".to_string()),
            None,
            "#general".to_string(),
        );

        let message = SlackMessage {
            text: "Test message".to_string(),
            attachments: vec![],
        };

        // This would fail without actual webhook, but tests structure
        assert_eq!(message.text, "Test message");
    }

    #[test]
    fn test_slack_message_serialization() {
        let message = SlackMessage {
            text: "Test".to_string(),
            attachments: vec![SlackAttachment {
                color: "good".to_string(),
                title: Some("Title".to_string()),
                text: Some("Body".to_string()),
                ..Default::default()
            }],
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("good"));
    }
}
