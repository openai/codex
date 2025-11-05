use super::storage::AuditStorage;
use super::storage::FileStorage;
use super::types::*;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::Duration;
use tokio::time::interval;
use tracing::debug;
use tracing::error;
use tracing::info;

/// Audit logger for tracking system events
#[derive(Clone)]
pub struct AuditLogger {
    storage: Arc<dyn AuditStorage>,
    session_id: Option<String>,
    auto_flush_interval: Duration,
}

impl AuditLogger {
    /// Create a new audit logger with file storage
    pub async fn new(log_dir: PathBuf) -> Result<Self> {
        let storage = Arc::new(FileStorage::new(log_dir).await?) as Arc<dyn AuditStorage>;

        let logger = Self {
            storage,
            session_id: None,
            auto_flush_interval: Duration::from_secs(5),
        };

        // Start background flush task
        logger.start_auto_flush().await;

        info!("Audit logger initialized");

        Ok(logger)
    }

    /// Set session ID for all events
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Log an audit event
    pub async fn log_event(&self, mut event: AuditEvent) -> Result<()> {
        // Set session ID if available
        if let Some(session_id) = &self.session_id {
            event.session_id = Some(session_id.clone());
        }

        debug!("Logging audit event: {}", event.id);

        self.storage.write_event(&event).await?;

        // Check if rotation is needed
        self.storage.rotate_if_needed().await?;

        Ok(())
    }

    /// Log agent execution start
    pub async fn log_agent_start(&self, agent_name: &str, goal: &str) -> Result<()> {
        let event = AuditEvent::new(
            agent_name.to_string(),
            AuditEventType::AgentExecution(AgentExecutionEvent {
                agent_name: agent_name.to_string(),
                status: ExecutionStatus::Started,
                goal: goal.to_string(),
                start_time: chrono::Utc::now().to_rfc3339(),
                end_time: None,
                duration_secs: None,
                tokens_used: 0,
                artifacts: vec![],
                error: None,
            }),
        );

        self.log_event(event).await
    }

    /// Log agent execution completion
    pub async fn log_agent_complete(
        &self,
        agent_name: &str,
        goal: &str,
        start_time: &str,
        tokens_used: usize,
        artifacts: Vec<String>,
    ) -> Result<()> {
        let now = chrono::Utc::now();
        let start = chrono::DateTime::parse_from_rfc3339(start_time)?.with_timezone(&chrono::Utc);
        let duration_secs = (now - start).num_milliseconds() as f64 / 1000.0;

        let event = AuditEvent::new(
            agent_name.to_string(),
            AuditEventType::AgentExecution(AgentExecutionEvent {
                agent_name: agent_name.to_string(),
                status: ExecutionStatus::Completed,
                goal: goal.to_string(),
                start_time: start_time.to_string(),
                end_time: Some(now.to_rfc3339()),
                duration_secs: Some(duration_secs),
                tokens_used,
                artifacts,
                error: None,
            }),
        );

        self.log_event(event).await
    }

    /// Log agent execution failure
    pub async fn log_agent_failure(
        &self,
        agent_name: &str,
        goal: &str,
        start_time: &str,
        error_msg: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now();
        let start = chrono::DateTime::parse_from_rfc3339(start_time)?.with_timezone(&chrono::Utc);
        let duration_secs = (now - start).num_milliseconds() as f64 / 1000.0;

        let event = AuditEvent::new(
            agent_name.to_string(),
            AuditEventType::AgentExecution(AgentExecutionEvent {
                agent_name: agent_name.to_string(),
                status: ExecutionStatus::Failed,
                goal: goal.to_string(),
                start_time: start_time.to_string(),
                end_time: Some(now.to_rfc3339()),
                duration_secs: Some(duration_secs),
                tokens_used: 0,
                artifacts: vec![],
                error: Some(error_msg.to_string()),
            }),
        );

        self.log_event(event).await
    }

    /// Log API call
    #[allow(clippy::too_many_arguments)]
    pub async fn log_api_call(
        &self,
        provider: &str,
        model: &str,
        prompt_tokens: usize,
        completion_tokens: usize,
        latency_ms: u64,
        prompt_preview: &str,
        response_preview: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now();
        let request_time = now.to_rfc3339();
        let response_time =
            Some((now + chrono::Duration::milliseconds(latency_ms as i64)).to_rfc3339());

        let event = AuditEvent::new(
            format!("{provider}/{model}"),
            AuditEventType::ApiCall(ApiCallEvent {
                provider: provider.to_string(),
                model: model.to_string(),
                request_time,
                response_time,
                latency_ms: Some(latency_ms),
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
                status_code: Some(200),
                error: None,
                prompt_preview: prompt_preview.chars().take(200).collect(),
                response_preview: response_preview.map(|s| s.chars().take(200).collect()),
            }),
        );

        self.log_event(event).await
    }

