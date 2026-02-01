//! Shell snapshot capture and management.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::watch;
use tokio::time::timeout;

use crate::shell_types::Shell;
use crate::shell_types::ShellType;
use crate::snapshot::scripts::bash_snapshot_script;
use crate::snapshot::scripts::powershell_snapshot_script;
use crate::snapshot::scripts::sh_snapshot_script;
use crate::snapshot::scripts::zsh_snapshot_script;

/// Default timeout for snapshot capture operations.
const DEFAULT_SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);

/// Default retention period for snapshot files (7 days).
const DEFAULT_SNAPSHOT_RETENTION: Duration = Duration::from_secs(60 * 60 * 24 * 7);

/// Default directory name for shell snapshots.
const DEFAULT_SNAPSHOT_DIR: &str = "shell_snapshots";

/// Configuration for shell snapshotting.
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Directory to store snapshot files.
    pub snapshot_dir: PathBuf,
    /// Timeout for snapshot capture operations.
    pub timeout: Duration,
    /// How long to retain snapshot files before cleanup.
    pub retention: Duration,
}

impl SnapshotConfig {
    /// Creates a new config with the given home directory.
    pub fn new(cocode_home: &Path) -> Self {
        Self {
            snapshot_dir: cocode_home.join(DEFAULT_SNAPSHOT_DIR),
            timeout: DEFAULT_SNAPSHOT_TIMEOUT,
            retention: DEFAULT_SNAPSHOT_RETENTION,
        }
    }

    /// Returns the default snapshot directory name.
    pub fn default_dir_name() -> &'static str {
        DEFAULT_SNAPSHOT_DIR
    }

    /// Returns the default retention duration.
    pub fn default_retention() -> Duration {
        DEFAULT_SNAPSHOT_RETENTION
    }
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_dir: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".cocode")
                .join(DEFAULT_SNAPSHOT_DIR),
            timeout: DEFAULT_SNAPSHOT_TIMEOUT,
            retention: DEFAULT_SNAPSHOT_RETENTION,
        }
    }
}

/// A captured shell environment snapshot.
///
/// When dropped, the snapshot file is automatically deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellSnapshot {
    /// Path to the snapshot file.
    pub path: PathBuf,
}

impl ShellSnapshot {
    /// Starts asynchronous shell snapshotting.
    ///
    /// This spawns a background task that captures the shell environment
    /// and sends the result through a watch channel. The shell's snapshot
    /// receiver is updated to receive the snapshot when ready.
    ///
    /// # Arguments
    ///
    /// * `config` - Snapshot configuration
    /// * `session_id` - Unique session identifier for the snapshot file
    /// * `shell` - The shell to snapshot (will be mutated to receive the snapshot)
    pub fn start_snapshotting(config: SnapshotConfig, session_id: &str, shell: &mut Shell) {
        let (shell_snapshot_tx, shell_snapshot_rx) = watch::channel(None);
        shell.set_shell_snapshot_receiver(shell_snapshot_rx);

        let snapshot_shell = shell.clone();
        let snapshot_session_id = session_id.to_string();

        tokio::spawn(async move {
            let snapshot = Self::try_new(&config, &snapshot_session_id, &snapshot_shell)
                .await
                .map(Arc::new);

            if snapshot.is_some() {
                tracing::info!("Shell snapshot created for session {snapshot_session_id}");
            } else {
                tracing::warn!("Failed to create shell snapshot for session {snapshot_session_id}");
            }

            let _ = shell_snapshot_tx.send(snapshot);
        });
    }

    /// Attempts to create a new shell snapshot synchronously.
    ///
    /// Returns `None` if snapshot creation fails for any reason (unsupported
    /// shell, timeout, validation failure, etc.).
    pub async fn try_new(config: &SnapshotConfig, session_id: &str, shell: &Shell) -> Option<Self> {
        // Determine file extension based on shell type
        let extension = match shell.shell_type() {
            ShellType::PowerShell => "ps1",
            _ => "sh",
        };

        let path = config
            .snapshot_dir
            .join(format!("{session_id}.{extension}"));

        // Create the snapshot
        let snapshot = match write_shell_snapshot(shell, &path, config.timeout).await {
            Ok(path) => {
                tracing::debug!("Shell snapshot written to: {}", path.display());
                Some(Self { path })
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to create shell snapshot for {}: {err:?}",
                    shell.name()
                );
                None
            }
        };

