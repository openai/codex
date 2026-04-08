use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::rollout::list::find_thread_path_by_id_str;
use anyhow::Result;
use codex_protocol::ThreadId;
pub use codex_shell::SNAPSHOT_DIR;
pub use codex_shell::SNAPSHOT_RETENTION;
pub use codex_shell::ShellSnapshot;
use codex_shell::remove_snapshot_file;
pub use codex_shell::snapshot_session_id_from_file_name;
use tokio::fs;

/// Removes shell snapshots that either lack a matching session rollout file or
/// whose rollouts have not been updated within the retention window.
/// The active session id is exempt from cleanup.
pub async fn cleanup_stale_snapshots(codex_home: &Path, active_session_id: ThreadId) -> Result<()> {
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);

    let mut entries = match fs::read_dir(&snapshot_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    let now = SystemTime::now();
    let active_session_id = active_session_id.to_string();

    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }

        let path = entry.path();

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(session_id) = snapshot_session_id_from_file_name(&file_name) else {
            remove_snapshot_file(&path).await;
            continue;
        };
        if session_id == active_session_id {
            continue;
        }

        let rollout_path = find_thread_path_by_id_str(codex_home, session_id).await?;
        let Some(rollout_path) = rollout_path else {
            remove_snapshot_file(&path).await;
            continue;
        };

        let modified = match fs::metadata(&rollout_path).await.and_then(|m| m.modified()) {
            Ok(modified) => modified,
            Err(err) => {
                tracing::warn!(
                    "Failed to check rollout age for snapshot {}: {err:?}",
                    path.display()
                );
                continue;
            }
        };

        if now
            .duration_since(modified)
            .ok()
            .is_some_and(|age| age >= SNAPSHOT_RETENTION)
        {
            remove_snapshot_file(&path).await;
        }
    }

    Ok(())
}

pub(crate) fn spawn_stale_snapshot_cleanup(codex_home: PathBuf, active_session_id: ThreadId) {
    tokio::spawn(async move {
        if let Err(err) = cleanup_stale_snapshots(&codex_home, active_session_id).await {
            tracing::warn!("Failed to clean up shell snapshots: {err:?}");
        }
    });
}

#[cfg(test)]
#[path = "shell_snapshot_tests.rs"]
mod tests;
