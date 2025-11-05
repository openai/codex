//! Telemetry events for blueprint operations
//!
//! Privacy-respecting event collection for Blueprint Mode.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;

/// Telemetry event types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Blueprint lifecycle events
    BlueprintStart,
    BlueprintGenerate,
    BlueprintApprove,
    BlueprintReject,
    BlueprintExport,

    // Execution events
    ExecStart,
    ExecResult,

    // Research events
    ResearchStart,
    ResearchComplete,

    // Webhook events
    WebhookSent,
    WebhookFailed,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlueprintStart => write!(f, "bp.start"),
            Self::BlueprintGenerate => write!(f, "bp.generate"),
            Self::BlueprintApprove => write!(f, "bp.approve"),
            Self::BlueprintReject => write!(f, "bp.reject"),
            Self::BlueprintExport => write!(f, "bp.export"),
            Self::ExecStart => write!(f, "exec.start"),
            Self::ExecResult => write!(f, "exec.result"),
            Self::ResearchStart => write!(f, "research.start"),
            Self::ResearchComplete => write!(f, "research.complete"),
            Self::WebhookSent => write!(f, "webhook.sent"),
            Self::WebhookFailed => write!(f, "webhook.failed"),
        }
    }
}

/// A telemetry event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Event ID (UUID)
    pub id: String,

    /// Event type
    pub event_type: EventType,

    /// Timestamp (UTC)
    pub timestamp: DateTime<Utc>,

    /// Session ID (hashed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// User ID (hashed, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Blueprint ID (hashed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blueprint_id: Option<String>,

    /// Custom metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TelemetryEvent {
    /// Create a new telemetry event
    pub fn new(event_type: EventType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type,
            timestamp: Utc::now(),
            session_id: None,
            user_id: None,
            blueprint_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Set session ID (will be hashed)
    pub fn with_session_id(mut self, session_id: impl AsRef<str>) -> Self {
        self.session_id = Some(hash_id(session_id.as_ref()));
        self
    }

    /// Set user ID (will be hashed)
    pub fn with_user_id(mut self, user_id: impl AsRef<str>) -> Self {
        self.user_id = Some(hash_id(user_id.as_ref()));
        self
    }

    /// Set blueprint ID (will be hashed)
    pub fn with_blueprint_id(mut self, blueprint_id: impl AsRef<str>) -> Self {
        self.blueprint_id = Some(hash_id(blueprint_id.as_ref()));
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), json_value);
        }
        self
    }
}

/// Hash an ID for privacy (SHA-256)
pub fn hash_id(id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Sanitize URL to domain-only (for privacy)
pub fn sanitize_url(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = TelemetryEvent::new(EventType::BlueprintStart)
            .with_session_id("session-123")
            .with_user_id("user-456")
            .with_blueprint_id("bp-789")
            .with_metadata("mode", "orchestrated");

        assert_eq!(event.event_type, EventType::BlueprintStart);
        assert!(event.session_id.is_some());
        assert!(event.user_id.is_some());
        assert!(event.blueprint_id.is_some());
        assert!(event.metadata.contains_key("mode"));
    }

    #[test]
    fn test_hash_id() {
        let hashed1 = hash_id("test-id");
        let hashed2 = hash_id("test-id");

        // Same input produces same hash
        assert_eq!(hashed1, hashed2);

        // Hash is different from input
        assert_ne!(hashed1, "test-id");

        // Hash is deterministic
        assert_eq!(hashed1.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn test_sanitize_url() {
        assert_eq!(
            sanitize_url("https://github.com/user/repo"),
            Some("github.com".to_string())
        );

        assert_eq!(
            sanitize_url("https://api.example.com:8080/v1/endpoint"),
            Some("api.example.com".to_string())
        );

        assert_eq!(sanitize_url("invalid-url"), None);
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::BlueprintStart.to_string(), "bp.start");
        assert_eq!(EventType::ExecResult.to_string(), "exec.result");
        assert_eq!(EventType::ResearchComplete.to_string(), "research.complete");
    }
}
