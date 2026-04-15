use crate::shell::Shell;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub struct ShellSnapshot {
    pub path: PathBuf,
    pub cwd: PathBuf,
}

impl ShellSnapshot {
    pub fn start_snapshotting(
        _codex_home: PathBuf,
        _session_id: ThreadId,
        _session_cwd: PathBuf,
        shell: &mut Shell,
        _session_telemetry: SessionTelemetry,
    ) -> watch::Sender<Option<Arc<ShellSnapshot>>> {
        let (tx, rx) = watch::channel(None);
        shell.shell_snapshot = rx;
        tx
    }

    pub fn refresh_snapshot(
        _codex_home: PathBuf,
        _session_id: ThreadId,
        _session_cwd: PathBuf,
        _shell: Shell,
        shell_snapshot_tx: watch::Sender<Option<Arc<ShellSnapshot>>>,
        _session_telemetry: SessionTelemetry,
    ) {
        let _ = shell_snapshot_tx.send(None);
    }
}

pub async fn cleanup_stale_snapshots(
    _codex_home: &Path,
    _active_session_id: ThreadId,
) -> std::io::Result<()> {
    Ok(())
}
