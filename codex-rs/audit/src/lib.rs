//! Audit logging system for security-critical operations.
//!
//! This module provides privacy-aware audit logging for tracking
//! security decisions, file access, and command execution.

use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::debug;
use tracing::error;
use tracing::info;

/// Audit entry representing a security-relevant operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Timestamp of the operation (UTC).
    pub timestamp: DateTime<Utc>,

    /// Type of operation performed.
    pub operation: Operation,

    /// Target of the operation (sanitized for privacy).
    pub target: String,

    /// Security decision made.
    pub decision: Decision,

    /// Optional reason for the decision.
    pub reason: Option<String>,

    /// Session identifier for correlation.
    pub session_id: Option<String>,
}

/// Types of auditable operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// File read attempt.
    FileRead,

    /// File write attempt.
    FileWrite,

    /// Command execution attempt.
    CommandExec,

    /// Network access attempt.
    NetworkAccess,

    /// Process spawn attempt.
    ProcessSpawn,

    /// Security policy change.
    PolicyChange,

    /// Authentication event.
    Authentication,
}

/// Security decision for an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// Operation was allowed.
    Allowed,

    /// Operation was denied.
    Denied,

    /// Operation requires user approval.
    RequiresApproval,
}

impl AuditEntry {
    /// Creates a new audit entry.
    pub fn new(operation: Operation, target: String, decision: Decision) -> Self {
        Self {
            timestamp: Utc::now(),
            operation,
            target: sanitize_path(&target),
            decision,
            reason: None,
            session_id: None,
        }
    }

    /// Adds a reason to the audit entry.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Adds a session ID to the audit entry.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Converts the entry to a JSON string.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

/// Audit logger for security events.
pub struct AuditLogger {
    log_path: PathBuf,
    enabled: bool,
}

impl AuditLogger {
    /// Creates a new audit logger.
    pub fn new(log_path: impl Into<PathBuf>) -> Self {
        Self {
            log_path: log_path.into(),
            enabled: true,
        }
    }

    /// Creates a disabled audit logger (no-op).
    pub fn disabled() -> Self {
        Self {
            log_path: PathBuf::new(),
            enabled: false,
        }
    }

    /// Logs an audit entry.
    pub async fn log(&self, entry: AuditEntry) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        debug!(
            operation = ?entry.operation,
            decision = ?entry.decision,
            "Audit entry created"
        );

        let json = entry.to_json()?;

        // Ensure parent directory exists
        if let Some(parent) = self.log_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Append to audit log file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .await?;

        file.write_all(json.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        info!(
            path = ?self.log_path,
            "Audit entry written"
        );

        Ok(())
    }

    /// Logs a file read operation.
    pub async fn log_file_read(&self, path: impl AsRef<Path>, decision: Decision) -> Result<()> {
        let entry = AuditEntry::new(
            Operation::FileRead,
            path.as_ref().display().to_string(),
            decision,
        );
        self.log(entry).await
    }

    /// Logs a file write operation.
    pub async fn log_file_write(&self, path: impl AsRef<Path>, decision: Decision) -> Result<()> {
        let entry = AuditEntry::new(
            Operation::FileWrite,
            path.as_ref().display().to_string(),
            decision,
        );
        self.log(entry).await
    }

    /// Logs a command execution.
    pub async fn log_command_exec(
        &self,
        command: impl Into<String>,
        decision: Decision,
    ) -> Result<()> {
        let entry = AuditEntry::new(Operation::CommandExec, command.into(), decision);
        self.log(entry).await
    }

    /// Logs a network access attempt.
    pub async fn log_network_access(
        &self,
        url: impl Into<String>,
        decision: Decision,
    ) -> Result<()> {
        let entry = AuditEntry::new(Operation::NetworkAccess, url.into(), decision);
        self.log(entry).await
    }

    /// Reads all audit entries from the log file.
    pub async fn read_entries(&self) -> Result<Vec<AuditEntry>> {
        if !self.enabled || !self.log_path.exists() {
            return Ok(vec![]);
        }

        let content = tokio::fs::read_to_string(&self.log_path).await?;
        let mut entries = Vec::new();

        for line in content.lines() {
            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<AuditEntry>(line) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    error!(
                        error = %e,
                        line = %line,
                        "Failed to parse audit entry"
                    );
                }
            }
        }

        Ok(entries)
    }

    /// Clears the audit log.
    pub async fn clear(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.log_path.exists() {
            tokio::fs::remove_file(&self.log_path).await?;
        }

        Ok(())
    }
}

/// Sanitizes a path string to remove sensitive information.
///
/// Replaces usernames and home directories with placeholders.
pub fn sanitize_path(path: &str) -> String {
    let mut sanitized = path.to_string();

    // Replace Windows username
    if let Ok(username) = std::env::var("USERNAME") {
        sanitized = sanitized.replace(&username, "[USER]");
    }

    // Replace Unix username
    if let Ok(user) = std::env::var("USER") {
        sanitized = sanitized.replace(&user, "[USER]");
    }

    // Replace home directory
    if let Ok(home) = std::env::var("HOME") {
        sanitized = sanitized.replace(&home, "[HOME]");
    }

    // Replace USERPROFILE (Windows)
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        sanitized = sanitized.replace(&userprofile, "[HOME]");
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry::new(
            Operation::FileRead,
            "/path/to/file.txt".to_string(),
            Decision::Allowed,
        );

        assert!(matches!(entry.operation, Operation::FileRead));
        assert!(matches!(entry.decision, Decision::Allowed));
    }

    #[test]
    fn test_audit_entry_with_reason() {
        let entry = AuditEntry::new(Operation::CommandExec, "curl".to_string(), Decision::Denied)
            .with_reason("Network access forbidden");

        assert_eq!(entry.reason, Some("Network access forbidden".to_string()));
    }

    #[test]
    fn test_sanitize_path() {
        unsafe {
            std::env::set_var("USERNAME", "testuser");
        }
        let sanitized = sanitize_path("C:\\Users\\testuser\\file.txt");
        assert!(sanitized.contains("[USER]"));
        assert!(!sanitized.contains("testuser"));
    }

    #[tokio::test]
    async fn test_audit_logger_write() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.log");

        let logger = AuditLogger::new(&log_path);

        let entry = AuditEntry::new(
            Operation::FileRead,
            "test.txt".to_string(),
            Decision::Allowed,
        );

        logger.log(entry).await.unwrap();

        assert!(log_path.exists());
    }

    #[tokio::test]
    async fn test_audit_logger_read() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.log");

        let logger = AuditLogger::new(&log_path);

        // Write multiple entries
        logger
            .log_file_read("file1.txt", Decision::Allowed)
            .await
            .unwrap();
        logger
            .log_file_write("file2.txt", Decision::Denied)
            .await
            .unwrap();

        // Read back
        let entries = logger.read_entries().await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_disabled_logger() {
        let logger = AuditLogger::disabled();

        let entry = AuditEntry::new(
            Operation::FileRead,
            "test.txt".to_string(),
            Decision::Allowed,
        );

        // Should not error
        logger.log(entry).await.unwrap();
    }
}
