//! Stale snapshot cleanup utilities.
//!
//! Provides functions to clean up orphaned or expired shell snapshot files.

use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;
use std::time::SystemTime;

use anyhow::Result;
use tokio::fs;

/// Removes stale shell snapshot files from the snapshot directory.
///
/// A snapshot is considered stale if:
/// 1. It lacks a valid session ID format (no extension separator)
/// 2. It belongs to a session other than the active one and is older than the retention period
///
/// # Arguments
///
/// * `snapshot_dir` - Directory containing snapshot files
/// * `active_session_id` - Session ID to exempt from cleanup (currently active)
/// * `retention` - How long to keep inactive snapshots before removal
///
/// # Returns
///
/// Returns the number of snapshots removed, or an error if cleanup fails.
pub async fn cleanup_stale_snapshots(
    snapshot_dir: &Path,
    active_session_id: &str,
    retention: Duration,
) -> Result<i32> {
    let mut entries = match fs::read_dir(snapshot_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err.into()),
    };

    let now = SystemTime::now();
    let mut removed_count = 0;

    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }

        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        // Extract session ID from filename (format: {session_id}.{extension})
        let session_id = match file_name.rsplit_once('.') {
            Some((stem, _ext)) => stem,
            None => {
                // Invalid filename format, remove it
                remove_snapshot_file(&path).await;
                removed_count += 1;
                continue;
            }
        };

        // Don't remove the active session's snapshot
        if session_id == active_session_id {
            continue;
        }

        // Check if the snapshot is older than the retention period
        let modified = match fs::metadata(&path).await.and_then(|m| m.modified()) {
            Ok(modified) => modified,
            Err(err) => {
                tracing::warn!(
                    "Failed to check snapshot age for {}: {err:?}",
                    path.display()
                );
                continue;
            }
        };

        if let Ok(age) = now.duration_since(modified) {
            if age >= retention {
                remove_snapshot_file(&path).await;
                removed_count += 1;
            }
        }
    }

    Ok(removed_count)
}

/// Removes all snapshot files for a specific session.
///
/// This is useful for cleaning up after a session ends.
#[allow(dead_code)]
pub async fn cleanup_session_snapshots(snapshot_dir: &Path, session_id: &str) -> Result<()> {
    let mut entries = match fs::read_dir(snapshot_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        // Check if this file belongs to the target session
        if let Some((stem, _ext)) = file_name.rsplit_once('.') {
            if stem == session_id {
                remove_snapshot_file(&entry.path()).await;
            }
        }
    }

    Ok(())
}

/// Removes a snapshot file, logging any errors.
async fn remove_snapshot_file(path: &Path) {
    if let Err(err) = fs::remove_file(path).await {
        if err.kind() != ErrorKind::NotFound {
            tracing::warn!("Failed to delete shell snapshot at {:?}: {err:?}", path);
        }
    } else {
        tracing::debug!("Removed stale snapshot: {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cleanup_removes_old_snapshots() {
        let dir = tempdir().expect("create temp dir");
        let snapshot_dir = dir.path();

        // Create some snapshot files
        let old_snapshot = snapshot_dir.join("old-session.sh");
        let active_snapshot = snapshot_dir.join("active-session.sh");

        fs::write(&old_snapshot, "# old").await.expect("write old");
        fs::write(&active_snapshot, "# active")
            .await
            .expect("write active");

        // Set old snapshot's mtime to the past
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let old_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("time")
                .as_secs()
                - 60 * 60 * 24 * 8; // 8 days ago

            let ts = libc::timespec {
                tv_sec: old_time as libc::time_t,
                tv_nsec: 0,
            };
            let times = [ts, ts];
            let c_path =
                std::ffi::CString::new(old_snapshot.as_os_str().as_bytes()).expect("cstring");
            unsafe {
                libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0);
            }
        }

        // Run cleanup with 7-day retention
        let removed = cleanup_stale_snapshots(
            snapshot_dir,
            "active-session",
            Duration::from_secs(60 * 60 * 24 * 7),
        )
        .await
        .expect("cleanup");

        // On Unix, the old snapshot should have been removed
        #[cfg(unix)]
        {
            assert_eq!(removed, 1);
            assert!(!old_snapshot.exists());
        }

        // Active snapshot should still exist
        assert!(active_snapshot.exists());
    }

    #[tokio::test]
    async fn test_cleanup_skips_active_session() {
        let dir = tempdir().expect("create temp dir");
        let snapshot_dir = dir.path();

        let active_snapshot = snapshot_dir.join("my-session.sh");
        fs::write(&active_snapshot, "# active")
            .await
            .expect("write");

        // Cleanup should skip the active session even with zero retention
        let removed = cleanup_stale_snapshots(snapshot_dir, "my-session", Duration::from_secs(0))
            .await
            .expect("cleanup");

        assert_eq!(removed, 0);
        assert!(active_snapshot.exists());
    }

    #[tokio::test]
    async fn test_cleanup_removes_invalid_filenames() {
        let dir = tempdir().expect("create temp dir");
        let snapshot_dir = dir.path();

        // Create a file without extension
        let invalid = snapshot_dir.join("no-extension");
        fs::write(&invalid, "# invalid").await.expect("write");

        let removed =
            cleanup_stale_snapshots(snapshot_dir, "other-session", Duration::from_secs(0))
                .await
                .expect("cleanup");

        assert_eq!(removed, 1);
        assert!(!invalid.exists());
    }

    #[tokio::test]
    async fn test_cleanup_handles_missing_dir() {
        let dir = tempdir().expect("create temp dir");
        let nonexistent = dir.path().join("nonexistent");

        let removed = cleanup_stale_snapshots(&nonexistent, "session", Duration::from_secs(0))
            .await
            .expect("cleanup");

        assert_eq!(removed, 0);
    }

    #[tokio::test]
    async fn test_cleanup_session_snapshots() {
        let dir = tempdir().expect("create temp dir");
        let snapshot_dir = dir.path();

        let target_sh = snapshot_dir.join("target-session.sh");
        let target_ps1 = snapshot_dir.join("target-session.ps1");
        let other = snapshot_dir.join("other-session.sh");

        fs::write(&target_sh, "# target").await.expect("write");
        fs::write(&target_ps1, "# target ps1").await.expect("write");
        fs::write(&other, "# other").await.expect("write");

        cleanup_session_snapshots(snapshot_dir, "target-session")
            .await
            .expect("cleanup");

        assert!(!target_sh.exists());
        assert!(!target_ps1.exists());
        assert!(other.exists());
    }
}
