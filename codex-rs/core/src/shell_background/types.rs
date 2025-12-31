//! Type definitions for background shell execution.

use codex_protocol::ConversationId;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

// ============================================================================
// OutputBuffer - Thread-safe tweakcc output buffer
// ============================================================================

/// Default max size per buffer (100KB for tweakcc data between reads).
const DEFAULT_MAX_BUFFER_SIZE: usize = 100_000;

/// Thread-safe output buffer for tweakcc output.
///
/// This is an **tweakcc-only** buffer:
/// - `append()` adds data, truncating old data if exceeds max_size
/// - `take_all()` returns all data and clears the buffer
/// - Memory is released after each read
///
/// Allows concurrent appending (during execution) and reading (via BashOutput).
#[derive(Debug)]
pub struct OutputBuffer {
    /// Buffer for tweakcc output (cleared on each take_all).
    buffer: RwLock<String>,
    /// Maximum buffer size. If exceeded, old data is truncated.
    max_size: usize,
    /// Total bytes truncated since last take_all (for reporting).
    truncated_total: AtomicUsize,
}

impl Default for OutputBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputBuffer {
    /// Create a new empty output buffer with default max size.
    pub fn new() -> Self {
        Self::with_max_size(DEFAULT_MAX_BUFFER_SIZE)
    }

    /// Create a new output buffer with specified max size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            buffer: RwLock::new(String::new()),
            max_size,
            truncated_total: AtomicUsize::new(0),
        }
    }

    /// Append data to the buffer.
    ///
    /// If the buffer exceeds max_size, old data is truncated from the beginning
    /// (keeping the newest data). UTF-8 boundaries are respected.
    pub fn append(&self, data: &str) {
        if let Ok(mut buf) = self.buffer.write() {
            buf.push_str(data);

            // If exceeds max_size, truncate from beginning (keep newest)
            if buf.len() > self.max_size {
                let truncate_amount = buf.len() - self.max_size;
                let safe_start = Self::ceil_char_boundary(&buf, truncate_amount);
                buf.drain(..safe_start);

                self.truncated_total
                    .fetch_add(safe_start, Ordering::Relaxed);
                tracing::warn!(
                    "Output buffer exceeded max size ({}), truncated {} bytes",
                    self.max_size,
                    safe_start
                );
            }
        }
    }

    /// Get the current length of the buffer in bytes.
    pub fn len(&self) -> usize {
        self.buffer.read().map(|b| b.len()).unwrap_or(0)
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Take all content and clear the buffer.
    ///
    /// Returns (content, truncated_bytes_since_last_take).
    /// This is the primary read method - clears buffer after reading.
    pub fn take_all(&self) -> (String, usize) {
        let content = if let Ok(mut buf) = self.buffer.write() {
            std::mem::take(&mut *buf)
        } else {
            String::new()
        };
        let truncated = self.truncated_total.swap(0, Ordering::Relaxed);
        (content, truncated)
    }

    /// Find the smallest byte offset >= pos that is on a char boundary.
    fn ceil_char_boundary(s: &str, pos: usize) -> usize {
        if pos >= s.len() {
            return s.len();
        }
        // Walk forwards to find a valid char boundary
        let mut i = pos;
        while i < s.len() && !s.is_char_boundary(i) {
            i += 1;
        }
        i
    }
}

/// Shared output buffer wrapped in Arc.
pub type SharedOutputBuffer = Arc<OutputBuffer>;

/// Status of a background shell command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellStatus {
    /// Shell registered but not yet started (phase 1 of two-phase registration).
    Pending,
    /// Shell is actively running.
    Running,
    /// Shell completed successfully.
    Completed,
    /// Shell failed or was killed.
    Failed,
    /// Shell was explicitly killed.
    Killed,
    /// Shell timed out waiting for completion.
    Timeout,
}

