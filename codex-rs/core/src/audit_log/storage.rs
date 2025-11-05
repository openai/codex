use super::types::AuditEvent;
use anyhow::Context;
use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;
use tokio::sync::Mutex;

/// Audit storage trait
#[async_trait::async_trait]
pub trait AuditStorage: Send + Sync {
    /// Write an audit event
    async fn write_event(&self, event: &AuditEvent) -> Result<()>;

    /// Flush pending writes
    async fn flush(&self) -> Result<()>;

    /// Rotate log files if needed
    async fn rotate_if_needed(&self) -> Result<()>;
}

/// File-based audit storage (JSON Lines format)
pub struct FileStorage {
    log_file_path: PathBuf,
    writer: Mutex<Option<BufWriter<File>>>,
    max_file_size: u64,
    max_backups: usize,
}

impl FileStorage {
    /// Create a new file storage
    pub async fn new(log_dir: impl AsRef<Path>) -> Result<Self> {
        let log_dir = log_dir.as_ref();
        tokio::fs::create_dir_all(log_dir)
            .await
            .with_context(|| format!("Failed to create audit log directory: {log_dir:?}"))?;

        let log_file_path = log_dir.join("audit.jsonl");

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .await
            .with_context(|| format!("Failed to open audit log file: {log_file_path:?}"))?;

        let writer = BufWriter::new(file);

        Ok(Self {
            log_file_path,
            writer: Mutex::new(Some(writer)),
            max_file_size: 100 * 1024 * 1024, // 100MB
            max_backups: 10,
        })
    }

    /// Get current file size
    async fn current_file_size(&self) -> Result<u64> {
        let metadata = tokio::fs::metadata(&self.log_file_path).await?;
        Ok(metadata.len())
    }

    /// Perform log rotation
    async fn rotate_logs(&self) -> Result<()> {
        // Close current writer
        {
            let mut writer_guard = self.writer.lock().await;
            if let Some(mut writer) = writer_guard.take() {
                writer.flush().await?;
            }
        }

        // Rotate existing backups
        for i in (1..self.max_backups).rev() {
            let old_path = self.log_file_path.with_extension(format!("jsonl.{i}"));
            let new_path = self
                .log_file_path
                .with_extension(format!("jsonl.{}", i + 1));

            if old_path.exists() {
                tokio::fs::rename(&old_path, &new_path).await?;
            }
        }

        // Move current log to .1
        let backup_path = self.log_file_path.with_extension("jsonl.1");
        tokio::fs::rename(&self.log_file_path, &backup_path).await?;

        // Create new log file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
            .await?;

        let new_writer = BufWriter::new(file);

        // Update writer
        {
            let mut writer_guard = self.writer.lock().await;
            *writer_guard = Some(new_writer);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl AuditStorage for FileStorage {
    async fn write_event(&self, event: &AuditEvent) -> Result<()> {
        let json =
            serde_json::to_string(event).with_context(|| "Failed to serialize audit event")?;

        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            writer.write_all(json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }

        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            writer.flush().await?;
        }
        Ok(())
    }

    async fn rotate_if_needed(&self) -> Result<()> {
        let current_size = self.current_file_size().await?;

        if current_size > self.max_file_size {
            self.rotate_logs().await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_log::types::AuditEventType;
    use crate::audit_log::types::TokenUsageEvent;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_storage_write() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).await.unwrap();

        let event = AuditEvent::new(
            "test-agent".to_string(),
            AuditEventType::TokenUsage(TokenUsageEvent {
                entity_id: "test".to_string(),
                entity_type: "agent".to_string(),
                tokens_consumed: 100,
                budget_limit: Some(1000),
                budget_remaining: Some(900),
                utilization: 0.1,
                timestamp: chrono::Utc::now().to_rfc3339(),
                operation: "test".to_string(),
            }),
        );

        storage.write_event(&event).await.unwrap();
        storage.flush().await.unwrap();

        let log_file = temp_dir.path().join("audit.jsonl");
        assert!(log_file.exists());

        let contents = tokio::fs::read_to_string(&log_file).await.unwrap();
        assert!(contents.contains(&event.id));
    }

    #[tokio::test]
    async fn test_log_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = FileStorage::new(temp_dir.path()).await.unwrap();
        storage.max_file_size = 100; // Small size for testing

        // Write enough events to trigger rotation
        for i in 0..20 {
            let event = AuditEvent::new(
                format!("agent-{i}"),
                AuditEventType::TokenUsage(TokenUsageEvent {
                    entity_id: format!("agent-{i}"),
                    entity_type: "agent".to_string(),
                    tokens_consumed: 100,
                    budget_limit: Some(1000),
                    budget_remaining: Some(900),
                    utilization: 0.1,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    operation: "test".to_string(),
                }),
            );

            storage.write_event(&event).await.unwrap();
            storage.flush().await.unwrap();
            storage.rotate_if_needed().await.unwrap();
        }

        // Check that backup files exist
        let backup_path = temp_dir.path().join("audit.jsonl.1");
        assert!(backup_path.exists());
    }
}
