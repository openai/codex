//! Background shell store for managing running shell commands.

use super::types::BackgroundShell;
use super::types::SharedOutputBuffer;
use super::types::ShellOutput;
use super::types::ShellResult;
use super::types::ShellStatus;
use crate::system_reminder::generator::BackgroundTaskInfo;
use crate::system_reminder::generator::BackgroundTaskStatus;
use crate::system_reminder::generator::BackgroundTaskType;
use codex_protocol::ConversationId;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Store for managing background shell commands.
///
/// Thread-safe store that tracks running shell commands and their output.
/// Uses DashMap for concurrent access.
#[derive(Debug, Default)]
pub struct BackgroundShellStore {
    shells: DashMap<String, BackgroundShell>,
}

impl BackgroundShellStore {
    /// Create a new background shell store.
    pub fn new() -> Self {
        Self {
            shells: DashMap::new(),
        }
    }

    /// Generate a unique shell ID using UUID v4.
    pub fn generate_shell_id() -> String {
        format!("shell-{}", uuid::Uuid::new_v4())
    }

    /// Phase 1: Pre-register a shell command (before spawning).
    ///
    /// Returns the generated shell ID and cancellation token for the task.
    /// The `conversation_id` is used for session-scoped cleanup.
    pub fn register_pending(
        &self,
        conversation_id: Option<ConversationId>,
        command: String,
        description: String,
    ) -> (String, CancellationToken) {
        let shell_id = Self::generate_shell_id();
        let shell =
            BackgroundShell::new_pending(shell_id.clone(), conversation_id, command, description);
        let token = shell.cancellation_token.clone();
        self.shells.insert(shell_id.clone(), shell);
        (shell_id, token)
    }

    /// Phase 1 (variant): Pre-register a shell command with shared output buffers.
    ///
    /// Returns the shell ID, cancellation token, and shared stdout/stderr buffers.
    /// The buffers allow streaming output capture during execution.
    pub fn register_pending_with_buffer(
        &self,
        conversation_id: Option<ConversationId>,
        command: String,
        description: String,
    ) -> (
        String,
        CancellationToken,
        SharedOutputBuffer,
        SharedOutputBuffer,
    ) {
        let shell_id = Self::generate_shell_id();
        let shell =
            BackgroundShell::new_pending(shell_id.clone(), conversation_id, command, description);
        let token = shell.cancellation_token.clone();
        let stdout_buffer = shell.stdout_buffer.clone();
        let stderr_buffer = shell.stderr_buffer.clone();
        self.shells.insert(shell_id.clone(), shell);
        (shell_id, token, stdout_buffer, stderr_buffer)
    }

    /// Phase 2: Set the handle and transition to Running.
    pub fn set_running(&self, shell_id: &str, handle: JoinHandle<ShellResult>) {
        if let Some(mut shell) = self.shells.get_mut(shell_id) {
            shell.set_running(handle);
        }
    }

    /// Get the cancellation token for a shell.
    pub fn get_cancellation_token(&self, shell_id: &str) -> Option<CancellationToken> {
        self.shells
            .get(shell_id)
            .map(|s| s.cancellation_token.clone())
    }

    /// Get the current status of a shell.
    pub fn get_status(&self, shell_id: &str) -> Option<ShellStatus> {
        self.shells.get(shell_id).map(|s| s.status)
    }