impl ShellStatus {
    /// Check if the shell has finished (completed, failed, killed, or timed out).
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            ShellStatus::Completed
                | ShellStatus::Failed
                | ShellStatus::Killed
                | ShellStatus::Timeout
        )
    }
}

/// Result of a background shell execution.
#[derive(Debug, Clone)]
pub struct ShellResult {
    /// Combined stdout and stderr output.
    pub output: String,
    /// Exit code if process completed.
    pub exit_code: Option<i32>,
    /// Whether the execution was successful.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

impl Default for ShellResult {
    fn default() -> Self {
        Self {
            output: String::new(),
            exit_code: None,
            success: false,
            error: None,
        }
    }
}

/// A background shell command being tracked.
#[derive(Debug)]
pub struct BackgroundShell {
    /// Unique shell ID.
    pub shell_id: String,
    /// Conversation ID that owns this shell (for session-scoped cleanup).
    pub conversation_id: Option<ConversationId>,
    /// The command being executed.
    pub command: String,
    /// Human-readable description for system reminders.
    pub description: String,
    /// Current status.
    pub status: ShellStatus,
    /// Exit code when completed.
    pub exit_code: Option<i32>,
    /// Whether completion has been notified via system reminder.
    pub notified: bool,
    /// When the shell was started.
    pub created_at: Instant,
    /// Handle to the running task.
    pub handle: Option<JoinHandle<ShellResult>>,
    /// Cancellation token for graceful termination.
    pub cancellation_token: CancellationToken,
    /// Cached result when completed.
    pub result: Option<ShellResult>,
    /// Streaming stdout buffer (tweakcc, cleared on each read).
    pub stdout_buffer: SharedOutputBuffer,
    /// Streaming stderr buffer (tweakcc, cleared on each read).
    pub stderr_buffer: SharedOutputBuffer,
}

impl BackgroundShell {
    /// Create a new background shell in Pending status.
    pub fn new_pending(
        shell_id: String,
        conversation_id: Option<ConversationId>,
        command: String,
        description: String,
    ) -> Self {
        Self {
            shell_id,
            conversation_id,
            command,
            description,
            status: ShellStatus::Pending,
            exit_code: None,
            notified: false,
            created_at: Instant::now(),
            handle: None,
            cancellation_token: CancellationToken::new(),
            result: None,
            stdout_buffer: Arc::new(OutputBuffer::new()),
            stderr_buffer: Arc::new(OutputBuffer::new()),
        }
    }

    /// Transition to Running status with the given handle.
    pub fn set_running(&mut self, handle: JoinHandle<ShellResult>) {
        self.status = ShellStatus::Running;
        self.handle = Some(handle);
    }

    /// Mark as completed with the given result.
    pub fn set_completed(&mut self, result: ShellResult) {
        self.status = if result.success {
            ShellStatus::Completed
        } else {
            ShellStatus::Failed
        };
        self.exit_code = result.exit_code;
        self.result = Some(result);
    }

    /// Mark as killed.
    pub fn set_killed(&mut self) {
        self.status = ShellStatus::Killed;
    }

    /// Mark as timed out.
    pub fn set_timeout(&mut self) {
        self.status = ShellStatus::Timeout;
    }

