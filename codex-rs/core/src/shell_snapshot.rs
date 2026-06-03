use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;
use std::pin::Pin;
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
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::Instrument;
use tracing::info_span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub path: AbsolutePathBuf,
    pub cwd: AbsolutePathBuf,
}

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);
const SNAPSHOT_RETENTION: Duration = Duration::from_secs(60 * 60 * 24 * 3); // 3 days retention.
const SNAPSHOT_DIR: &str = "shell_snapshots";
const EXCLUDED_EXPORT_VARS: &[&str] = &["PWD", "OLDPWD"];

/// Persists executor-local shell snapshot files and owns retention cleanup.
/// Implementations must return local readable paths because shell execution sources snapshots.
pub trait ShellSnapshotStore: Send + Sync + 'static {
    /// Allocates the final and temporary paths for one snapshot generation.
    fn snapshot_paths(&self, session_id: ThreadId, shell_type: ShellType) -> ShellSnapshotPaths;

    /// Removes stale inactive snapshots according to the store's retention policy.
    fn cleanup_stale_snapshots(
        &self,
        active_session_id: ThreadId,
        state_db: Option<StateDbHandle>,
    ) -> ShellSnapshotStoreFuture<'_>;
}

/// Future returned by shell snapshot store cleanup work.
pub type ShellSnapshotStoreFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

#[derive(Clone)]
/// Codex Home backed executor-local shell snapshot store.
pub struct LocalShellSnapshotStore {
    codex_home: AbsolutePathBuf,
}

impl LocalShellSnapshotStore {
    /// Constructs a local shell snapshot store rooted at one Codex Home.
    pub fn from_codex_home(codex_home: &AbsolutePathBuf) -> Self {
        Self {
            codex_home: codex_home.clone(),
        }
    }

    fn snapshot_dir(&self) -> AbsolutePathBuf {
        self.codex_home.join(SNAPSHOT_DIR)
    }

    async fn cleanup_stale_snapshots_impl(
        &self,
        active_session_id: ThreadId,
        state_db: Option<StateDbHandle>,
    ) -> Result<()> {
        let snapshot_dir = self.snapshot_dir();

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
                find_thread_path_by_id_str(&self.codex_home, session_id, state_db.as_deref())
                    .await?;
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
}

impl ShellSnapshotStore for LocalShellSnapshotStore {
    fn snapshot_paths(&self, session_id: ThreadId, shell_type: ShellType) -> ShellSnapshotPaths {
        let extension = match shell_type {
            ShellType::PowerShell => "ps1",
            _ => "sh",
        };
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let snapshot_dir = self.snapshot_dir();

        ShellSnapshotPaths {
            path: snapshot_dir.join(format!("{session_id}.{nonce}.{extension}")),
            temp_path: snapshot_dir.join(format!("{session_id}.tmp-{nonce}")),
        }
    }

    fn cleanup_stale_snapshots(
        &self,
        active_session_id: ThreadId,
        state_db: Option<StateDbHandle>,
    ) -> ShellSnapshotStoreFuture<'_> {
        Box::pin(self.cleanup_stale_snapshots_impl(active_session_id, state_db))
    }
}

/// Final and temporary local paths for one shell snapshot generation.
pub struct ShellSnapshotPaths {
    /// Final path sourced by later shell execution.
    pub path: AbsolutePathBuf,
    /// Temporary path populated before validation and rename.
    pub temp_path: AbsolutePathBuf,
}

impl ShellSnapshot {
    pub(crate) fn start_snapshotting(
        shell_snapshot_store: Arc<dyn ShellSnapshotStore>,
        session_id: ThreadId,
        session_cwd: AbsolutePathBuf,
        shell: &mut Shell,
        session_telemetry: SessionTelemetry,
        state_db: Option<StateDbHandle>,
    ) -> watch::Sender<Option<Arc<ShellSnapshot>>> {
        let (shell_snapshot_tx, shell_snapshot_rx) = watch::channel(None);
        shell.shell_snapshot = shell_snapshot_rx;

        Self::spawn_snapshot_task(
            shell_snapshot_store,
            session_id,
            session_cwd,
            shell.clone(),
            shell_snapshot_tx.clone(),
            session_telemetry,
            state_db,
        );

        shell_snapshot_tx
    }

    pub(crate) fn refresh_snapshot(
        shell_snapshot_store: Arc<dyn ShellSnapshotStore>,
        session_id: ThreadId,
        session_cwd: AbsolutePathBuf,
        shell: Shell,
        shell_snapshot_tx: watch::Sender<Option<Arc<ShellSnapshot>>>,
        session_telemetry: SessionTelemetry,
        state_db: Option<StateDbHandle>,
    ) {
        Self::spawn_snapshot_task(
            shell_snapshot_store,
            session_id,
            session_cwd,
            shell,
            shell_snapshot_tx,
            session_telemetry,
            state_db,
        );
    }