    /// Log tool call
    #[allow(clippy::too_many_arguments)]
    pub async fn log_tool_call(
        &self,
        tool_name: &str,
        call_id: &str,
        parameters: &str,
        duration_ms: u64,
        success: bool,
        output_preview: &str,
        permission_granted: bool,
        sandbox_policy: Option<&str>,
    ) -> Result<()> {
        let event = AuditEvent::new(
            tool_name.to_string(),
            AuditEventType::ToolCall(ToolCallEvent {
                tool_name: tool_name.to_string(),
                call_id: call_id.to_string(),
                parameters: parameters.to_string(),
                execution_time: chrono::Utc::now().to_rfc3339(),
                duration_ms,
                success,
                output_preview: output_preview.chars().take(200).collect(),
                error: None,
                permission_granted,
                sandbox_policy: sandbox_policy.map(std::string::ToString::to_string),
            }),
        );

        self.log_event(event).await
    }

    /// Log token usage
    pub async fn log_token_usage(
        &self,
        entity_id: &str,
        entity_type: &str,
        tokens_consumed: usize,
        budget_limit: Option<usize>,
        operation: &str,
    ) -> Result<()> {
        let budget_remaining = budget_limit.map(|limit| limit.saturating_sub(tokens_consumed));
        let utilization = budget_limit
            .map(|limit| tokens_consumed as f64 / limit as f64)
            .unwrap_or(0.0);

        let event = AuditEvent::new(
            entity_id.to_string(),
            AuditEventType::TokenUsage(TokenUsageEvent {
                entity_id: entity_id.to_string(),
                entity_type: entity_type.to_string(),
                tokens_consumed,
                budget_limit,
                budget_remaining,
                utilization,
                timestamp: chrono::Utc::now().to_rfc3339(),
                operation: operation.to_string(),
            }),
        );

        self.log_event(event).await
    }

    /// Log security event
    pub async fn log_security_event(
        &self,
        severity: SecuritySeverity,
        category: SecurityCategory,
        message: &str,
        source: &str,
        action: SecurityAction,
    ) -> Result<()> {
        let event = AuditEvent::new(
            source.to_string(),
            AuditEventType::Security(SecurityEvent {
                severity,
                category,
                message: message.to_string(),
                source: source.to_string(),
                action,
                context: std::collections::HashMap::new(),
            }),
        );

        self.log_event(event).await
    }

    /// Flush pending writes
    pub async fn flush(&self) -> Result<()> {
        self.storage.flush().await
    }

    /// Start background auto-flush task
    async fn start_auto_flush(&self) {
        let storage = self.storage.clone();
        let flush_interval = self.auto_flush_interval;

        tokio::spawn(async move {
            let mut interval = interval(flush_interval);
            loop {
                interval.tick().await;
                if let Err(e) = storage.flush().await {
                    error!("Auto-flush failed: {}", e);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        assert!(logger.session_id.is_none());
    }

    #[tokio::test]
    async fn test_log_agent_start() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        let result = logger.log_agent_start("test-agent", "Test goal").await;
        assert!(result.is_ok());

        logger.flush().await.unwrap();

        let log_file = temp_dir.path().join("audit.jsonl");
        let contents = tokio::fs::read_to_string(&log_file).await.unwrap();
        assert!(contents.contains("test-agent"));
        assert!(contents.contains("Started"));
    }

    #[tokio::test]
    async fn test_log_token_usage() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        let result = logger
            .log_token_usage("agent-1", "agent", 1000, Some(5000), "LLM call")
            .await;
        assert!(result.is_ok());

        logger.flush().await.unwrap();

        let log_file = temp_dir.path().join("audit.jsonl");
        let contents = tokio::fs::read_to_string(&log_file).await.unwrap();
        assert!(contents.contains("agent-1"));
        assert!(contents.contains("\"tokens_consumed\":1000"));
    }

    #[tokio::test]
    async fn test_log_security_event() {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        let result = logger
            .log_security_event(
                SecuritySeverity::Warning,
                SecurityCategory::PermissionDenied,
                "File access denied",
                "file-tool",
                SecurityAction::Blocked,
            )
            .await;
        assert!(result.is_ok());

        logger.flush().await.unwrap();

        let log_file = temp_dir.path().join("audit.jsonl");
        let contents = tokio::fs::read_to_string(&log_file).await.unwrap();
        assert!(contents.contains("PermissionDenied"));
        assert!(contents.contains("Blocked"));
    }
}
