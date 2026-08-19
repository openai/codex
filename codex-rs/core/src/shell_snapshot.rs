use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use crate::StateDbHandle;
use crate::rollout::list::find_thread_path_by_id_str;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell::get_shell;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_exec_server::Environment;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_shell_command::shell_snapshot::snapshot_script;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::Instrument;
use tracing::info_span;

#[derive(Clone)]
pub(crate) struct ShellSnapshot {
    config: Option<Arc<ShellSnapshotConfig>>,
}

struct ShellSnapshotConfig {
    codex_home: AbsolutePathBuf,
    session_id: ThreadId,
    session_telemetry: SessionTelemetry,
    state_db: Option<StateDbHandle>,
}

pub(crate) struct ShellSnapshotFile {
    path: AbsolutePathBuf,
}

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);
const SNAPSHOT_RETENTION: Duration = Duration::from_secs(60 * 60 * 24 * 3); // 3 days retention.
const SNAPSHOT_DIR: &str = "shell_snapshots";

impl ShellSnapshot {
    pub(crate) fn new(
        codex_home: AbsolutePathBuf,
        session_id: ThreadId,
        session_telemetry: SessionTelemetry,
        state_db: Option<StateDbHandle>,
    ) -> Self {
        Self {
            config: Some(Arc::new(ShellSnapshotConfig {
                codex_home,
                session_id,
                session_telemetry,
                state_db,
            })),
        }
    }

    pub(crate) fn disabled() -> Self {
        Self { config: None }
    }

    pub(crate) async fn build(
        self,
        environment: Arc<Environment>,
        cwd: PathUri,
        shell: Option<Shell>,
    ) -> Option<Arc<ShellSnapshotFile>> {
        let config = self.config.as_ref()?;
        if environment.is_remote() {
            return None;
        }

        let shell = shell?;
        // TODO(anp): Migrate shell snapshot creation to accept PathUri and defer native
        // conversion to the spawned shell process.
        let cwd = cwd.to_abs_path().ok()?;
        Self::build_for_cwd(Arc::clone(config), cwd, shell).await
    }

    async fn build_for_cwd(
        config: Arc<ShellSnapshotConfig>,
        cwd: AbsolutePathBuf,
        shell: Shell,
    ) -> Option<Arc<ShellSnapshotFile>> {
        let snapshot_span = info_span!("shell_snapshot", thread_id = %config.session_id);
        async {
            let timer = config
                .session_telemetry
                .start_timer("codex.shell_snapshot.duration_ms", &[]);
            let snapshot = ShellSnapshot::try_create(
                &config.codex_home,
                config.session_id,
                &cwd,
                &shell,
                config.state_db.clone(),
            )
            .await;
            let success_tag = if snapshot.is_ok() { "true" } else { "false" };
            let _ = timer.map(|timer| timer.record(&[("success", success_tag)]));
            let mut counter_tags = vec![("success", success_tag)];
            if let Some(failure_reason) = snapshot.as_ref().err() {
                counter_tags.push(("failure_reason", *failure_reason));
            }
            config
                .session_telemetry
                .counter("codex.shell_snapshot", /*inc*/ 1, &counter_tags);
            snapshot.ok().map(Arc::new)
        }
        .instrument(snapshot_span)
        .await
    }

    async fn try_create(
        codex_home: &AbsolutePathBuf,
        session_id: ThreadId,
        session_cwd: &AbsolutePathBuf,
        shell: &Shell,
        state_db: Option<StateDbHandle>,
    ) -> std::result::Result<ShellSnapshotFile, &'static str> {
        // File to store the snapshot
        let extension = match shell.shell_type {
            ShellType::PowerShell => "ps1",
            _ => "sh",
        };
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = codex_home
            .join(SNAPSHOT_DIR)
            .join(format!("{session_id}.{nonce}.{extension}"));
        let temp_path = codex_home
            .join(SNAPSHOT_DIR)
            .join(format!("{session_id}.tmp-{nonce}"));

        // Clean the (unlikely) leaked snapshot files.
        let codex_home = codex_home.clone();
        let cleanup_session_id = session_id;
        tokio::spawn(async move {
            if let Err(err) =
                cleanup_stale_snapshots(&codex_home, cleanup_session_id, state_db).await
            {
                tracing::warn!("Failed to clean up shell snapshots: {err:?}");
            }
        });

        // Make the new snapshot.
        if let Err(err) = write_shell_snapshot(shell.shell_type, &temp_path, session_cwd).await {
            tracing::warn!(
                "Failed to create shell snapshot for {}: {err:?}",
                shell.name()
            );
            return Err("write_failed");
        }
        tracing::info!(
            "Shell snapshot successfully created: {}",
            temp_path.display()
        );

        if let Err(err) = validate_snapshot(shell, &temp_path, session_cwd).await {
            tracing::error!("Shell snapshot validation failed: {err:?}");
            remove_snapshot_file(&temp_path).await;
            return Err("validation_failed");
        }

