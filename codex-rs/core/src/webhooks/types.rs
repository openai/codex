//! Webhook types and payloads

use crate::plan::PlanState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Webhook service type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookService {
    /// GitHub commit status / PR comment
    GitHub,

    /// Slack message
    Slack,

    /// Generic HTTP POST
    Http,
}

/// Webhook payload for plan events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Plan ID
    pub plan_id: String,

    /// Blueprint title
    pub title: String,

    /// Current state
    pub state: String,

    /// Event summary
    pub summary: String,

    /// Optional competition score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<CompetitionScore>,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Execution mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    /// Artifacts (file paths)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<Vec<String>>,
}

/// Competition score details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionScore {
    /// Variant name (A/B/C)
    pub variant: String,

    /// Total score
    pub total: f64,

    /// Test score component
    pub tests: f64,

    /// Performance score component
    pub performance: f64,

    /// Simplicity score component
    pub simplicity: f64,

    /// Winner flag
    pub is_winner: bool,
}

impl WebhookPayload {
    /// Create a new webhook payload
    pub fn new(
        plan_id: String,
        title: String,
        state: PlanState,
        summary: String,
    ) -> Self {
        Self {
            plan_id,
            title,
            state: state.name().to_string(),
            summary,
            score: None,
            timestamp: Utc::now(),
            mode: None,
            artifacts: None,
        }
    }

    /// Add competition score
    pub fn with_score(mut self, score: CompetitionScore) -> Self {
        self.score = Some(score);
        self
    }

    /// Add execution mode
    pub fn with_mode(mut self, mode: String) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Add artifacts
    pub fn with_artifacts(mut self, artifacts: Vec<String>) -> Self {
        self.artifacts = Some(artifacts);
        self
    }
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Service type
    pub service: WebhookService,

    /// Webhook URL
    pub url: String,

    /// Secret for HMAC signing
    pub secret: String,

    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Timeout in seconds
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_timeout_secs() -> u64 {
    10
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::state::PlanState;

    #[test]
    fn test_webhook_payload_creation() {
        let payload = WebhookPayload::new(
            "bp-123".to_string(),
            "Test Blueprint".to_string(),
            PlanState::Approved {
                approved_by: "user".to_string(),
                approved_at: Utc::now(),
            },
            "Blueprint approved".to_string(),
        );

        assert_eq!(payload.plan_id, "bp-123");
        assert_eq!(payload.state, "approved");
    }

    #[test]
    fn test_webhook_payload_with_score() {
        let score = CompetitionScore {
            variant: "A".to_string(),
            total: 95.5,
            tests: 100.0,
            performance: 90.0,
            simplicity: 96.5,
            is_winner: true,
        };

        let payload = WebhookPayload::new(
            "bp-123".to_string(),
            "Test".to_string(),
            PlanState::Drafting,
            "Summary".to_string(),
        )
        .with_score(score.clone());

        assert!(payload.score.is_some());
        assert_eq!(payload.score.unwrap().variant, "A");
    }
}