    /// Get output from a background shell with tweakcc read and filter support.
    ///
    /// # Arguments
    /// - `shell_id`: The shell identifier
    /// - `block`: If true, waits for completion up to `timeout`
    /// - `timeout`: Maximum wait time when blocking
    /// - `filter`: Optional regex pattern to filter output lines
    /// - `limit`: Maximum bytes to return per call
    ///
    /// Each call consumes buffered output (buffer is cleared after reading).
    pub async fn get_output(
        &self,
        shell_id: &str,
        block: bool,
        timeout: Duration,
        filter: Option<&str>,
        limit: usize,
    ) -> Option<ShellOutput> {
        // First check current status
        let (should_block_wait, cancel_token) = {
            let Some(shell) = self.shells.get(shell_id) else {
                return None;
            };

            match shell.status {
                ShellStatus::Pending => {
                    if block {
                        drop(shell);
                        return self
                            .wait_for_completion(shell_id, timeout, filter, limit)
                            .await;
                    }
                    return Some(self.build_output(&shell, filter, limit));
                }
                ShellStatus::Running => {
                    if block {
                        let token = shell.cancellation_token.clone();
                        drop(shell);
                        (true, Some(token))
                    } else {
                        // Return current streaming output (non-blocking)
                        let output = self.build_output(&shell, filter, limit);
                        return Some(output);
                    }
                }
                ShellStatus::Completed
                | ShellStatus::Failed
                | ShellStatus::Killed
                | ShellStatus::Timeout => {
                    let output = self.build_output(&shell, filter, limit);
                    return Some(output);
                }
            }
        };

        if !should_block_wait {
            return None;
        }

        let cancel_token = cancel_token?;

        // Take handle outside the read lock
        let handle = {
            let Some(mut shell_mut) = self.shells.get_mut(shell_id) else {
                return None;
            };
            shell_mut.handle.take()
        };

        // If handle was already taken, fall back to polling
        if handle.is_none() {
            return self
                .wait_for_completion(shell_id, timeout, filter, limit)
                .await;
        }

        let handle = handle?;

        let result = tokio::select! {
            res = handle => {
                match res {
                    Ok(result) => Some(result),
                    Err(e) => Some(ShellResult {
                        output: String::new(),
                        exit_code: None,
                        success: false,
                        error: Some(format!("Task panicked or was cancelled: {e}")),
                    }),
                }
            }
            _ = tokio::time::sleep(timeout) => {
                if let Some(mut shell) = self.shells.get_mut(shell_id) {
                    shell.set_timeout();
                }
                return self.build_current_output(shell_id, filter, limit);
            }
            _ = cancel_token.cancelled() => {
                return self.build_current_output(shell_id, filter, limit);
            }
        };

        // Update the shell with the result
        if let Some(mut shell) = self.shells.get_mut(shell_id) {
            if let Some(ref r) = result {
                shell.set_completed(r.clone());
            }
        }

        self.build_current_output(shell_id, filter, limit)
    }

    /// Build ShellOutput from current shell state.
    ///
    /// Uses `take_all()` to consume buffered output (clears buffer after reading).
    fn build_output(
        &self,
        shell: &BackgroundShell,
        filter: Option<&str>,
        limit: usize,
    ) -> ShellOutput {
        // Take all content from buffers (clears them)
        let (mut stdout, stdout_truncated_bytes) = shell.stdout_buffer.take_all();
        let (mut stderr, stderr_truncated_bytes) = shell.stderr_buffer.take_all();

        // Track if we need to truncate due to limit
        let mut limit_truncated = false;
        if stdout.len() > limit {
            // Find safe UTF-8 boundary
            let mut end = limit;
            while end > 0 && !stdout.is_char_boundary(end) {
                end -= 1;
            }
            stdout.truncate(end);
            limit_truncated = true;
        }
        if stderr.len() > limit {
            let mut end = limit;
            while end > 0 && !stderr.is_char_boundary(end) {
                end -= 1;
            }
            stderr.truncate(end);
            limit_truncated = true;
        }

        // Apply filter if provided
        let filter_pattern = if let Some(pattern) = filter {
            if let Ok(re) = regex::Regex::new(pattern) {
                stdout = stdout
                    .lines()
                    .filter(|l| re.is_match(l))
                    .collect::<Vec<_>>()
                    .join("\n");
                stderr = stderr
                    .lines()
                    .filter(|l| re.is_match(l))
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(pattern.to_string())
            } else {
                None
            }
        } else {
            None
        };

        let stdout_lines = stdout.lines().count() as i32;
        let stderr_lines = stderr.lines().count() as i32;

        let status = match shell.status {
            ShellStatus::Pending | ShellStatus::Running => "running",
            ShellStatus::Completed => "completed",
            ShellStatus::Failed | ShellStatus::Timeout => "failed",
            ShellStatus::Killed => "killed",
        };

        // has_more is true if:
        // - Shell is still running (more output may come)
        // - We truncated due to limit (more data was available but not returned)
        let has_more =
            matches!(shell.status, ShellStatus::Running | ShellStatus::Pending) || limit_truncated;

        ShellOutput {
            shell_id: shell.shell_id.clone(),
            command: shell.command.clone(),
            status: status.to_string(),
            exit_code: shell.exit_code,
            stdout,
            stderr,
            stdout_lines,
            stderr_lines,
            timestamp: chrono::Utc::now().to_rfc3339(),
            filter_pattern,
            has_more,
            stdout_truncated: stdout_truncated_bytes as i32,
            stderr_truncated: stderr_truncated_bytes as i32,
        }
    }