    fn spawn_snapshot_task(
        shell_snapshot_store: Arc<dyn ShellSnapshotStore>,
        session_id: ThreadId,
        session_cwd: AbsolutePathBuf,
        snapshot_shell: Shell,
        shell_snapshot_tx: watch::Sender<Option<Arc<ShellSnapshot>>>,
        session_telemetry: SessionTelemetry,
        state_db: Option<StateDbHandle>,
    ) {
        let snapshot_span = info_span!("shell_snapshot", thread_id = %session_id);
        tokio::spawn(
            async move {
                let cleanup_shell_snapshot_store = Arc::clone(&shell_snapshot_store);
                let cleanup_session_id = session_id;
                let cleanup_state_db = state_db.clone();
                tokio::spawn(async move {
                    if let Err(err) = cleanup_shell_snapshot_store
                        .cleanup_stale_snapshots(cleanup_session_id, cleanup_state_db)
                        .await
                    {
                        tracing::warn!("Failed to clean up shell snapshots: {err:?}");
                    }
                });

                let timer = session_telemetry.start_timer("codex.shell_snapshot.duration_ms", &[]);
                let snapshot = ShellSnapshot::try_new(
                    shell_snapshot_store.as_ref(),
                    session_id,
                    &session_cwd,
                    &snapshot_shell,
                )
                .await
                .map(Arc::new);
                let success = snapshot.is_ok();
                let success_tag = if success { "true" } else { "false" };
                let _ = timer.map(|timer| timer.record(&[("success", success_tag)]));
                let mut counter_tags = vec![("success", success_tag)];
                if let Some(failure_reason) = snapshot.as_ref().err() {
                    counter_tags.push(("failure_reason", *failure_reason));
                }
                session_telemetry.counter("codex.shell_snapshot", /*inc*/ 1, &counter_tags);
                let _ = shell_snapshot_tx.send(snapshot.ok());
            }
            .instrument(snapshot_span),
        );
    }

    async fn try_new(
        shell_snapshot_store: &dyn ShellSnapshotStore,
        session_id: ThreadId,
        session_cwd: &AbsolutePathBuf,
        shell: &Shell,
    ) -> std::result::Result<Self, &'static str> {
        let ShellSnapshotPaths { path, temp_path } =
            shell_snapshot_store.snapshot_paths(session_id, shell.shell_type.clone());

        // Make the new snapshot.
        if let Err(err) =
            write_shell_snapshot(shell.shell_type.clone(), &temp_path, session_cwd).await
        {
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

        Ok(Self {
            path,
            cwd: session_cwd.clone(),
        })
    }
}