        // Validate the snapshot
        if let Some(ref snapshot) = snapshot {
            if let Err(err) = validate_snapshot(shell, &snapshot.path, config.timeout).await {
                tracing::error!("Shell snapshot validation failed: {err:?}");
                // Clean up the invalid snapshot
                let _ = fs::remove_file(&snapshot.path).await;
                return None;
            }
        }

        snapshot
    }

    /// Returns the path to the snapshot file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ShellSnapshot {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.path) {
            // Only warn if the file actually existed
            if err.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    "Failed to delete shell snapshot at {:?}: {err:?}",
                    self.path
                );
            }
        }
    }
}

/// Writes a shell snapshot to the specified path.
async fn write_shell_snapshot(
    shell: &Shell,
    output_path: &Path,
    timeout: Duration,
) -> Result<PathBuf> {
    let shell_type = shell.shell_type();

    if *shell_type == ShellType::PowerShell || *shell_type == ShellType::Cmd {
        bail!("Shell snapshot not yet supported for {shell_type:?}");
    }

    // Capture the snapshot
    let raw_snapshot = capture_snapshot(shell, timeout).await?;
    let snapshot = strip_snapshot_preamble(&raw_snapshot)?;

    // Create parent directory if needed
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).await.with_context(|| {
            format!("Failed to create snapshot directory: {}", parent.display())
        })?;
    }

    // Write the snapshot file
    fs::write(output_path, snapshot)
        .await
        .with_context(|| format!("Failed to write snapshot to: {}", output_path.display()))?;

    Ok(output_path.to_path_buf())
}

/// Captures a shell environment snapshot.
async fn capture_snapshot(shell: &Shell, snapshot_timeout: Duration) -> Result<String> {
    let script = match shell.shell_type() {
        ShellType::Zsh => zsh_snapshot_script(),
        ShellType::Bash => bash_snapshot_script(),
        ShellType::Sh => sh_snapshot_script(),
        ShellType::PowerShell => powershell_snapshot_script().to_string(),
        ShellType::Cmd => bail!("Shell snapshotting is not supported for cmd"),
    };

    run_script_with_timeout(shell, &script, snapshot_timeout, true).await
}

/// Strips any output before the snapshot marker.
fn strip_snapshot_preamble(snapshot: &str) -> Result<String> {
    let marker = "# Snapshot file";
    let Some(start) = snapshot.find(marker) else {
        bail!("Snapshot output missing marker '{marker}'");
    };

    Ok(snapshot[start..].to_string())
}

/// Validates a snapshot by attempting to source it.
#[cfg_attr(test, allow(dead_code))]
pub(crate) async fn validate_snapshot(
    shell: &Shell,
    snapshot_path: &Path,
    timeout: Duration,
) -> Result<()> {
    let script = format!("set -e; . \"{}\"", snapshot_path.display());
    run_script_with_timeout(shell, &script, timeout, false)
        .await
        .map(|_| ())
}

