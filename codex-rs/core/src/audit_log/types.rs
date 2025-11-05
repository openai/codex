use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Top-level audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Event ID (UUID)
    pub id: String,
    /// Timestamp (ISO 8601)
    pub timestamp: String,
    /// Session ID
    pub session_id: Option<String>,
    /// User/Agent ID
    pub actor_id: String,
    /// Event type
    pub event_type: AuditEventType,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Event type enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AuditEventType {
    /// Agent execution event
    AgentExecution(AgentExecutionEvent),
    /// API call event
    ApiCall(ApiCallEvent),
    /// Tool call event
    ToolCall(ToolCallEvent),
    /// Token usage event
    TokenUsage(TokenUsageEvent),
    /// Security event
    Security(SecurityEvent),
}

/// Agent execution lifecycle event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionEvent {
    /// Agent name
    pub agent_name: String,
    /// Execution status
    pub status: ExecutionStatus,
    /// Task/Goal description
    pub goal: String,
    /// Start time (ISO 8601)
    pub start_time: String,
    /// End time (ISO 8601, if completed)
    pub end_time: Option<String>,
    /// Duration in seconds
    pub duration_secs: Option<f64>,
    /// Tokens used
    pub tokens_used: usize,
    /// Artifacts generated
    pub artifacts: Vec<String>,
    /// Error message (if failed)
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    Started,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// LLM API call event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCallEvent {
    /// API provider (e.g., "openai", "anthropic")
    pub provider: String,
    /// Model used
    pub model: String,
    /// Request timestamp
    pub request_time: String,
    /// Response timestamp
    pub response_time: Option<String>,
    /// Latency in milliseconds
    pub latency_ms: Option<u64>,
    /// Tokens in prompt
    pub prompt_tokens: usize,
    /// Tokens in completion
    pub completion_tokens: usize,
    /// Total tokens
    pub total_tokens: usize,
    /// HTTP status code
    pub status_code: Option<u16>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Truncated prompt (first 200 chars)
    pub prompt_preview: String,
    /// Truncated response (first 200 chars)
    pub response_preview: Option<String>,
}

/// Tool call event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEvent {
    /// Tool name
    pub tool_name: String,
    /// Tool call ID
    pub call_id: String,
    /// Parameters (JSON)
    pub parameters: String,
    /// Execution time
    pub execution_time: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Success status
    pub success: bool,
    /// Output (truncated)
    pub output_preview: String,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Permission check result
    pub permission_granted: bool,
    /// Sandbox policy applied
    pub sandbox_policy: Option<String>,
}

/// Token usage tracking event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageEvent {
    /// Agent or user ID
    pub entity_id: String,
    /// Entity type ("agent", "user", "session")
    pub entity_type: String,
    /// Tokens consumed
    pub tokens_consumed: usize,
    /// Budget limit
    pub budget_limit: Option<usize>,
    /// Remaining budget
    pub budget_remaining: Option<usize>,
    /// Budget utilization (0.0 - 1.0)
    pub utilization: f64,
    /// Timestamp
    pub timestamp: String,
    /// Operation description
    pub operation: String,
}

/// Security-related event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    /// Event severity
    pub severity: SecuritySeverity,
    /// Event category
    pub category: SecurityCategory,
    /// Detailed message
    pub message: String,
    /// Source (agent name, tool name, etc.)
    pub source: String,
    /// Action taken
    pub action: SecurityAction,
    /// Additional context
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecuritySeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityCategory {
    PermissionDenied,
    SandboxViolation,
    BudgetExceeded,
    UnauthorizedAccess,
    SuspiciousActivity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityAction {
    Allowed,
    Blocked,
    Logged,
    Escalated,
}

impl AuditEvent {
    /// Create a new audit event with generated ID and timestamp
    pub fn new(actor_id: String, event_type: AuditEventType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: None,
            actor_id,
            event_type,
            metadata: HashMap::new(),
        }
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(
            "test-agent".to_string(),
            AuditEventType::AgentExecution(AgentExecutionEvent {
                agent_name: "test-agent".to_string(),
                status: ExecutionStatus::Started,
                goal: "Test goal".to_string(),
                start_time: chrono::Utc::now().to_rfc3339(),
                end_time: None,
                duration_secs: None,
                tokens_used: 0,
                artifacts: vec![],
                error: None,
            }),
        );

        assert!(!event.id.is_empty());
        assert!(!event.timestamp.is_empty());
        assert_eq!(event.actor_id, "test-agent");
    }

    #[test]
    fn test_event_serialization() {
        let event = AuditEvent::new(
            "agent-1".to_string(),
            AuditEventType::TokenUsage(TokenUsageEvent {
                entity_id: "agent-1".to_string(),
                entity_type: "agent".to_string(),
                tokens_consumed: 1000,
                budget_limit: Some(5000),
                budget_remaining: Some(4000),
                utilization: 0.2,
                timestamp: chrono::Utc::now().to_rfc3339(),
                operation: "LLM API call".to_string(),
            }),
        );

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.id, deserialized.id);
        assert_eq!(event.actor_id, deserialized.actor_id);
    }
}
