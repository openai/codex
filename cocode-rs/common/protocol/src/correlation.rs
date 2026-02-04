//! Correlation types for request-response tracking.
//!
//! These types enable tracking which events correspond to which commands,
//! providing better observability and debugging capabilities.

use serde::Deserialize;
use serde::Serialize;

use crate::LoopEvent;

/// A unique identifier for a command submission.
///
/// Used to correlate events back to the command that triggered them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubmissionId(pub String);

impl SubmissionId {
    /// Create a new submission ID with a random UUID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Create a submission ID from an existing string.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the inner string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Default for SubmissionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SubmissionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SubmissionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SubmissionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for SubmissionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A loop event with optional correlation information.
///
/// Wraps a [`LoopEvent`] with an optional [`SubmissionId`] to enable
/// tracking which command triggered this event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelatedEvent {
    /// The correlation ID linking this event to its originating command.
    ///
    /// This is `None` for events that are not triggered by a specific command,
    /// such as background task completions or system-initiated events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<SubmissionId>,

    /// The underlying loop event.
    pub event: LoopEvent,
}

impl CorrelatedEvent {
    /// Create a new correlated event with the given correlation ID.
    pub fn new(event: LoopEvent, correlation_id: Option<SubmissionId>) -> Self {
        Self {
            correlation_id,
            event,
        }
    }

    /// Create a correlated event without a correlation ID.
    pub fn uncorrelated(event: LoopEvent) -> Self {
        Self {
            correlation_id: None,
            event,
        }
    }

    /// Create a correlated event with a correlation ID.
    pub fn correlated(event: LoopEvent, id: SubmissionId) -> Self {
        Self {
            correlation_id: Some(id),
            event,
        }
    }

    /// Check if this event has a correlation ID.
    pub fn has_correlation(&self) -> bool {
        self.correlation_id.is_some()
    }

    /// Get the correlation ID if present.
    pub fn correlation_id(&self) -> Option<&SubmissionId> {
        self.correlation_id.as_ref()
    }

    /// Get a reference to the underlying event.
    pub fn event(&self) -> &LoopEvent {
        &self.event
    }

    /// Consume self and return the underlying event.
    pub fn into_event(self) -> LoopEvent {
        self.event
    }

    /// Consume self and return both the correlation ID and event.
    pub fn into_parts(self) -> (Option<SubmissionId>, LoopEvent) {
        (self.correlation_id, self.event)
    }
}

impl From<LoopEvent> for CorrelatedEvent {
    fn from(event: LoopEvent) -> Self {
        Self::uncorrelated(event)
    }
}

impl From<(LoopEvent, SubmissionId)> for CorrelatedEvent {
    fn from((event, id): (LoopEvent, SubmissionId)) -> Self {
        Self::correlated(event, id)
    }
}

impl From<(LoopEvent, Option<SubmissionId>)> for CorrelatedEvent {
    fn from((event, id): (LoopEvent, Option<SubmissionId>)) -> Self {
        Self::new(event, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submission_id_new() {
        let id1 = SubmissionId::new();
        let id2 = SubmissionId::new();
        // UUIDs should be unique
        assert_ne!(id1, id2);
        // Should be valid UUID format (36 chars with hyphens)
        assert_eq!(id1.as_str().len(), 36);
    }

    #[test]
    fn test_submission_id_from_string() {
        let id = SubmissionId::from_string("test-id");
        assert_eq!(id.as_str(), "test-id");
        assert_eq!(id.to_string(), "test-id");
    }

    #[test]
    fn test_submission_id_conversions() {
        let id: SubmissionId = "test".into();
        assert_eq!(id.as_str(), "test");

        let id: SubmissionId = String::from("test2").into();
        assert_eq!(id.as_str(), "test2");

        let inner = id.into_inner();
        assert_eq!(inner, "test2");
    }

    #[test]
    fn test_correlated_event_uncorrelated() {
        let event = LoopEvent::StreamRequestStart;
        let correlated = CorrelatedEvent::uncorrelated(event.clone());

        assert!(!correlated.has_correlation());
        assert!(correlated.correlation_id().is_none());
        assert!(matches!(correlated.event(), LoopEvent::StreamRequestStart));
    }

    #[test]
    fn test_correlated_event_with_id() {
        let event = LoopEvent::StreamRequestStart;
        let id = SubmissionId::from_string("sub-123");
        let correlated = CorrelatedEvent::correlated(event, id.clone());

        assert!(correlated.has_correlation());
        assert_eq!(correlated.correlation_id().unwrap().as_str(), "sub-123");
    }

    #[test]
    fn test_correlated_event_into_parts() {
        let event = LoopEvent::StreamRequestStart;
        let id = SubmissionId::from_string("sub-123");
        let correlated = CorrelatedEvent::correlated(event, id);

        let (correlation, event) = correlated.into_parts();
        assert!(correlation.is_some());
        assert!(matches!(event, LoopEvent::StreamRequestStart));
    }

    #[test]
    fn test_correlated_event_from_conversions() {
        // From LoopEvent
        let event = LoopEvent::StreamRequestStart;
        let correlated: CorrelatedEvent = event.into();
        assert!(!correlated.has_correlation());

        // From (LoopEvent, SubmissionId)
        let event = LoopEvent::StreamRequestStart;
        let id = SubmissionId::from_string("id");
        let correlated: CorrelatedEvent = (event, id).into();
        assert!(correlated.has_correlation());

        // From (LoopEvent, Option<SubmissionId>)
        let event = LoopEvent::StreamRequestStart;
        let correlated: CorrelatedEvent = (event, None).into();
        assert!(!correlated.has_correlation());
    }

    #[test]
    fn test_correlated_event_serde() {
        let event = LoopEvent::TurnStarted {
            turn_id: "turn-1".to_string(),
            turn_number: 1,
        };
        let id = SubmissionId::from_string("sub-123");
        let correlated = CorrelatedEvent::correlated(event, id);

        let json = serde_json::to_string(&correlated).unwrap();
        assert!(json.contains("sub-123"));
        assert!(json.contains("turn_started"));

        let parsed: CorrelatedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.correlation_id().unwrap().as_str(), "sub-123");
    }

    #[test]
    fn test_submission_id_serde() {
        let id = SubmissionId::from_string("test-id");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"test-id\"");

        let parsed: SubmissionId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_str(), "test-id");
    }
}
