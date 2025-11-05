//! Telemetry storage
//!
//! Persists telemetry events to JSONL files.

use super::events::TelemetryEvent;
use anyhow::Context;
use anyhow::Result;
use std::fs::OpenOptions;
use std::fs::{self};
use std::io::Write;
use std::path::PathBuf;
use tokio::sync::Mutex;

/// Telemetry storage
pub struct TelemetryStorage {
    /// Base directory for telemetry logs
    base_dir: PathBuf,

    /// File handle (mutex-protected)
    file_handle: Mutex<Option<std::fs::File>>,
}

impl TelemetryStorage {
    /// Create a new telemetry storage
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        // Ensure directory exists
        fs::create_dir_all(&base_dir).with_context(|| {
            format!(
                "Failed to create telemetry directory: {}",
                base_dir.display()
            )
        })?;

        Ok(Self {
            base_dir,
            file_handle: Mutex::new(None),
        })
    }

    /// Store a telemetry event
    pub async fn store(&self, event: &TelemetryEvent) -> Result<()> {
        let mut file_handle = self.file_handle.lock().await;

        // Get or create file for today
        let file = if let Some(file) = file_handle.as_mut() {
            file
        } else {
            let path = self.get_today_log_path();
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .with_context(|| format!("Failed to open telemetry log: {}", path.display()))?;
            *file_handle = Some(file);
            file_handle.as_mut().unwrap()
        };

        // Write event as JSONL
        let json = serde_json::to_string(event)?;
        writeln!(file, "{}", json)?;
        file.flush()?;

        Ok(())
    }

    /// Get today's log file path
    fn get_today_log_path(&self) -> PathBuf {
        let date = chrono::Utc::now().format("%Y-%m-%d");
        self.base_dir.join(format!("telemetry-{}.jsonl", date))
    }

    /// List all telemetry log files
    pub fn list_logs(&self) -> Result<Vec<PathBuf>> {
        let mut logs = Vec::new();

        if !self.base_dir.exists() {
            return Ok(logs);
        }

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                logs.push(path);
            }
        }

        logs.sort();
        Ok(logs)
    }

    /// Read events from a log file
    pub fn read_log(&self, path: &PathBuf) -> Result<Vec<TelemetryEvent>> {
        let content = fs::read_to_string(path)?;
        let mut events = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<TelemetryEvent>(line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    eprintln!("Failed to parse telemetry event: {}", e);
                }
            }
        }

        Ok(events)
    }

    /// Rotate old logs (delete logs older than N days)
    pub fn rotate_logs(&self, days: u64) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let mut deleted = 0;

        for log in self.list_logs()? {
            // Parse date from filename
            if let Some(filename) = log.file_name().and_then(|s| s.to_str()) {
                if let Some(date_str) = filename
                    .strip_prefix("telemetry-")
                    .and_then(|s| s.strip_suffix(".jsonl"))
                {
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        let datetime = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                        if datetime < cutoff {
                            fs::remove_file(&log)?;
                            deleted += 1;
                        }
                    }
                }
            }
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::events::EventType;

    #[tokio::test]
    async fn test_storage_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = TelemetryStorage::new(temp_dir.path().to_path_buf()).unwrap();

        assert!(temp_dir.path().exists());
    }

    #[tokio::test]
    async fn test_store_and_read() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = TelemetryStorage::new(temp_dir.path().to_path_buf()).unwrap();

        let event = TelemetryEvent::new(EventType::BlueprintStart)
            .with_session_id("test-session")
            .with_metadata("test", "value");

        storage.store(&event).await.unwrap();

        let logs = storage.list_logs().unwrap();
        assert_eq!(logs.len(), 1);

        let events = storage.read_log(&logs[0]).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::BlueprintStart);
    }

    #[tokio::test]
    async fn test_list_logs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = TelemetryStorage::new(temp_dir.path().to_path_buf()).unwrap();

        // Initially empty
        assert_eq!(storage.list_logs().unwrap().len(), 0);

        // Store an event
        let event = TelemetryEvent::new(EventType::BlueprintStart);
        storage.store(&event).await.unwrap();

        // Now one log file
        assert_eq!(storage.list_logs().unwrap().len(), 1);
    }
}