    /// Build output for current shell state by ID.
    fn build_current_output(
        &self,
        shell_id: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> Option<ShellOutput> {
        let shell = self.shells.get(shell_id)?;
        let output = self.build_output(&shell, filter, limit);
        Some(output)
    }

    /// Wait for a shell to complete with polling.
    async fn wait_for_completion(
        &self,
        shell_id: &str,
        timeout: Duration,
        filter: Option<&str>,
        limit: usize,
    ) -> Option<ShellOutput> {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(100);

        while start.elapsed() < timeout {
            if let Some(shell) = self.shells.get(shell_id) {
                if shell.status.is_finished() {
                    let output = self.build_output(&shell, filter, limit);
                    return Some(output);
                }
            } else {
                return None;
            }
            tokio::time::sleep(poll_interval).await;
        }

        // Timeout
        if let Some(mut shell) = self.shells.get_mut(shell_id) {
            shell.set_timeout();
        }

        self.build_current_output(shell_id, filter, limit)
    }

    /// Kill a running shell.
    ///
    /// Uses cancellation token to signal the task to stop, which allows
    /// any awaiting get_output() calls to return immediately.
    pub fn kill(&self, shell_id: &str) -> Result<(), String> {
        let Some(mut shell) = self.shells.get_mut(shell_id) else {
            return Err(format!("Shell '{shell_id}' not found"));
        };

        match shell.status {
            ShellStatus::Running => {
                // Signal cancellation - this will wake up any waiting get_output() calls
                shell.cancellation_token.cancel();
                // Also abort the task handle if we still have it
                if let Some(handle) = shell.handle.take() {
                    handle.abort();
                }
                shell.set_killed();
                Ok(())
            }
            ShellStatus::Pending => {
                shell.cancellation_token.cancel();
                shell.set_killed();
                Ok(())
            }
            ShellStatus::Completed
            | ShellStatus::Failed
            | ShellStatus::Killed
            | ShellStatus::Timeout => Err(format!(
                "Shell '{shell_id}' is already finished with status {:?}",
                shell.status
            )),
        }
    }

    /// List all shells that need system reminder notifications.
    ///
    /// Returns shells that:
    /// - Belong to the specified conversation (if provided), AND
    /// - Have unread output (buffer not empty), OR
    /// - Have finished but not been notified
    ///
    /// If `conversation_id` is None, returns shells from all conversations.
    pub fn list_for_reminder(
        &self,
        conversation_id: Option<&ConversationId>,
    ) -> Vec<BackgroundTaskInfo> {
        self.shells
            .iter()
            .filter(|r| {
                let shell = r.value();

                // Filter by conversation if specified
                if let Some(conv_id) = conversation_id {
                    if shell.conversation_id.as_ref() != Some(conv_id) {
                        return false;
                    }
                }

                // has_unread = buffer not empty (tweakcc-only storage)
                let has_unread = shell.has_unread_output();
                has_unread || (shell.status.is_finished() && !shell.notified)
            })
            .map(|r| {
                let shell = r.value();
                BackgroundTaskInfo {
                    task_id: shell.shell_id.clone(),
                    task_type: BackgroundTaskType::Shell,
                    command: Some(shell.command.clone()),
                    description: shell.description.clone(),
                    status: match shell.status {
                        ShellStatus::Pending | ShellStatus::Running => {
                            BackgroundTaskStatus::Running
                        }
                        ShellStatus::Completed => BackgroundTaskStatus::Completed,
                        ShellStatus::Failed | ShellStatus::Killed | ShellStatus::Timeout => {
                            BackgroundTaskStatus::Failed
                        }
                    },
                    exit_code: shell.exit_code,
                    has_new_output: shell.has_unread_output(),
                    notified: shell.notified,
                }
            })
            .collect()
    }

    /// Mark a shell as notified (after system reminder has been sent).
    ///
    /// Only sets `notified = true` for finished shells (running shells may have more output).
    /// Note: `has_new_output` is now derived from buffer state, not stored.
    pub fn mark_notified(&self, shell_id: &str) {
        if let Some(mut shell) = self.shells.get_mut(shell_id) {
            // Only mark as fully notified if the shell has finished
            // For running shells, notification will be re-triggered when more output arrives
            if shell.status.is_finished() {
                shell.notified = true;
            }
        }
    }

    /// Batch mark multiple shells as notified.
    ///
    /// More efficient than calling mark_notified() in a loop when marking
    /// multiple shells, as it reduces lock contention.
    pub fn mark_all_notified(&self, shell_ids: &[String]) {
        for id in shell_ids {
            if let Some(mut shell) = self.shells.get_mut(id) {
                if shell.status.is_finished() {
                    shell.notified = true;
                }
            }
        }
    }

    /// List all shell IDs.
    pub fn list_shell_ids(&self) -> Vec<String> {
        self.shells.iter().map(|r| r.key().clone()).collect()
    }

    /// Remove completed shells older than specified duration.
    ///
    /// Also cleans up stale pending shells that have been pending for too long
    /// (e.g., if the process crashed between register_pending and set_running).
    pub fn cleanup_old(&self, older_than: Duration) {
        // Pending shells older than 5 minutes are considered stale
        const PENDING_TIMEOUT: Duration = Duration::from_secs(5 * 60);

        let now = Instant::now();
        self.shells.retain(|_, shell| {
            match shell.status {
                // Clean up stale pending shells (never transitioned to running)
                ShellStatus::Pending => now.duration_since(shell.created_at) < PENDING_TIMEOUT,
                // Keep running shells
                ShellStatus::Running => true,
                // Remove old finished shells
                _ => now.duration_since(shell.created_at) < older_than,
            }
        });
    }

    /// Remove all shells belonging to a specific conversation.
    ///
    /// This is called during session cleanup to ensure shells are properly
    /// cleaned up when a conversation ends, rather than waiting for time-based cleanup.
    ///
    /// Running shells are killed before removal.
    pub fn cleanup_by_conversation(&self, conversation_id: &ConversationId) {
        // First, kill any running shells for this conversation
        let shells_to_kill: Vec<String> = self
            .shells
            .iter()
            .filter(|r| {
                r.conversation_id.as_ref() == Some(conversation_id)
                    && matches!(r.status, ShellStatus::Running | ShellStatus::Pending)
            })
            .map(|r| r.key().clone())
            .collect();

        for shell_id in shells_to_kill {
            let _ = self.kill(&shell_id);
        }

        // Then remove all shells for this conversation
        self.shells
            .retain(|_, shell| shell.conversation_id.as_ref() != Some(conversation_id));
    }
}

/// Shared background shell store wrapped in Arc.
pub type SharedBackgroundShellStore = Arc<BackgroundShellStore>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_shell_id() {
        let id1 = BackgroundShellStore::generate_shell_id();
        let id2 = BackgroundShellStore::generate_shell_id();
        assert!(id1.starts_with("shell-"));
        assert!(id2.starts_with("shell-"));
        // IDs should be different (with very high probability)
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_register_pending() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token) =
            store.register_pending(None, "echo hello".to_string(), "Test command".to_string());
        assert!(shell_id.starts_with("shell-"));

