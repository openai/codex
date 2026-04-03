use super::*;
use anyhow::Result;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::time::Duration;
#[cfg(unix)]
use std::time::SystemTime;
use tempfile::tempdir;

async fn write_rollout_stub(codex_home: &Path, session_id: ThreadId) -> Result<PathBuf> {
    let dir = codex_home
        .join("sessions")
        .join("2025")
        .join("01")
        .join("01");
    fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("rollout-2025-01-01T00-00-00-{session_id}.jsonl"));
    fs::write(&path, "").await?;
    Ok(path)
}

#[test]
fn snapshot_file_name_parser_supports_legacy_and_suffixed_names() {
    let session_id = "019cf82b-6a62-7700-bbbd-46909794ef89";

    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.sh")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.123.sh")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.tmp-123")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name("not-a-snapshot.txt"),
        None
    );
}

#[tokio::test]
async fn cleanup_stale_snapshots_removes_orphans_and_keeps_live() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let live_session = ThreadId::new();
    let orphan_session = ThreadId::new();
    let live_snapshot = snapshot_dir.join(format!("{live_session}.123.sh"));
    let orphan_snapshot = snapshot_dir.join(format!("{orphan_session}.456.sh"));
    let invalid_snapshot = snapshot_dir.join("not-a-snapshot.txt");

    write_rollout_stub(codex_home, live_session).await?;
    fs::write(&live_snapshot, "live").await?;
    fs::write(&orphan_snapshot, "orphan").await?;
    fs::write(&invalid_snapshot, "invalid").await?;

    cleanup_stale_snapshots(codex_home, ThreadId::new()).await?;

    assert_eq!(live_snapshot.exists(), true);
    assert_eq!(orphan_snapshot.exists(), false);
    assert_eq!(invalid_snapshot.exists(), false);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cleanup_stale_snapshots_removes_stale_rollouts() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let stale_session = ThreadId::new();
    let stale_snapshot = snapshot_dir.join(format!("{stale_session}.123.sh"));
    let rollout_path = write_rollout_stub(codex_home, stale_session).await?;
    fs::write(&stale_snapshot, "stale").await?;

    set_file_mtime(&rollout_path, SNAPSHOT_RETENTION + Duration::from_secs(60))?;

    cleanup_stale_snapshots(codex_home, ThreadId::new()).await?;

    assert_eq!(stale_snapshot.exists(), false);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cleanup_stale_snapshots_skips_active_session() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let active_session = ThreadId::new();
    let active_snapshot = snapshot_dir.join(format!("{active_session}.123.sh"));
    let rollout_path = write_rollout_stub(codex_home, active_session).await?;
    fs::write(&active_snapshot, "active").await?;

    set_file_mtime(&rollout_path, SNAPSHOT_RETENTION + Duration::from_secs(60))?;

    cleanup_stale_snapshots(codex_home, active_session).await?;

    assert_eq!(active_snapshot.exists(), true);
    Ok(())
}

#[cfg(unix)]
fn set_file_mtime(path: &Path, age: Duration) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs()
        .saturating_sub(age.as_secs());
    let tv_sec = now
        .try_into()
        .map_err(|_| anyhow::anyhow!("Snapshot mtime is out of range for libc::timespec"))?;
    let ts = libc::timespec { tv_sec, tv_nsec: 0 };
    let times = [ts, ts];
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let result = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