/// Runs a shell script with a timeout.
async fn run_script_with_timeout(
    shell: &Shell,
    script: &str,
    snapshot_timeout: Duration,
    use_login_shell: bool,
) -> Result<String> {
    let args = shell.derive_exec_args(script, use_login_shell);
    let shell_name = shell.name();

    let mut handler = Command::new(&args[0]);
    handler.args(&args[1..]);

    // Detach from TTY on Unix to prevent issues with shell initialization
    #[cfg(unix)]
    unsafe {
        handler.pre_exec(|| {
            // Detach from controlling terminal
            let _ = libc::setsid();
            Ok(())
        });
    }

    handler.kill_on_drop(true);

    let output = timeout(snapshot_timeout, handler.output())
        .await
        .map_err(|_| anyhow!("Snapshot command timed out for {shell_name}"))?
        .with_context(|| format!("Failed to execute {shell_name}"))?;

    if !output.status.success() {
        let status = output.status;
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Snapshot command exited with status {status}: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_strip_snapshot_preamble_success() {
        let snapshot = "noise\n# Snapshot file\nexport PATH=/bin\n";
        let cleaned = strip_snapshot_preamble(snapshot).expect("should succeed");
        assert_eq!(cleaned, "# Snapshot file\nexport PATH=/bin\n");
    }

    #[test]
    fn test_strip_snapshot_preamble_requires_marker() {
        let result = strip_snapshot_preamble("missing header");
        assert!(result.is_err());
    }

    #[test]
    fn test_strip_snapshot_preamble_marker_at_start() {
        let snapshot = "# Snapshot file\nexport FOO=bar\n";
        let cleaned = strip_snapshot_preamble(snapshot).expect("should succeed");
        assert_eq!(cleaned, snapshot);
    }

    #[test]
    fn test_snapshot_config_default() {
        let config = SnapshotConfig::default();
        assert!(
            config
                .snapshot_dir
                .to_string_lossy()
                .contains("shell_snapshots")
        );
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert_eq!(config.retention, Duration::from_secs(60 * 60 * 24 * 7));
    }

    #[test]
    fn test_snapshot_config_new() {
        let home = PathBuf::from("/home/test/.cocode");
        let config = SnapshotConfig::new(&home);
        assert_eq!(config.snapshot_dir, home.join("shell_snapshots"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_write_and_validate_bash_snapshot() {
        use crate::shell_types::get_shell;

        let Some(shell) = get_shell(ShellType::Bash, None) else {
            // Skip test if bash is not available
            return;
        };

        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("test_snapshot.sh");

        let result = write_shell_snapshot(&shell, &path, Duration::from_secs(10)).await;
        assert!(result.is_ok(), "Failed to write snapshot: {result:?}");

        let content = fs::read_to_string(&path).await.expect("read snapshot");
        assert!(content.contains("# Snapshot file"));
        assert!(content.contains("# exports"));

        // Validate the snapshot
        let validate_result = validate_snapshot(&shell, &path, Duration::from_secs(10)).await;
        assert!(
            validate_result.is_ok(),
            "Validation failed: {validate_result:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn test_write_and_validate_zsh_snapshot() {
        use crate::shell_types::get_shell;

        let Some(shell) = get_shell(ShellType::Zsh, None) else {
            return;
        };

        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("test_snapshot.sh");

        let result = write_shell_snapshot(&shell, &path, Duration::from_secs(10)).await;
        assert!(result.is_ok(), "Failed to write snapshot: {result:?}");

        let content = fs::read_to_string(&path).await.expect("read snapshot");
        assert!(content.contains("# Snapshot file"));
        assert!(content.contains("# setopts"));
    }

    #[tokio::test]
    async fn test_shell_snapshot_try_new() {
        use crate::shell_types::default_user_shell;

        let shell = default_user_shell();
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = SnapshotConfig::new(dir.path());

        let snapshot = ShellSnapshot::try_new(&config, "test-session", &shell).await;

        // On Unix systems with bash/zsh, this should succeed
        #[cfg(unix)]
        {
            assert!(snapshot.is_some(), "Snapshot creation failed on Unix");
            let snapshot = snapshot.unwrap();
            assert!(snapshot.path.exists());

            // Snapshot should be cleaned up on drop
            let path = snapshot.path.clone();
            drop(snapshot);
            assert!(!path.exists());
        }
    }

    /// Tests that the Drop implementation correctly deletes the snapshot file.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_snapshot_drop_deletes_file() {
        use crate::shell_types::default_user_shell;

        let shell = default_user_shell();
        let dir = tempfile::tempdir().expect("create temp dir");
        let config = SnapshotConfig::new(dir.path());

        let snapshot = ShellSnapshot::try_new(&config, "drop-test", &shell)
            .await
            .expect("snapshot should be created");

        let path = snapshot.path.clone();
        assert!(path.exists(), "snapshot file should exist before drop");

        drop(snapshot);

        assert!(!path.exists(), "snapshot file should be deleted after drop");
    }

    /// Tests that validate_snapshot rejects invalid/malformed snapshots.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_validate_snapshot_rejects_invalid() {
        use crate::shell_types::default_user_shell;

        let shell = default_user_shell();
        let dir = tempfile::tempdir().expect("create temp dir");
        let invalid_snapshot = dir.path().join("invalid.sh");

        // Write a snapshot that will fail when sourced (exit 1)
        fs::write(&invalid_snapshot, "exit 1")
            .await
            .expect("write invalid snapshot");

        let result = validate_snapshot(&shell, &invalid_snapshot, Duration::from_secs(5)).await;

        assert!(
            result.is_err(),
            "validation should fail for invalid snapshot that exits with error"
        );
    }
}