        if let Err(err) = fs::rename(&temp_path, &path).await {
            tracing::warn!("Failed to finalize shell snapshot: {err:?}");
            remove_snapshot_file(&temp_path).await;
            return Err("write_failed");
        }

        Ok(ShellSnapshotFile { path })
    }
}

impl ShellSnapshotFile {
    pub(crate) fn path(&self) -> AbsolutePathBuf {
        self.path.clone()
    }
}

impl Drop for ShellSnapshotFile {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.path) {
            tracing::warn!(
                "Failed to delete shell snapshot at {:?}: {err:?}",
                self.path
            );
        }
    }
}

async fn write_shell_snapshot(
    shell_type: ShellType,
    output_path: &AbsolutePathBuf,
    cwd: &AbsolutePathBuf,
) -> Result<()> {
    if shell_type == ShellType::PowerShell || shell_type == ShellType::Cmd {
        bail!("Shell snapshot not supported yet for {shell_type:?}");
    }
    let shell =
        get_shell(shell_type).with_context(|| format!("No available shell for {shell_type:?}"))?;

    let raw_snapshot = capture_snapshot(&shell, cwd).await?;
    let snapshot = strip_snapshot_preamble(&raw_snapshot)?;

    if let Some(parent) = output_path.parent() {
        let parent_display = parent.display();
        fs::create_dir_all(&parent)
            .await
            .with_context(|| format!("Failed to create snapshot parent {parent_display}"))?;
    }

    let snapshot_path = output_path.display();
    fs::write(output_path, snapshot)
        .await
        .with_context(|| format!("Failed to write snapshot to {snapshot_path}"))?;

    Ok(())
}

async fn capture_snapshot(shell: &Shell, cwd: &AbsolutePathBuf) -> Result<String> {
    let shell_type = shell.shell_type;
    let script = snapshot_script(shell_type)
        .ok_or_else(|| anyhow!("Shell snapshotting is not yet supported for {shell_type:?}"))?;
    run_shell_script(shell, &script, cwd).await
}

fn strip_snapshot_preamble(snapshot: &str) -> Result<String> {
    let marker = "# Snapshot file";
    let Some(start) = snapshot.find(marker) else {
        bail!("Snapshot output missing marker {marker}");
    };

    Ok(snapshot[start..].to_string())
}

async fn validate_snapshot(
    shell: &Shell,
    snapshot_path: &AbsolutePathBuf,
    cwd: &AbsolutePathBuf,
) -> Result<()> {
    let snapshot_path_display = snapshot_path.display();
    let script = format!("set -e; . \"{snapshot_path_display}\"");
    run_script_with_timeout(
        shell,
        &script,
        SNAPSHOT_TIMEOUT,
        /*use_login_shell*/ false,
        cwd,
    )
    .await
    .map(|_| ())
}

async fn run_shell_script(shell: &Shell, script: &str, cwd: &AbsolutePathBuf) -> Result<String> {
    run_script_with_timeout(
        shell,
        script,
        SNAPSHOT_TIMEOUT,
        /*use_login_shell*/ true,
        cwd,
    )
    .await
}

async fn run_script_with_timeout(
    shell: &Shell,
    script: &str,
    snapshot_timeout: Duration,
    use_login_shell: bool,
    cwd: &AbsolutePathBuf,
) -> Result<String> {
    let args = shell.derive_exec_args(script, use_login_shell);
    let shell_name = shell.name();

    // Handler is kept as guard to control the drop. The `mut` pattern is required because .args()
    // returns a ref of handler.
    let mut handler = Command::new(&args[0]);
    codex_protocol::shell_environment::scrub_non_inheritable_env_vars(handler.as_std_mut());
    handler.args(&args[1..]);
    handler.stdin(Stdio::null());
    handler.current_dir(cwd);
    #[cfg(unix)]
    unsafe {
        handler.pre_exec(|| {
            codex_utils_pty::process_group::detach_from_tty()?;
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

/// Removes shell snapshots that either lack a matching session rollout file or
/// whose rollouts have not been updated within the retention window.
/// The active session id is exempt from cleanup.
pub async fn cleanup_stale_snapshots(
    codex_home: &AbsolutePathBuf,
    active_session_id: ThreadId,
    state_db: Option<StateDbHandle>,
) -> Result<()> {
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

        let rollout_path =
            find_thread_path_by_id_str(codex_home, session_id, state_db.as_deref()).await?;
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

async fn remove_snapshot_file(path: &Path) {
    if let Err(err) = fs::remove_file(path).await {
        tracing::warn!("Failed to delete shell snapshot at {:?}: {err:?}", path);
    }
}

fn snapshot_session_id_from_file_name(file_name: &str) -> Option<&str> {
    let (stem, extension) = file_name.rsplit_once('.')?;
    match extension {
        "sh" | "ps1" => Some(
            stem.split_once('.')
                .map_or(stem, |(session_id, _generation)| session_id),
        ),
        _ if extension.starts_with("tmp-") => Some(stem),
        _ => None,
    }
}

#[cfg(test)]
#[path = "shell_snapshot_tests.rs"]
mod tests;
