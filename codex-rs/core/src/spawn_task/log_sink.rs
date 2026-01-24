use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

/// Thread-safe log file writer for SpawnTask events.
///
/// Since SpawnAgent runs in the same process (via tokio::spawn),
/// we cannot redirect stdout/stderr. Instead, we explicitly log
/// events to a dedicated file per task.
pub struct LogFileSink {
    file: Arc<Mutex<File>>,
}

impl LogFileSink {
    /// Create a new log file sink.
    ///
    /// Creates parent directories if they don't exist.
    /// Opens file in append mode for crash recovery.
    pub fn new(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }

    /// Log a message with timestamp.
    pub fn log(&self, msg: &str) {
        if let Ok(mut f) = self.file.lock() {
            let timestamp = chrono::Utc::now().format("%H:%M:%S");
            let _ = writeln!(f, "[{timestamp}] {msg}");
        }
    }

    /// Log iteration progress.
    pub fn log_iteration(&self, iteration: i32, succeeded: i32, failed: i32) {
        self.log(&format!(
            "=== Iteration {iteration} complete: {succeeded} succeeded, {failed} failed ==="
        ));
    }

    /// Log task start.
    pub fn log_start(&self, task_id: &str, condition: &str) {
        self.log(&format!("Starting SpawnTask: {task_id}"));
        self.log(&format!("Condition: {condition}"));
    }

    /// Log task completion.
    pub fn log_complete(&self, status: &str) {
        self.log(&format!("Task completed with status: {status}"));
    }

    /// Log an error.
    pub fn log_error(&self, error: &str) {
        self.log(&format!("ERROR: {error}"));
    }
}

impl Clone for LogFileSink {
    fn clone(&self) -> Self {
        Self {
            file: Arc::clone(&self.file),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::TempDir;

    #[test]
    fn test_log_sink() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let sink = LogFileSink::new(&log_path).unwrap();
        sink.log("Test message 1");
        sink.log("Test message 2");
        sink.log_iteration(1, 1, 0);

        // Read the log file
        let mut content = String::new();
        File::open(&log_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        assert!(content.contains("Test message 1"));
        assert!(content.contains("Test message 2"));
        assert!(content.contains("Iteration 1 complete"));
    }

    #[test]
    fn test_log_sink_clone() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("clone_test.log");

        let sink1 = LogFileSink::new(&log_path).unwrap();
        let sink2 = sink1.clone();

        sink1.log("From sink1");
        sink2.log("From sink2");

        let mut content = String::new();
        File::open(&log_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        assert!(content.contains("From sink1"));
        assert!(content.contains("From sink2"));
    }
}
