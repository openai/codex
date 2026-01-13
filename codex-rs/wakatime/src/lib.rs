//! WakaTime integration for Codex CLI.
//!
//! Sends heartbeats to the WakaTime CLI to track coding time spent in Codex sessions.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::process::Command;
use tokio::sync::Mutex;

/// Configuration for WakaTime integration.
#[derive(Debug, Clone)]
pub struct WakaTimeConfig {
    /// Whether WakaTime tracking is enabled.
    pub enabled: bool,
    /// Path to the wakatime-cli binary. Defaults to ~/.wakatime/wakatime-cli.
    pub cli_path: PathBuf,
    /// Project name override. If not set, uses the git project name.
    pub project: Option<String>,
}

impl Default for WakaTimeConfig {
    fn default() -> Self {
        let cli_path = dirs::home_dir()
            .map(|h| h.join(".wakatime").join("wakatime-cli"))
            .unwrap_or_else(|| PathBuf::from("wakatime-cli"));

        Self {
            enabled: false,
            cli_path,
            project: None,
        }
    }
}

/// Tracks AI-generated line changes for a file.
#[derive(Debug, Clone, Copy, Default)]
pub struct AiLineChanges {
    /// Number of lines added by AI.
    pub lines_added: i32,
    /// Number of lines changed/deleted by AI.
    pub lines_changed: i32,
}

/// Rate limiting: 1 heartbeat per minute per file.
const RATE_LIMIT_SECS: u64 = 60;

/// Plugin identifier sent to WakaTime.
const PLUGIN_NAME: &str = "codex-cli/1.0";

/// WakaTime tracker that sends heartbeats for file activity.
pub struct WakaTimeTracker {
    cli_path: PathBuf,
    project: String,
    /// Tracks the last heartbeat time per file path for rate limiting.
    last_heartbeats: Arc<Mutex<HashMap<PathBuf, Instant>>>,
}

impl WakaTimeTracker {
    /// Creates a new WakaTime tracker if enabled.
    ///
    /// Returns `None` if WakaTime is disabled or the CLI binary doesn't exist.
    pub fn new(config: &WakaTimeConfig, project: &str) -> Option<Self> {
        if !config.enabled {
            tracing::debug!("WakaTime integration is disabled");
            return None;
        }

        // Check if wakatime-cli exists
        if !config.cli_path.exists() {
            tracing::warn!(
                "WakaTime CLI not found at {:?}, disabling integration",
                config.cli_path
            );
            return None;
        }

        let project_name = config
            .project
            .clone()
            .unwrap_or_else(|| project.to_string());

        tracing::info!(
            "WakaTime integration enabled, project: {}, cli: {:?}",
            project_name,
            config.cli_path
        );

        Some(Self {
            cli_path: config.cli_path.clone(),
            project: project_name,
            last_heartbeats: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Records file activity and sends a heartbeat if rate limit allows.
    ///
    /// # Arguments
    /// * `path` - The file path that was accessed
    /// * `is_write` - Whether this is a write operation (bypasses rate limit)
    /// * `ai_changes` - Optional AI line changes for this file
    pub async fn on_file_activity(
        &self,
        path: &Path,
        is_write: bool,
        ai_changes: Option<AiLineChanges>,
    ) {
        let should_send = {
            let mut last = self.last_heartbeats.lock().await;
            let now = Instant::now();

            // Writes always send heartbeats; reads are rate-limited
            if is_write {
                last.insert(path.to_path_buf(), now);
                true
            } else {
                match last.get(path) {
                    Some(last_time)
                        if now.duration_since(*last_time).as_secs() < RATE_LIMIT_SECS =>
                    {
                        false
                    }
                    _ => {
                        last.insert(path.to_path_buf(), now);
                        true
                    }
                }
            }
        };

        if should_send {
            self.send_heartbeat(path, is_write, ai_changes).await;
        }
    }

    /// Sends a heartbeat to wakatime-cli.
    async fn send_heartbeat(&self, path: &Path, is_write: bool, ai_changes: Option<AiLineChanges>) {
        let mut cmd = Command::new(&self.cli_path);
        cmd.arg("--entity")
            .arg(path)
            .arg("--plugin")
            .arg(PLUGIN_NAME)
            .arg("--project")
            .arg(&self.project);

        if is_write {
            cmd.arg("--write");
        }

        if let Some(changes) = ai_changes {
            cmd.arg("--ai-line-changes").arg(format!(
                "added={},changed={}",
                changes.lines_added, changes.lines_changed
            ));
        }

        // Run in background, don't block on result
        match cmd.spawn() {
            Ok(mut child) => {
                // Spawn a task to wait for the child to avoid zombies
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
                tracing::debug!(
                    "Sent WakaTime heartbeat for {:?} (write={})",
                    path,
                    is_write
                );
            }
            Err(e) => {
                tracing::warn!("Failed to send WakaTime heartbeat: {e}");
            }
        }
    }

    /// Shuts down the tracker, sending a final heartbeat if needed.
    pub async fn shutdown(&self) {
        tracing::debug!("WakaTime tracker shutting down");
        // The last heartbeats map will be dropped, no special cleanup needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WakaTimeConfig::default();
        assert!(!config.enabled);
        assert!(config.cli_path.to_string_lossy().contains("wakatime-cli"));
    }

    #[test]
    fn test_tracker_disabled_when_not_enabled() {
        let config = WakaTimeConfig::default();
        let tracker = WakaTimeTracker::new(&config, "test-project");
        assert!(tracker.is_none());
    }

    #[test]
    fn test_ai_line_changes_default() {
        let changes = AiLineChanges::default();
        assert_eq!(changes.lines_added, 0);
        assert_eq!(changes.lines_changed, 0);
    }
}
