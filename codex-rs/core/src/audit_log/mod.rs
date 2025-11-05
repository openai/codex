/// Audit logging system for tracking agent execution, API calls, and token usage
///
/// Provides transparent tracking of:
/// - Agent execution history (start/end times, results, errors)
/// - LLM API calls (models, prompts, responses, tokens)
/// - Tool invocations (which tools were used, parameters, results)
/// - Security events (permission checks, sandbox violations)
///
/// Logs are written in JSON Lines format for easy parsing and analysis.
mod logger;
mod storage;
mod types;

pub use logger::AuditLogger;
pub use storage::AuditStorage;
pub use storage::FileStorage;
pub use types::AgentExecutionEvent;
pub use types::ApiCallEvent;
pub use types::AuditEvent;
pub use types::AuditEventType;
pub use types::ExecutionStatus;
pub use types::SecurityEvent;
pub use types::TokenUsageEvent;
pub use types::ToolCallEvent;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global audit logger instance
static AUDIT_LOGGER: once_cell::sync::Lazy<Arc<RwLock<Option<AuditLogger>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(None)));

/// Initialize the global audit logger
pub async fn init_audit_logger(log_dir: PathBuf) -> Result<()> {
    let logger = AuditLogger::new(log_dir).await?;
    let mut global = AUDIT_LOGGER.write().await;
    *global = Some(logger);
    Ok(())
}

/// Get the global audit logger
pub async fn get_audit_logger() -> Option<Arc<RwLock<AuditLogger>>> {
    let global = AUDIT_LOGGER.read().await;
    global
        .as_ref()
        .map(|logger| Arc::new(RwLock::new(logger.clone())))
}

/// Log an audit event to the global logger
pub async fn log_audit_event(event: AuditEvent) -> Result<()> {
    let global = AUDIT_LOGGER.read().await;
    if let Some(logger) = global.as_ref() {
        logger.log_event(event).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_audit_logger_init() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_path_buf();

        let result = init_audit_logger(log_dir).await;
        assert!(result.is_ok());

        let logger = get_audit_logger().await;
        assert!(logger.is_some());
    }
}