        let status = store.get_status(&shell_id);
        assert_eq!(status, Some(ShellStatus::Pending));
    }

    #[tokio::test]
    async fn test_set_running() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token) =
            store.register_pending(None, "echo hello".to_string(), "Test command".to_string());

        let handle = tokio::spawn(async {
            ShellResult {
                output: "hello\n".to_string(),
                exit_code: Some(0),
                success: true,
                error: None,
            }
        });

        store.set_running(&shell_id, handle);

        let status = store.get_status(&shell_id);
        assert_eq!(status, Some(ShellStatus::Running));
    }

    #[test]
    fn test_list_shell_ids() {
        let store = BackgroundShellStore::new();
        assert!(store.list_shell_ids().is_empty());

        store.register_pending(None, "cmd1".to_string(), "desc1".to_string());
        store.register_pending(None, "cmd2".to_string(), "desc2".to_string());

        let ids = store.list_shell_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_kill_pending() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token) =
            store.register_pending(None, "sleep 100".to_string(), "Long command".to_string());

        let result = store.kill(&shell_id);
        assert!(result.is_ok());

        let status = store.get_status(&shell_id);
        assert_eq!(status, Some(ShellStatus::Killed));
    }

    #[test]
    fn test_kill_not_found() {
        let store = BackgroundShellStore::new();
        let result = store.kill("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_by_conversation() {
        let store = BackgroundShellStore::new();
        let conv1 = ConversationId::new();
        let conv2 = ConversationId::new();

        // Register shells for different conversations
        let (shell1, _) =
            store.register_pending(Some(conv1), "cmd1".to_string(), "desc1".to_string());
        let (shell2, _) =
            store.register_pending(Some(conv1), "cmd2".to_string(), "desc2".to_string());
        let (shell3, _) =
            store.register_pending(Some(conv2), "cmd3".to_string(), "desc3".to_string());
        let (shell4, _) = store.register_pending(None, "cmd4".to_string(), "desc4".to_string());

        assert_eq!(store.list_shell_ids().len(), 4);

        // Cleanup conversation 1
        store.cleanup_by_conversation(&conv1);

        // Should have 2 shells left (conv2 and None)
        let remaining = store.list_shell_ids();
        assert_eq!(remaining.len(), 2);
        assert!(!remaining.contains(&shell1));
        assert!(!remaining.contains(&shell2));
        assert!(remaining.contains(&shell3));
        assert!(remaining.contains(&shell4));
    }

    #[test]
    fn test_register_pending_with_buffer() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token, stdout_buffer, stderr_buffer) = store.register_pending_with_buffer(
            None,
            "echo hello".to_string(),
            "Test command".to_string(),
        );

        assert!(shell_id.starts_with("shell-"));
        assert!(stdout_buffer.is_empty());
        assert!(stderr_buffer.is_empty());

        // Write to stdout buffer
        stdout_buffer.append("hello");
        assert_eq!(stdout_buffer.len(), 5);

        // take_all() returns content and clears buffer
        let (content, _) = stdout_buffer.take_all();
        assert_eq!(content, "hello");
        assert!(stdout_buffer.is_empty());

        // Write to stderr buffer
        stderr_buffer.append("error");
        assert_eq!(stderr_buffer.len(), 5);

        let (content, _) = stderr_buffer.take_all();
        assert_eq!(content, "error");
    }

    #[tokio::test]
    async fn test_incremental_output_basic() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token, stdout_buffer, _stderr_buffer) =
            store.register_pending_with_buffer(None, "echo hello".to_string(), "Test".to_string());

        // Write some output
        stdout_buffer.append("line1\n");
        stdout_buffer.append("line2\n");

        // Simulate completing the shell
        let handle = tokio::spawn(async {
            ShellResult {
                output: "line1\nline2\n".to_string(),
                exit_code: Some(0),
                success: true,
                error: None,
            }
        });
        store.set_running(&shell_id, handle);

        // Wait a bit for the task to complete
        tokio::time::sleep(Duration::from_millis(50)).await;

        // First read (tweakcc mode)
        let output1 = store
            .get_output(&shell_id, false, Duration::from_secs(1), None, 100)
            .await;
        assert!(output1.is_some());
        let out1 = output1.unwrap();
        assert!(!out1.stdout.is_empty());
        assert_eq!(out1.stdout_lines, 2);

        // Buffer should be cleared after read (tweakcc-only storage)
        let shell = store.shells.get(&shell_id).unwrap();
        assert!(!shell.has_unread_output());
    }

    #[tokio::test]
    async fn test_incremental_reads_clear_buffer() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token, stdout_buffer, _stderr_buffer) =
            store.register_pending_with_buffer(None, "test".to_string(), "Test".to_string());

        // Write output
        stdout_buffer.append("output1\n");
        stdout_buffer.append("output2\n");

        // Complete the shell
        if let Some(mut shell) = store.shells.get_mut(&shell_id) {
            shell.set_completed(ShellResult {
                output: "output1\noutput2\n".to_string(),
                exit_code: Some(0),
                success: true,
                error: None,
            });
        }

        // First read should get all data and clear buffer
        let output1 = store
            .get_output(&shell_id, false, Duration::from_secs(1), None, 100)
            .await
            .unwrap();
        assert!(!output1.stdout.is_empty());
        assert!(output1.stdout.contains("output1"));

        // Second read should get nothing (buffer was cleared after first read)
        let output2 = store
            .get_output(&shell_id, false, Duration::from_secs(1), None, 100)
            .await
            .unwrap();
        assert!(output2.stdout.is_empty());
        assert!(!output2.has_more);
    }

    #[tokio::test]
    async fn test_limit_truncates_output() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token, stdout_buffer, _stderr_buffer) =
            store.register_pending_with_buffer(None, "test".to_string(), "Test".to_string());

        // Write more than limit
        stdout_buffer.append("12345678901234567890"); // 20 chars

        // Complete the shell
        if let Some(mut shell) = store.shells.get_mut(&shell_id) {
            shell.set_completed(ShellResult {
                output: "12345678901234567890".to_string(),
                exit_code: Some(0),
                success: true,
                error: None,
            });
        }

        // Read with limit=5
        let output = store
            .get_output(&shell_id, false, Duration::from_secs(1), None, 5)
            .await
            .unwrap();

        assert_eq!(output.stdout.len(), 5);
        assert!(output.has_more); // More data available
    }

    #[tokio::test]
    async fn test_has_more_flag() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token, stdout_buffer, _stderr_buffer) =
            store.register_pending_with_buffer(None, "test".to_string(), "Test".to_string());

        stdout_buffer.append("12345");

        // Complete the shell
        if let Some(mut shell) = store.shells.get_mut(&shell_id) {
            shell.set_completed(ShellResult {
                output: "12345".to_string(),
                exit_code: Some(0),
                success: true,
                error: None,
            });
        }

        // Read all data - has_more should be false
        let output = store
            .get_output(&shell_id, false, Duration::from_secs(1), None, 100)
            .await
            .unwrap();

        assert!(!output.has_more); // All data read on completed shell

        // Read again - should get empty since buffer was cleared
        let output2 = store
            .get_output(&shell_id, false, Duration::from_secs(1), None, 100)
            .await
            .unwrap();

        assert!(output2.stdout.is_empty());
        assert!(!output2.has_more);
    }

    #[tokio::test]
    async fn test_filter_output_lines() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token, stdout_buffer, stderr_buffer) =
            store.register_pending_with_buffer(None, "test".to_string(), "Test".to_string());

        stdout_buffer.append("info: starting\n");
        stdout_buffer.append("error: something failed\n");
        stdout_buffer.append("warning: be careful\n");
        stdout_buffer.append("info: done\n");

        stderr_buffer.append("DEBUG: trace info\n");
        stderr_buffer.append("ERROR: critical issue\n");

        // Complete the shell
        if let Some(mut shell) = store.shells.get_mut(&shell_id) {
            shell.set_completed(ShellResult {
                output: String::new(),
                exit_code: Some(0),
                success: true,
                error: None,
            });
        }

        // Filter for error|warning
        let output = store
            .get_output(
                &shell_id,
                false,
                Duration::from_secs(1),
                Some("error|warning"),
                1000,
            )
            .await
            .unwrap();

        assert!(output.stdout.contains("error:"));
        assert!(output.stdout.contains("warning:"));
        assert!(!output.stdout.contains("info:"));
        assert_eq!(output.filter_pattern, Some("error|warning".to_string()));
    }

    #[tokio::test]
    async fn test_separate_stdout_stderr() {
        let store = BackgroundShellStore::new();
        let (shell_id, _token, stdout_buffer, stderr_buffer) =
            store.register_pending_with_buffer(None, "test".to_string(), "Test".to_string());

        stdout_buffer.append("stdout line 1\n");
        stdout_buffer.append("stdout line 2\n");
        stderr_buffer.append("stderr line 1\n");

        // Complete the shell
        if let Some(mut shell) = store.shells.get_mut(&shell_id) {
            shell.set_completed(ShellResult {
                output: String::new(),
                exit_code: Some(0),
                success: true,
                error: None,
            });
        }

        let output = store
            .get_output(&shell_id, false, Duration::from_secs(1), None, 1000)
            .await
            .unwrap();

        assert!(output.stdout.contains("stdout line 1"));
        assert!(output.stdout.contains("stdout line 2"));
        assert!(output.stderr.contains("stderr line 1"));
        assert_eq!(output.stdout_lines, 2);
        assert_eq!(output.stderr_lines, 1);
    }
}