    /// Check if there is unread output in the buffers.
    ///
    /// This is derived from buffer state, not a stored flag.
    pub fn has_unread_output(&self) -> bool {
        !self.stdout_buffer.is_empty() || !self.stderr_buffer.is_empty()
    }
}

/// Helper function for serde skip_serializing_if.
fn is_zero(n: &i32) -> bool {
    *n == 0
}

/// Output information returned by BashOutput tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellOutput {
    /// Shell ID.
    pub shell_id: String,
    /// The command being executed.
    pub command: String,
    /// Current status: "running", "completed", "failed", "killed".
    pub status: String,
    /// Exit code if completed.
    pub exit_code: Option<i32>,
    /// Stdout output (may be partial for tweakcc reads).
    pub stdout: String,
    /// Stderr output (may be partial for tweakcc reads).
    pub stderr: String,
    /// Number of lines in stdout.
    pub stdout_lines: i32,
    /// Number of lines in stderr.
    pub stderr_lines: i32,
    /// ISO timestamp of when output was captured.
    pub timestamp: String,
    /// Applied filter pattern (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_pattern: Option<String>,
    /// Whether there is more output to read.
    #[serde(default)]
    pub has_more: bool,
    /// Bytes truncated from stdout due to buffer overflow.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub stdout_truncated: i32,
    /// Bytes truncated from stderr due to buffer overflow.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub stderr_truncated: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_buffer_basic() {
        let buffer = OutputBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);

        buffer.append("hello");
        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), 5);

        // take_all() returns content and clears buffer
        let (content, truncated) = buffer.take_all();
        assert_eq!(content, "hello");
        assert_eq!(truncated, 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_output_buffer_append() {
        let buffer = OutputBuffer::new();
        buffer.append("hello");
        buffer.append(" world");
        assert_eq!(buffer.len(), 11);

        let (content, _) = buffer.take_all();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_output_buffer_take_all_clears() {
        let buffer = OutputBuffer::new();
        buffer.append("first");

        let (content1, _) = buffer.take_all();
        assert_eq!(content1, "first");
        assert!(buffer.is_empty());

        // Buffer is now empty
        let (content2, _) = buffer.take_all();
        assert!(content2.is_empty());

        // Append new data after take_all
        buffer.append("second");
        let (content3, _) = buffer.take_all();
        assert_eq!(content3, "second");
    }

    #[test]
    fn test_output_buffer_truncation() {
        // Create buffer with small max size
        let buffer = OutputBuffer::with_max_size(10);

        // Append data exceeding max size
        buffer.append("12345");
        buffer.append("67890"); // Now 10 bytes, at limit
        assert_eq!(buffer.len(), 10);

        buffer.append("ABCDE"); // Now 15 bytes, will truncate 5

        // Should have truncated old data, kept newest
        assert!(buffer.len() <= 10);

        let (content, truncated) = buffer.take_all();
        // Should contain newest data
        assert!(content.contains("ABCDE"));
        // Should have truncated some bytes
        assert!(truncated > 0);
    }

    #[test]
    fn test_output_buffer_utf8_boundary_truncation() {
        // Create buffer that forces truncation in middle of UTF-8
        let buffer = OutputBuffer::with_max_size(10);

        // "你好" is 6 bytes (3 bytes per Chinese character)
        buffer.append("你好"); // 6 bytes
        buffer.append("world"); // 5 bytes, total 11 bytes

        // Should truncate from beginning, respecting UTF-8 boundaries
        let (content, _) = buffer.take_all();
        // Content should be valid UTF-8
        assert!(!content.is_empty());
        // Should contain the newest data
        assert!(content.contains("world") || content.contains("好"));
    }

    #[test]
    fn test_shell_status_is_finished() {
        assert!(!ShellStatus::Pending.is_finished());
        assert!(!ShellStatus::Running.is_finished());
        assert!(ShellStatus::Completed.is_finished());
        assert!(ShellStatus::Failed.is_finished());
        assert!(ShellStatus::Killed.is_finished());
        assert!(ShellStatus::Timeout.is_finished());
    }

    #[test]
    fn test_shell_result_default() {
        let result = ShellResult::default();
        assert!(result.output.is_empty());
        assert!(result.exit_code.is_none());
        assert!(!result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_background_shell_has_unread_output() {
        let shell = BackgroundShell::new_pending(
            "shell-1".to_string(),
            None,
            "echo test".to_string(),
            "Test".to_string(),
        );

        // Initially no unread output
        assert!(!shell.has_unread_output());

        // After appending data, has unread output
        shell.stdout_buffer.append("output");
        assert!(shell.has_unread_output());

        // After taking output, no unread output
        let _ = shell.stdout_buffer.take_all();
        assert!(!shell.has_unread_output());
    }
}