impl Drop for ShellSnapshot {
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
    let shell = get_shell(shell_type.clone(), /*path*/ None)
        .with_context(|| format!("No available shell for {shell_type:?}"))?;

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
    let shell_type = shell.shell_type.clone();
    match shell_type {
        ShellType::Zsh => run_shell_script(shell, &zsh_snapshot_script(), cwd).await,
        ShellType::Bash => run_shell_script(shell, &bash_snapshot_script(), cwd).await,
        ShellType::Sh => run_shell_script(shell, &sh_snapshot_script(), cwd).await,
        ShellType::PowerShell => run_shell_script(shell, powershell_snapshot_script(), cwd).await,
        ShellType::Cmd => bail!("Shell snapshotting is not yet supported for {shell_type:?}"),
    }
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

fn excluded_exports_regex() -> String {
    EXCLUDED_EXPORT_VARS.join("|")
}

fn zsh_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [[ -n "$ZDOTDIR" ]]; then
  rc="$ZDOTDIR/.zshrc"
else
  rc="$HOME/.zshrc"
fi
[[ -r "$rc" ]] && . "$rc"
print '# Snapshot file'
print '# Unset all aliases to avoid conflicts with functions'
print 'unalias -a 2>/dev/null || true'
print '# Functions'
functions
print ''
setopt_count=$(setopt | wc -l | tr -d ' ')
print "# setopts $setopt_count"
setopt | sed 's/^/setopt /'
print ''
alias_count=$(alias -L | wc -l | tr -d ' ')
print "# aliases $alias_count"
alias -L
print ''
export_lines=$(export -p | awk '
/^(export|declare -x|typeset -x) / {
  line=$0
  name=line
  sub(/^(export|declare -x|typeset -x) /, "", name)
  sub(/=.*/, "", name)
  if (name ~ /^(EXCLUDED_EXPORTS)$/) {
    next
  }
  if (name ~ /^[A-Za-z_][A-Za-z0-9_]*$/) {
    print line
  }
}')
export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
print "# exports $export_count"
if [[ -n "$export_lines" ]]; then
  print -r -- "$export_lines"
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

fn bash_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [ -z "$BASH_ENV" ] && [ -r "$HOME/.bashrc" ]; then
  . "$HOME/.bashrc"
fi
echo '# Snapshot file'
echo '# Unset all aliases to avoid conflicts with functions'
unalias -a 2>/dev/null || true
echo '# Functions'
declare -f
echo ''
bash_opts=$(set -o | awk '$2=="on"{print $1}')
bash_opt_count=$(printf '%s\n' "$bash_opts" | sed '/^$/d' | wc -l | tr -d ' ')
echo "# setopts $bash_opt_count"
if [ -n "$bash_opts" ]; then
  printf 'set -o %s\n' $bash_opts
fi
echo ''
alias_count=$(alias -p | wc -l | tr -d ' ')
echo "# aliases $alias_count"
alias -p
echo ''
export_lines=$(
  while IFS= read -r name; do
    if [[ "$name" =~ ^(EXCLUDED_EXPORTS)$ ]]; then
      continue
    fi
    if [[ ! "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
      continue
    fi
    declare -xp "$name" 2>/dev/null || true
  done < <(compgen -e)
)
export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
echo "# exports $export_count"
if [ -n "$export_lines" ]; then
  printf '%s\n' "$export_lines"
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

fn sh_snapshot_script() -> String {
    let excluded = excluded_exports_regex();
    let script = r##"if [ -n "$ENV" ] && [ -r "$ENV" ]; then
  . "$ENV"
fi
echo '# Snapshot file'
echo '# Unset all aliases to avoid conflicts with functions'
unalias -a 2>/dev/null || true
echo '# Functions'
if command -v typeset >/dev/null 2>&1; then
  typeset -f
elif command -v declare >/dev/null 2>&1; then
  declare -f
fi
echo ''
if set -o >/dev/null 2>&1; then
  sh_opts=$(set -o | awk '$2=="on"{print $1}')
  sh_opt_count=$(printf '%s\n' "$sh_opts" | sed '/^$/d' | wc -l | tr -d ' ')
  echo "# setopts $sh_opt_count"
  if [ -n "$sh_opts" ]; then
    printf 'set -o %s\n' $sh_opts
  fi
else
  echo '# setopts 0'
fi
echo ''
if alias >/dev/null 2>&1; then
  alias_count=$(alias | wc -l | tr -d ' ')
  echo "# aliases $alias_count"
  alias
  echo ''
else
  echo '# aliases 0'
fi
if export -p >/dev/null 2>&1; then
  export_lines=$(export -p | awk '
/^(export|declare -x|typeset -x) / {
  line=$0
  name=line
  sub(/^(export|declare -x|typeset -x) /, "", name)
  sub(/=.*/, "", name)
  if (name ~ /^(EXCLUDED_EXPORTS)$/) {
    next
  }
  if (name ~ /^[A-Za-z_][A-Za-z0-9_]*$/) {
    print line
  }
}')
  export_count=$(printf '%s\n' "$export_lines" | sed '/^$/d' | wc -l | tr -d ' ')
  echo "# exports $export_count"
  if [ -n "$export_lines" ]; then
    printf '%s\n' "$export_lines"
  fi
else
  export_count=$(env | sort | awk -F= '$1 ~ /^[A-Za-z_][A-Za-z0-9_]*$/ { count++ } END { print count }')
  echo "# exports $export_count"
  env | sort | while IFS='=' read -r key value; do
    case "$key" in
      ""|[0-9]*|*[!A-Za-z0-9_]*|EXCLUDED_EXPORTS) continue ;;
    esac
    escaped=$(printf "%s" "$value" | sed "s/'/'\"'\"'/g")
    printf "export %s='%s'\n" "$key" "$escaped"
  done
fi
"##;
    script.replace("EXCLUDED_EXPORTS", &excluded)
}

fn powershell_snapshot_script() -> &'static str {
    r##"$ErrorActionPreference = 'Stop'
Write-Output '# Snapshot file'
Write-Output '# Unset all aliases to avoid conflicts with functions'
Write-Output 'Remove-Item Alias:* -ErrorAction SilentlyContinue'
Write-Output '# Functions'
Get-ChildItem Function: | ForEach-Object {
    "function {0} {{`n{1}`n}}" -f $_.Name, $_.Definition
}
Write-Output ''
$aliases = Get-Alias
Write-Output ("# aliases " + $aliases.Count)
$aliases | ForEach-Object {
    "Set-Alias -Name {0} -Value {1}" -f $_.Name, $_.Definition
}
Write-Output ''
$envVars = Get-ChildItem Env:
Write-Output ("# exports " + $envVars.Count)
$envVars | ForEach-Object {
    $escaped = $_.Value -replace "'", "''"
    "`$env:{0}='{1}'" -f $_.Name, $escaped
}
"##
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
